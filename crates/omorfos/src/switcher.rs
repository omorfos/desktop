//! The switcher on `Alt+Tab`, drawn by the compositor as the menu is
//! (ADR 38): opened by a binding, walked by it and by the arrows while the
//! keys held with it are, and the window it is at chosen when they are let
//! go. Which windows, in what order and where each is shown is the desktop
//! model's business (`omorfos_core::Switcher`).
//!
//! `switcher.style` has it show them in rows, or one behind another on a
//! diagonal (ADR 75), each window's place painted once as large as it is
//! in front and drawn smaller further back. Going on to the next window
//! turns the queue there over [`TURN`], unless motion is to be reduced.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use omorfos_config::{Color, Config, Easing, Modifiers, Part, State, States, SwitcherStyle, Theme};
use omorfos_core::geometry::{Point as DomainPoint, Rect, Size as DomainSize};
use omorfos_core::{QueueMetrics, Switcher, SwitcherLayout, SwitcherMetrics, Used, WindowId};
use omorfos_ui::{Area, Image};
use smithay::backend::renderer::element::solid::{SolidColorBuffer, SolidColorRenderElement};
use smithay::backend::renderer::element::{Id, Kind};
use smithay::backend::renderer::{ImportAll, ImportMem, Renderer, Texture};
use smithay::input::keyboard::{Keysym, ModifiersState};
use smithay::output::Output;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::utils::{Logical, Point, Rectangle};
use smithay::reexports::wayland_protocols::xdg::toplevel_icon::v1::server::xdg_toplevel_icon_manager_v1::XdgToplevelIconManagerV1;
use smithay::reexports::wayland_server::backend::ObjectId;

use crate::Scale;
use crate::bridge;
use crate::compositor::{Compositor, Element, Painted, View, color, logical};
use crate::glass::{self, Pane};
use crate::icons::{Icon, Making, Sizes, WindowIcons};
use crate::outputs::domain;
use crate::paint::{self, Places, Shown};
use crate::pictures::{fitted, shape, whole};
use crate::shadows::Shadowing;

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
/// The widest the window in front of the queue is made.
const WIDEST: u32 = 720;
/// How long the queue takes to turn to the next window, as long as
/// `docs/DESIGN.md` gives something moving to where it was sent.
const TURN: Duration = Duration::from_millis(240);
/// The farthest the queue is let fall behind the keys, in windows, so that
/// Tab held down does not send it spinning.
const FARTHEST: f64 = 2.0;
/// How long the keys that opened the switcher are held before it is shown,
/// so that a quick `Alt+Tab` goes back to the window before without it
/// showing for a moment, as GNOME's and Windows's do.
const SHOWN_AFTER: Duration = Duration::from_millis(200);
/// How long the switcher takes to fade in once it is shown, so that held
/// just long enough for it to be shown, and let go, it is hardly seen
/// rather than flashing.
const FADE_IN: Duration = Duration::from_millis(120);

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
    /// The icons programs are making for their windows, by the number of
    /// each, until it is set on one.
    pub(crate) making: HashMap<ObjectId, Making>,
    /// What dims the screen behind the switcher, kept as large as the
    /// screen and of the theme's colour, so that it is the same from one
    /// frame to the next.
    dim: SolidColorBuffer,
}

impl Switch {
    /// Keeps the dim behind the switcher as large as `screen`, the one it is
    /// on, and of the colour `config`'s theme gives it.
    fn dim_behind(&mut self, config: &Config, screen: Rect) {
        let backdrop = config.theme.style(Part::SwitcherBackdrop, States::NONE);
        let size = (
            screen.size.width.cast_signed(),
            screen.size.height.cast_signed(),
        );
        self.dim.update(size, color(backdrop.ground()));
    }

