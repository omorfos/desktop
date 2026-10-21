//! The switcher on `Alt+Tab`, drawn by the compositor as the menu is
//! (ADR 38): opened by a binding, walked by it and by the arrows while the
//! keys held with it are, and the window it is at chosen when they are let
//! go. Which windows, in what order and where each is shown is the desktop
//! model's business (`omorfos_core::Switcher`).

use std::sync::Arc;
use std::time::Duration;

use omorfos_config::{Color, Config, Modifiers, Part, State, States, Theme};
use omorfos_core::geometry::{Point as DomainPoint, Rect, Size as DomainSize};
use omorfos_core::{Switcher, SwitcherLayout, SwitcherMetrics, Used, WindowId};
use omorfos_ui::{Area, Image};
use smithay::backend::renderer::element::AsRenderElements as _;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::element::texture::TextureRenderElement;
use smithay::backend::renderer::element::utils::CropRenderElement;
use smithay::backend::renderer::element::{Id, Kind};
use smithay::backend::renderer::{ImportAll, ImportMem, Renderer, Texture};
use smithay::input::keyboard::{Keysym, ModifiersState};
use smithay::output::Output;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::utils::{Logical, Point, Rectangle, Transform};
use smithay::wayland::xdg_toplevel_icon::XdgToplevelIconManager;

use crate::Scale;
use crate::bridge;
use crate::compositor::{Compositor, Element, Painted, View};
use crate::icons::{Icon, WindowIcons};
use crate::outputs::domain;
use crate::paint::{self, Places, Shown};

/// The room between one window's place and the next, in units of layout.
const GAP: u32 = 8;
/// The least room between the switcher and the edges of the screen.
const MARGIN: u32 = 24;
/// The room between a window's picture and its title.
const UNDER: u32 = 6;
/// How tall a picture is for how wide it is: that of most screens.
const SHAPE: (u32, u32) = (10, 16);
/// The narrowest a window's place is made, whatever the theme says.
const NARROWEST: u32 = 80;

/// The switcher, open or not, and what it goes by.
#[derive(Debug)]
pub(crate) struct Switch {
    /// The switcher, while it is open.
    open: Option<Switching>,
    /// The windows in the order they were last used.
    used: Used,
    /// The keys held with the binding being carried out, but for Shift;
    /// none for what was not asked by a key.
    pub(crate) held: Modifiers,
    /// The icons of windows.
    pub(crate) icons: WindowIcons,
    /// The protocol by which programs give their windows icons.
    _given: XdgToplevelIconManager,
}

impl Switch {
    /// The switcher, closed, and the protocol for icons offered on
    /// `display`, asking for icons of the size the switcher draws them
    /// largest.
    pub(crate) fn offer(display: &DisplayHandle) -> Self {
        let mut given = XdgToplevelIconManager::new::<Compositor>(display);
        given.add_icon_size(paint::LARGE_ICON.cast_signed());
        Self {
            open: None,
            used: Used::default(),
            held: Modifiers::NONE,
            icons: WindowIcons::default(),
            _given: given,
        }
    }
}

/// The switcher while it is open.
#[derive(Debug)]
pub(crate) struct Switching {
    switcher: Switcher,
    /// The keys held with the one that opened it, but for Shift, which
    /// goes back: letting go of any of them chooses. None, as when it was
    /// opened from a menu, leaves it open until Enter, Escape or a click.
    held: Modifiers,
    /// The screen it is on, as it was when it opened.
    screen: Rect,
    /// Its pixels, kept until they would be painted differently.
    painted: Option<Drawn>,
}

/// The switcher's pixels, and what they were painted from.
#[derive(Debug)]
struct Drawn {
    layout: SwitcherLayout,
    shown: Vec<Shown<Icon>>,
    /// What the renderer knows each icon given as pixels by, beside the
    /// title and large, so that it is drawn again only when it changes.
    ids: Vec<[Id; 2]>,
    config: Arc<Config>,
    scale: Scale,
    pixels: Painted,
}

