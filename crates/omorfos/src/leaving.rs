//! Whether to start the machine again or turn it off, asked first: what
//! the `restart` and `shut_down` actions do while `power.ask` is on, from
//! the root menu, a key or `omorfosctl` alike.
//!
//! The question is a card in the middle of the screen it was asked on,
//! drawn as the theme's `question` says, with a button that cancels and one
//! that goes on. It has the pointer and the keys that answer it while it is
//! up, and is answered as ADR 111 has every box that asks: the keyboard
//! starts on Cancel, Tab and the arrows move it, Enter and space press the
//! button it is on, and Escape, or a click anywhere else, cancels; the
//! button that goes on does nothing until the question has been up for
//! half a second. Left alone, it counts down a minute and does it,
//! as GNOME's does: whoever walked away from a machine being turned off
//! did not mean it to stay on.

use omorfos_config::{Part, State, States, Theme};
use omorfos_core::geometry::{Point as DomainPoint, Rect, Size as DomainSize};
use omorfos_i18n::{Value, Words};
use omorfos_ui::question::Asked;
use omorfos_ui::{Answering, Area, Canvas, Held};
use smithay::input::keyboard::Keysym;
use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle};
use std::time::{Duration, Instant};

use crate::Scale;
use crate::compositor::{Compositor, Painted};
use crate::paint::Text;

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
    /// How it is being answered, made anew as it is asked.
    answering: Option<Answering<Duration, 2>>,
}

/// The button that cancels, where the keyboard starts, and the one that
/// goes on.
const CANCEL: usize = 0;
const GO_ON: usize = 1;

/// What a key is to the question, by what it types, if it is the
/// question's at all.
pub(crate) fn held(keysym: Keysym) -> Option<Held> {
    match keysym {
        Keysym::Return | Keysym::KP_Enter | Keysym::space => Some(Held::Enter),
        Keysym::Escape => Some(Held::Escape),
        Keysym::Tab | Keysym::Right => Some(Held::On),
        Keysym::ISO_Left_Tab | Keysym::Left => Some(Held::Back),
        _ => None,
    }
}

impl Leave {
    /// The user was there just now: what is asked waits its time again,
    /// counted from the next time it is looked at.
    pub(crate) const fn stirred(&mut self) {
        self.until = None;
    }

