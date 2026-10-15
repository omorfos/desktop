//! Finding the panels and the titlebar again on a workspace made for a
//! fullscreen window.
//!
//! Such a workspace shows the window and nothing else. Pushing the pointer
//! on against the top of the screen, or resting it there, brings back
//! whatever panels are along the top, and under them the window's titlebar;
//! against the bottom, the panels along the bottom. They are laid over the window, which is not
//! told to be any other size, and go again when the pointer leaves them or
//! a key is pressed.
//!
//! All of it is the compositor's doing. A fullscreen program can draw what
//! looks like a desktop; it cannot answer the pointer resting against an
//! edge. See `docs/adr/0036-workspaces-that-come-and-go.md`.

use std::time::{Duration, Instant};

use omorfos_config::{Part, States};
use smithay::desktop::layer_map_for_output;
use smithay::utils::{Logical, Rectangle};
use smithay::wayland::shell::wlr_layer::Layer;

use crate::compositor::Compositor;

/// How long the pointer rests against an edge before what is there comes
/// back. Long enough that a pointer on its way to something the program
/// has at the edge, or left there, brings nothing.
const REST: Duration = Duration::from_millis(600);

/// How far the pointer is pushed on past an edge it is against, in units of
/// layout, within `PRESSING` of being pushed at all, before what is there
/// comes back without waiting. A pointer that only reaches the edge, on
/// its way to something the program has there, has gone no further.
/// These are what GNOME asks of its pressure barriers.
const PUSH: f64 = 100.0;

/// How long, in the thousandths of a second input is timed in, a push has
/// to reach `PUSH` in. After that it begins again.
const PRESSING: u32 = 1000;

/// How long, in the same, the pointer has been held against an edge before
/// pushing on counts. A pointer thrown at the edge, as a touchpad throws it,
/// goes on past it for a few hundredths of a second after it arrives, and
/// further than a deliberate push does; one pushed on goes on for longer.
const SETTLE: u32 = 150;

/// How long what was brought back stays once the pointer has stopped
/// somewhere else. While the pointer is moving it stays, so that a
/// titlebar half a screen from the edge that brought it back is reached.
const LINGER: Duration = Duration::from_secs(1);

/// An edge of an output that answers to the pointer resting against it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    Top,
    Bottom,
}

/// Where a panel is drawn on an output, and whether at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Placing {
    /// Where panels are: over the windows of a desktop.
    Usual,
    /// Brought back over a fullscreen window.
    Raised,
    /// Not shown, the output being given over to a fullscreen window.
    Hidden,
}

/// What has been brought back over a fullscreen window, and what is about
/// to be.
#[derive(Debug, Default)]
pub(crate) struct Reveal {
    /// The edge the pointer is against, of which output, and since when.
    resting: Option<(usize, Side, Instant)>,
    /// The edge whose panels are back, and of which output.
    shown: Option<(usize, Side)>,
    /// Set by a key, which sends what was brought back away: nothing
    /// comes back until the pointer has left the edge and returned.
    quiet: bool,
    /// How far the pointer has been pushed on past the top and past the
    /// bottom of the output it is on, while against the edge, and when it
    /// began to be.
    pushed: (f64, f64),
    pressed: Option<u32>,
    /// When the pointer first moved while held against the edge.
    held: Option<u32>,
    /// Where the pointer last was, and when it last moved.
    last: (f64, f64),
    moved: Option<Instant>,
    /// When what was brought back goes, the pointer having stopped
    /// somewhere else.
    until: Option<Instant>,
}

impl Reveal {
    /// When what the pointer is resting for is due, if it is resting for
    /// anything.
    pub(crate) fn deadline(&self) -> Option<Instant> {
        let waiting = self.resting.filter(|_| self.shown.is_none() && !self.quiet);
        waiting.map(|(.., since)| since + REST).or(self.until)
    }

