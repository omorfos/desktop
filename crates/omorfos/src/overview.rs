//! The overview (ADR 76): every window of the workspace shown on a screen
//! laid out small side by side, to pick one from, drawn by the compositor
//! as the switcher is (ADR 38). Where each goes is the desktop model's
//! business (`omorfos_core::overview`).
//!
//! Opened, each window glides from where it is on the screen to its place
//! in the overview over [`GLIDE`], and back again as it closes, while the
//! screen behind dims and blurs; the windows themselves are not drawn on
//! that screen meanwhile. A window that is minimized has nowhere on the
//! screen to glide from, and fades in and out where it is in the overview.
//! With `appearance.reduce_motion` every window fades in and out in its
//! place. The window picked, by the pointer or the arrows, has a border
//! round it and its title under it; Enter or a click goes to it, Escape or
//! a click elsewhere closes the overview where it was. The window gone to
//! is given focus once the overview has closed, so that it does not come
//! up over its own picture on the way.

use std::sync::Arc;
use std::time::Duration;

use omorfos_config::{Config, Easing, Part, State, States};
use omorfos_core::geometry::{Point as DomainPoint, Rect};
use omorfos_core::{OverviewMetrics, WindowId, overview};
use smithay::backend::renderer::element::AsRenderElements as _;
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::element::solid::{SolidColorBuffer, SolidColorRenderElement};
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::element::utils::CropRenderElement;
use smithay::backend::renderer::{ImportAll, ImportMem, Renderer, Texture};
use smithay::desktop::layer_map_for_output;
use smithay::input::keyboard::Keysym;
use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle};
use smithay::wayland::shell::wlr_layer::Layer;

use crate::Scale;
use crate::compositor::{Compositor, Element, Painted, View, color, layer_origin};
use crate::glass::{self, Pane};
use crate::outputs::domain;
use crate::paint;
use crate::pictures::{Exact, whole};

/// How long a window takes to glide to its place in the overview, and
/// back, as long as `docs/DESIGN.md` gives something moving to where it
/// was sent.
const GLIDE: Duration = Duration::from_millis(240);
/// The room between one window and the next, in units of layout.
const GAP: u32 = 24;
/// The least room between the overview and the edges of the screen.
const MARGIN: u32 = 32;
/// How tall each workspace in the strip is, as a share of the screen.
const STRIP: f64 = 0.12;
/// How far inside the bottom of the window picked its title is.
const INSIDE: u32 = 8;

/// Where the overview shows what it shows, while it is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Overviewed {
    /// Each window, where it is shown once it has glided there, in the
    /// order they are walked.
    pub windows: Vec<(WindowId, Rect)>,
    /// The window picked.
    pub picked: Option<WindowId>,
    /// Each desktop of the screen, in the strip, and whether it is the one
    /// shown.
    pub workspaces: Vec<(Rect, bool)>,
    /// The workspace under the pointer, by its place in the strip, or, one
    /// past the last, the button that makes a new one.
    pub hovered: Option<usize>,
    /// The button after the strip that makes a new workspace.
    pub add: Rect,
}

/// The overview, open or not.
#[derive(Debug, Default)]
pub(crate) struct Overview {
    /// The overview, while it is open, or closing.
    open: Option<Looking>,
    /// What dims the screen behind it, kept as large as the screen and of
    /// the theme's colour, so that it is the same from one frame to the
    /// next.
    dim: SolidColorBuffer,
    /// The desktop's colour behind each workspace in the strip, kept as
    /// large as it, in the order of the strip.
    grounds: Vec<SolidColorBuffer>,
    /// The blank of each private window to be blank, kept as large as it
    /// is drawn: by the window, and the workspace in the strip it is
    /// drawn on, or none for where it is laid out itself.
    blanks: Vec<(Blank, SolidColorBuffer)>,
}

/// Which of a private window's blanks one is: on which workspace in the
/// strip, by its place there, or none for where it is laid out itself.
type Blank = (Option<usize>, WindowId);

/// The overview while it is open.
#[derive(Debug)]
struct Looking {
    /// The screen it is on, as it was when it opened.
    screen: Rect,
    /// Which of the desktop's screens that is.
    output: usize,
    /// When it was opened, by the compositor's clock.
    opened: Duration,
    /// When each window came into it, and so began to glide to its place:
    /// as it opened, or as its workspace was gone to.
    entered: Vec<(WindowId, Duration)>,
    /// The workspace in the strip under the pointer, by its place there.
    hovered: Option<usize>,
    /// The window picked, by the pointer or the arrows.
    picked: Option<WindowId>,
    /// Closing: when it began to, and the window to be gone to once it
    /// has.
    closing: Option<(Duration, Option<WindowId>)>,
    /// The border and title of the window picked, kept until they would
    /// be painted differently.
    painted: Option<Drawn>,
}

