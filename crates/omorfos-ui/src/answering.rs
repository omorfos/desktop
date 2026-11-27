//! How a box that asks is answered (ADR 111): only by what was meant for
//! it. The keyboard starts on the button that refuses; the buttons that
//! give answer nothing until the box has been on the screen for
//! [`ARMED_AFTER`], one pressed too soon starting it again, to
//! [`ARMED_AT_MOST`], and the keyboard moved to one waiting
//! [`ARMED_AFTER`] there; a key counts as it is let go, and only if it
//! went down while the box was up; a press counts as it is let go on the
//! button it went down on; and either that went down too soon gives
//! nothing, however late it is let go. What refuses answers at once.
//!
//! It is a model and no more: each box that asks feeds it what it is told
//! of keys, the pointer and the time, and draws the button that has the
//! keyboard as `:focus`.

use std::time::Duration;

use crate::Moment;

/// How long after a box is first shown its buttons that give answer
/// nothing: as long as browsers keep a question from being answered by
/// what was meant for something else.
pub const ARMED_AFTER: Duration = Duration::from_millis(500);

/// How long after a box is first shown a press too soon may keep its
/// buttons that give from answering, at most.
pub const ARMED_AT_MOST: Duration = Duration::from_secs(2);

/// How many keys going down are kept count of at once; another held as
/// well counts for nothing as it is let go.
const HELD_AT_ONCE: usize = 8;

/// What a key is to a box that asks.
///
/// The buttons are side by side, so the keyboard moves among them by Tab
/// and the arrows across, and not round from the last to the first: one
/// key meant for something else, an arrow in a list, does not take it from
/// refusing to giving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Held {
    /// Enter or space: presses the button the keyboard is on.
    Enter,
    /// Escape: refuses, from anywhere.
    Escape,
    /// Tab or Right: the keyboard to the next button, if there is one.
    On,
    /// Shift+Tab or Left: the keyboard to the button before, if there is
    /// one.
    Back,
    /// Anything else, which answers nothing.
    Other,
}

/// A box that asks, of `N` buttons, being answered, by time `T`.
#[derive(Debug, Clone)]
pub struct Answering<T, const N: usize> {
    /// The button that refuses, where the keyboard starts.
    refuses: usize,
    /// The button the keyboard is on.
    focus: usize,
    /// When the box was first shown, and when its buttons that give may
    /// answer from.
    shown: Option<(T, T)>,
    /// From when the button the keyboard was moved to may answer by it.
    rested: Option<T>,
    /// The keys that went down while the box was up, and are down still,
    /// each with whether it went down too soon to give.
    down: [(u32, bool); HELD_AT_ONCE],
    held: usize,
    /// The button the pointer went down on, if it went down on one, and
    /// whether too soon to give.
    pressed: Option<(usize, bool)>,
}

impl<T: Moment + PartialOrd, const N: usize> Answering<T, N> {
    /// A box whose button `refuses` refuses, with the keyboard on it.
    #[must_use]
    pub const fn new(refuses: usize) -> Self {
        Self {
            refuses,
            focus: refuses,
            shown: None,
            rested: None,
            down: [(0, false); HELD_AT_ONCE],
            held: 0,
            pressed: None,
        }
    }

    /// Takes in that the box is on the screen at `now`; only the first
    /// time counts.
    pub fn shown(&mut self, now: T) {
        if self.shown.is_none() {
            self.shown = Some((now, now.after(ARMED_AFTER)));
        }
    }

    /// The button the keyboard is on.
    #[must_use]
    pub const fn focus(&self) -> usize {
        self.focus
    }

    /// Takes a key, by its `code`, going down or up, at `now`: the button
    /// it answers, if it answers one.
    pub fn key(&mut self, code: u32, held: Held, pressed: bool, now: T) -> Option<usize> {
        let shown = self.shown?;
        if pressed {
            let early = self.early(shown, now);
            if self.held < HELD_AT_ONCE {
                self.down[self.held] = (code, early);
                self.held += 1;
            }
            return None;
        }
        let at = self.down[..self.held]
            .iter()
            .position(|one| one.0 == code)?;
        let (_, early) = self.down[at];
        self.held -= 1;
        self.down[at] = self.down[self.held];
        match held {
            Held::Enter => {
                let moved = self.rested.is_some_and(|rested| now < rested);
                // Gone down after the wait, it is let go after it too.
                let gives = self.focus != self.refuses;
                (!(gives && (early || moved))).then_some(self.focus)
            }
            Held::Escape => Some(self.refuses),
            Held::On => self.moved((self.focus + 1).min(N.saturating_sub(1)), now),
            Held::Back => self.moved(self.focus.saturating_sub(1), now),
            Held::Other => None,
        }
    }

