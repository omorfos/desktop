//! Looks on their way from one to another.
//!
//! A theme says how long a part's colours take to change when its state
//! does. Whatever draws parts that change state keeps a [`Motion`], asks
//! it for their looks and not the theme, and draws again for as long as it
//! says something is still on its way.

use std::collections::HashMap;
use std::time::Duration;

use omorfos_config::{Part, Presence, State, States, Style, Theme};

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
    /// What is in the picture, by when it first was, or none if it is taken
    /// to have been there all along, and whether it is in the one being
    /// drawn.
    present: HashMap<(Part, u64), (Option<Duration>, bool)>,
    parts: HashMap<(Part, u64), Changing>,
    fades: HashMap<(Part, u64), Fading>,
}

/// Something shown only while it is wanted, as it was last drawn.
#[derive(Debug)]
struct Fading {
    /// Whether it is wanted, and since when.
    wanted: bool,
    since: Duration,
    /// How much of it there was to see then.
    from: f32,
    /// Whether it is in the picture being drawn.
    seen: bool,
}

/// One part as it was last drawn.
#[derive(Debug)]
struct Changing {
    /// The states it was in, and those the part it is in was in.
    states: (States, States),
    /// The look it set out from and when, while it is on its way.
    from: Option<(Style, Duration)>,
    /// Whether it is in the picture being drawn.
    seen: bool,
}

/// How much of a part there is to see, and how large it is drawn, as it
/// comes and goes: each a share of how it is once there.
///
/// This is the one place both are worked out, from what the theme says a
/// part arrives as and leaves as.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Seen {
    /// From 0, none of it, to 1, as much as there is once it is there.
    pub opacity: f32,
    /// How large it is, 1 being its size once there.
    pub scale: f32,
}

impl Seen {
    /// All of it, as large as it is.
    pub const WHOLE: Self = Self {
        opacity: 1.0,
        scale: 1.0,
    };

    /// `share` of the way from the look `from` to `to`, of a part that is
    /// `there` once it has arrived.
    fn between(from: Presence, to: Presence, there: Presence, share: f32) -> Self {
        let between = |from: f32, to: f32| from + (to - from) * share;
        let opacity = between(f32::from(from.opacity), f32::from(to.opacity));
        let scale = between(f32::from(from.scale), f32::from(to.scale));
        Self {
            // Drawn over a part as faint as it is once there.
            opacity: (opacity / f32::from(there.opacity.max(1))).min(1.0),
            scale: scale / f32::from(Presence::WHOLE.scale),
        }
    }