/// What the overview draws besides the windows, and what it was painted
/// from.
#[derive(Debug)]
struct Drawn {
    config: Arc<Config>,
    scale: Scale,
    /// How large each workspace in the strip is.
    strip: (u32, u32),
    /// The border of a workspace in the strip: as it is, shown, under the
    /// pointer, and both.
    rings: [Painted; 4],
    /// The button that makes a new workspace: as it is, and under the
    /// pointer.
    adds: [Painted; 2],
    /// The border and title of the window picked, if there is one.
    picked: Option<Picked>,
}

/// The border and title of the window picked, and what they were painted
/// from.
#[derive(Debug)]
struct Picked {
    window: WindowId,
    title: String,
    size: (u32, u32),
    ring: Painted,
    label: Painted,
}

/// A window in the overview: where it is drawn now, how much of it is
/// seen, and where it goes once it has glided there.
#[derive(Debug, Clone, Copy)]
struct Card {
    window: WindowId,
    now: Exact,
    seen: f32,
    place: Rect,
}

/// How the overview is laid out on `screen`.
fn metrics(screen: Rect) -> OverviewMetrics {
    let strip = f64::from(screen.size.height) * STRIP;
    OverviewMetrics {
        strip: whole(strip).max(0).cast_unsigned(),
        gap: GAP,
        margin: MARGIN,
    }
}

/// How far a glide that began at `began` has got by `clock`, from 0 to 1,
/// easing in and out.
fn glided(began: Duration, clock: Duration) -> f32 {
    let share = clock.saturating_sub(began).as_secs_f32() / GLIDE.as_secs_f32();
    Easing::EaseInOut.at(share.min(1.0))
}

/// `from` and `to` mixed, `share` of the way from one to the other.
fn between(from: Exact, to: Exact, share: f32) -> Exact {
    let share = f64::from(share);
    let mix = |from: f64, to: f64| (to - from).mul_add(share, from);
    (
        mix(from.0, to.0),
        mix(from.1, to.1),
        mix(from.2, to.2),
        mix(from.3, to.3),
    )
}

/// A rectangle of the desktop model, worked out in fractions.
fn exact(rect: Rect) -> Exact {
    (
        f64::from(rect.origin.x),
        f64::from(rect.origin.y),
        f64::from(rect.size.width),
        f64::from(rect.size.height),
    )
}

impl Overview {
    /// Closes the overview at once, choosing nothing.
    pub(crate) fn close(&mut self) {
        self.open = None;
    }
}

impl Looking {
    /// How far from closed it is at `clock`: all of it until it closes, and
    /// none once it has.
    fn closing(&self, clock: Duration) -> f32 {
        let closing = self.closing.map(|(began, _)| glided(began, clock));
        1.0 - closing.unwrap_or_default()
    }

    /// How open it is at `clock`: none as it opens, all of it once open,
    /// and none again once it has closed.
    fn open(&self, clock: Duration) -> f32 {
        glided(self.opened, clock) * self.closing(clock)
    }

    /// How far `window` has got to its place at `clock`: none as it comes
    /// in, all the way once there, and none again once it has closed.
    fn arrived(&self, window: WindowId, clock: Duration) -> f32 {
        let entered = self.entered.iter().find(|(entered, _)| *entered == window);
        let entered = entered.map_or(clock, |(_, at)| *at);
        glided(entered, clock) * self.closing(clock)
    }

    /// Whether anything of it is on its way, and to be drawn again a frame
    /// on.
    fn moving(&self, clock: Duration) -> bool {
        let entered = self.entered.iter().map(|(_, at)| *at);
        let closing = self.closing.map(|(began, _)| began);
        let latest = entered.chain(closing).fold(self.opened, Duration::max);
        clock < latest + GLIDE
    }
}

impl Compositor {
    /// Opens the overview on the screen the pointer is on, or closes it
    /// where it was.
    pub(crate) fn toggle_overview(&mut self) {
        let clock = self.clock();
        if let Some(looking) = self.cards.overview.open.as_mut() {
            looking.closing.get_or_insert((clock, None));
            return;
        }
        self.close_switcher(false);
        let head = self.pointer_head();
        let screen = head.map(|head| domain(head.area())).unwrap_or_default();
        let output = head.map(|head| head.output.clone());
        let output = self
            .heads
            .iter()
            .position(|head| Some(&head.output) == output.as_ref());
        let picked = self.desktop.focused();
        self.cards.overview.open = Some(Looking {
            screen,
            output: output.unwrap_or_default(),
            opened: clock,
            entered: Vec::new(),
            hovered: None,
            picked,
            closing: None,
            painted: None,
        });
    }

