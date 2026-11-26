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
use omorfos_core::{
    QueueMetrics, Switcher, SwitcherItem, SwitcherLayout, SwitcherMetrics, Used, WindowId,
};
use omorfos_ui::{Area, Image};
use smithay::backend::renderer::element::solid::{SolidColorBuffer, SolidColorRenderElement};
use smithay::backend::renderer::element::{Id, Kind};
use smithay::backend::renderer::{ImportAll, ImportMem, Renderer, Texture};
use smithay::input::keyboard::{Keysym, ModifiersState};
use smithay::output::Output;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::utils::{Logical, Point};
use smithay::reexports::wayland_protocols::xdg::toplevel_icon::v1::server::xdg_toplevel_icon_manager_v1::XdgToplevelIconManagerV1;
use smithay::reexports::wayland_server::backend::ObjectId;

use crate::Scale;
use crate::bridge;
use crate::compositor::{Compositor, Element, Painted, Told, View, color, logical};
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
    /// Whether it shows programs, each once, and brings the one chosen
    /// forward whole, rather than windows.
    programs: bool,
    /// Whether it shows the windows of one program alone, as the key that
    /// goes on to the next window of a program opened it, which goes on
    /// through them.
    of_program: bool,
    /// When the row of the program picked's other windows began to be
    /// raised, by the compositor's clock: once the key that goes on to the
    /// next window of a program first went through them.
    raised: Option<Duration>,
    /// When that row began to come up from below the screen, as the
    /// program was picked.
    peeked: Option<Duration>,
    /// The row of the program picked before, going down out of sight from
    /// where it was, and since when.
    leaving: Option<(Vec<SwitcherItem>, Duration)>,
    /// The row of the program's windows, too long for the screen, on its
    /// way to keep the one gone to in sight: how many windows short of it
    /// it set out, and when, by the compositor's clock.
    slide: Option<(f64, Duration)>,
    /// The window the ring was round before the one gone to, its ring
    /// fading as the next comes in, and since when.
    ring_from: Option<(WindowId, Duration)>,
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
    /// What the renderer knows each icon given as pixels by, beside the
    /// title and large, so that it is drawn again only when it changes.
    ids: Vec<[Id; 2]>,
    config: Arc<Config>,
    scale: Scale,
    /// As it is on the screen, where a private window is shown as any
    /// other is, to the user it belongs to.
    screen: Face,
    /// As it is in pictures of the screen, if that is otherwise: a private
    /// window there without its title, and blank, its icon large, unless
    /// it is to be blurred.
    pictures: Option<Face>,
    /// The shadow it casts, as a part on the second level does.
    shadow: Option<Shadowing>,
    /// The ring round each of the program's windows in the queue's row,
    /// as large as its picture there, and that size, for the one gone to
    /// and the one before to each have its own.
    rings: Vec<(WindowId, Ring)>,
}

impl Drawn {
    /// What it shows on the screen, or in pictures of it.
    fn face(&self, screen: bool) -> &Face {
        let pictures = self.pictures.as_ref().filter(|_| !screen);
        pictures.unwrap_or(&self.screen)
    }
}

/// What the switcher shows of its windows, painted.
#[derive(Debug)]
struct Face {
    /// What it shows of each window, in the order of the windows painted.
    shown: Vec<Shown<Icon>>,
    /// The rows in their box, or the box under the queue with the title of
    /// the one picked.
    pixels: Painted,
    /// Each window's place in the queue, as large as it is in front; none
    /// in rows.
    cards: Vec<Card>,
}

/// A window's place in the queue, painted, and what it was painted from,
/// so that only those that change are painted again as it turns.
#[derive(Debug, Clone)]
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
/// out, the queue `turning` windows short of where it stands, and the
/// row of the program's windows `peeked` of the way up from below the
/// screen and `raised` of the way from there, the ring in it `sliding`
/// windows short of the one gone to.
fn laid_out(
    config: &Config,
    switching: &Switching,
    (turning, peeked, raised, sliding): (f64, f64, f64, f64),
) -> SwitcherLayout {
    let (switcher, screen) = (&switching.switcher, switching.screen);
    match config.general.switcher.style {
        SwitcherStyle::Flat => switcher.layout(screen, metrics(&config.theme).0),
        SwitcherStyle::Diagonal => switcher.queue(
            screen,
            queue_metrics(&config.theme),
            (turning, peeked, raised, sliding),
        ),
    }
}