    /// Notes how far the pointer would have gone past the top or the bottom
    /// of its output, down being further, as only a mouse or touchpad
    /// moving it on against the edge says: if it was against that edge
    /// already, `held` says, as at the top and at the bottom.
    ///
    /// A push counts once the pointer has been against the edge for
    /// `SETTLE`, and from when it began, at `time`, for a second; one that
    /// has not gone far enough by then begins again.
    pub(crate) fn push(&mut self, past: f64, (top, bottom): (bool, bool), time: u32) {
        // Only once the pointer has been against the edge for a while: what
        // a throw carries on past it first does not count.
        self.held = self.held.or((top || bottom).then_some(time));
        let settled = self
            .held
            .is_some_and(|held| time.wrapping_sub(held) >= SETTLE);
        let (up, down) = ((-past).max(0.0), past.max(0.0));
        let up = up * f64::from(u8::from(top && settled));
        let down = down * f64::from(u8::from(bottom && settled));
        let pressed = (up > 0.0 || down > 0.0).then_some(time);
        let began = self.pressed.or(pressed);
        let stale = began.is_some_and(|began| time.wrapping_sub(began) > PRESSING);
        if stale {
            self.pushed = (0.0, 0.0);
        }
        self.pressed = if stale { pressed } else { began };
        self.pushed.0 += up;
        self.pushed.1 += down;
    }

    /// How far the pointer has been pushed on past an edge.
    const fn pushed(&self, side: Side) -> f64 {
        match side {
            Side::Top => self.pushed.0,
            Side::Bottom => self.pushed.1,
        }
    }

    /// Sends away what was brought back, as a key does, and a press in a
    /// window.
    pub(crate) const fn dismiss(&mut self) {
        self.shown = None;
        self.until = None;
        self.quiet = true;
    }
}

impl Compositor {
    /// Whether an output, counted in the order they were added, is showing
    /// a workspace made for a fullscreen window.
    fn given_over(&self, output: usize) -> bool {
        let mut places = self.desktop.workspaces();
        places.any(|place| place.output == output && place.shown && !place.tiles.is_empty())
    }

    /// The edge of an output given over to a fullscreen window that the
    /// pointer is within `reach` of, if it is at one.
    fn edge(&self, reach: impl Fn(usize, Side) -> f64) -> Option<(usize, Side)> {
        let point = self.pointer_location;
        let index = self.heads.iter().position(|head| head.contains(point))?;
        let area = self.heads[index].area().to_f64();
        let top = point.y < area.loc.y + reach(index, Side::Top);
        let bottom = point.y >= area.loc.y + area.size.h - reach(index, Side::Bottom);
        let side = top.then_some(Side::Top);
        let side = side.or(bottom.then_some(Side::Bottom));
        side.filter(|_| self.given_over(index))
            .map(|side| (index, side))
    }

    /// Whether the pointer is against the top and the bottom of the output
    /// it is on.
    pub(crate) fn held_at(&self) -> (bool, bool) {
        let against = self.edge(|_, _| 1.0).map(|(_, side)| side);
        (against == Some(Side::Top), against == Some(Side::Bottom))
    }

