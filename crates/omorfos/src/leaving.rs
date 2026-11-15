//! Whether to start the machine again or turn it off, asked first: what
//! the `restart` and `shut_down` actions do while `power.ask` is on, from
//! the root menu, a key or `omorfosctl` alike.
//!
//! The question is a card in the middle of the screen it was asked on,
//! drawn as the theme's `question` says, with a button that cancels and one
//! that goes on. It has the pointer and Enter and Escape while it is up:
//! Enter or the button that goes on does it, and Escape, Cancel or a click
//! anywhere else does not. Left alone, it counts down a minute and does it,
//! as GNOME's does: whoever walked away from a machine being turned off
//! did not mean it to stay on.

use omorfos_config::{Part, State, States, Theme};
use omorfos_core::geometry::{Point as DomainPoint, Rect, Size as DomainSize};
use omorfos_i18n::{Value, Words};
use omorfos_ui::question::Asked;
use omorfos_ui::{Area, Canvas};
use smithay::input::keyboard::Keysym;
use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle};
use std::time::{Duration, Instant};

use crate::Scale;
use crate::compositor::{Compositor, Painted};
use crate::paint::Text;

/// How long the question waits, left alone, before it is taken as a yes.
const WAITS: Duration = Duration::from_secs(60);

/// How wide the card is; the rest of its layout is a question's.
const WIDTH: u32 = 380;

/// The question whether to start the machine again or turn it off.
#[derive(Debug, Default)]
pub(crate) struct Leave {
    /// What is asked, and where the pointer was as it was asked.
    pub(crate) asked: Option<(Leaving, Point<f64, Logical>)>,
    /// When it is done of itself: none until the compositor next knows
    /// what time it is.
    until: Option<Instant>,
    /// The question as it is shown.
    pub(crate) shown: Option<Asking>,
}

impl Leave {
    /// When something is next due, while it is asked: a second less to
    /// wait, or its being done.
    pub(crate) fn deadline(&self) -> Option<Instant> {
        let until = self.until.filter(|_| self.asked.is_some())?;
        let left = self.shown.as_ref().map_or(0, |asking| asking.left);
        let next = Duration::from_secs(left.saturating_sub(1));
        Some(until.checked_sub(next).unwrap_or(until))
    }
}

/// What the machine is asked to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Leaving {
    /// Start again.
    Restart,
    /// Turn off.
    ShutDown,
    /// End the session.
    LogOut,
}

impl Leaving {
    /// What `omorfos-session` is asked, to do it; nothing to end the
    /// session, which the compositor does itself.
    const fn request(self) -> Option<&'static str> {
        match self {
            Self::Restart => Some("restart"),
            Self::ShutDown => Some("shut-down"),
            Self::LogOut => None,
        }
    }

    /// The messages of its question and of the button that does it.
    const fn said(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Restart => ("restart-ask", "restart", "restart-in"),
            Self::ShutDown => ("shut-down-ask", "shut-down", "shut-down-in"),
            Self::LogOut => ("log-out-ask", "log-out", "log-out-in"),
        }
    }
}

/// A button of the question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Choice {
    Cancel,
    GoOn,
}

/// The question as it is shown.
#[derive(Debug)]
pub(crate) struct Asking {
    what: Leaving,
    pub(crate) output: Output,
    /// The screen's place, which it is in the middle of.
    area: Rectangle<i32, Logical>,
    scale: Scale,
    theme: Theme,
    /// The button the pointer was on as it was painted.
    hovered: Option<Choice>,
    /// How many seconds it said were left.
    left: u64,
    /// Where it is, and its buttons.
    pub(crate) at: Point<i32, Logical>,
    whole: Rect,
    cancel: Rect,
    go_on: Rect,
    pub(crate) pixels: Painted,
}

impl Asking {
    /// The button at `point`, if one is there.
    pub(crate) fn choice_at(&self, point: DomainPoint) -> Option<Choice> {
        let buttons = [(Choice::Cancel, self.cancel), (Choice::GoOn, self.go_on)];
        let mut under = buttons.into_iter().filter(|(_, at)| at.contains(point));
        under.next().map(|(choice, _)| choice)
    }