    /// This, and `other` as well: what has gone while it was still coming,
    /// which leaves from as much of it as had come.
    #[must_use]
    pub fn and(self, other: Self) -> Self {
        Self {
            opacity: self.opacity * other.opacity,
            scale: self.scale * other.scale,
        }
    }
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
        for fading in self.fades.values_mut() {
            fading.seen = false;
        }
    }

    /// How much of the `part` that goes by `key` there is to see in the
    /// picture being drawn, and how large it is: as its theme's
    /// `@starting-style` says when it is first in a picture, and as it is
    /// once there after the time the theme gives the part's colours to
    /// change.
    ///
    /// It is asked of whatever comes and goes, in every picture it is in.
    /// Something left out of a picture comes in afresh the next time.
    pub fn presence(&mut self, theme: &Theme, part: Part, key: u64) -> Seen {
        let now = self.now;
        let (since, seen) = self.present.entry((part, key)).or_insert((Some(now), true));
        *seen = true;
        let settled = theme.style(part, States::NONE);
        let coming = settled.transition;
        let share = since.map_or(1.0, |since| coming.progress(now.saturating_sub(since)));
        self.moving |= share < 1.0;
        let settled = settled.presence;
        Seen::between(theme.starting(part), settled, settled, share)
    }

    /// How much there is to see of the `part` that goes by `key`, from 0 to
    /// 1, which is shown only while it is `wanted`, as a button is while
    /// the pointer is near: it comes over the time the theme gives the
    /// part to come in, and goes over its time `:closing`, setting out from
    /// as much of it as there was when that changed. Asked of it in every
    /// picture, wanted or not, for as long as it could be there; one not
    /// yet asked after is not there.
    pub fn fade(&mut self, theme: &Theme, part: Part, key: u64, wanted: bool) -> f32 {
        let now = self.now;
        let fading = self.fades.entry((part, key)).or_insert(Fading {
            wanted: false,
            since: now,
            from: 0.0,
            seen: true,
        });
        fading.seen = true;
        let states = |wanted| {
            if wanted {
                States::NONE
            } else {
                States::NONE.with(State::Closing)
            }
        };
        let at = |fading: &Fading| {
            let transition = theme.style(part, states(fading.wanted)).transition;
            let share = transition.progress(now.saturating_sub(fading.since));
            let to = if fading.wanted { 1.0 } else { 0.0 };
            // Where there was nothing to go, nothing is on its way.
            let share = if (fading.from - to).abs() < f32::EPSILON {
                1.0
            } else {
                share
            };
            (fading.from + (to - fading.from) * share, share)
        };
        if fading.wanted != wanted {
            fading.from = at(fading).0;
            fading.wanted = wanted;
            fading.since = now;
        }
        let (seen, share) = at(fading);
        self.moving |= share < 1.0;
        seen
    }

    /// Takes the `part` that goes by `key` to have been in the picture all
    /// along, if it is not in it yet: for something that slides into view
    /// with what is around it, rather than comes into being.
    pub fn arrived(&mut self, part: Part, key: u64) {
        self.present.entry((part, key)).or_insert((None, false));
    }

    /// How the `part` that goes by `key` looks in the picture being drawn,
    /// now that it is in `states`.
    ///
    /// A part drawn for the first time has the look of its state at once.
    /// One whose state has changed since it was last drawn sets out from
    /// how it looked then.
    pub fn style(&mut self, theme: &Theme, part: Part, key: u64, states: States) -> Style {
        self.style_in(theme, part, key, (states, States::NONE))
    }

    /// How the `part` that goes by `key` looks in the picture being drawn,
    /// now that it is in `states` and the part it is in in `holder`, as
    /// [`Theme::style_in`] says: as [`Motion::style`], setting out from how
    /// it looked when either changes.
    pub fn style_in(
        &mut self,
        theme: &Theme,
        part: Part,
        key: u64,
        (states, holder): (States, States),
    ) -> Style {
        let now = self.now;
        let states = (states, holder);
        let to = theme.style_in(part, states.0, states.1);
        let changing = self.parts.entry((part, key)).or_insert(Changing {
            states,
            from: None,
            seen: true,
        });
        changing.seen = true;
        if changing.states != states {
            let was = theme.style_in(part, changing.states.0, changing.states.1);
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

    /// How much there is still to see of a `part` that went from the
    /// picture at `since`, and how large it is: all of it at that moment,
    /// on its way to how it is when it is `:closing`, and nothing once the
    /// time the theme gives the part to leave in, its transition then, has
    /// gone by.
    ///
    /// Whoever asks keeps drawing what has gone, as it says, and stops
    /// asking once it is no more.
    pub fn leaving(&mut self, theme: &Theme, part: Part, since: Duration) -> Option<Seen> {
        let closing = theme.style(part, States::NONE.with(State::Closing));
        let share = closing.transition.progress(self.now.saturating_sub(since));
        let left = share < 1.0;
        self.moving |= left;
        let settled = theme.style(part, States::NONE).presence;
        left.then(|| Seen::between(settled, closing.presence, settled, share))
    }

    /// Ends the picture, forgetting the parts that were not in it. Returns
    /// whether anything in it is still on its way, so that it has to be
    /// drawn again shortly: see [`Motion::deadline`].
    pub fn end(&mut self) -> bool {
        self.parts.retain(|_, part| part.seen);
        self.present.retain(|_, (_, seen)| *seen);
        self.fades.retain(|_, fading| fading.seen);
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
                          task:closing { opacity: 0; scale: 50%; }
                          clock { background: #f00; transition: none; }
                          clock:hover { background: #00f; }
                          clock:closing { opacity: 0; }
                          @starting-style { task, clock { opacity: 0; scale: 80% } }";
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

    /// How much of a part shown while `wanted` there is in a picture drawn
    /// at `now`, and whether the picture has to be drawn again.
    fn faded(motion: &mut Motion, wanted: bool, now: u64) -> (f32, bool) {
        motion.begin(millis(now));
        let seen = motion.fade(&theme(), Part::Task, 3, wanted);
        (seen, motion.end())
    }

    #[test]
    fn what_is_shown_only_while_wanted_comes_over_its_time_and_goes_over_its_time_closing() {
        let theme = "task { transition: 100ms linear; } task:closing { transition: 50ms linear; }";
        let theme = Theme::parse(theme).unwrap();
        let mut motion = Motion::new();
        let mut faded = |wanted, at| {
            motion.begin(millis(at));
            let seen = motion.fade(&theme, Part::Task, 3, wanted);
            (seen, motion.end())
        };
        let near = |(seen, moving): (f32, bool), wanted: f32| {
            ((seen - wanted).abs() < 0.01).then_some(moving)
        };
        assert_eq!(
            near(faded(false, 0), 0.0),
            Some(false),
            "not there until wanted"
        );
        assert_eq!(near(faded(true, 1000), 0.0), Some(true));
        assert_eq!(near(faded(true, 1050), 0.5), Some(true));
        assert_eq!(near(faded(true, 1100), 1.0), Some(false));
        assert_eq!(near(faded(false, 2000), 1.0), Some(true));
        assert_eq!(near(faded(false, 2025), 0.5), Some(true), "quicker going");
        assert_eq!(near(faded(false, 2050), 0.0), Some(false));
    }

    #[test]
    fn what_turns_back_while_fading_sets_out_from_as_much_as_there_was() {
        let mut motion = Motion::new();
        faded(&mut motion, true, 0);
        let (half, _) = faded(&mut motion, true, 50);
        let (turned, moving) = faded(&mut motion, false, 50);
        assert!((turned - half).abs() < 0.01, "no jump: {half} {turned}");
        assert!(moving);
        assert_eq!(faded(&mut motion, false, 150), (0.0, false));
    }

    #[test]
    fn what_was_left_out_of_a_picture_comes_in_afresh_when_next_wanted() {
        let mut motion = Motion::new();
        faded(&mut motion, true, 0);
        assert_eq!(faded(&mut motion, true, 200), (1.0, false));
        motion.begin(millis(300));
        motion.end();
        let (again, moving) = faded(&mut motion, true, 400);
        assert!(again < 0.01, "{again}");
        assert!(moving);
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
    fn a_button_gets_to_its_look_over_time_when_the_controls_it_is_in_change_state() {
        let theme = Theme::parse(
            "button-close { background: #f00; transition: 100ms linear; }\n\
             windowcontrols:hover button-close { background: #00f; }",
        )
        .unwrap();
        let mut motion = Motion::new();
        let mut drawn = |holder, at| {
            motion.begin(millis(at));
            let style = motion.style_in(&theme, Part::ButtonClose, 1, (States::NONE, holder));
            (style.background, motion.end())
        };
        drawn(States::NONE, 0);
        // The pointer comes onto another of the three: it sets out at once.
        assert_eq!(drawn(HOVERED, 0), (RED, true));
        let (half, moving) = drawn(HOVERED, 50);
        assert!(near(half, PURPLE), "{half:?}");
        assert!(moving);
        assert_eq!(drawn(HOVERED, 100), (BLUE, false));
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

    /// A share in hundredths.
    fn hundredths(share: f32) -> i32 {
        #[expect(clippy::cast_possible_truncation, reason = "it is from 0 to 200")]
        let hundredths = (share * 100.0).round() as i32;
        hundredths
    }

    /// How much of one part there is to see in a picture drawn at `now`,
    /// in hundredths, and whether the picture has to be drawn again.
    fn present(motion: &mut Motion, part: Part, now: u64) -> (i32, bool) {
        motion.begin(millis(now));
        let seen = motion.presence(&theme(), part, 7);
        (hundredths(seen.opacity), motion.end())
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
    fn something_that_arrived_with_what_is_around_it_is_all_there_at_once() {
        let mut motion = Motion::new();
        motion.begin(millis(1000));
        motion.arrived(Part::Task, 7);
        let seen = motion.presence(&theme(), Part::Task, 7);
        assert_eq!(seen, Seen::WHOLE);
        assert!(!motion.end(), "nothing on its way");
        // Once it is in the picture, it is there as anything else is.
        assert_eq!(present(&mut motion, Part::Task, 1010), (100, false));
    }

    #[test]
    fn something_whose_theme_gives_it_no_time_is_all_there_at_once() {
        let mut motion = Motion::new();
        assert_eq!(present(&mut motion, Part::Clock, 0), (100, false));
    }

    #[test]
    fn something_comes_from_its_starting_style_to_how_it_is_once_there() {
        let (theme, mut motion) = (theme(), Motion::new());
        let mut seen = |now: u64| {
            motion.begin(millis(now));
            let seen = motion.presence(&theme, Part::Task, 7);
            (hundredths(seen.opacity), hundredths(seen.scale))
        };
        assert_eq!(seen(0), (0, 80));
        assert_eq!(seen(50), (50, 90));
        assert_eq!(seen(100), (100, 100));
        // Without one, it is there as it is from the first.
        let plain = Theme::parse("task { transition: 100ms linear; }").unwrap();
        let mut motion = Motion::new();
        motion.begin(millis(0));
        assert_eq!(motion.presence(&plain, Part::Task, 7), Seen::WHOLE);
        // One that is faint once there comes from nothing to that, and is
        // drawn as a share of it.
        let faint = "task { opacity: 0.5; transition: 100ms linear; }\n\
                     @starting-style { task { opacity: 0 } }";
        let faint = Theme::parse(faint).unwrap();
        motion.begin(millis(50));
        let seen = motion.presence(&faint, Part::Task, 8);
        motion.begin(millis(100));
        let half = motion.presence(&faint, Part::Task, 8).opacity;
        assert_eq!((hundredths(seen.opacity), hundredths(half)), (0, 50));
    }

    #[test]
    fn something_that_has_gone_goes_to_how_it_is_closing_and_is_then_no_more() {
        let (theme, mut motion) = (theme(), Motion::new());
        let mut left = |now: u64| {
            motion.begin(millis(now));
            let left = motion.leaving(&theme, Part::Task, millis(1000));
            let left = left.map(|seen| (hundredths(seen.opacity), hundredths(seen.scale)));
            (left, motion.end())
        };
        assert_eq!(left(1000), (Some((100, 100)), true));
        assert_eq!(left(1050), (Some((50, 75)), true));
        assert_eq!(left(1100), (None, false));
        // However much of it is seen as it is closing.
        let kept = Theme::parse("task { transition: 100ms linear; }").unwrap();
        let mut motion = Motion::new();
        motion.begin(millis(1050));
        let seen = motion.leaving(&kept, Part::Task, millis(1000));
        assert_eq!(seen, Some(Seen::WHOLE));
        motion.begin(millis(1100));
        assert_eq!(motion.leaving(&kept, Part::Task, millis(1000)), None);
    }

    #[test]
    fn what_went_while_it_was_still_coming_goes_from_where_it_had_got_to() {
        let half = Seen {
            opacity: 0.5,
            scale: 0.9,
        };
        let going = Seen {
            opacity: 0.5,
            scale: 0.5,
        };
        let both = half.and(going);
        assert_eq!((hundredths(both.opacity), hundredths(both.scale)), (25, 45));
    }

    #[test]
    fn something_leaves_over_the_time_its_theme_gives_it_closing() {
        let theme = Theme::parse(
            "launcher { transition: 200ms linear; }\n\
             launcher:closing { transition: 50ms linear; opacity: 0; }\n\
             @starting-style { launcher { opacity: 0 } }",
        )
        .unwrap();
        let mut motion = Motion::new();
        motion.begin(millis(0));
        let none = motion.presence(&theme, Part::Launcher, 1).opacity;
        assert_eq!(hundredths(none), 0);
        motion.begin(millis(100));
        let half = motion.presence(&theme, Part::Launcher, 1).opacity;
        assert_eq!(hundredths(half), 50, "comes over 200ms");
        let mut left = |now: u64| {
            motion.begin(millis(now));
            let left = motion.leaving(&theme, Part::Launcher, millis(1000));
            left.map(|seen| hundredths(seen.opacity))
        };
        assert_eq!(left(1025), Some(50), "and goes over 50ms");
        assert_eq!(left(1050), None);
    }

    #[test]
    fn something_whose_theme_gives_it_no_time_has_gone_at_once() {
        let mut motion = Motion::new();
        motion.begin(millis(1000));
        assert_eq!(motion.leaving(&theme(), Part::Clock, millis(1000)), None);
        assert!(!motion.end());
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
