//! The icons of windows: the one a program gives its window
//! (`xdg-toplevel-icon`), or else its application's, from its desktop
//! entry or by its name.
//!
//! An icon a program names is looked up in the icon themes, as the
//! notifications and the launcher look theirs up; one it gives as pixels is
//! in memory it shares, which only the renderer reads (ADR 40), so it is
//! drawn by the renderer and not painted.
//!
//! The protocol is served here rather than by Smithay, which sends a
//! program away when it destroys a buffer it gave as an icon before the
//! icon, as the protocol says to. Programs built on Chromium do that, and
//! the desktops they are tried on let them: an icon that has lost its
//! pixels here is drawn as though it had none (ADR 54).

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

use omorfos_config::Color;
use omorfos_icons::Icons;
use omorfos_ui::{Area, Image};
use smithay::backend::renderer::element::texture::TextureRenderElement;
use smithay::backend::renderer::element::{Id, Kind};
use smithay::backend::renderer::{
    BufferType, ErasedContextId, ImportAll, ImportMem, Renderer, Texture, buffer_dimensions,
    buffer_type,
};
use smithay::reexports::wayland_protocols::xdg::toplevel_icon::v1::server::{
    xdg_toplevel_icon_manager_v1::{self, XdgToplevelIconManagerV1},
    xdg_toplevel_icon_v1::{self, XdgToplevelIconV1},
};
use smithay::reexports::wayland_server::backend::{ClientId, ObjectId};
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
};
use smithay::utils::{Logical, Point, Transform};
use smithay::wayland::compositor::{Cacheable, with_states};

use crate::Scale;
use crate::compositor::{Compositor, Element, View};
use crate::paint;

/// The icon a program gave a window, double-buffered with the window's
/// surface: the name of one of a theme, and the pixels it gave, each
/// buffer with the scale it is for.
#[derive(Debug, Clone, Default)]
pub(crate) struct GivenIcon {
    name: Option<String>,
    buffers: Vec<(WlBuffer, i32)>,
}

impl Cacheable for GivenIcon {
    fn commit(&mut self, _: &DisplayHandle) -> Self {
        self.clone()
    }

    fn merge_into(self, into: &mut Self, _: &DisplayHandle) {
        *into = self;
    }
}

/// An icon being made, by the number of its object: what it has been
/// given so far, and whether it has been set on a window, after which it
/// may not change.
#[derive(Debug, Default)]
pub(crate) struct Making {
    icon: GivenIcon,
    set: bool,
}

/// The sizes icons are asked for, told each program that binds the
/// protocol.
#[derive(Debug)]
pub(crate) struct Sizes(pub(crate) Vec<i32>);

/// Where a window's icon comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Icon {
    /// An icon of a theme, or a picture's path from a desktop entry.
    Named(String),
    /// Pixels the program gave, in memory it shares.
    Given(WlBuffer),
}

/// Where icons are installed, and those looked up so far.
#[derive(Debug, Default)]
pub(crate) struct WindowIcons {
    /// Where programs install what they share, desktop entries among it.
    data: Vec<PathBuf>,
    icons: Icons,
    /// The icon each application's desktop entry names, by identifier.
    entries: HashMap<String, Option<String>>,
    /// The icons programs gave as pixels, as each renderer has them, so that
    /// each is handed to the graphics card once and not with every frame.
    textures: RefCell<Textures>,
}

/// What the renderer made of each icon a program gave as pixels, by the
/// buffer and the renderer's context, with the buffer, until the program
/// destroys it.
#[derive(Default)]
struct Textures(HashMap<(ObjectId, ErasedContextId), (WlBuffer, Box<dyn Any + Send>)>);

impl std::fmt::Debug for Textures {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("Textures")
            .field(&self.0.len())
            .finish()
    }
}

impl WindowIcons {
    /// The icons installed under `data`, the user's own first (see
    /// [`omorfos_icons::data_directories`]).
    pub(crate) fn new(data: Vec<PathBuf>) -> Self {
        let icons = Icons::new(omorfos_icons::icon_directories(&data));
        Self {
            data,
            icons,
            entries: HashMap::new(),
            textures: RefCell::default(),
        }
    }