/// A ring round a window's picture in the queue's row, painted, and how
/// large the picture is.
type Ring = (Painted, (u32, u32));

/// The ring round each of the program's windows in the queue's row, where
/// `layout` has them, of `shapes`, as large as its picture there: nothing
/// but its border.
fn paint_rings(
    theme: &Theme,
    (layout, shapes): (&SwitcherLayout, &[(i32, i32)]),
    scale: Scale,
) -> Vec<(WindowId, Ring)> {
    let blank = Shown {
        title: String::new(),
        icon: None,
        pictured: true,
    };
    let rings = layout.program.iter().zip(shapes).map(|(item, shape)| {
        let room = (
            0.0,
            0.0,
            f64::from(item.place.size.width),
            f64::from(item.place.size.height),
        );
        let (_, _, across, down) = fitted(*shape, room);
        let picture = (whole(across).cast_unsigned(), whole(down).cast_unsigned());
        let ring = paint::switcher_window(theme, picture, true, &blank, scale);
        (item.window, (Painted::new(&ring), picture))
    });
    rings.collect()
}

/// The ring `painted` round a picture of its size, in the middle of
/// `place`, as `theme` draws the one picked, as see-through as `opacity`
/// says.
fn ring_round<R>(
    renderer: &mut R,
    (painted, (across, down)): &Ring,
    place: Rect,
    (theme, view, opacity): (&Theme, View, f32),
) -> Option<Element<R>>
where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Clone + Texture + Send + 'static,
{
    let edges = theme
        .style(Part::SwitcherWindow, States::NONE.with(State::Active))
        .inset();
    let left = f64::from(place.origin.x) + (f64::from(place.size.width) - f64::from(*across)) / 2.0;
    let top = f64::from(place.origin.y) + (f64::from(place.size.height) - f64::from(*down)) / 2.0;
    let at =
        Point::<f64, Logical>::from((left - f64::from(edges.left), top - f64::from(edges.top)));
    let size = (
        (across + edges.across()).cast_signed(),
        (down + edges.down()).cast_signed(),
    );
    painted.stretched(renderer, (at, size.into()), view, opacity)
}

/// A share out of one, as the renderer takes how much of something is
/// seen.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a share from 0 to 1 by the clamp"
)]
fn f32_of(share: f64) -> f32 {
    share.clamp(0.0, 1.0) as f32
}

/// All of `place`, as a picture's room.
fn whole_place(place: Rect) -> crate::pictures::Exact {
    (
        f64::from(place.origin.x),
        f64::from(place.origin.y),
        f64::from(place.size.width),
        f64::from(place.size.height),
    )
}

/// How far along something begun `since` is at `clock`, over the time the
/// queue takes to turn, eased: all the way at once if `still`, and
/// `before` if it has not begun.
fn along(since: Option<Duration>, clock: Duration, still: bool, before: f64) -> f64 {
    since.map_or(before, |began| {
        let share = clock.saturating_sub(began).as_secs_f32() / TURN.as_secs_f32();
        if still {
            1.0
        } else {
            f64::from(Easing::EaseInOut.at(share.min(1.0)))
        }
    })
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
        for item in laid_out(config, switching, (turning, 1.0, 0.0, 0.0)).items {
            if !windows.contains(&item.window) {
                windows.push(item.window);
            }
        }
    }
    windows
}