    /// Whether `point` is on it.
    pub(crate) fn contains(&self, point: DomainPoint) -> bool {
        self.whole.contains(point)
    }

    /// The screen it is on, where it is, and the screen's scale: what the
    /// session is sealed round, as for a prompt that asks for a secret.
    pub(crate) fn sealed(&self) -> (Output, Area, Scale) {
        let (origin, size) = (self.whole.origin, self.whole.size);
        let area = (origin.x, origin.y, size.width, size.height);
        (self.output.clone(), area, self.scale)
    }
}

impl Compositor {
    /// Starts the machine again or turns it off, as `what` says: once the
    /// user has said so, while `power.ask` is on.
    pub(crate) fn leave(&mut self, what: Leaving) {
        if self.config().general.power.ask {
            self.cards.leave.asked = Some((what, self.pointer_location));
            self.cards.leave.until = None;
        } else {
            self.go(what);
        }
    }

    /// Does `what`.
    fn go(&mut self, what: Leaving) {
        match what.request() {
            Some(request) => self.ask_session(&[request]),
            None => self.quit(),
        }
    }

    /// Takes the user's answer to the question: whether to go on.
    pub(crate) fn answer_leaving(&mut self, go_on: bool) {
        let asked = self.cards.leave.asked.take().filter(|_| go_on);
        if let Some((what, _)) = asked {
            self.go(what);
        }
    }

    /// Takes a key while the question is up: Enter says yes, and Escape
    /// no. Says whether the key was the question's.
    pub(crate) fn leaving_key(&mut self, keysym: Keysym) -> bool {
        let answer = match keysym {
            Keysym::Return | Keysym::KP_Enter => Some(true),
            Keysym::Escape => Some(false),
            _ => None,
        };
        let answer = answer.filter(|_| self.cards.leave.asked.is_some());
        answer
            .inspect(|go_on| self.answer_leaving(*go_on))
            .is_some()
    }

    /// Takes a button pressed, or let go, at `point`, while the question is
    /// up, which has the pointer meanwhile. Says whether it is up.
    pub(crate) fn leaving_press(&mut self, pressed: bool, point: DomainPoint) -> bool {
        let up = self.cards.leave.asked.is_some();
        if up && pressed {
            self.press_leaving(point);
        }
        up
    }

    /// Takes a press at `point` while the question is up: on a button, its
    /// answer; anywhere else but on the card, no.
    fn press_leaving(&mut self, point: DomainPoint) {
        let asking = self.cards.leave.shown.as_ref();
        let choice = asking.and_then(|asking| asking.choice_at(point));
        let on = asking.is_some_and(|asking| asking.contains(point));
        match choice {
            Some(choice) => self.answer_leaving(choice == Choice::GoOn),
            None if !on => self.answer_leaving(false),
            None => {}
        }
    }