    /// Brings back what the pointer has rested for, and sends away what it
    /// has left.
    pub(crate) fn reveal(&mut self, now: Instant) {
        let point = (self.pointer_location.x, self.pointer_location.y);
        if self.hover.reveal.last != point {
            self.hover.reveal.last = point;
            self.hover.reveal.moved = Some(now);
        }
        // Against the very edge to bring something back. To keep it, the
        // pointer is on it, a panel or the titlebar under the panels, or
        // is still on its way somewhere: it goes when the pointer has
        // stopped elsewhere.
        let against = self.edge(|_, _| 1.0);
        let over = self.edge(|output, side| self.reach(output, side));
        let on_it = |shown: &(usize, Side)| over == Some(*shown);
        let stopped = self.hover.reveal.moved.map(|moved| moved + LINGER);
        let lingering = stopped.is_some_and(|stopped| now < stopped);
        let here = self
            .heads
            .iter()
            .position(|head| head.contains(self.pointer_location));
        let stays = |shown: &(usize, Side)| {
            let given_over = here == Some(shown.0) && self.given_over(shown.0);
            given_over && (on_it(shown) || lingering)
        };
        let kept = self.hover.reveal.shown.filter(stays);
        self.hover.reveal.until = kept.filter(|shown| !on_it(shown)).and(stopped);
        let since = |(output, side)| {
            let already = self.hover.reveal.resting;
            let already = already.filter(|(on, at, _)| (*on, *at) == (output, side));
            (output, side, already.map_or(now, |(.., since)| since))
        };
        self.hover.reveal.resting = against.map(since);
        self.hover.reveal.quiet = self.hover.reveal.quiet && against.is_some();
        // Pushed on against an edge, or rested against it for long enough:
        // what was pushed counts only while the pointer stays against it.
        let quiet = self.hover.reveal.quiet;
        let pushed = |(_, side): &(usize, Side)| self.hover.reveal.pushed(*side) >= PUSH && !quiet;
        let pushed = against.filter(pushed);
        let rested = |(.., since): &(usize, Side, Instant)| {
            now.saturating_duration_since(*since) >= REST && !quiet
        };
        let due = self.hover.reveal.resting.filter(rested);
        let due = due.map(|(output, side, _)| (output, side)).or(pushed);
        self.hover.reveal.shown = kept.or(due);
        self.hover.reveal.pushed = against.map_or((0.0, 0.0), |_| self.hover.reveal.pushed);
        self.hover.reveal.pressed = self.hover.reveal.pressed.filter(|_| against.is_some());
        self.hover.reveal.held = self.hover.reveal.held.filter(|_| against.is_some());
        // The titlebar is laid over the window under whatever panels are
        // along the top.
        for output in 0..self.heads.len() {
            let top = self.hover.reveal.shown == Some((output, Side::Top));
            let down = top.then(|| self.extent(output, Side::Top));
            self.desktop.reveal(output, down);
        }
    }

    /// How far in from an edge of an output what was brought back reaches:
    /// the panels along it, and at the top the titlebar under them. At
    /// least the edge itself.
    fn reach(&self, output: usize, side: Side) -> f64 {
        let titlebar = self.config().theme.style(Part::Titlebar, States::NONE);
        let titlebar = if side == Side::Top {
            titlebar.height
        } else {
            0
        };
        f64::from((self.extent(output, side) + titlebar).max(1))
    }

    /// How far in from an edge of an output the panels along it reach.
    fn extent(&self, output: usize, side: Side) -> u32 {
        let head = &self.heads[output];
        let height = head.area().size.h;
        let layers = layer_map_for_output(&head.output);
        let along = layers.layers_on(Layer::Top).filter_map(|surface| {
            let area = layers.layer_geometry(surface).unwrap_or_default();
            let (top, bottom) = (area.loc.y, area.loc.y + area.size.h);
            match side {
                Side::Top => (top <= 0).then_some(bottom),
                Side::Bottom => (bottom >= height).then_some(height - top),
            }
        });
        along.max().unwrap_or(0).unsigned_abs()
    }

    /// Where a panel of an output is drawn, and whether at all, going by
    /// which layer it is on and where on the output it is.
    ///
    /// The panel's place is given, not looked up: whoever asks is going
    /// through the output's panels and holds them.
    pub(crate) fn standing(
        &self,
        output: usize,
        layer: Layer,
        area: Rectangle<i32, Logical>,
    ) -> Placing {
        if layer != Layer::Top || !self.given_over(output) {
            return Placing::Usual;
        }
        let height = self.heads[output].area().size.h;
        let along = |side: Side| match side {
            Side::Top => area.loc.y <= 0,
            Side::Bottom => area.loc.y + area.size.h >= height,
        };
        let shown = self.hover.reveal.shown;
        let shown = shown.filter(|(on, side)| *on == output && along(*side));
        shown.map_or(Placing::Hidden, |_| Placing::Raised)
    }
}