/// An icon a program gave as pixels, drawn by the renderer in the middle
/// of `area`, as large as fits but no larger than its pixels, which would
/// only make it blocky. Icons are square, as the protocol has them. Nothing
/// if the renderer cannot read it.
fn given_icon<R>(
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
    let texture = renderer
        .import_buffer(buffer, None, &[])
        .and_then(Result::ok);
    let context = renderer.context_id();
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

/// How many pixels across an icon is looked up at, to be drawn sharp at
/// `scale` where it is drawn largest.
fn icon_pixels(scale: Scale) -> u32 {
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

/// The colour of a window's title in the switcher, which an icon that is
/// only a shape is drawn in.
fn ink(theme: &Theme, picked: bool) -> Color {
    let states = States::NONE.when(State::Active, picked);
    theme.style(Part::SwitcherTitle, states).color
}

/// The modifiers held, but for Shift.
pub(crate) fn held_with(state: &ModifiersState) -> Modifiers {
    let unshifted = ModifiersState {
        shift: false,
        ..*state
    };
    bridge::modifiers(&unshifted)
}

/// How the switcher is laid out under `theme`, and how tall each window's
/// picture is in its place.
fn metrics(theme: &Theme) -> (SwitcherMetrics, u32) {
    let item = theme.style(Part::SwitcherItem, States::NONE);
    let width = item.min_width.max(NARROWEST);
    let room = width.saturating_sub(item.inset().across());
    let picture = room * SHAPE.0 / SHAPE.1;
    let title = paint::line_height(theme.font(Part::SwitcherTitle, States::NONE).size.get());
    let height = item.inset().down() + picture + UNDER + title;
    let metrics = SwitcherMetrics {
        item: DomainSize::new(width, height),
        gap: GAP,
        inset: theme.style(Part::Switcher, States::NONE).inset().left,
        margin: MARGIN,
    };
    (metrics, picture)
}

impl Compositor {
    /// Opens the switcher, going back if `back`, on the screen the pointer
    /// is on; or, while it is open, goes on to the next window or back.
    pub(crate) fn switch_windows(&mut self, back: bool) {
        if let Some(switching) = &mut self.switch.open {
            if back {
                switching.switcher.previous();
            } else {
                switching.switcher.next();
            }
            return;
        }
        let windows = self.desktop.windows().map(|listed| listed.window.id);
        let windows = self.switch.used.sorted(windows);
        let screen = self
            .pointer_head()
            .map(|head| domain(head.area()))
            .unwrap_or_default();
        self.switch.open = Switcher::open(windows, back).map(|switcher| Switching {
            switcher,
            held: self.switch.held,
            screen,
            painted: None,
        });
    }

    /// Closes the switcher, choosing the window it is at if `chosen`.
    pub(crate) fn close_switcher(&mut self, chosen: bool) {
        let picked = self
            .switch
            .open
            .take()
            .map(|switching| switching.switcher.picked());
        if let Some(window) = picked.filter(|_| chosen) {
            self.desktop.activate(window);
        }
    }

    /// Takes a key pressed while the switcher is open: Escape closes it,
    /// Enter chooses, and the arrows walk it. Returns whether the key was
    /// the switcher's; any other is left to bindings, among them the one
    /// that goes on to the next window.
    pub(crate) fn switcher_key(&mut self, keysym: Keysym) -> bool {
        let Some(switching) = &mut self.switch.open else {
            return false;
        };
        match keysym {
            Keysym::Escape => self.close_switcher(false),
            Keysym::Return | Keysym::KP_Enter => self.close_switcher(true),
            Keysym::Left | Keysym::Up => switching.switcher.previous(),
            Keysym::Right | Keysym::Down => switching.switcher.next(),
            _ => return false,
        }
        true
    }

    /// Takes the keys held once one has been let go: if any of those held
    /// with the one that opened the switcher is no longer, the window it is
    /// at is chosen.
    pub(crate) fn switcher_let_go(&mut self, state: &ModifiersState) {
        let held = self.switch.open.as_ref().map(|switching| switching.held);
        let let_go = held.filter(|held| *held != Modifiers::NONE);
        if let_go.is_some_and(|held| !held_with(state).contains(held)) {
            self.close_switcher(true);
        }
    }

    /// Where the switcher and each window in it are shown, while it is
    /// open, as the theme lays it out.
    #[must_use]
    pub fn switcher_layout(&self) -> Option<SwitcherLayout> {
        let (metrics, _) = metrics(&self.store.current().theme);
        let open = self.switch.open.as_ref();
        open.map(|switching| switching.switcher.layout(switching.screen, metrics))
    }

    /// Whether the switcher is open.
    pub(crate) const fn switcher_open(&self) -> bool {
        self.switch.open.is_some()
    }

    /// Takes a press of the pointer while the switcher is open: on a
    /// window it chooses it, anywhere else it closes the switcher.
    pub(crate) fn switcher_press(&mut self, at: DomainPoint) {
        let theme = &self.store.current().theme;
        let (metrics, _) = metrics(theme);
        let open = self.switch.open.as_mut();
        let under = open.and_then(|switching| {
            let layout = switching.switcher.layout(switching.screen, metrics);
            let under = layout.items.iter().find(|(_, place, _)| place.contains(at));
            let window = under.map(|(window, _, _)| *window)?;
            switching.switcher.pick(window);
            Some(window)
        });
        self.close_switcher(under.is_some());
    }

    /// Notes the window in use, unless the switcher is open, whose order
    /// stands still; and closes the switcher while the session is locked,
    /// or a prompt asks for a secret over it.
    pub(crate) fn note_use(&mut self) {
        if self.is_locked() || self.sealed().is_some() {
            self.switch.open = None;
        }
        if self.switch.open.is_none() {
            self.switch.used.note(self.desktop.focused());
        }
    }

    /// Forgets a window that has gone, and takes it out of the switcher.
    pub(crate) fn forget_use(&mut self, window: WindowId) {
        self.switch.used.forget(window);
        let switching = self.switch.open.take();
        self.switch.open = switching.and_then(|mut switching| {
            switching.switcher = switching.switcher.without(window)?;
            Some(switching)
        });
    }

    /// Paints the switcher again if what it shows, the theme or the scale
    /// changed since it was painted.
    pub(crate) fn repaint_switcher(&mut self) {
        let config = Arc::clone(self.store.current());
        let scale = self.pointer_scale();
        // Taken out while it is painted, and put back.
        let Some(mut switching) = self.switch.open.take() else {
            return;
        };
        let (metrics, picture) = metrics(&config.theme);
        let layout = switching.switcher.layout(switching.screen, metrics);
        let pixels = icon_pixels(scale);
        let mut icons = std::mem::take(&mut self.switch.icons);
        let shown: Vec<Shown<Icon>> = layout
            .items
            .iter()
            .map(|(window, _, picked)| {
                let ink = ink(&config.theme, *picked);
                let listed = self.desktop.window(*window);
                let mapped = self.mapped(*window);
                let app_id = listed
                    .map(|listed| listed.app_id.as_str())
                    .unwrap_or_default();
                let icon = mapped.and_then(|mapped| icons.of(&mapped.surface, app_id, pixels, ink));
                Shown {
                    title: listed
                        .map(|listed| listed.title.clone())
                        .unwrap_or_default(),
                    icon,
                    pictured: mapped.is_some() && !self.desktop.private(*window),
                }
            })
            .collect();
        let same = |drawn: &Drawn| {
            drawn.layout == layout
                && drawn.shown == shown
                && Arc::ptr_eq(&drawn.config, &config)
                && drawn.scale == scale
        };
        if !switching.painted.as_ref().is_some_and(same) {
            let painted: Vec<Shown<Option<Image>>> = layout
                .items
                .iter()
                .zip(&shown)
                .map(|((_, _, picked), shown)| {
                    let ink = ink(&config.theme, *picked);
                    let icon = shown.icon.as_ref().map(|icon| match icon {
                        Icon::Named(name) => icons.picture(name, pixels, ink).cloned(),
                        Icon::Given(_) => None,
                    });
                    Shown {
                        title: shown.title.clone(),
                        icon,
                        pictured: shown.pictured,
                    }
                })
                .collect();
            let theme = &config.theme;
            let canvas = paint::switcher(&mut self.text, theme, &layout, &painted, picture, scale);
            let pixels = Painted::new(&canvas);
            switching.painted = Some(Drawn {
                ids: shown.iter().map(|_| [Id::new(), Id::new()]).collect(),
                layout,
                shown,
                config,
                scale,
                pixels,
            });
        }
        self.switch.icons = icons;
        self.switch.open = Some(switching);
    }

    /// The switcher, on the screen it is on: each window's picture, made
    /// small to fit its place, over the switcher's own pixels. A window a
    /// rule says is private has its title and no picture.
    pub(crate) fn switcher_elements<R>(
        &self,
        renderer: &mut R,
        output: &Output,
        view: View,
    ) -> Vec<Element<R>>
    where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let switching = self.switch.open.as_ref().filter(|switching| {
            let here = self.head(output).map(|head| domain(head.area()));
            here == Some(switching.screen)
        });
        let Some(drawn) = switching.and_then(|switching| switching.painted.as_ref()) else {
            return Vec::new();
        };
        let theme = &drawn.config.theme;
        let (_, tall) = metrics(theme);
        let mut elements = Vec::new();
        let given = drawn.layout.items.iter().zip(&drawn.shown).zip(&drawn.ids);
        for (((_, place, picked), shown), ids) in given {
            let Some(Icon::Given(buffer)) = &shown.icon else {
                continue;
            };
            let states = States::NONE.when(State::Active, *picked);
            let place = (
                place.origin.x,
                place.origin.y,
                place.size.width,
                place.size.height,
            );
            let places = Places::of(place, theme, states, tall, true);
            let large = (!shown.pictured).then_some(places.large);
            let areas = [Some(places.icon), large].into_iter().zip(ids);
            for (area, id) in areas.filter_map(|(area, id)| Some((area?, id))) {
                elements.extend(given_icon(renderer, buffer, area, id, view));
            }
        }
        for (window, place, picked) in &drawn.layout.items {
            let states = States::NONE.when(State::Active, *picked);
            let style = drawn.config.theme.style(Part::SwitcherItem, states);
            let room = (
                place.origin.x.saturating_add_unsigned(style.inset().left),
                place.origin.y.saturating_add_unsigned(style.inset().top),
                place.size.width.saturating_sub(style.inset().across()),
                tall,
            );
            let mapped = self
                .mapped(*window)
                .filter(|_| !self.desktop.private(*window));
            let Some(mapped) = mapped else { continue };
            // As small as it has to be to fit, its shape kept, in the
            // middle of where it goes.
            let size = mapped.window.geometry().size;
            let across = f64::from(room.2) / f64::from(size.w.max(1));
            let down = f64::from(room.3) / f64::from(size.h.max(1));
            let factor = across.min(down);
            let shown = (f64::from(size.w) * factor, f64::from(size.h) * factor);
            let left = f64::from(room.0) + (f64::from(room.2) - shown.0) / 2.0;
            let top = f64::from(room.1) + (f64::from(room.3) - shown.1) / 2.0;
            let corner = view.pixel(Point::<f64, Logical>::from((left, top)));
            let scale = view.scale.factor() * factor;
            let within = mapped.window.geometry().loc.to_f64().to_physical(scale);
            let at = corner - within.to_i32_round();
            let picture = smithay::utils::Size::<f64, Logical>::from(shown);
            let crop = Rectangle::new(
                corner,
                picture.to_physical(view.scale.factor()).to_i32_round(),
            );
            let scale = smithay::utils::Scale::from(scale);
            let surfaces = mapped
                .window
                .render_elements::<WaylandSurfaceRenderElement<R>>(renderer, at, scale, 1.0);
            let cropped = surfaces
                .into_iter()
                .filter_map(|surface| CropRenderElement::from_element(surface, scale, crop));
            elements.extend(cropped.map(Element::Cropped));
        }
        let at =
            Point::<i32, Logical>::from((drawn.layout.area.origin.x, drawn.layout.area.origin.y));
        elements.extend(drawn.pixels.element(renderer, at.to_f64(), view, 1.0));
        elements
    }

    /// Tells each window shown in the switcher on `output` that what it
    /// drew has been shown, so that its picture there is kept up to date
    /// as it would be on the screen.
    pub(crate) fn send_switched_frames(&self, output: &Output, elapsed: Duration) {
        let here = self.head(output).map(|head| domain(head.area()));
        let switching = self
            .switch
            .open
            .as_ref()
            .filter(|switching| here == Some(switching.screen));
        let Some(drawn) = switching.and_then(|switching| switching.painted.as_ref()) else {
            return;
        };
        let shown = drawn
            .layout
            .items
            .iter()
            .filter_map(|(window, _, _)| self.mapped(*window));
        for mapped in shown {
            let output = output.clone();
            mapped
                .window
                .send_frame(&output, elapsed, Some(Duration::ZERO), |_, _| {
                    Some(output.clone())
                });
        }
    }
}
