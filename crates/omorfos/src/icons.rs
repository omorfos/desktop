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

use std::collections::HashMap;
use std::path::PathBuf;

use omorfos_config::Color;
use omorfos_icons::Icons;
use omorfos_ui::Image;
use smithay::backend::renderer::{BufferType, buffer_dimensions, buffer_type};
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
use smithay::wayland::compositor::{Cacheable, with_states};

use crate::compositor::Compositor;

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

    /// The picture of an icon of a theme, as [`Self::of`] found it.
    pub(crate) fn picture(&mut self, name: &str, pixels: u32, ink: Color) -> Option<&Image> {
        self.icons.get(name, pixels, ink)
    }
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
