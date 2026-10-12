//! Looks on their way from one to another.
//!
//! A theme says how long a part's colours take to change when its state
//! does. Whatever draws parts that change state keeps a [`Motion`], asks
//! it for their looks and not the theme, and draws again for as long as it
//! says something is still on its way.

use std::collections::HashMap;
use std::time::Duration;

use omorfos_config::{Part, States, Style, Theme};

/// The parts being drawn, and how far each has got towards the look of
/// the state it is in.
///
/// Parts of the same kind are told apart by a number of the caller's
/// choosing, which has to stay with the same thing from one picture to the
/// next: a window's, or the place of a row.
#[derive(Debug, Default)]
pub struct Motion {
    now: Duration,
    moving: bool,
    /// When the picture is to be drawn again, if anything in it is still
    /// on its way.
    again: Option<Duration>,
    /// What is in the picture, by when it first was, and whether it is in
    /// the one being drawn.
    present: HashMap<(Part, u64), (Duration, bool)>,
    parts: HashMap<(Part, u64), Changing>,
}

/// One part as it was last drawn.
#[derive(Debug)]
struct Changing {
    /// The states it was in.
    states: States,
    /// The look it set out from and when, while it is on its way.
    from: Option<(Style, Duration)>,
    /// Whether it is in the picture being drawn.
    seen: bool,
}

/// The look a part has at `now`, on its way to `to`.
fn shown(to: Style, from: Option<(Style, Duration)>, now: Duration) -> Style {
    from.map_or(to, |(from, since)| {
        let share = to.transition.progress(now.saturating_sub(since));
        to.blended(&from, share)
    })
}

impl Motion {
    /// How long there is between one picture and the next while something
    /// is on its way: about what a screen takes to show one.
    pub const FRAME: Duration = Duration::from_millis(16);

    /// Nothing drawn yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Begins a picture of how things are at `now`, a time counted from
    /// whenever the caller likes.
    pub fn begin(&mut self, now: Duration) {
        self.now = now;
        self.moving = false;
        for part in self.parts.values_mut() {
            part.seen = false;
        }
        for (_, seen) in self.present.values_mut() {
            *seen = false;
        }
    }

    /// How much of the `part` that goes by `key` there is to see in the
    /// picture being drawn: from 0, nothing, when it is first in a picture,
    /// to 1, all of it, once the time the theme gives the part's colours to
    /// change has gone by.
    ///
    /// It is asked of whatever comes and goes, in every picture it is in.
    /// Something left out of a picture comes in afresh the next time.
    pub fn presence(&mut self, theme: &Theme, part: Part, key: u64) -> f32 {
        let now = self.now;
        let (since, seen) = self.present.entry((part, key)).or_insert((now, true));
        *seen = true;
        let coming = theme.style(part, States::NONE).transition;
        let share = coming.progress(now.saturating_sub(*since));
        self.moving |= share < 1.0;
        share
    }

    /// How the `part` that goes by `key` looks in the picture being drawn,
    /// now that it is in `states`.
    ///
    /// A part drawn for the first time has the look of its state at once.
    /// One whose state has changed since it was last drawn sets out from
    /// how it looked then.
    pub fn style(&mut self, theme: &Theme, part: Part, key: u64, states: States) -> Style {
        let now = self.now;
        let to = theme.style(part, states);
        let changing = self.parts.entry((part, key)).or_insert(Changing {
            states,
            from: None,
            seen: true,
        });
        changing.seen = true;
        if changing.states != states {
            let was = theme.style(part, changing.states);
            changing.from = Some((shown(was, changing.from, now), now));
            changing.states = states;
        }
        // What has arrived is no longer on its way.
        let arrived =
            |(_, since): &(Style, Duration)| now.saturating_sub(*since) >= to.transition.lasts();
        changing.from = changing.from.filter(|from| !arrived(from));
        self.moving |= changing.from.is_some();
        shown(to, changing.from, now)
    }