    /// The keyboard moved to `focus` at `now`: a button that gives answers
    /// by it only a moment after it came there.
    fn moved(&mut self, focus: usize, now: T) -> Option<usize> {
        if focus != self.focus {
            self.focus = focus;
            self.rested = Some(now.after(ARMED_AFTER));
        }
        None
    }

    /// Whether what goes down at `now`, the box `shown` as it was, is too
    /// soon to give, which starts the wait again.
    fn early(&mut self, (first, armed): (T, T), now: T) -> bool {
        let early = now < armed;
        if early {
            self.wait(first, now);
        }
        early
    }

    /// Takes the pointer's button going down or up, on button `on` if it
    /// is on one, at `now`: the button it answers, if it answers one.
    pub fn press(&mut self, on: Option<usize>, pressed: bool, now: T) -> Option<usize> {
        let shown = self.shown?;
        if pressed {
            let early = on.is_some_and(|on| on != self.refuses) && self.early(shown, now);
            self.pressed = on.map(|on| (on, early));
            return None;
        }
        let (went, early) = self.pressed.take()?;
        let button = on.filter(|button| went == *button)?;
        // Gone down after the wait, it is let go after it too.
        (!(early && button != self.refuses)).then_some(button)
    }

    /// Something that would give came too soon, at `now`, the box first
    /// shown at `first`: the buttons that give wait again, to
    /// [`ARMED_AT_MOST`] after then.
    fn wait(&mut self, first: T, now: T) {
        let again = now.after(ARMED_AFTER);
        let most = first.after(ARMED_AT_MOST);
        self.shown = Some((first, if again < most { again } else { most }));
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    const fn ms(millis: u64) -> Duration {
        Duration::from_millis(millis)
    }

    /// A box of two buttons, the first refusing, shown at the start.
    fn shown() -> Answering<Duration, 2> {
        let mut answering = Answering::new(0);
        answering.shown(ms(0));
        answering
    }

    /// A key, by `code`, pressed and let go at `at`.
    fn tap(
        answering: &mut Answering<Duration, 2>,
        code: u32,
        held: Held,
        at: Duration,
    ) -> Option<usize> {
        let down = answering.key(code, held, true, at);
        assert_eq!(down, None, "nothing answers as a key goes down");
        answering.key(code, held, false, at)
    }

    /// The pointer pressed on `down` and let go on `up` at `at`.
    fn click(
        answering: &mut Answering<Duration, 2>,
        (down, up): (Option<usize>, Option<usize>),
        at: Duration,
    ) -> Option<usize> {
        assert_eq!(answering.press(down, true, at), None);
        answering.press(up, false, at)
    }

    #[test]
    fn the_keyboard_starts_on_refusing_and_moves_without_going_round() {
        let mut answering = shown();
        assert_eq!(answering.focus(), 0);
        tap(&mut answering, 23, Held::Back, ms(1));
        assert_eq!(answering.focus(), 0, "not round, back to the last");
        assert_eq!(tap(&mut answering, 23, Held::On, ms(1)), None);
        assert_eq!(answering.focus(), 1);
        tap(&mut answering, 23, Held::On, ms(2));
        assert_eq!(answering.focus(), 1, "not round to the first");
        tap(&mut answering, 23, Held::Back, ms(3));
        assert_eq!(answering.focus(), 0);
        tap(&mut answering, 23, Held::On, ms(3));
        assert_eq!(tap(&mut answering, 30, Held::Other, ms(4)), None);
        assert_eq!(answering.focus(), 1);
    }

    #[test]
    fn enter_presses_the_button_the_keyboard_is_on() {
        let mut answering = shown();
        assert_eq!(tap(&mut answering, 28, Held::Enter, ms(10_000)), Some(0));
    }

    #[test]
    fn the_button_that_gives_answers_only_half_a_second_after_the_keyboard_comes_to_it() {
        // The keyboard moved to it at 1 ms: from 501 ms.
        let mut answering = shown();
        tap(&mut answering, 15, Held::On, ms(1));
        assert_eq!(tap(&mut answering, 28, Held::Enter, ms(500)), None);
        let mut answering = shown();
        tap(&mut answering, 15, Held::On, ms(1));
        assert_eq!(tap(&mut answering, 28, Held::Enter, ms(501)), Some(1));
        // Moved there long after the box was up, Tab and Enter at once, as
        // a form is gone through, is nothing.
        let mut answering = shown();
        tap(&mut answering, 15, Held::On, ms(5000));
        assert_eq!(tap(&mut answering, 28, Held::Enter, ms(5100)), None);
        assert_eq!(tap(&mut answering, 28, Held::Enter, ms(5600)), Some(1));
    }

    #[test]
    fn a_press_that_went_down_too_soon_answers_nothing_let_go_late() {
        let mut answering = shown();
        assert_eq!(answering.press(Some(1), true, ms(100)), None);
        assert_eq!(
            answering.press(Some(1), false, ms(900)),
            None,
            "held across"
        );
        let mut answering = shown();
        tap(&mut answering, 15, Held::On, ms(1));
        assert_eq!(answering.key(28, Held::Enter, true, ms(300)), None);
        assert_eq!(
            answering.key(28, Held::Enter, false, ms(900)),
            None,
            "held across"
        );
        // Refusing is answered however soon it went down.
        let mut answering = shown();
        assert_eq!(answering.press(Some(0), true, ms(1)), None);
        assert_eq!(answering.press(Some(0), false, ms(2)), Some(0));
    }

    #[test]
    fn refusing_answers_at_once() {
        let mut answering = shown();
        tap(&mut answering, 15, Held::On, ms(1));
        assert_eq!(tap(&mut answering, 1, Held::Escape, ms(1)), Some(0));
        let mut answering = shown();
        assert_eq!(click(&mut answering, (Some(0), Some(0)), ms(1)), Some(0));
    }

    #[test]
    fn a_key_down_before_the_box_was_up_answers_nothing() {
        let mut answering = Answering::<Duration, 2>::new(0);
        assert_eq!(answering.key(28, Held::Enter, true, ms(0)), None);
        answering.shown(ms(0));
        assert_eq!(answering.key(28, Held::Enter, false, ms(3000)), None);
        assert_eq!(tap(&mut answering, 28, Held::Enter, ms(3001)), Some(0));
        // A key let go that is not held, as one taken in twice, is nothing.
        assert_eq!(answering.key(28, Held::Enter, false, ms(3002)), None);
        // Eight keys held at once are kept count of; a ninth is not, and
        // counts for nothing as it is let go.
        for code in 100..109 {
            assert_eq!(answering.key(code, Held::Other, true, ms(3003)), None);
        }
        assert_eq!(answering.key(108, Held::Escape, false, ms(3004)), None);
        assert_eq!(answering.key(107, Held::Escape, false, ms(3004)), Some(0));
    }

    #[test]
    fn a_press_counts_only_let_go_where_it_went_down() {
        let mut answering = shown();
        assert_eq!(click(&mut answering, (Some(1), None), ms(1000)), None);
        assert_eq!(click(&mut answering, (Some(1), Some(0)), ms(1000)), None);
        assert_eq!(click(&mut answering, (None, Some(1)), ms(1000)), None);
        assert_eq!(
            answering.press(Some(1), false, ms(1000)),
            None,
            "never down"
        );
        assert_eq!(click(&mut answering, (Some(1), Some(1)), ms(1000)), Some(1));
    }

    #[test]
    fn presses_too_soon_keep_the_button_that_gives_from_answering() {
        let mut answering = shown();
        for at in [100, 400, 700] {
            assert_eq!(
                click(&mut answering, (Some(1), Some(1)), ms(at)),
                None,
                "{at}"
            );
        }
        assert_eq!(click(&mut answering, (Some(1), Some(1)), ms(1200)), Some(1));
        // Pressed every 300 ms from the first: not before two seconds.
        let mut answering = shown();
        for at in (0..2000).step_by(300) {
            assert_eq!(
                click(&mut answering, (Some(1), Some(1)), ms(at)),
                None,
                "{at}"
            );
        }
        assert_eq!(click(&mut answering, (Some(1), Some(1)), ms(2100)), Some(1));
    }

    #[test]
    fn nothing_answers_before_the_box_is_shown() {
        let mut answering = Answering::<Duration, 2>::new(0);
        assert_eq!(click(&mut answering, (Some(0), Some(0)), ms(5000)), None);
        assert_eq!(answering.key(1, Held::Escape, true, ms(5000)), None);
        assert_eq!(answering.key(1, Held::Escape, false, ms(5000)), None);
        // Shown twice, the first counts.
        answering.shown(ms(6000));
        answering.shown(ms(9000));
        tap(&mut answering, 15, Held::On, ms(6000));
        assert_eq!(tap(&mut answering, 28, Held::Enter, ms(6500)), Some(1));
    }
}