/// How far the queue has still to turn at `clock`, how far the row of the
/// program's windows has come up from below the screen, and how far it is
/// raised from there, and how far the ring round the one gone to among
/// them has still to slide, as `switching` has them moving, or not if
/// `still`.
fn moved(switching: &Switching, clock: Duration, still: bool) -> (f64, f64, f64, f64) {
    (
        turned(switching.turn, clock),
        along(switching.peeked, clock, still, 1.0),
        along(switching.raised, clock, still, 0.0),
        turned(switching.slide, clock),
    )
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

/// Paints what the switcher shows as `config` lays it out, `layout` as it
/// stands, of `windows` as large as `shapes`: `shown` of each, `picked`
/// set apart, those of `kept` that nothing of has changed taken as they
/// are.
fn face(
    (text, icons): &mut (&mut paint::Text, &mut WindowIcons),
    config: &Config,
    (layout, windows, shapes): (&SwitcherLayout, &[WindowId], &[(i32, i32)]),
    (shown, picked, kept): (&[Shown<Icon>], WindowId, Vec<Card>),
    scale: Scale,
) -> Face {
    let pixels = crate::icons::icon_pixels(paint::LARGE_ICON, scale);
    let painted: Vec<Shown<Option<Image>>> = windows
        .iter()
        .zip(shown)
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
    let painting = (layout, windows, shown, painted.as_slice(), shapes);
    let (canvas, cards) = paint_switcher(text, config, painting, (picked, kept), scale);
    Face {
        shown: shown.to_vec(),
        pixels: Painted::new(&canvas),
        cards,
    }
}

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
    /// is on, of `programs` each once by its window used last or of every
    /// window; or, while it is open, goes on to the next or back.
    pub(crate) fn switch_windows(&mut self, back: bool, programs: bool) {
        let (clock, still) = (self.clock(), self.switcher_still());
        if self.switch.open.is_some() {
            self.leave_row();
        }
        if let Some(switching) = &mut self.switch.open {
            switching.turn(!back, clock, still);
            self.show_switched_program();
            return;
        }
        let windows = self.desktop.windows().map(|listed| listed.window.id);
        let windows = self.switch.used.sorted(windows);
        let windows = if programs {
            self.desktop.first_of_each_program(&windows)
        } else {
            windows
        };
        self.open_switcher(windows, back, (programs, false));
    }

    /// Opens the switcher on `windows`, going back if `back`, on the screen
    /// the pointer is on: of `programs`, or of every window, or of the
    /// windows of one program alone if `of_program`. It is shown a moment
    /// after, while keys are held, so that a quick press goes back without
    /// it, and at once otherwise.
    fn open_switcher(
        &mut self,
        windows: Vec<WindowId>,
        back: bool,
        (programs, of_program): (bool, bool),
    ) {
        let clock = self.clock();
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
            programs,
            of_program,
            raised: None,
            peeked: None,
            leaving: None,
            slide: None,
            ring_from: None,
        });
        self.show_switched_program();
    }

    /// Opens the switcher on the windows of the program in front alone, in
    /// the order they were last used, going back if `back`, while keys are
    /// held, as a quick press of them goes back to the window of the
    /// program used before, and holding them shows them to go through.
    /// Says whether it did: not for a program of one window, nor without
    /// keys held, as a script asks.
    pub(crate) fn switch_program_windows(&mut self, back: bool) -> bool {
        let focused = self.desktop.focused();
        let windows = focused.map(|window| self.desktop.program_of(window));
        let windows = self.switch.used.sorted(windows.into_iter().flatten());
        let opens = self.switch.held != Modifiers::NONE && windows.len() > 1;
        if opens {
            self.open_switcher(windows, back, (false, true));
        }
        opens
    }

    /// Has the switcher of programs, open, show the windows of the program
    /// picked, in the order of use, to be gone through.
    fn show_switched_program(&mut self) {
        let clock = self.clock();
        let switching = self.switch.open.as_mut();
        let Some(switching) = switching.filter(|switching| switching.programs) else {
            return;
        };
        let windows = self.desktop.program_of(switching.switcher.picked());
        let windows = self.switch.used.sorted(windows);
        switching.switcher.show_program(windows);
        switching.raised = None;
        switching.slide = None;
        switching.ring_from = None;
        switching.peeked = Some(clock.max(switching.shown));
    }

    /// Has the row of the program picked, if there is one, go down out of
    /// sight from where it is, as another is about to be picked.
    fn leave_row(&mut self) {
        let (clock, still) = (self.clock(), self.switcher_still());
        let config = Arc::clone(self.store.current());
        self.switch.open.iter_mut().for_each(|switching| {
            let row = laid_out(&config, switching, moved(switching, clock, still)).program;
            if !row.is_empty() {
                switching.leaving = Some((row, clock));
            }
        });
    }

    /// Goes on to the next window of the program the switcher of programs
    /// has picked, or back if `back`, the row of them raised as it first
    /// does, unless motion is to be reduced. The switcher of every window
    /// takes it and does nothing, rather than turn the windows behind it.
    /// Says whether the switcher was open to take it.
    pub(crate) fn switch_within(&mut self, back: bool) -> bool {
        let (clock, still) = (self.clock(), self.switcher_still());
        let Some(switching) = self.switch.open.as_mut() else {
            return false;
        };
        // Where the ring is in the row now, to slide on from.
        let at = |switcher: &Switcher| {
            let chosen = switcher.chosen();
            let index = switcher
                .program()
                .iter()
                .position(|window| *window == chosen);
            f64::from(u32::try_from(index.unwrap_or(0)).unwrap_or(0))
        };
        // Opened on one program's windows, it goes on through them.
        if switching.of_program {
            switching.turn(!back, clock, still);
            return true;
        }
        let was = at(&switching.switcher) - turned(switching.slide, clock);
        let before = switching.switcher.chosen();
        if switching.switcher.go_within(back) {
            switching.raised = switching.raised.or(Some(clock));
            let from = at(&switching.switcher) - was;
            switching.slide = (!still).then_some((from, clock));
            switching.ring_from = (!still).then_some((before, clock));
        }
        true
    }

    /// Whether the switcher goes from one window to the next at once: in
    /// rows, and with motion reduced.
    fn switcher_still(&self) -> bool {
        let general = &self.config().general;
        general.appearance.reduce_motion || general.switcher.style == SwitcherStyle::Flat
    }

    /// The switcher while it is open and shown.
    /// The windows the switcher shows, while it is shown, and those of the
    /// program picked: none otherwise.
    pub(crate) fn switcher_windows(&self) -> Vec<WindowId> {
        let shown = self.shown_switcher().map(|switching| &switching.switcher);
        let mut windows = shown.map_or_else(Vec::new, |switcher| switcher.windows().to_vec());
        let program = shown.map_or(&[][..], |switcher| switcher.program());
        let leaving = self
            .shown_switcher()
            .and_then(|switching| switching.leaving.as_ref());
        let leaving = leaving
            .into_iter()
            .flat_map(|(items, _)| items.iter().map(|item| &item.window));
        for window in program.iter().chain(leaving) {
            if !windows.contains(window) {
                windows.push(*window);
            }
        }
        windows
    }

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
        let open = self.switch.open.as_ref();
        let turn = open.and_then(|switching| switching.turn);
        let raised = open.and_then(|switching| switching.raised);
        let slide = open.and_then(|switching| switching.slide);
        // The row of the one picked before goes down while the next comes
        // up, which began at the same moment, or later, so that is enough.
        let peeked = open.and_then(|switching| switching.peeked);
        let moving = |began: Duration| self.clock() < began + TURN;
        turn.is_some_and(|(_, began)| moving(began))
            || raised.is_some_and(moving)
            || slide.is_some_and(|(_, began)| moving(began))
            || peeked.is_some_and(moving)
    }

    /// Closes the switcher, choosing the window it is at if `chosen`.
    pub(crate) fn close_switcher(&mut self, chosen: bool) {
        let picked = self
            .switch
            .open
            .take()
            .map(|switching| (switching.switcher.chosen(), switching.programs));
        match picked.filter(|_| chosen) {
            Some((window, true)) => self.desktop.activate_program(window),
            Some((window, false)) => self.desktop.activate(window),
            None => {}
        }
    }

    /// Takes a key pressed while the switcher is open: Escape closes it,
    /// Enter chooses, and the arrows walk it. Returns whether the key was
    /// the switcher's; any other is left to bindings, among them the one
    /// that goes on to the next window.
    pub(crate) fn switcher_key(&mut self, keysym: Keysym) -> bool {
        let (clock, still) = (self.clock(), self.switcher_still());
        if self.switch.open.is_none() {
            return false;
        }
        let forward = match keysym {
            Keysym::Escape => {
                self.close_switcher(false);
                return true;
            }
            Keysym::Return | Keysym::KP_Enter => {
                self.close_switcher(true);
                return true;
            }
            Keysym::Left | Keysym::Up => false,
            Keysym::Right | Keysym::Down => true,
            _ => return false,
        };
        self.leave_row();
        let open = self.switch.open.iter_mut();
        open.for_each(|switching| switching.turn(forward, clock, still));
        self.show_switched_program();
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
        let (clock, still) = (self.clock(), self.switcher_still());
        open.map(|switching| laid_out(config, switching, moved(switching, clock, still)))
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
        let (clock, still) = (self.clock(), self.switcher_still());
        // Before it is shown, nothing of it is there to be clicked on.
        let open = self.switch.open.as_mut();
        let open = open.filter(|switching| clock >= switching.shown);
        let under = open.and_then(|switching| {
            let layout = laid_out(&config, switching, moved(switching, clock, still));
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
        // One in a frame of ours is pictured in it, and has its frame's shape.
        let framed = |window: WindowId| {
            let frame = self.desktop.frame_of(window);
            let frame = frame.filter(|frame| frame.decoration.is_some());
            frame.map(|frame| {
                let size = frame.geometry.size;
                (size.width.cast_signed(), size.height.cast_signed())
            })
        };
        let shapes = windows.iter().map(|window| {
            let drawn = || self.mapped(*window).map_or(front, shape);
            framed(*window).unwrap_or_else(drawn)
        });
        shapes.collect()
    }

    /// What `shown`, of `windows`, is in pictures of the screen, if that is
    /// otherwise: a private window there has no title, and is blank, its
    /// icon large, unless it is to be blurred.
    fn in_pictures(&self, windows: &[WindowId], shown: &[Shown<Icon>]) -> Option<Vec<Shown<Icon>>> {
        let hidden: Vec<Shown<Icon>> = windows
            .iter()
            .zip(shown)
            .map(|(window, shown)| {
                let private = self.desktop.private(*window);
                Shown {
                    title: if private {
                        String::new()
                    } else {
                        shown.title.clone()
                    },
                    icon: shown.icon.clone(),
                    pictured: shown.pictured && !self.blank(*window),
                }
            })
            .collect();
        (hidden != shown).then_some(hidden)
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
        // As it stands, the row of the program's windows where it ends.
        let up = if switching.raised.is_some() { 1.0 } else { 0.0 };
        let layout = laid_out(&config, &switching, (0.0, 1.0, up, 0.0));
        let windows = shown_windows(&config, &switching, &layout);
        self.switch.dim_behind(&config, switching.screen);
        let picked = switching.switcher.chosen();
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
                    pictured: mapped.is_some(),
                }
            })
            .collect();
        let hidden = self.in_pictures(&windows, &shown);
        let same = |drawn: &Drawn| {
            drawn.layout == layout
                && drawn.windows == windows
                && drawn.screen.shown == shown
                && drawn.pictures.as_ref().map(|face| &face.shown) == hidden.as_ref()
                && Arc::ptr_eq(&drawn.config, &config)
                && drawn.scale == scale
        };
        if !switching.painted.as_ref().is_some_and(same) {
            let theme = &config.theme;
            // What was painted before, but for the shadow, is kept only as
            // long as it was painted as it is now.
            let had = switching.painted.take();
            let same = |drawn: &Drawn| Arc::ptr_eq(&drawn.config, &config) && drawn.scale == scale;
            let painted_so = had.as_ref().is_some_and(same);
            let (kept, kept_hidden, had) = had.map_or((Vec::new(), Vec::new(), None), |drawn| {
                let hidden = drawn.pictures.map_or_default(|face| face.cards);
                (drawn.screen.cards, hidden, drawn.shadow)
            });
            let keep = |cards: Vec<Card>| if painted_so { cards } else { Vec::new() };
            let shapes = self.shapes(&windows, &layout);
            let painting = (&layout, windows.as_slice(), shapes.as_slice());
            let mut tools = (&mut self.text, &mut icons);
            let screen = face(
                &mut tools,
                &config,
                painting,
                (&shown, picked, keep(kept)),
                scale,
            );
            let pictures = hidden.map(|hidden| {
                // What is the same in pictures is taken as it is painted
                // for the screen.
                let mut kept = keep(kept_hidden);
                kept.extend(screen.cards.iter().cloned());
                face(
                    &mut tools,
                    &config,
                    painting,
                    (&hidden, picked, kept),
                    scale,
                )
            });
            let style = theme.style(Part::Switcher, States::NONE);
            let area = &layout.area;
            let placed = (
                area.origin.x,
                area.origin.y,
                area.size.width,
                area.size.height,
            );
            let shadow = self.animation.shades.cast(&style, scale, placed, had);
            let row: Vec<WindowId> = layout.program.iter().map(|item| item.window).collect();
            let row = self.shapes(&row, &layout);
            let rings = paint_rings(theme, (&layout, &row), scale);
            switching.painted = Some(Drawn {
                ids: shown.iter().map(|_| [Id::new(), Id::new()]).collect(),
                layout,
                windows,
                config,
                scale,
                screen,
                pictures,
                shadow,
                rings,
            });
        }
        self.switch.icons = icons;
        self.switch.open = Some(switching);
    }

    /// Adds the switcher, on the screen it is on: each window's picture,
    /// made small to fit its place, over the switcher's own pixels, and its
    /// glass if the theme makes it glass. A window a rule says is private
    /// is shown as any other on the `screen` itself, to the user it belongs
    /// to; in a picture of the screen, without its title, and blank or
    /// blurred as the rule says.
    pub(crate) fn switcher_elements<R>(
        &self,
        renderer: &mut R,
        output: &Output,
        (view, screen): (View, bool),
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
            let drawn = (switching, drawn, screen);
            self.queue_elements(renderer, drawn, seen, (elements, panes));
        } else {
            self.rows_elements(renderer, (drawn, screen), seen, (elements, panes));
        }
        self.switcher_backdrop(output, drawn, seen, (elements, panes));
    }

    /// Adds the rows: each window's picture, made small to fit its place,
    /// over the switcher's own pixels, and the box's shadow and glass, as
    /// much of all of it `seen` as the switcher is; as they are on the
    /// `screen` or in pictures of it.
    fn rows_elements<R>(
        &self,
        renderer: &mut R,
        (drawn, screen): (&Drawn, bool),
        (view, seen): (View, f32),
        (elements, panes): (&mut Vec<Element<R>>, &mut Vec<Pane>),
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let theme = &drawn.config.theme;
        let (_, tall) = metrics(theme);
        let face = drawn.face(screen);
        let given = drawn.layout.items.iter().zip(&face.shown).zip(&drawn.ids);
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
            let (window, pass) = ((item.window, seen), (view, screen));
            if !self.framed_picture(renderer, window, room, pass, (elements, panes)) {
                self.picture_elements(renderer, window, room, pass, (elements, panes));
            }
        }
        // Behind the one picked, its program's others, each pictured in its
        // place as a window in a row is, made as much smaller as its place.
        let full = metrics(theme).0.item.width;
        let style = theme.style(Part::SwitcherItem, States::NONE);
        let room = |place: Rect| {
            let share = f64::from(place.size.width) / f64::from(full.max(1));
            let inset = style.inset();
            (
                f64::from(inset.left).mul_add(share, f64::from(place.origin.x)),
                f64::from(inset.top).mul_add(share, f64::from(place.origin.y)),
                f64::from(place.size.width) - f64::from(inset.across()) * share,
                f64::from(tall) * share,
            )
        };
        let shown = (view, seen, screen);
        self.program_elements(renderer, (&drawn.layout, room), shown, (elements, panes));
        let at =
            Point::<i32, Logical>::from((drawn.layout.area.origin.x, drawn.layout.area.origin.y));
        elements.extend(face.pixels.element(renderer, at.to_f64(), view, seen));
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
        let whole = head.map(|head| view.pixels(head.area()));
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
    /// as the switcher is, as it is on the `screen` or in pictures of it.
    fn queue_elements<R>(
        &self,
        renderer: &mut R,
        (switching, drawn, screen): (&Switching, &Drawn, bool),
        (view, seen): (View, f32),
        (elements, panes): (&mut Vec<Element<R>>, &mut Vec<Pane>),
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let theme = &drawn.config.theme;
        let below = area(drawn.layout.area);
        let face = drawn.face(screen);
        let first = face.shown.first().zip(drawn.ids.first());
        let first = first.map(|(shown, ids)| (&shown.icon, ids));
        if let Some((Some(Icon::Given(buffer)), [beside, _])) = first {
            let (icon, _) = paint::title_places(below, theme, true);
            let icons = &self.switch.icons;
            elements.extend(icons.given(renderer, buffer, (icon, seen), beside, view));
        }
        let at = Point::<f64, Logical>::from((f64::from(below.0), f64::from(below.1)));
        elements.extend(face.pixels.element(renderer, at, view, seen));
        switcher_box(renderer, drawn, (view, seen), (elements, panes));
        let layout = laid_out(
            &drawn.config,
            switching,
            moved(switching, self.clock(), self.switcher_still()),
        );
        let front = drawn
            .layout
            .items
            .first()
            .map_or(1, |item| item.place.size.width);
        let front = f64::from(front.max(1));
        let painted = drawn.windows.iter().zip(&face.shown);
        let painted = painted.zip(face.cards.iter().zip(&drawn.ids));
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
            let pass = (view, screen);
            if !self.framed_picture(renderer, window, picture, pass, (elements, panes)) {
                self.picture_elements(renderer, window, picture, pass, (elements, panes));
            }
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
        // Over the program's windows, round the one gone to among them, its
        // ring, as the one in front has it, coming in as the one round the
        // one before goes.
        let clock = self.clock();
        // Each its own, as large as its picture.
        let ring_of = |window: WindowId| {
            let found = drawn.rings.iter().find(|(ringed, _)| *ringed == window);
            found.map(|(_, ring)| ring)
        };
        let ring = layout.focus.zip(ring_of(switching.switcher.chosen()));
        let shift = switching
            .ring_from
            .map(|(_, began)| along(Some(began), clock, false, 1.0));
        let coming = shift.unwrap_or(1.0);
        let lit = (theme, view, seen * f32_of(coming));
        let ring = ring.and_then(|(focus, ring)| ring_round(renderer, ring, focus, lit));
        elements.extend(ring);
        let before = switching
            .ring_from
            .and_then(|(window, _)| layout.program.iter().find(|item| item.window == window));
        let before = before.and_then(|item| Some(item.place).zip(ring_of(item.window)));
        let fading = (theme, view, seen * f32_of(1.0 - coming));
        let before = before.and_then(|(place, ring)| ring_round(renderer, ring, place, fading));
        elements.extend(before);
        let shown = (view, seen, screen);
        self.program_elements(renderer, (&layout, whole_place), shown, (elements, panes));
        self.leaving_elements(renderer, switching, shown, (elements, panes));
    }

    /// Adds the row of the program picked before, on its way down out of
    /// sight from where it was, while it is.
    fn leaving_elements<R>(
        &self,
        renderer: &mut R,
        switching: &Switching,
        shown: (View, f32, bool),
        drawn: (&mut Vec<Element<R>>, &mut Vec<Pane>),
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let clock = self.clock();
        let leaving = switching.leaving.as_ref();
        let leaving = leaving.filter(|(_, began)| clock < *began + TURN && !self.switcher_still());
        let Some((items, began)) = leaving else {
            return;
        };
        let along = along(Some(*began), clock, false, 0.0);
        let screen = switching.screen;
        let bottom = f64::from(screen.origin.y) + f64::from(screen.size.height);
        let gone = items.iter().map(|item| {
            let top = f64::from(item.place.origin.y);
            let top = (bottom - top).mul_add(along, top);
            let at = DomainPoint::new(item.place.origin.x, whole(top));
            SwitcherItem {
                place: Rect::new(at, item.place.size),
                ..*item
            }
        });
        let leaving = SwitcherLayout {
            area: screen,
            items: Vec::new(),
            program: gone.collect(),
            focus: None,
        };
        self.program_elements(renderer, (&leaving, whole_place), shown, drawn);
    }

    /// Adds the pictures of the program picked's other windows, where
    /// `layout` has them, each in the `room` of its place, as see-through
    /// as it says and as the switcher is.
    fn program_elements<R>(
        &self,
        renderer: &mut R,
        (layout, room): (&SwitcherLayout, impl Fn(Rect) -> crate::pictures::Exact),
        (view, seen, screen): (View, f32, bool),
        (elements, panes): (&mut Vec<Element<R>>, &mut Vec<Pane>),
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        for item in &layout.program {
            let opacity = f32::from(item.opacity) / 1000.0 * seen;
            let room = room(item.place);
            let (window, pass) = ((item.window, opacity), (view, screen));
            if !self.framed_picture(renderer, window, room, pass, (elements, panes)) {
                self.picture_elements(renderer, window, room, pass, (elements, panes));
            }
        }
    }

    /// Hands `told` each window shown in the switcher on `output`, which
    /// is told that what it drew has been shown, so that its picture there
    /// is kept up to date as it would be on the screen.
    pub(crate) fn each_switched(&self, output: &Output, told: &mut dyn FnMut(Told<'_>)) {
        let here = self.head(output).map(|head| domain(head.area()));
        let switching = self
            .shown_switcher()
            .filter(|switching| here == Some(switching.screen));
        let Some(drawn) = switching.and_then(|switching| switching.painted.as_ref()) else {
            return;
        };
        let items = drawn.layout.items.iter().chain(&drawn.layout.program);
        let shown = items.filter_map(|item| self.mapped(item.window));
        for mapped in shown {
            told(Told::Window(&mapped.window));
        }
    }
}