    /// Where the overview shows each window once it has glided there, in
    /// the order they are walked, and the one picked; nothing while it is
    /// closed.
    #[must_use]
    pub fn overview_layout(&self) -> Option<Overviewed> {
        let looking = self.cards.overview.open.as_ref()?;
        let cards = self.cards(looking, self.clock());
        let strip = self.strip(looking);
        Some(Overviewed {
            windows: cards.iter().map(|card| (card.window, card.place)).collect(),
            picked: looking.picked,
            workspaces: strip
                .iter()
                .map(|(rect, _, _, shown)| (*rect, *shown))
                .collect(),
            hovered: looking.hovered,
            add: self.adder(looking),
        })
    }

    /// Whether the overview is open, or still closing.
    pub(crate) const fn overview_open(&self) -> bool {
        self.cards.overview.open.is_some()
    }

    /// Whether the overview is on its way open or closed, and to be drawn
    /// again a frame on.
    pub(crate) fn overview_moving(&self) -> bool {
        let looking = self.cards.overview.open.as_ref();
        looking.is_some_and(|looking| looking.moving(self.clock()))
    }

    /// Whether a window whose frame is `outer` is on the screen the
    /// overview is on, and is not drawn there while it is open.
    pub(crate) fn hidden_by_overview(&self, outer: Rect) -> bool {
        let looking = self.cards.overview.open.as_ref();
        looking.is_some_and(|looking| on_screen(outer, looking.screen))
    }

    /// The windows of the workspace shown on the overview's screen, each
    /// with where it is now on the screen, or, if it is minimized, where
    /// it would be, and whether it is on the screen to glide from.
    fn overview_windows(&self, output: usize) -> Vec<(WindowId, Rect, bool)> {
        let shown = self.desktop.shown_workspaces().nth(output);
        let listed = self.desktop.windows().filter(|listed| {
            listed.output == output && Some(listed.workspace) == shown && listed.shown
        });
        let windows = listed.filter_map(|listed| {
            let id = listed.window.id;
            let on = self.scene.iter().find(|shown| shown.id == id);
            let on = on.zip(self.mapped(id)).map(|(shown, mapped)| {
                let size = mapped.window.geometry().size;
                let origin = DomainPoint::new(shown.origin.x, shown.origin.y);
                let size = omorfos_core::geometry::Size::new(
                    size.w.max(1).cast_unsigned(),
                    size.h.max(1).cast_unsigned(),
                );
                Rect::new(origin, size)
            });
            let away = || self.desktop.frame_of(id).map(|frame| frame.geometry);
            on.map(|rect| (id, rect, true))
                .or_else(|| away().map(|rect| (id, rect, false)))
        });
        windows.collect()
    }

    /// The desktops of screen `output`, in order: where each is among the
    /// screen's workspaces, which desktop it is, and whether it is shown. A
    /// workspace made for one fullscreen window is not among them.
    fn overview_desktops(&self, output: usize) -> Vec<(usize, usize, bool)> {
        let places = self
            .desktop
            .workspaces()
            .filter(|place| place.output == output);
        let desktops = places.filter_map(|place| {
            place
                .desktop
                .map(|desktop| (place.index, desktop, place.shown))
        });
        desktops.collect()
    }

    /// Each desktop in the overview's strip: where it is shown, where it is
    /// among the screen's workspaces, which desktop it is, and whether it
    /// is shown.
    fn strip(&self, looking: &Looking) -> Vec<(Rect, usize, usize, bool)> {
        let desktops = self.overview_desktops(looking.output);
        let tiles = desktops.len() + 1;
        let layout = overview(looking.screen, &[], tiles, metrics(looking.screen));
        let strip = layout.workspaces.into_iter().zip(desktops);
        let strip = strip.map(|(rect, (index, desktop, shown))| (rect, index, desktop, shown));
        strip.collect()
    }

    /// Where the button after the strip that makes a new workspace is.
    fn adder(&self, looking: &Looking) -> Rect {
        let tiles = self.overview_desktops(looking.output).len() + 1;
        let layout = overview(looking.screen, &[], tiles, metrics(looking.screen));
        layout.workspaces.last().copied().unwrap_or_default()
    }