    /// Starts the minute the question waits, as of `now`, and does it once
    /// that is up. Then puts it in the middle of the screen it was asked
    /// on, or of the first if that has gone, while it is asked, saying how
    /// many seconds are left, painted again only if how it would look has
    /// changed; or takes it away.
    pub(crate) fn ask_to_leave(&mut self, now: Instant) {
        let asked = self.cards.leave.asked.is_some();
        let until = asked.then(|| *self.cards.leave.until.get_or_insert(now + WAITS));
        if until.is_some_and(|until| until <= now) {
            self.answer_leaving(true);
        }
        // Each whole second or part of one left is said as one.
        let left = until.map_or(0, |until| {
            let left = until.saturating_duration_since(now);
            left.as_secs() + u64::from(left.subsec_nanos() > 0)
        });
        let before = self.cards.leave.shown.take();
        let Some((what, asked)) = self.cards.leave.asked else {
            return;
        };
        let head = self.head_at(asked).or(self.primary_head());
        let placed = head.map(|head| (head.output.clone(), head.area(), head.scale));
        let theme = self.config().theme.clone();
        let pointer = self.pointer_position();
        let hovered = before.as_ref().and_then(|asking| asking.choice_at(pointer));
        let tools = (&mut self.text, &self.words);
        self.cards.leave.shown = placed.and_then(|(output, area, scale)| {
            let same = |asking: &Asking| {
                asking.what == what
                    && asking.output == output
                    && asking.area == area
                    && asking.scale == scale
                    && asking.theme == theme
                    && asking.hovered == hovered
                    && asking.left == left
            };
            before.filter(same).or_else(|| {
                let asked = (what, hovered, left);
                let (canvas, (cancel, go_on)) = paint(tools, &theme, asked, scale);
                let (width, height) = canvas.units();
                let middle = |room: i32, length: u32| (room - length.cast_signed()) / 2;
                let within = (middle(area.size.w, width), middle(area.size.h, height));
                let at = area.loc + Point::from(within);
                let on = |(x, y, wide, high): Area| {
                    let origin = DomainPoint::new(at.x + x, at.y + y);
                    Rect::new(origin, DomainSize::new(wide, high))
                };
                Some(Asking {
                    what,
                    output,
                    area,
                    scale,
                    theme,
                    hovered,
                    left,
                    at,
                    whole: on((0, 0, width, height)),
                    cancel: on(cancel),
                    go_on: on(go_on),
                    pixels: Painted::new(&canvas),
                })
            })
        });
    }

    /// Where the buttons of the question are, while it is shown: the one
    /// that cancels, then the one that goes on. For tests, which press them.
    #[must_use]
    pub fn leaving_buttons(&self) -> Option<(Area, Area)> {
        let area = |rect: Rect| {
            let (origin, size) = (rect.origin, rect.size);
            (origin.x, origin.y, size.width, size.height)
        };
        let asking = self.cards.leave.shown.as_ref();
        asking.map(|asking| (area(asking.cancel), area(asking.go_on)))
    }
}

/// Paints the question whether to do `what`, the button `hovered` lit and
/// how many seconds are `left` before it is done, and says where its
/// buttons are on it: the one that cancels, then the one that goes on,
/// which has the keyboard and is filled with the accent.
fn paint(
    (text, words): (&mut Text, &Words),
    theme: &Theme,
    (what, hovered, left): (Leaving, Option<Choice>, u64),
    scale: Scale,
) -> (Canvas, (Area, Area)) {
    let (question, going, counting) = what.said();
    let (question, going) = (words.say(question, &[]), words.say(going, &[]));
    let left = i64::try_from(left).unwrap_or(i64::MAX);
    let counting = words.say(counting, &[("left", Value::Number(left))]);
    let (unsaved, cancel) = (words.say("leave-unsaved", &[]), words.say("cancel", &[]));
    let detail_font = theme.font(Part::QuestionDetail, States::NONE);
    let inset = theme.style(Part::Question, States::NONE).inset();
    let mut lines = vec![counting];
    lines.extend(text.wrap(
        &unsaved,
        WIDTH.saturating_sub(inset.across()),
        detail_font,
        3,
    ));
    let asked = Asked {
        question: &question,
        lines: &lines,
        room: 0,
        buttons: [cancel.as_str(), going.as_str()],
    };
    let laid = omorfos_ui::question::lay_out(text, theme, WIDTH, &asked);
    // The one that goes on is picked out and has the keyboard.
    let states = [Choice::Cancel, Choice::GoOn].map(|choice| {
        let going_on = choice == Choice::GoOn;
        States::NONE
            .when(State::Hovered, hovered == Some(choice))
            .when(State::Active, going_on)
            .when(State::Focused, going_on)
    });
    let canvas = omorfos_ui::question::paint_question(text, theme, (&asked, &laid), states, scale);
    let [cancelling, go_on] = laid.buttons;
    (canvas, (cancelling, go_on))
}