    /// When something is next due, while it is asked: a second less to
    /// wait, or its being done.
    pub(crate) fn deadline(&self) -> Option<Instant> {
        let until = self.until.filter(|_| self.asked.is_some())?;
        let left = self
            .shown
            .as_ref()
            .and_then(|asking| asking.left)
            .unwrap_or(0);
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
    /// The button the keyboard was on as it was painted.
    focus: usize,
    /// How many seconds it said were left, if it counts them.
    left: Option<u64>,
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
            self.cards.leave.answering = Some(Answering::new(CANCEL));
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

    /// Takes a key, by its `code`, going down or up, while the question is
    /// up: says whether the key was the question's. It is answered as the
    /// key is let go, if it went down while the question was up.
    pub(crate) fn leaving_key(&mut self, code: u32, keysym: Keysym, pressed: bool) -> bool {
        let now = self.clock();
        let leave = &mut self.cards.leave;
        let answering = leave.answering.as_mut().filter(|_| leave.asked.is_some());
        let Some((answering, held)) = answering.zip(held(keysym)) else {
            return false;
        };
        let chosen = answering.key(code, held, pressed, now);
        if let Some(button) = chosen {
            self.answer_leaving(button == GO_ON);
        }
        true
    }

    /// Takes a button pressed, or let go, at `point`, while the question is
    /// up, which has the pointer meanwhile: a button is answered as it is
    /// let go on what it went down on, and a press anywhere but on the
    /// card cancels. Says whether it is up.
    pub(crate) fn leaving_press(&mut self, pressed: bool, point: DomainPoint) -> bool {
        let now = self.clock();
        let leave = &mut self.cards.leave;
        let Some(answering) = leave.answering.as_mut().filter(|_| leave.asked.is_some()) else {
            return false;
        };
        let asking = leave.shown.as_ref();
        let on = asking.and_then(|asking| asking.choice_at(point));
        let on_card = asking.is_some_and(|asking| asking.contains(point));
        let button = on.map(|choice| {
            if choice == Choice::GoOn {
                GO_ON
            } else {
                CANCEL
            }
        });
        if pressed && !on_card {
            self.answer_leaving(false);
            return true;
        }
        if let Some(button) = answering.press(button, pressed, now) {
            self.answer_leaving(button == GO_ON);
        }
        true
    }

    /// Starts the minute the question waits, as of `now`, and does it once
    /// that is up. Then puts it in the middle of the screen it was asked
    /// on, or of the first if that has gone, while it is asked, saying how
    /// many seconds are left, painted again only if how it would look has
    /// changed; or takes it away.
    pub(crate) fn ask_to_leave(&mut self, now: Instant) {
        // As long as `power.wait` says, from when it was asked or the user
        // was last seen to be there; with none, until it is answered.
        let wait = u64::from(self.config().general.power.wait.get());
        let wait = Duration::from_secs(wait);
        let counts = self.cards.leave.asked.is_some() && !wait.is_zero();
        // Nothing counted, nothing is due: not even what was, before the
        // wait was set to none while it was asked.
        if !counts {
            self.cards.leave.until = None;
        }
        let until = counts.then(|| *self.cards.leave.until.get_or_insert(now + wait));
        if until.is_some_and(|until| until <= now) {
            self.answer_leaving(true);
        }
        // Each whole second or part of one left is said as one.
        let left = until.map(|until| {
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
        let clock = self.clock();
        let answering = self.cards.leave.answering.as_mut();
        let focus = answering.map_or(CANCEL, |answering| {
            answering.shown(clock);
            answering.focus()
        });
        let tools = (&mut self.text, &self.words);
        self.cards.leave.shown = placed.and_then(|(output, area, scale)| {
            let same = |asking: &Asking| {
                asking.what == what
                    && asking.output == output
                    && asking.area == area
                    && asking.scale == scale
                    && asking.theme == theme
                    && asking.hovered == hovered
                    && asking.focus == focus
                    && asking.left == left
            };
            before.filter(same).or_else(|| {
                let asked = (what, (hovered, focus), left);
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
                    focus,
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

/// Paints the question whether to do `what`, the button `hovered` lit, the
/// one the keyboard is on, `focus`, as such, and how many seconds are
/// `left` before it is done, and says where its buttons are on it: the one
/// that cancels, then the one that goes on, which is filled with the
/// accent.
fn paint(
    (text, words): (&mut Text, &Words),
    theme: &Theme,
    (what, (hovered, focus), left): (Leaving, (Option<Choice>, usize), Option<u64>),
    scale: Scale,
) -> (Canvas, (Area, Area)) {
    let (question, going, counting) = what.said();
    let (question, going) = (words.say(question, &[]), words.say(going, &[]));
    // How many seconds are left, said only where they are counted.
    let counting = left.map(|left| {
        let left = i64::try_from(left).unwrap_or(i64::MAX);
        words.say(counting, &[("left", Value::Number(left))])
    });
    let (unsaved, cancel) = (words.say("leave-unsaved", &[]), words.say("cancel", &[]));
    let detail_font = theme.font(Part::QuestionDetail, States::NONE);
    let inset = theme.style(Part::Question, States::NONE).inset();
    let mut lines: Vec<String> = counting.into_iter().collect();
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
    // The one that goes on is picked out; the keyboard is where it is.
    let states = [Choice::Cancel, Choice::GoOn].map(|choice| {
        let going_on = choice == Choice::GoOn;
        let index = if going_on { GO_ON } else { CANCEL };
        States::NONE
            .when(State::Hovered, hovered == Some(choice))
            .when(State::Active, going_on)
            .when(State::Focused, index == focus)
    });
    let canvas = omorfos_ui::question::paint_question(text, theme, (&asked, &laid), states, scale);
    let [cancelling, go_on] = laid.buttons;
    (canvas, (cancelling, go_on))
}