    /// Each window in the overview, as it is drawn at `clock`, in the order
    /// they are read and walked.
    fn cards(&self, looking: &Looking, clock: Duration) -> Vec<Card> {
        let windows = self.overview_windows(looking.output);
        let places: Vec<(WindowId, Rect)> =
            windows.iter().map(|(id, rect, _)| (*id, *rect)).collect();
        let tiles = self.overview_desktops(looking.output).len() + 1;
        let layout = overview(looking.screen, &places, tiles, metrics(looking.screen));
        let still = self.config().general.appearance.reduce_motion;
        let cards = layout.windows.into_iter().map(|(window, place)| {
            let arrived = looking.arrived(window, clock);
            let from = windows
                .iter()
                .find(|(id, _, on)| *id == window && *on && !still);
            let to = exact(place);
            let (now, seen) = from.map_or((to, arrived), |(_, from, _)| {
                (between(exact(*from), to, arrived), 1.0)
            });
            Card {
                window,
                now,
                seen,
                place,
            }
        });
        cards.collect()
    }

    /// Closes the overview once it has finished closing, and goes to the
    /// window chosen; and closes it while the session is locked, or a
    /// prompt asks for a secret over it.
    pub(crate) fn settle_overview(&mut self) {
        if self.is_locked() || self.sealed().is_some() {
            self.cards.overview.open = None;
        }
        let clock = self.clock();
        let closed = self.cards.overview.open.as_ref().and_then(|looking| {
            let (began, chosen) = looking.closing?;
            (clock >= began + GLIDE).then_some(chosen)
        });
        if let Some(chosen) = closed {
            self.cards.overview.open = None;
            if let Some(window) = chosen {
                self.desktop.activate(window);
            }
        }
    }

    /// Closes the overview, going to `chosen` once it has.
    fn close_overview(&mut self, chosen: Option<WindowId>) {
        let clock = self.clock();
        if let Some(looking) = self.cards.overview.open.as_mut() {
            looking.closing.get_or_insert((clock, chosen));
        }
    }

    /// Takes a key pressed while the overview is open: Escape closes it,
    /// Enter goes to the window picked, and the arrows walk the windows.
    /// Returns whether the key was the overview's; any other is left to
    /// bindings, Alt+Tab among them.
    pub(crate) fn overview_key(&mut self, keysym: Keysym) -> bool {
        // Taken out while it is steered, and put back.
        let Some(mut looking) = self.cards.overview.open.take() else {
            return false;
        };
        let clock = self.clock();
        let cards = self.cards(&looking, clock);
        let at = cards
            .iter()
            .position(|card| Some(card.window) == looking.picked);
        let count = cards.len().max(1);
        let walk = |by: usize| {
            let at = at.map_or(0, |at| (at + by) % count);
            cards.get(at).map(|card| card.window)
        };
        let ours = match keysym {
            Keysym::Escape => {
                looking.closing.get_or_insert((clock, None));
                true
            }
            Keysym::Return | Keysym::KP_Enter => {
                looking.closing.get_or_insert((clock, looking.picked));
                true
            }
            Keysym::Right | Keysym::Down => {
                looking.picked = walk(1).or(looking.picked);
                true
            }
            Keysym::Left | Keysym::Up => {
                looking.picked = walk(count - 1).or(looking.picked);
                true
            }
            _ => false,
        };
        self.cards.overview.open = Some(looking);
        ours
    }

    /// The window in the overview at `at`, if one is there.
    fn overview_under(&self, at: DomainPoint) -> Option<WindowId> {
        let looking = self.cards.overview.open.as_ref()?;
        let cards = self.cards(looking, self.clock());
        let under = cards.iter().find(|card| card.place.contains(at));
        under.map(|card| card.window)
    }

    /// The workspace in the overview's strip at `at`, if one is there: its
    /// place in the strip, and which desktop it is.
    fn overview_strip_under(&self, at: DomainPoint) -> Option<(usize, usize)> {
        let looking = self.cards.overview.open.as_ref()?;
        let strip = self.strip(looking).into_iter().enumerate();
        let mut under = strip.filter(|(_, (rect, _, _, _))| rect.contains(at));
        under
            .next()
            .map(|(place, (_, _, desktop, _))| (place, desktop))
    }

    /// The button that makes a new workspace, by its place after the strip,
    /// if it is at `at`.
    fn overview_adder_under(&self, at: DomainPoint) -> Option<usize> {
        let looking = self.cards.overview.open.as_ref()?;
        let place = self.overview_desktops(looking.output).len();
        self.adder(looking).contains(at).then_some(place)
    }