    /// Ends the picture, forgetting the parts that were not in it. Returns
    /// whether anything in it is still on its way, so that it has to be
    /// drawn again shortly: see [`Motion::deadline`].
    pub fn end(&mut self) -> bool {
        self.parts.retain(|_, part| part.seen);
        self.present.retain(|_, (_, seen)| *seen);
        self.again = self.moving.then(|| self.now.saturating_add(Self::FRAME));
        self.moving
    }

    /// When the picture is to be drawn again, if anything in the last one
    /// was still on its way.
    #[must_use]
    pub const fn deadline(&self) -> Option<Duration> {
        self.again
    }

    /// Whether it is time, at `now`, to draw the picture again. It says so
    /// once.
    pub fn due(&mut self, now: Duration) -> bool {
        self.again.take_if(|due| *due <= now).is_some()
    }
}

#[cfg(test)]
mod tests {
    use omorfos_config::{Color, State};

    use super::*;

    const RED: Color = Color::rgb(255, 0, 0);
    const BLUE: Color = Color::rgb(0, 0, 255);
    const PURPLE: (u8, u8, u8) = (128, 0, 128);
    const HOVERED: States = States::NONE.with(State::Hovered);

    fn theme() -> Theme {
        let stylesheet = "task { background: #f00; transition: 100ms linear; }
                          task:hover { background: #00f; }
                          clock { background: #f00; transition: none; }
                          clock:hover { background: #00f; }";
        Theme::parse(stylesheet).unwrap()
    }

    fn millis(count: u64) -> Duration {
        Duration::from_millis(count)
    }

    /// Whether a colour is `wanted`, give or take what rounding does.
    fn near(color: Color, wanted: (u8, u8, u8)) -> bool {
        let far = [
            color.red.abs_diff(wanted.0),
            color.green.abs_diff(wanted.1),
            color.blue.abs_diff(wanted.2),
        ];
        far.into_iter().all(|apart| apart <= 2)
    }

    /// The background of one part in a picture drawn at `now`, and whether
    /// the picture has to be drawn again.
    fn drawn(motion: &mut Motion, part: Part, states: States, now: u64) -> (Color, bool) {
        motion.begin(millis(now));
        let style = motion.style(&theme(), part, 7, states);
        (style.background, motion.end())
    }

    #[test]
    fn a_part_drawn_for_the_first_time_looks_as_its_state_says() {
        let mut motion = Motion::new();
        assert_eq!(motion.deadline(), None);
        assert_eq!(drawn(&mut motion, Part::Task, HOVERED, 0), (BLUE, false));
        assert_eq!(drawn(&mut motion, Part::Task, HOVERED, 500), (BLUE, false));
    }

    #[test]
    fn a_part_whose_state_changes_gets_to_its_new_look_over_time() {
        let mut motion = Motion::new();
        drawn(&mut motion, Part::Task, States::NONE, 1000);
        assert_eq!(drawn(&mut motion, Part::Task, HOVERED, 1000), (RED, true));
        let (half, moving) = drawn(&mut motion, Part::Task, HOVERED, 1050);
        assert!(near(half, PURPLE), "{half:?}");
        assert!(moving);
        assert_eq!(drawn(&mut motion, Part::Task, HOVERED, 1100), (BLUE, false));
        assert_eq!(drawn(&mut motion, Part::Task, HOVERED, 9000), (BLUE, false));
    }

    #[test]
    fn a_part_that_turns_back_half_way_sets_out_from_where_it_had_got_to() {
        let mut motion = Motion::new();
        drawn(&mut motion, Part::Task, States::NONE, 0);
        drawn(&mut motion, Part::Task, HOVERED, 0);
        let (half, _) = drawn(&mut motion, Part::Task, States::NONE, 50);
        assert!(near(half, PURPLE), "no jump as it turns: {half:?}");
        let (later, moving) = drawn(&mut motion, Part::Task, States::NONE, 100);
        assert!(near(later, (191, 0, 64)), "{later:?}");
        assert!(moving);
        assert_eq!(
            drawn(&mut motion, Part::Task, States::NONE, 150),
            (RED, false)
        );
    }

    #[test]
    fn a_part_whose_theme_gives_it_no_time_changes_at_once() {
        let mut motion = Motion::new();
        drawn(&mut motion, Part::Clock, States::NONE, 0);
        assert_eq!(drawn(&mut motion, Part::Clock, HOVERED, 0), (BLUE, false));
    }

    #[test]
    fn parts_of_a_kind_are_told_apart_and_each_goes_its_own_way() {
        let (theme, mut motion) = (theme(), Motion::new());
        motion.begin(millis(0));
        motion.style(&theme, Part::Task, 1, States::NONE);
        motion.style(&theme, Part::Task, 2, States::NONE);
        motion.end();
        motion.begin(millis(50));
        let first = motion.style(&theme, Part::Task, 1, HOVERED);
        let second = motion.style(&theme, Part::Task, 2, States::NONE);
        assert_eq!((first.background, second.background), (RED, RED));
        assert!(motion.end());
        motion.begin(millis(100));
        let first = motion.style(&theme, Part::Task, 1, HOVERED);
        let second = motion.style(&theme, Part::Task, 2, States::NONE);
        assert!(near(first.background, PURPLE), "{first:?}");
        assert_eq!(second.background, RED);
    }

    #[test]
    fn a_picture_with_something_on_its_way_is_due_again_a_moment_later() {
        let mut motion = Motion::new();
        drawn(&mut motion, Part::Task, States::NONE, 1000);
        assert_eq!(motion.deadline(), None);
        assert!(!motion.due(millis(5000)), "nothing to draw again");

        drawn(&mut motion, Part::Task, HOVERED, 1000);
        assert_eq!(motion.deadline(), Some(millis(1016)));
        assert!(!motion.due(millis(1010)), "not yet");
        assert!(motion.due(millis(1016)));
        assert!(!motion.due(millis(1016)), "said once");
        assert_eq!(motion.deadline(), None);

        drawn(&mut motion, Part::Task, HOVERED, 1100);
        assert_eq!(motion.deadline(), None, "it has arrived");
    }

    /// How much of one part there is to see in a picture drawn at `now`,
    /// in hundredths, and whether the picture has to be drawn again.
    fn present(motion: &mut Motion, part: Part, now: u64) -> (i32, bool) {
        motion.begin(millis(now));
        let share = motion.presence(&theme(), part, 7);
        #[expect(clippy::cast_possible_truncation, reason = "it is from 0 to 100")]
        let hundredths = (share * 100.0).round() as i32;
        (hundredths, motion.end())
    }

    #[test]
    fn something_that_comes_into_the_picture_is_all_there_after_a_while() {
        let mut motion = Motion::new();
        assert_eq!(present(&mut motion, Part::Task, 1000), (0, true));
        assert_eq!(motion.deadline(), Some(millis(1016)));
        assert_eq!(present(&mut motion, Part::Task, 1050), (50, true));
        assert_eq!(present(&mut motion, Part::Task, 1100), (100, false));
        assert_eq!(present(&mut motion, Part::Task, 5000), (100, false));
        assert_eq!(motion.deadline(), None);
    }

    #[test]
    fn something_that_was_out_of_the_picture_comes_in_afresh() {
        let mut motion = Motion::new();
        present(&mut motion, Part::Task, 0);
        present(&mut motion, Part::Task, 500);
        motion.begin(millis(600));
        motion.end();
        assert_eq!(present(&mut motion, Part::Task, 700), (0, true));
    }

    #[test]
    fn something_whose_theme_gives_it_no_time_is_all_there_at_once() {
        let mut motion = Motion::new();
        assert_eq!(present(&mut motion, Part::Clock, 0), (100, false));
    }

    #[test]
    fn a_part_left_out_of_a_picture_is_forgotten() {
        let mut motion = Motion::new();
        drawn(&mut motion, Part::Task, States::NONE, 0);
        motion.begin(millis(10));
        assert!(!motion.end());
        // Had it been remembered it would set out from red.
        assert_eq!(drawn(&mut motion, Part::Task, HOVERED, 20), (BLUE, false));
    }
}
