//! Night light: the screens made warmer after dark, by the hours of the
//! day or where the user is, and switched by hand until it next turns,
//! each change fading in over a moment.

use std::time::{Duration, Instant, SystemTime};

use jiff::civil::date;
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Zoned};
use omorfos_config::Action;

use crate::common::Session;

/// Night light on, at 4000 K by night, with the day from seven to seven.
const ON: &str = "night_light: { enabled: true, temperature: 4000 }\n\
                  appearance: { day: { from: '07:00', to: '19:00' } }\n";

/// A moment, written in UTC, on a day in `year`.
fn utc(year: i16, hour: i8, minute: i8) -> Zoned {
    let moment = date(year, 1, 1).at(hour, minute, 0, 0);
    moment.to_zoned(TimeZone::UTC).unwrap_or_default()
}

/// A session with `config`, its clock started, as one is from its first
/// turn.
fn started(config: &str) -> Session {
    let mut session = Session::with_config(&[("config.yaml", config)]);
    session.step();
    session
}

/// How warm the screens are once a change has had time to fade in.
fn settled(session: &mut Session) -> u16 {
    session.advance(Duration::from_secs(3));
    session.compositor().night_kelvin()
}

#[test]
fn night_light_warms_the_screens_after_dark_and_leaves_them_by_day() {
    let mut session = started(ON);
    session.compositor_mut().light_the_night(&utc(2099, 23, 0));
    assert_eq!(settled(&mut session), 4000);
    session.compositor_mut().light_the_night(&utc(2099, 19, 10));
    assert_eq!(settled(&mut session), 5191, "on its way");
    session.compositor_mut().light_the_night(&utc(2099, 12, 0));
    assert_eq!(settled(&mut session), 6500);
}

#[test]
fn switched_off_by_hand_it_stays_off_until_the_day_and_comes_back_the_next_night() {
    let mut session = started(ON);
    let compositor = session.compositor_mut();
    compositor.light_the_night(&utc(2099, 23, 0));
    compositor.perform(&Action::NightLight);
    assert_eq!(settled(&mut session), 6500);
    let compositor = session.compositor_mut();
    compositor.light_the_night(&utc(2099, 3, 0));
    assert_eq!(settled(&mut session), 6500, "still off that night");
    let compositor = session.compositor_mut();
    compositor.light_the_night(&utc(2099, 12, 0));
    compositor.light_the_night(&utc(2099, 23, 0));
    assert_eq!(settled(&mut session), 4000, "on again the next");
}

#[test]
fn switched_on_by_hand_where_it_is_off_it_is_as_warm_as_it_says_until_switched_again() {
    let config = "night_light: { temperature: 3000 }\n";
    let mut session = started(config);
    assert_eq!(session.compositor().night_kelvin(), 6500, "off unless set");
    session.compositor_mut().perform(&Action::NightLight);
    assert_eq!(settled(&mut session), 3000);
    session.compositor_mut().light_the_night(&utc(2099, 12, 0));
    assert_eq!(settled(&mut session), 3000, "kept, as nothing turns");
    let compositor = session.compositor_mut();
    compositor.perform(&Action::NightLight);
    compositor.perform(&Action::NightLight);
    compositor.perform(&Action::NightLight);
    assert_eq!(settled(&mut session), 6500);
}

#[test]
fn the_schedule_is_asked_again_as_it_falls_due_and_not_before() {
    let mut session = started(ON);
    let compositor = session.compositor_mut();
    let (now, wall) = (Instant::now(), SystemTime::now());
    compositor.light_the_night(&utc(2099, 23, 0));
    assert!(compositor.night_due(now, wall) > Some(now), "not yet due");
    compositor.light_the_night_if_due(wall);
    assert_eq!(settled(&mut session), 4000, "not asked again");
    let compositor = session.compositor_mut();
    // Asked long ago, it is due, and asked again now.
    compositor.light_the_night(&utc(2020, 23, 0));
    assert_eq!(compositor.night_due(now, wall), Some(now));
    compositor.light_the_night_if_due(wall);
    assert!(compositor.night_due(now, wall) > Some(now));
}

#[test]
fn a_change_fades_in_over_a_moment_and_is_drawn_again_until_it_has() {
    let config = "night_light: { temperature: 3000 }\n";
    let mut session = started(config);
    session.compositor_mut().perform(&Action::NightLight);
    assert_eq!(session.compositor().night_kelvin(), 6500, "not at once");
    let now = session.now;
    let due = session.compositor().deadline();
    assert!(
        due.is_some_and(|due| due > now && due <= now + Duration::from_millis(20)),
        "{due:?}"
    );
    session.advance(Duration::from_secs(1));
    let halfway = session.compositor().night_kelvin();
    assert!(3000 < halfway && halfway < 6500, "{halfway}");
    // Switched back halfway, it goes back from where it had got to.
    session.compositor_mut().perform(&Action::NightLight);
    assert_eq!(session.compositor().night_kelvin(), halfway);
    session.advance(Duration::from_millis(500));
    let back = session.compositor().night_kelvin();
    assert!(halfway < back && back < 6500, "{back}");
    session.advance(Duration::from_secs(2));
    assert_eq!(session.compositor().night_kelvin(), 6500);
    assert_eq!(session.compositor().deadline(), None, "and no more to do");
}

#[test]
fn the_first_temperature_a_compositor_is_asked_for_is_there_at_once() {
    // As one started again by night has the screens warm already. The day
    // is put hours from now, so that it is night whenever this is run.
    let hour = |hours: i64| (Zoned::now() + SignedDuration::from_hours(hours)).hour();
    let config = format!(
        "night_light: {{ enabled: true }}\n\
         appearance: {{ day: {{ from: '{:02}:00', to: '{:02}:00' }} }}\n",
        hour(6),
        hour(7)
    );
    let session = Session::with_config(&[("config.yaml", &config)]);
    let kelvin = session.compositor().night_kelvin();
    assert!(kelvin < 6500, "{kelvin}");
}