    /// Takes the pointer moving to `at`: while the overview is open, the
    /// window under it is picked, and the workspace under it is lit.
    /// Returns whether the overview is open, and so has the pointer.
    pub(crate) fn overview_motion(&mut self, at: DomainPoint) -> bool {
        let under = self.overview_under(at);
        let strip = self.overview_strip_under(at).map(|(place, _)| place);
        let adding = self.overview_adder_under(at);
        let Some(looking) = self.cards.overview.open.as_mut() else {
            return false;
        };
        let strip = strip.or(adding);
        looking.picked = under.or(looking.picked);
        looking.hovered = strip;
        true
    }

    /// Takes a press of the pointer while the overview is open: on the
    /// button after the strip it makes a new workspace and goes to it; on a
    /// workspace in the strip it goes to it, and shows its windows; on a
    /// window it goes to it; anywhere else it closes the overview.
    pub(crate) fn overview_press(&mut self, at: DomainPoint) {
        let adding = self.overview_adder_under(at);
        let screen = self.cards.overview.open.as_ref();
        if let Some((_, looking)) = adding.zip(screen) {
            // Made at the end, and gone to, empty, for windows to come.
            let origin = looking.screen.origin;
            self.desktop.turn_to(origin);
            self.desktop.new_workspace();
            return;
        }
        let strip = self.overview_strip_under(at);
        let screen = self
            .cards
            .overview
            .open
            .as_ref()
            .map(|looking| looking.screen);
        if let Some(((_, desktop), screen)) = strip.zip(screen) {
            // Gone to at once, its windows gliding in from where they are.
            self.desktop.turn_to(screen.origin);
            self.desktop.switch_workspace(desktop);
            return;
        }
        let under = self.overview_under(at);
        self.close_overview(under);
    }

    /// Paints the border and the title of the window picked again, if
    /// what they show, the theme or the scale changed since.
    pub(crate) fn repaint_overview(&mut self) {
        let config = Arc::clone(self.store.current());
        let scale = self.pointer_scale();
        let clock = self.clock();
        let Some(mut looking) = self.cards.overview.open.take() else {
            return;
        };
        let backdrop = config.theme.style(Part::OverviewBackdrop, States::NONE);
        let size = (
            looking.screen.size.width.cast_signed(),
            looking.screen.size.height.cast_signed(),
        );
        self.cards
            .overview
            .dim
            .update(size, color(backdrop.ground()));
        let cards = self.cards(&looking, clock);
        // A window comes in now, unless it was already in.
        let had = std::mem::take(&mut looking.entered);
        looking.entered = cards
            .iter()
            .map(|card| {
                let had = had.iter().find(|(window, _)| *window == card.window);
                had.copied().unwrap_or((card.window, clock))
            })
            .collect();
        self.size_solids(&looking, &cards, &config);
        let strip = self.strip(&looking);
        let strip = strip.first().map_or((1, 1), |(rect, _, _, _)| {
            (rect.size.width.max(1), rect.size.height.max(1))
        });
        let picked = cards
            .iter()
            .find(|card| Some(card.window) == looking.picked)
            .or_else(|| cards.first());
        let title = |window| {
            let listed = self.desktop.window(window);
            listed
                .map(|listed| listed.title.clone())
                .unwrap_or_default()
        };
        let wanted = picked.map(|card| {
            let size = (card.place.size.width, card.place.size.height);
            (card.window, title(card.window), size)
        });
        let window = wanted.as_ref().map(|(window, _, _)| *window);
        let same = |drawn: &Drawn| {
            let had = drawn.picked.as_ref();
            let had = had.map(|picked| (picked.window, &picked.title, picked.size));
            had == wanted
                .as_ref()
                .map(|(window, title, size)| (*window, title, *size))
                && Arc::ptr_eq(&drawn.config, &config)
                && drawn.scale == scale
                && drawn.strip == strip
        };
        if !looking.painted.as_ref().is_some_and(same) {
            let theme = &config.theme;
            let ring =
                |states| Painted::new(&paint::overview_workspace(theme, strip, states, scale));
            let shown = States::NONE.with(State::Active);
            let rings = [
                ring(States::NONE),
                ring(shown),
                ring(States::NONE.with(State::Hovered)),
                ring(shown.with(State::Hovered)),
            ];
            let add = |hovered| Painted::new(&paint::overview_add(theme, strip, hovered, scale));
            let adds = [add(false), add(true)];
            let text = &mut self.text;
            let picked = wanted.map(|(window, title, size)| {
                let ring = Painted::new(&paint::overview_window(theme, size, scale));
                let tall = paint::overview_title_height(theme);
                let wide = size.0.saturating_sub(2 * INSIDE).max(1);
                let label = paint::overview_title(text, theme, (wide, tall), &title, scale);
                Picked {
                    window,
                    title,
                    size,
                    ring,
                    label: Painted::new(&label),
                }
            });
            looking.painted = Some(Drawn {
                config,
                scale,
                strip,
                rings,
                adds,
                picked,
            });
        }
        // The one picked is one shown, whatever had focus.
        looking.picked = window;
        self.cards.overview.open = Some(looking);
    }