    /// The icon of a window of the application `app_id`, to be drawn
    /// `pixels` across beside text of the colour `ink`: the one its
    /// program names, if a theme has it; else the pixels it gave; else
    /// the one its desktop entry names, or the one of its own name.
    pub(crate) fn of(
        &mut self,
        surface: &WlSurface,
        app_id: &str,
        pixels: u32,
        ink: Color,
    ) -> Option<Icon> {
        // What the renderer kept of pixels their programs have destroyed is
        // let go of here, as an icon is looked up, as well as when one is
        // drawn: an icon whose pixels are gone is not drawn again.
        let textures = &mut self.textures.get_mut().0;
        textures.retain(|_, (kept, _)| kept.is_alive());
        let (named, given) = with_states(surface, |states| {
            let mut cached = states.cached_state.get::<GivenIcon>();
            let current = cached.current();
            (current.name.clone(), largest(&current.buffers, pixels))
        });
        // A program names an icon of a theme. A path would have the
        // compositor read a file of the program's choosing.
        let named = named.filter(|name| !name.contains('/'));
        let mut found = |name: &str| self.icons.get(name, pixels, ink).is_some();
        if let Some(name) = named.filter(|name| found(name)) {
            return Some(Icon::Named(name));
        }
        if let Some(buffer) = given {
            return Some(Icon::Given(buffer));
        }
        let data = &self.data;
        let entry = self
            .entries
            .entry(app_id.to_owned())
            .or_insert_with(|| omorfos_icons::icon_of(app_id, data))
            .clone();
        let mut found = |name: &str| self.icons.get(name, pixels, ink).is_some();
        let name = entry
            .into_iter()
            .chain([app_id.to_owned()])
            .find(|name| found(name));
        name.map(Icon::Named)
    }

    /// An icon a program gave as pixels, drawn by the renderer in the middle
    /// of `area`, as large as fits but no larger than its pixels, which would
    /// only make it blocky. Icons are square, as the protocol has them. Nothing
    /// if the renderer cannot read it.
    pub(crate) fn given<R>(
        &self,
        renderer: &mut R,
        buffer: &WlBuffer,
        area: Area,
        id: &Id,
        view: View,
    ) -> Option<Element<R>>
    where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let context = renderer.context_id();
        let key = (buffer.id(), context.clone().erased());
        let mut textures = self.textures.borrow_mut();
        // Those of icons their programs have destroyed are let go of.
        textures.0.retain(|_, (kept, _)| kept.is_alive());
        let kept = textures.0.get(&key);
        let kept = kept.and_then(|(_, texture)| texture.downcast_ref::<R::TextureId>());
        let kept = kept.cloned();
        // An icon's pixels do not change once it is given, so it is read
        // once, and kept.
        let mut imported = || {
            renderer
                .import_buffer(buffer, None, &[])
                .and_then(Result::ok)
        };
        let texture = kept.or_else(|| {
            imported().inspect(|texture| {
                let held = (
                    buffer.clone(),
                    Box::new(texture.clone()) as Box<dyn Any + Send>,
                );
                textures.0.insert(key, held);
            })
        });
        texture.map(|texture| {
            let pixels = texture.size().w.max(1);
            // As many units of layout as it has pixels at this scale, which
            // is no more than it has pixels, an `i32`.
            #[expect(clippy::cast_possible_truncation, reason = "see above")]
            let most = (f64::from(pixels) / view.scale.factor()).floor() as i32;
            let room = area.2.min(area.3).cast_signed();
            let side = room.min(most.max(1));
            let left = area.0 + (area.2.cast_signed() - side) / 2;
            let top = area.1 + (area.3.cast_signed() - side) / 2;
            let at = view.pixel(Point::<f64, Logical>::from((
                f64::from(left),
                f64::from(top),
            )));
            Element::Texture(TextureRenderElement::from_static_texture(
                id.clone(),
                context,
                at.to_f64(),
                texture,
                1,
                Transform::Normal,
                None,
                None,
                Some((side, side).into()),
                None,
                Kind::Unspecified,
            ))
        })
    }

    /// The picture of an icon of a theme, as [`Self::of`] found it.
    pub(crate) fn picture(&mut self, name: &str, pixels: u32, ink: Color) -> Option<&Image> {
        self.icons.get(name, pixels, ink)
    }
}

impl Compositor {
    /// How many icons that programs gave as pixels the renderer keeps,
    /// drawn once and drawn from again.
    #[must_use]
    pub fn icon_textures(&self) -> usize {
        self.switch.icons.textures.borrow().0.len()
    }
}