    /// The switcher, closed, and the protocol for icons offered on
    /// `display`, asking for icons of the size the switcher draws them
    /// largest.
    pub(crate) fn offer(display: &DisplayHandle) -> Self {
        let sizes = Sizes(vec![paint::LARGE_ICON.cast_signed()]);
        display.create_global::<Compositor, XdgToplevelIconManagerV1, _>(1, sizes);
        Self {
            open: None,
            used: Used::default(),
            held: Modifiers::NONE,
            icons: WindowIcons::default(),
            making: HashMap::new(),
            dim: SolidColorBuffer::default(),
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
    /// When it is shown, by the compositor's clock: a moment after it was
    /// opened by keys, and at once otherwise.
    shown: Duration,
    /// The queue on its way round: how far it had still to turn, in
    /// windows, and when it set out, by the compositor's clock.
    turn: Option<(f64, Duration)>,
    /// Its pixels, kept until they would be painted differently.
    painted: Option<Drawn>,
}

impl Switching {
    /// Goes on to the next window, or back if not `forward`, the queue
    /// turning there from where it is at `clock`, unless it is `still`.
    fn turn(&mut self, forward: bool, clock: Duration, still: bool) {
        if forward {
            self.switcher.next();
        } else {
            self.switcher.previous();
        }
        let way = if forward { -1.0 } else { 1.0 };
        let from = (turned(self.turn, clock) + way).clamp(-FARTHEST, FARTHEST);
        self.turn = (!still).then_some((from, clock));
    }
}

/// The switcher's pixels, and what they were painted from.
#[derive(Debug)]
struct Drawn {
    /// Where it all is, as it stands, turned no further.
    layout: SwitcherLayout,
    /// The windows painted: those in rows, or those the queue shows as it
    /// stands and on its way there, the one picked first.
    windows: Vec<WindowId>,
    shown: Vec<Shown<Icon>>,
    /// What the renderer knows each icon given as pixels by, beside the
    /// title and large, so that it is drawn again only when it changes.
    ids: Vec<[Id; 2]>,
    config: Arc<Config>,
    scale: Scale,
    /// The rows in their box, or the box under the queue with the title of
    /// the one picked.
    pixels: Painted,
    /// Each window's place in the queue, as large as it is in front; none
    /// in rows.
    cards: Vec<Card>,
    /// The shadow it casts, as a part on the second level does.
    shadow: Option<Shadowing>,
}

/// A window's place in the queue, painted, and what it was painted from,
/// so that only those that change are painted again as it turns.
#[derive(Debug)]
struct Card {
    window: WindowId,
    picked: bool,
    shown: Shown<Icon>,
    /// How large the one in front is.
    size: DomainSize,
    /// How large the window is.
    shape: (i32, i32),
    /// How large its picture is in front.
    picture: (u32, u32),
    /// Its border round its picture, and its background where that is
    /// not shown.
    pixels: Painted,
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

/// How the switcher stands the windows on a diagonal under `theme`, its
/// box under them as tall as the title in it.
fn queue_metrics(theme: &Theme) -> QueueMetrics {
    let style = theme.style(Part::Switcher, States::NONE);
    let font = theme.font(Part::SwitcherTitle, States::NONE.with(State::Active));
    QueueMetrics {
        widest: WIDEST,
        title: paint::line_height(font.size.get()) + style.inset().down(),
        gap: 2 * GAP,
        margin: MARGIN,
    }
}

/// How far the queue has still to turn at `clock`, in windows, easing in
/// and out on its way.
fn turned(turn: Option<(f64, Duration)>, clock: Duration) -> f64 {
    turn.map_or(0.0, |(from, began)| {
        let share = clock.saturating_sub(began).as_secs_f32() / TURN.as_secs_f32();
        from * f64::from(1.0 - Easing::EaseInOut.at(share.min(1.0)))
    })
}

/// Where the switcher and each window in it are shown, as `config` lays it
/// out, the queue `turning` windows short of where it stands.
fn laid_out(config: &Config, switching: &Switching, turning: f64) -> SwitcherLayout {
    let (switcher, screen) = (&switching.switcher, switching.screen);
    match config.general.switcher.style {
        SwitcherStyle::Flat => switcher.layout(screen, metrics(&config.theme).0),
        SwitcherStyle::Diagonal => switcher.queue(screen, queue_metrics(&config.theme), turning),
    }
}

/// The windows the switcher may show before it is painted again, as
/// `config` lays it out: those `layout` shows as it stands, and, while the
/// queue turns, those it shows on the way there from where it set out.
fn shown_windows(config: &Config, switching: &Switching, layout: &SwitcherLayout) -> Vec<WindowId> {
    let from = switching.turn.map_or(0.0, |(from, _)| from);
    let on_the_way = [from, -2.0, -1.0, 1.0, 2.0]
        .into_iter()
        .filter(|turning| turning * from > 0.0 && turning.abs() <= from.abs());
    let mut windows: Vec<WindowId> = layout.items.iter().map(|item| item.window).collect();
    for turning in on_the_way {
        for item in laid_out(config, switching, turning).items {
            if !windows.contains(&item.window) {
                windows.push(item.window);
            }
        }
    }
    windows
}

/// What the switcher is painted from: where it all is as it stands, the
/// windows painted, what is shown of each, that as it is painted, and how
/// large each window is.
type Painting<'a> = (
    &'a SwitcherLayout,
    &'a [WindowId],
    &'a [Shown<Icon>],
    &'a [Shown<Option<Image>>],
    &'a [(i32, i32)],
);

/// Paints the switcher as `config` lays it out, `layout` as it stands, with
/// what is `shown` of each of `windows`, `painted` as it is to be painted:
/// the rows in their box; or the box under the queue, with the title of
/// `picked`, and each window's place, as large as it is in front, those
/// `kept` from before taken as they are where nothing of them changed.
fn paint_switcher(
    text: &mut paint::Text,
    config: &Config,
    (layout, windows, shown, painted, shapes): Painting<'_>,
    (picked, mut kept): (WindowId, Vec<Card>),
    scale: Scale,
) -> (paint::Canvas, Vec<Card>) {
    let theme = &config.theme;
    match config.general.switcher.style {
        SwitcherStyle::Flat => {
            let (_, picture) = metrics(theme);
            let canvas = paint::switcher(text, theme, layout, painted, picture, scale);
            (canvas, Vec::new())
        }
        SwitcherStyle::Diagonal => {
            let size = layout.area.size;
            let first = painted.first();
            let title = first.map_or("", |shown| shown.title.as_str());
            let icon = first.and_then(|shown| shown.icon.as_ref());
            let canvas = paint::switcher_title(
                text,
                theme,
                (size.width, size.height),
                (title, icon.and_then(Option::as_ref)),
                scale,
            );
            let front = layout.items.first().map(|item| item.place.size);
            let front = front.unwrap_or_default();
            let painting = windows.iter().zip(shown).zip(painted).zip(shapes);
            let cards = painting.map(|(((window, shown), painting), shape)| {
                let picked = *window == picked;
                let same = |card: &Card| {
                    (card.window, card.picked, card.size, card.shape)
                        == (*window, picked, front, *shape)
                        && card.shown == *shown
                };
                let at = kept.iter().position(same);
                let pixels = at.map(|at| kept.swap_remove(at).pixels);
                // Its picture as large as it is in front.
                let room = (0.0, 0.0, f64::from(front.width), f64::from(front.height));
                let (_, _, across, down) = fitted(*shape, room);
                let picture = (whole(across).cast_unsigned(), whole(down).cast_unsigned());
                Card {
                    window: *window,
                    picked,
                    shown: shown.clone(),
                    size: front,
                    shape: *shape,
                    picture,
                    pixels: pixels.unwrap_or_else(|| {
                        let card = paint::switcher_window(theme, picture, picked, painting, scale);
                        Painted::new(&card)
                    }),
                }
            });
            (canvas, cards.collect())
        }
    }
}

/// Adds the switcher's shadow under what has been added of it, and its
/// glass, as much of each `seen` as the switcher is.
fn switcher_box<R>(
    renderer: &mut R,
    drawn: &Drawn,
    (view, seen): (View, f32),
    (elements, panes): (&mut Vec<Element<R>>, &mut Vec<Pane>),
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Clone + Texture + Send + 'static,
{
    let glass = elements.len();
    if let Some(shadow) = &drawn.shadow {
        shadow.elements(renderer, seen, view, elements);
    }
    let style = drawn.config.theme.style(Part::Switcher, States::NONE);
    let area = (logical(drawn.layout.area), view);
    let key = (glass::key(&"switcher"), glass, elements.len());
    panes.extend(Pane::of(&style, area, key, seen));
}

/// How much of the switcher is seen at `clock`: none as it is shown, and
/// all of it once it has faded in, easing out.
fn seen_at(switching: &Switching, clock: Duration) -> f32 {
    let gone = clock.saturating_sub(switching.shown);
    Easing::EaseOut.at((gone.as_secs_f32() / FADE_IN.as_secs_f32()).min(1.0))
}

/// A rectangle of the desktop model as the painting code has it.
const fn area(rect: Rect) -> Area {
    (
        rect.origin.x,
        rect.origin.y,
        rect.size.width,
        rect.size.height,
    )
}

impl Compositor {
    /// Opens the switcher, going back if `back`, on the screen the pointer
    /// is on; or, while it is open, goes on to the next window or back.
    pub(crate) fn switch_windows(&mut self, back: bool) {
        let (clock, still) = (self.clock(), self.switcher_still());
        if let Some(switching) = &mut self.switch.open {
            switching.turn(!back, clock, still);
            return;
        }
        let windows = self.desktop.windows().map(|listed| listed.window.id);
        let windows = self.switch.used.sorted(windows);
        let screen = self
            .pointer_head()
            .map(|head| domain(head.area()))
            .unwrap_or_default();
        let held = self.switch.held;
        let wait = if held == Modifiers::NONE {
            Duration::ZERO
        } else {
            SHOWN_AFTER
        };
        self.switch.open = Switcher::open(windows, back).map(|switcher| Switching {
            switcher,
            held,
            screen,
            shown: clock + wait,
            turn: None,
            painted: None,
        });
    }

    /// Whether the switcher goes from one window to the next at once: in
    /// rows, and with motion reduced.
    fn switcher_still(&self) -> bool {
        let general = &self.config().general;
        general.appearance.reduce_motion || general.switcher.style == SwitcherStyle::Flat
    }

    /// The switcher while it is open and shown.
    fn shown_switcher(&self) -> Option<&Switching> {
        let open = self.switch.open.as_ref();
        open.filter(|switching| self.clock() >= switching.shown)
    }

    /// When the switcher, open, is to be shown, if it is not yet, by the
    /// compositor's clock.
    pub(crate) fn switcher_due(&self) -> Option<Duration> {
        let open = self.switch.open.as_ref();
        let hidden = open.filter(|switching| self.clock() < switching.shown);
        hidden.map(|switching| switching.shown)
    }

    /// Whether the switcher is fading in, and to be drawn again a frame on.
    pub(crate) fn switcher_fading(&self) -> bool {
        let shown = self.shown_switcher();
        shown.is_some_and(|switching| self.clock() < switching.shown + FADE_IN)
    }

    /// Whether the queue is turning, and to be drawn again a frame on.
    pub(crate) fn switcher_turning(&self) -> bool {
        let turn = self
            .switch
            .open
            .as_ref()
            .and_then(|switching| switching.turn);
        turn.is_some_and(|(_, began)| self.clock() < began + TURN)
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
        let (clock, still) = (self.clock(), self.switcher_still());
        let Some(switching) = &mut self.switch.open else {
            return false;
        };
        match keysym {
            Keysym::Escape => self.close_switcher(false),
            Keysym::Return | Keysym::KP_Enter => self.close_switcher(true),
            Keysym::Left | Keysym::Up => switching.turn(false, clock, still),
            Keysym::Right | Keysym::Down => switching.turn(true, clock, still),
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
    /// open, as the theme and the configuration lay it out, and as far as
    /// the queue has turned.
    #[must_use]
    pub fn switcher_layout(&self) -> Option<SwitcherLayout> {
        let config = self.store.current();
        let open = self.switch.open.as_ref();
        open.map(|switching| laid_out(config, switching, turned(switching.turn, self.clock())))
    }

    /// Whether the switcher is open.
    pub(crate) const fn switcher_open(&self) -> bool {
        self.switch.open.is_some()
    }

    /// Takes a press of the pointer while the switcher is open: on a
    /// window it chooses the one in front there, anywhere else it closes
    /// the switcher.
    pub(crate) fn switcher_press(&mut self, at: DomainPoint) {
        let config = Arc::clone(self.store.current());
        let clock = self.clock();
        // Before it is shown, nothing of it is there to be clicked on.
        let open = self.switch.open.as_mut();
        let open = open.filter(|switching| clock >= switching.shown);
        let under = open.and_then(|switching| {
            let layout = laid_out(&config, switching, turned(switching.turn, clock));
            let under = layout.items.iter().find(|item| item.place.contains(at));
            let window = under.map(|item| item.window)?;
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

    /// Forgets a window that has gone, and takes it out of the switcher,
    /// which stands still where it has got to.
    pub(crate) fn forget_use(&mut self, window: WindowId) {
        self.switch.used.forget(window);
        let switching = self.switch.open.take();
        self.switch.open = switching.and_then(|mut switching| {
            switching.switcher = switching.switcher.without(window)?;
            switching.turn = None;
            Some(switching)
        });
    }

    /// How large each of `windows` is, and one not mapped as large as the
    /// place in front in `layout`.
    fn shapes(&self, windows: &[WindowId], layout: &SwitcherLayout) -> Vec<(i32, i32)> {
        let front = layout.items.first().map(|item| item.place.size);
        let front = front.unwrap_or_default();
        let front = (front.width.cast_signed(), front.height.cast_signed());
        let shapes = windows
            .iter()
            .map(|window| self.mapped(*window).map_or(front, shape));
        shapes.collect()
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
        let layout = laid_out(&config, &switching, 0.0);
        let windows = shown_windows(&config, &switching, &layout);
        self.switch.dim_behind(&config, switching.screen);
        let picked = switching.switcher.picked();
        let pixels = crate::icons::icon_pixels(paint::LARGE_ICON, scale);
        let mut icons = std::mem::take(&mut self.switch.icons);
        let shown: Vec<Shown<Icon>> = windows
            .iter()
            .map(|window| {
                let ink = ink(&config.theme, *window == picked);
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
                    pictured: mapped.is_some() && !self.blank(*window),
                }
            })
            .collect();
        let same = |drawn: &Drawn| {
            drawn.layout == layout
                && drawn.windows == windows
                && drawn.shown == shown
                && Arc::ptr_eq(&drawn.config, &config)
                && drawn.scale == scale
        };
        if !switching.painted.as_ref().is_some_and(same) {
            let painted: Vec<Shown<Option<Image>>> = windows
                .iter()
                .zip(&shown)
                .map(|(window, shown)| {
                    let ink = ink(&config.theme, *window == picked);
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
            // What was painted before, but for the shadow, is kept only as
            // long as it was painted as it is now.
            let had = switching.painted.take();
            let same = |drawn: &Drawn| Arc::ptr_eq(&drawn.config, &config) && drawn.scale == scale;
            let painted_so = had.as_ref().is_some_and(same);
            let (mut kept, had) =
                had.map_or((Vec::new(), None), |drawn| (drawn.cards, drawn.shadow));
            kept.retain(|_| painted_so);
            let shapes = self.shapes(&windows, &layout);
            let painting = (
                &layout,
                windows.as_slice(),
                shown.as_slice(),
                painted.as_slice(),
                shapes.as_slice(),
            );
            let (canvas, cards) =
                paint_switcher(&mut self.text, &config, painting, (picked, kept), scale);
            let pixels = Painted::new(&canvas);
            let style = theme.style(Part::Switcher, States::NONE);
            let area = &layout.area;
            let placed = (
                area.origin.x,
                area.origin.y,
                area.size.width,
                area.size.height,
            );
            let shadow = self.animation.shades.cast(&style, scale, placed, had);
            switching.painted = Some(Drawn {
                ids: shown.iter().map(|_| [Id::new(), Id::new()]).collect(),
                layout,
                windows,
                shown,
                config,
                scale,
                pixels,
                cards,
                shadow,
            });
        }
        self.switch.icons = icons;
        self.switch.open = Some(switching);
    }

    /// Adds the switcher, on the screen it is on: each window's picture,
    /// made small to fit its place, over the switcher's own pixels, and its
    /// glass if the theme makes it glass. A window a rule says is private
    /// has its title and no picture.
    pub(crate) fn switcher_elements<R>(
        &self,
        renderer: &mut R,
        output: &Output,
        view: View,
        (elements, panes): (&mut Vec<Element<R>>, &mut Vec<Pane>),
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let switching = self.shown_switcher().filter(|switching| {
            let here = self.head(output).map(|head| domain(head.area()));
            here == Some(switching.screen)
        });
        let painted = switching
            .and_then(|switching| switching.painted.as_ref().map(|drawn| (switching, drawn)));
        let Some((switching, drawn)) = painted else {
            return;
        };
        let seen = (view, seen_at(switching, self.clock()));
        if drawn.config.general.switcher.style == SwitcherStyle::Diagonal {
            self.queue_elements(renderer, (switching, drawn), seen, (elements, panes));
        } else {
            self.rows_elements(renderer, drawn, seen, (elements, panes));
        }
        self.switcher_backdrop(output, drawn, seen, (elements, panes));
    }

    /// Adds the rows: each window's picture, made small to fit its place,
    /// over the switcher's own pixels, and the box's shadow and glass, as
    /// much of all of it `seen` as the switcher is.
    fn rows_elements<R>(
        &self,
        renderer: &mut R,
        drawn: &Drawn,
        (view, seen): (View, f32),
        (elements, panes): (&mut Vec<Element<R>>, &mut Vec<Pane>),
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let theme = &drawn.config.theme;
        let (_, tall) = metrics(theme);
        let given = drawn.layout.items.iter().zip(&drawn.shown).zip(&drawn.ids);
        for ((item, shown), ids) in given {
            let Some(Icon::Given(buffer)) = &shown.icon else {
                continue;
            };
            let states = States::NONE.when(State::Active, item.picked);
            let places = Places::of(area(item.place), theme, states, tall, true);
            let large = (!shown.pictured).then_some(places.large);
            let areas = [Some(places.icon), large].into_iter().zip(ids);
            for (area, id) in areas.filter_map(|(area, id)| Some((area?, id))) {
                elements.extend(
                    self.switch
                        .icons
                        .given(renderer, buffer, (area, seen), id, view),
                );
            }
        }
        for item in &drawn.layout.items {
            let states = States::NONE.when(State::Active, item.picked);
            let style = drawn.config.theme.style(Part::SwitcherItem, states);
            let place = item.place;
            let room = (
                f64::from(place.origin.x.saturating_add_unsigned(style.inset().left)),
                f64::from(place.origin.y.saturating_add_unsigned(style.inset().top)),
                f64::from(place.size.width.saturating_sub(style.inset().across())),
                f64::from(tall),
            );
            let window = (item.window, seen);
            self.picture_elements(renderer, window, room, view, (elements, panes));
        }
        let at =
            Point::<i32, Logical>::from((drawn.layout.area.origin.x, drawn.layout.area.origin.y));
        elements.extend(drawn.pixels.element(renderer, at.to_f64(), view, seen));
        switcher_box(renderer, drawn, (view, seen), (elements, panes));
    }

    /// Adds what is behind the switcher on `output`, under all of it: the
    /// dim of its screen, and the blur of what is under that, as the theme
    /// says, as much of each `seen` as the switcher is.
    fn switcher_backdrop<R>(
        &self,
        output: &Output,
        drawn: &Drawn,
        (view, seen): (View, f32),
        (elements, panes): (&mut Vec<Element<R>>, &mut Vec<Pane>),
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let scale = view.scale.factor();
        let dim = &self.switch.dim;
        let dim = SolidColorRenderElement::from_buffer(dim, (0, 0), scale, seen, Kind::Unspecified);
        elements.push(Element::Solid(dim));
        let backdrop = drawn
            .config
            .theme
            .style(Part::SwitcherBackdrop, States::NONE);
        let head = self
            .head(output)
            .filter(|_| backdrop.blur > 0 && backdrop.see_through());
        let whole = head.map(|head| {
            let area = head.area();
            let from = view.pixel(area.loc.to_f64());
            let to = view.pixel((area.loc + area.size.to_point()).to_f64());
            Rectangle::new(from, (to - from).to_size())
        });
        let key = glass::key(&("the switcher's backdrop", output.name()));
        let under = elements.len();
        let blurred = whole.and_then(|whole| {
            Pane::behind(
                vec![whole],
                (backdrop.blur, view),
                (key, under, under),
                seen,
            )
        });
        panes.extend(blurred);
    }

    /// Adds the queue: the box under it with the title of the window
    /// picked, over everything, and then each window, the one in front
    /// first, its picture over its place, both as far round as the queue
    /// has turned and as faint as it is there; and all of it as much `seen`
    /// as the switcher is.
    fn queue_elements<R>(
        &self,
        renderer: &mut R,
        (switching, drawn): (&Switching, &Drawn),
        (view, seen): (View, f32),
        (elements, panes): (&mut Vec<Element<R>>, &mut Vec<Pane>),
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let theme = &drawn.config.theme;
        let below = area(drawn.layout.area);
        let first = drawn.shown.first().zip(drawn.ids.first());
        let first = first.map(|(shown, ids)| (&shown.icon, ids));
        if let Some((Some(Icon::Given(buffer)), [beside, _])) = first {
            let (icon, _) = paint::title_places(below, theme, true);
            let icons = &self.switch.icons;
            elements.extend(icons.given(renderer, buffer, (icon, seen), beside, view));
        }
        let at = Point::<f64, Logical>::from((f64::from(below.0), f64::from(below.1)));
        elements.extend(drawn.pixels.element(renderer, at, view, seen));
        switcher_box(renderer, drawn, (view, seen), (elements, panes));
        let layout = laid_out(
            &drawn.config,
            switching,
            turned(switching.turn, self.clock()),
        );
        let front = drawn
            .layout
            .items
            .first()
            .map_or(1, |item| item.place.size.width);
        let front = f64::from(front.max(1));
        let painted = drawn.windows.iter().zip(&drawn.shown);
        let painted = painted.zip(drawn.cards.iter().zip(&drawn.ids));
        let cards = layout.items.iter().filter_map(|item| {
            let found = painted
                .clone()
                .find(|((window, _), _)| **window == item.window);
            found.map(|((_, shown), (card, ids))| (item, shown, card, ids))
        });
        for (item, shown, card, ids) in cards {
            let opacity = f32::from(item.opacity) / 1000.0 * seen;
            let place = area(item.place);
            let share = f64::from(place.2) / front;
            // Its picture as large as it is in front, made smaller with its
            // place, in the middle of it; and round it, its border.
            let across = f64::from(card.picture.0) * share;
            let down = f64::from(card.picture.1) * share;
            let left = f64::from(place.0) + (f64::from(place.2) - across) / 2.0;
            let top = f64::from(place.1) + (f64::from(place.3) - down) / 2.0;
            let states = States::NONE.when(State::Active, item.picked);
            let edges = theme.style(Part::SwitcherWindow, states).inset();
            let out = |edge: u32| f64::from(edge) * share;
            if let Some(Icon::Given(buffer)) = shown.icon.as_ref().filter(|_| !shown.pictured) {
                let side = (f64::from(paint::LARGE_ICON) * share).min(across).min(down);
                let large = (
                    whole(left + (across - side) / 2.0),
                    whole(top + (down - side) / 2.0),
                    whole(side).cast_unsigned(),
                    whole(side).cast_unsigned(),
                );
                let icons = &self.switch.icons;
                elements.extend(icons.given(renderer, buffer, (large, opacity), &ids[1], view));
            }
            let (window, picture) = ((item.window, opacity), (left, top, across, down));
            self.picture_elements(renderer, window, picture, view, (elements, panes));
            let at = Point::<f64, Logical>::from((left - out(edges.left), top - out(edges.top)));
            let size = (
                whole(across + out(edges.left) + out(edges.right)),
                whole(down + out(edges.top) + out(edges.bottom)),
            );
            let ring = card
                .pixels
                .stretched(renderer, (at, size.into()), view, opacity);
            elements.extend(ring);
        }
    }

    /// Tells each window shown in the switcher on `output` that what it
    /// drew has been shown, so that its picture there is kept up to date
    /// as it would be on the screen.
    pub(crate) fn send_switched_frames(&self, output: &Output, elapsed: Duration) {
        let here = self.head(output).map(|head| domain(head.area()));
        let switching = self
            .shown_switcher()
            .filter(|switching| here == Some(switching.screen));
        let Some(drawn) = switching.and_then(|switching| switching.painted.as_ref()) else {
            return;
        };
        let shown = drawn
            .layout
            .items
            .iter()
            .filter_map(|item| self.mapped(item.window));
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