    /// Adds the overview, on the screen it is on: over each window's
    /// picture, the border of the one picked and its title, and under them
    /// all the dim and blur of the screen, each as open as the overview is.
    pub(crate) fn overview_elements<R>(
        &self,
        renderer: &mut R,
        output: &Output,
        view: View,
        (elements, panes): (&mut Vec<Element<R>>, &mut Vec<Pane>),
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let here = self.head(output).map(|head| domain(head.area()));
        let looking = self.cards.overview.open.as_ref();
        let looking = looking.filter(|looking| here == Some(looking.screen));
        let drawn =
            looking.and_then(|looking| looking.painted.as_ref().map(|drawn| (looking, drawn)));
        let Some((looking, drawn)) = drawn else {
            return;
        };
        let clock = self.clock();
        let open = looking.open(clock);
        let cards = self.cards(looking, clock);
        let theme = &drawn.config.theme;
        let picked = drawn.picked.as_ref().and_then(|picked| {
            let card = cards.iter().find(|card| card.window == picked.window);
            card.map(|card| (picked, card))
        });
        if let Some((picked, card)) = picked {
            let (left, top, across, down) = card.now;
            let share = across / f64::from(card.place.size.width.max(1));
            let style = theme.style(Part::OverviewWindow, States::NONE.with(State::Active));
            let edges = style.inset();
            let out = |edge: u32| f64::from(edge) * share;
            let tall = f64::from(paint::overview_title_height(theme)) * share;
            let inside = out(INSIDE);
            let label = Point::<f64, Logical>::from((left + inside, top + down - inside - tall));
            let wide = whole(2.0f64.mul_add(-inside, across)).max(1);
            let size = (wide, whole(tall).max(1));
            elements.extend(
                picked
                    .label
                    .stretched(renderer, (label, size.into()), view, open),
            );
            let ring = Point::<f64, Logical>::from((left - out(edges.left), top - out(edges.top)));
            let size = (
                whole(across + out(edges.left) + out(edges.right)),
                whole(down + out(edges.top) + out(edges.bottom)),
            );
            elements.extend(
                picked
                    .ring
                    .stretched(renderer, (ring, size.into()), view, open),
            );
        }
        for card in &cards {
            let blanks = &self.cards.overview.blanks;
            let blank = blanks
                .iter()
                .find(|(blank, _)| *blank == (None, card.window));
            let solid = blank.map(|(_, blank)| {
                let at = view.pixel(Point::<f64, Logical>::from((card.now.0, card.now.1)));
                let scale = view.scale.factor();
                SolidColorRenderElement::from_buffer(blank, at, scale, card.seen, Kind::Unspecified)
            });
            elements.extend(solid.map(Element::Solid));
            let window = (card.window, card.seen);
            self.picture_elements(renderer, window, card.now, view, (elements, panes));
        }
        for (at, (rect, index, _, shown)) in self.strip(looking).into_iter().enumerate() {
            let place = (at, index, shown, looking.hovered == Some(at));
            let miniature = (output, looking.screen, rect);
            self.miniature_elements(
                renderer,
                (drawn, place, miniature),
                (view, open),
                (elements, panes),
            );
        }
        // After them, the button that makes a new workspace.
        let add = self.adder(looking);
        let hovered = looking.hovered == Some(self.overview_desktops(looking.output).len());
        let add_at =
            Point::<f64, Logical>::from((f64::from(add.origin.x), f64::from(add.origin.y)));
        let size = (add.size.width.cast_signed(), add.size.height.cast_signed());
        let button = drawn
            .adds
            .get(usize::from(hovered))
            .unwrap_or(&drawn.adds[0]);
        elements.extend(button.stretched(renderer, (add_at, size.into()), view, open));
        // The screen behind, dimmed, and blurred if the theme says so.
        let scale = view.scale.factor();
        let dim = &self.cards.overview.dim;
        let dim = SolidColorRenderElement::from_buffer(dim, (0, 0), scale, open, Kind::Unspecified);
        elements.push(Element::Solid(dim));
        let backdrop = theme.style(Part::OverviewBackdrop, States::NONE);
        let blurred = backdrop.blur > 0 && backdrop.see_through();
        let area = here.filter(|_| blurred).map(|screen| {
            let area = crate::compositor::logical(screen);
            let from = view.pixel(area.loc.to_f64());
            let to = view.pixel((area.loc + area.size.to_point()).to_f64());
            Rectangle::new(from, (to - from).to_size())
        });
        let key = glass::key(&("the overview's backdrop", output.name()));
        let under = elements.len();
        let blur = area.and_then(|area| {
            Pane::behind(vec![area], (backdrop.blur, view), (key, under, under), open)
        });
        panes.extend(blur);
    }