/// How many pixels across an icon is looked up at, to be drawn sharp at
/// `scale` where it is drawn largest.
pub(crate) fn icon_pixels(scale: Scale) -> u32 {
    let side = f64::from(paint::LARGE_ICON) * scale.factor();
    // At most a few hundred, so neither negative nor too large.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "see above"
    )]
    let pixels = side.ceil() as u32;
    pixels
}

/// Of the buffers an icon was given that the program has not destroyed,
/// the smallest at least `pixels` across, and failing that the largest.
fn largest(buffers: &[(WlBuffer, i32)], pixels: u32) -> Option<WlBuffer> {
    let kept = buffers.iter().filter(|(buffer, _)| buffer.is_alive());
    let sized = kept.map(|(buffer, _)| {
        let across = buffer_dimensions(buffer).map_or(0, |size| size.w);
        (u32::try_from(across).unwrap_or(0), buffer)
    });
    sized
        .min_by_key(|(across, _)| (*across < pixels, across.abs_diff(pixels)))
        .map(|(_, buffer)| buffer.clone())
}

impl GlobalDispatch<XdgToplevelIconManagerV1, Sizes> for Compositor {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        manager: New<XdgToplevelIconManagerV1>,
        sizes: &Sizes,
        data_init: &mut DataInit<'_, Self>,
    ) {
        let manager = data_init.init(manager, ());
        for size in &sizes.0 {
            manager.icon_size(*size);
        }
        manager.done();
    }
}

impl Dispatch<XdgToplevelIconManagerV1, ()> for Compositor {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &XdgToplevelIconManagerV1,
        request: xdg_toplevel_icon_manager_v1::Request,
        (): &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            xdg_toplevel_icon_manager_v1::Request::CreateIcon { id } => {
                let icon = data_init.init(id, ());
                state.switch.making.insert(icon.id(), Making::default());
            }
            xdg_toplevel_icon_manager_v1::Request::SetIcon { toplevel, icon } => {
                // None takes back the icon the window had.
                let made = icon.and_then(|icon| state.switch.making.get_mut(&icon.id()));
                let given = made.map_or_else(GivenIcon::default, |made| {
                    made.set = true;
                    made.icon.clone()
                });
                // Every window that can be asked about is one Smithay knows.
                let window = state.xdg_shell.get_toplevel(&toplevel);
                let _ = window.map(|window| {
                    with_states(window.wl_surface(), |states| {
                        *states.cached_state.get::<GivenIcon>().pending() = given;
                    });
                });
            }
            _ => {}
        }
    }
}

impl Dispatch<XdgToplevelIconV1, ()> for Compositor {
    fn request(
        state: &mut Self,
        _: &Client,
        icon: &XdgToplevelIconV1,
        request: xdg_toplevel_icon_v1::Request,
        (): &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        // Every icon is being made from when it is created until it goes.
        let made = state.switch.making.entry(icon.id()).or_default();
        if made.set && !matches!(request, xdg_toplevel_icon_v1::Request::Destroy) {
            icon.post_error(
                xdg_toplevel_icon_v1::Error::Immutable,
                "the icon has already been set on a window",
            );
            return;
        }
        match request {
            xdg_toplevel_icon_v1::Request::SetName { icon_name } => {
                made.icon.name = Some(icon_name);
            }
            xdg_toplevel_icon_v1::Request::AddBuffer { buffer, scale } => {
                let shared = matches!(buffer_type(&buffer), Some(BufferType::Shm));
                let size = buffer_dimensions(&buffer).filter(|size| size.w == size.h);
                let Some(size) = size.filter(|_| shared) else {
                    icon.post_error(
                        xdg_toplevel_icon_v1::Error::InvalidBuffer,
                        "an icon is to be given in shared memory, and square",
                    );
                    return;
                };
                // One of the same size and scale takes the place of the
                // one before.
                let buffers = &mut made.icon.buffers;
                let same = |(given, given_scale): &(WlBuffer, i32)| {
                    *given_scale == scale && buffer_dimensions(given) == Some(size)
                };
                buffers.retain(|given| !same(given));
                buffers.push((buffer, scale));
            }
            _ => {}
        }
    }

    fn destroyed(state: &mut Self, _: ClientId, icon: &XdgToplevelIconV1, (): &()) {
        // What was set on a window stays set there, in the window's state.
        let gone: ObjectId = icon.id();
        state.switch.making.remove(&gone);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_kept_of_programs_pixels_is_shown_by_how_many_not_what() {
        let icons = WindowIcons::default();
        assert!(format!("{icons:?}").contains("Textures(0)"));
    }
}