    /// Adds a workspace in the overview's strip, `rect` on `screen`, its
    /// place among `output`'s workspaces `index`, as open as `open`: its
    /// border, lit if it is `shown` or `hovered`, over its windows where
    /// they stand on it, over its wallpaper, over the desktop's colour.
    #[expect(
        clippy::type_complexity,
        reason = "what is drawn, where, and how, each a few things"
    )]
    fn miniature_elements<R>(
        &self,
        renderer: &mut R,
        (drawn, (strip_at, index, shown, hovered), (output, screen, rect)): (
            &Drawn,
            (usize, usize, bool, bool),
            (&Output, Rect, Rect),
        ),
        (view, open): (View, f32),
        (elements, panes): (&mut Vec<Element<R>>, &mut Vec<Pane>),
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let place = exact(rect);
        let at = Point::<f64, Logical>::from((place.0, place.1));
        let size = (
            rect.size.width.cast_signed(),
            rect.size.height.cast_signed(),
        );
        let ring = usize::from(shown) + 2 * usize::from(hovered);
        let ring = drawn.rings.get(ring).unwrap_or(&drawn.rings[0]);
        elements.extend(ring.stretched(renderer, (at, size.into()), view, open));
        let factor = place.2 / f64::from(screen.size.width.max(1));
        let blanks = &self.cards.overview.blanks;
        let looking = (output_index(self, output), screen);
        for (window, room) in self.miniature_windows(looking, (index, shown), place) {
            let blank = blanks
                .iter()
                .find(|(blank, _)| *blank == (Some(strip_at), window));
            let solid = blank.map(|(_, blank)| {
                let at = within(room, place);
                let at = view.pixel(Point::<f64, Logical>::from((at.0, at.1)));
                let scale = view.scale.factor();
                SolidColorRenderElement::from_buffer(blank, at, scale, open, Kind::Unspecified)
            });
            elements.extend(solid.map(Element::Solid));
            let rooms = (room, place, strip_at + 1);
            self.picture_within(renderer, (window, open), rooms, view, (elements, panes));
        }
        // Its wallpaper, as the screen's is, cut to it.
        let layers = layer_map_for_output(output);
        let crop = |at: Point<f64, Logical>| view.pixel(at);
        let from = crop(at);
        let to = crop(Point::from((place.0 + place.2, place.1 + place.3)));
        let crop = Rectangle::new(from, (to - from).to_size());
        let scale = smithay::utils::Scale::from(view.scale.factor() * factor);
        for surface in layers.layers_on(Layer::Background).rev() {
            let origin = layer_origin(&layers, surface).to_f64();
            let corner = Point::<f64, Logical>::from((
                origin.x.mul_add(factor, place.0),
                origin.y.mul_add(factor, place.1),
            ));
            let surfaces = surface.render_elements::<WaylandSurfaceRenderElement<R>>(
                renderer,
                view.pixel(corner),
                scale,
                open,
            );
            let cropped = surfaces
                .into_iter()
                .filter_map(|surface| CropRenderElement::from_element(surface, scale, crop));
            elements.extend(cropped.map(Element::Cropped));
        }
        let ground = self.cards.overview.grounds.get(strip_at).map(|ground| {
            let at = view.pixel(at);
            let scale = view.scale.factor();
            SolidColorRenderElement::from_buffer(ground, at, scale, open, Kind::Unspecified)
        });
        elements.extend(ground.map(Element::Solid));
    }

    /// The windows of a workspace in the overview's strip, `place`, of
    /// screen `output` that is `screen`, its place among the screen's
    /// workspaces `index`: each with where it is drawn there, as small as
    /// the workspace is, the topmost first, as the scene has them on the
    /// one `shown` and in the order they were made on the others.
    fn miniature_windows(
        &self,
        (output, screen): (Option<usize>, Rect),
        (index, shown): (usize, bool),
        place: Exact,
    ) -> Vec<(WindowId, Exact)> {
        let factor = place.2 / f64::from(screen.size.width.max(1));
        let small = |(left, top, across, down): Exact| {
            (
                (left - f64::from(screen.origin.x)).mul_add(factor, place.0),
                (top - f64::from(screen.origin.y)).mul_add(factor, place.1),
                across * factor,
                down * factor,
            )
        };
        let mut windows: Vec<(WindowId, Rect)> = if shown {
            let scene = self.scene.iter().rev();
            let here = scene.filter(|on| on_screen(on.outer, screen));
            here.map(|on| (on.id, on.outer)).collect()
        } else {
            let listed = self.desktop.windows().filter(|listed| {
                Some(listed.output) == output
                    && listed.workspace == index
                    && listed.shown
                    && !listed.minimized
            });
            let framed = listed.filter_map(|listed| {
                let id = listed.window.id;
                self.desktop.frame_of(id).map(|frame| (id, frame.geometry))
            });
            framed.collect()
        };
        if !shown {
            windows.reverse();
        }
        let windows = windows.into_iter();
        windows
            .map(|(window, frame)| (window, small(exact(frame))))
            .collect()
    }

    /// Tells each window the overview on `output` shows, on its own or on a
    /// workspace in its strip, that what it drew has been shown, so that it
    /// is kept up to date there as it would be on the screen.
    pub(crate) fn send_overview_frames(&self, output: &Output, elapsed: Duration) {
        let here = self.head(output).map(|head| domain(head.area()));
        let looking = self.cards.overview.open.as_ref();
        let looking = looking.filter(|looking| here == Some(looking.screen));
        let shown = looking.into_iter().flat_map(|looking| {
            let listed = self.desktop.windows();
            let on = listed.filter(move |listed| listed.output == looking.output && listed.shown);
            on.filter_map(|listed| self.mapped(listed.window.id))
        });
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

/// Whether a frame that is `outer` is on `screen`: its middle is.
fn on_screen(outer: Rect, screen: Rect) -> bool {
    let middle = DomainPoint::new(
        outer.origin.x + outer.size.width.cast_signed() / 2,
        outer.origin.y + outer.size.height.cast_signed() / 2,
    );
    screen.contains(middle)
}

/// Which of the desktop's screens `output` is.
fn output_index(compositor: &Compositor, output: &Output) -> Option<usize> {
    compositor
        .heads
        .iter()
        .position(|head| head.output == *output)
}

impl Compositor {
    /// Keeps the desktop's colour behind each workspace in the strip, and
    /// each blank of a private window to be blank, as large as it is drawn
    /// with `cards` as they are, and of `config`'s colours.
    fn size_solids(&mut self, looking: &Looking, cards: &[Card], config: &Config) {
        let theme = &config.theme;
        let ground = color(theme.style(Part::Desktop, States::NONE).ground());
        let capture = color(theme.style(Part::Capture, States::NONE).ground());
        let size = |(_, _, across, down): Exact| (whole(across).max(1), whole(down).max(1));
        let strip = self.strip(looking);
        let laid = cards.iter().filter(|card| self.blank(card.window));
        let mut blanks: Vec<(Blank, Exact)> =
            laid.map(|card| ((None, card.window), card.now)).collect();
        let screen = (Some(looking.output), looking.screen);
        for (at, (rect, index, _, shown)) in strip.iter().enumerate() {
            let place = exact(*rect);
            let windows = self.miniature_windows(screen, (*index, *shown), place);
            let blank = windows
                .into_iter()
                .filter(|(window, _)| self.blank(*window));
            blanks.extend(blank.map(|(window, room)| ((Some(at), window), within(room, place))));
        }
        let overview = &mut self.cards.overview;
        overview
            .grounds
            .resize_with(strip.len(), SolidColorBuffer::default);
        for (buffer, (rect, _, _, _)) in overview.grounds.iter_mut().zip(&strip) {
            buffer.update(size(exact(*rect)), ground);
        }
        let mut had = std::mem::take(&mut overview.blanks);
        overview.blanks = blanks
            .into_iter()
            .map(|(blank, rect)| {
                let kept = had.iter().position(|(kept, _)| *kept == blank);
                let mut buffer = kept.map(|at| had.swap_remove(at).1).unwrap_or_default();
                buffer.update(size(rect), capture);
                (blank, buffer)
            })
            .collect();
    }
}

/// The part of `rect` within `clip`, or nothing of it where they do not
/// meet.
fn within(rect: Exact, clip: Exact) -> Exact {
    let left = rect.0.max(clip.0);
    let top = rect.1.max(clip.1);
    let right = (rect.0 + rect.2).min(clip.0 + clip.2);
    let bottom = (rect.1 + rect.3).min(clip.1 + clip.3);
    (left, top, (right - left).max(0.0), (bottom - top).max(0.0))
}
