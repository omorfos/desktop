//! Night light: the screens made warmer after dark, by the hours of the
//! day or where the user is, and switched by hand until it next turns.

use std::time::{Instant, SystemTime};

use jiff::Zoned;
use jiff::civil::date;
use jiff::tz::TimeZone;
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

#[test]
fn night_light_warms_the_screens_after_dark_and_leaves_them_by_day() {
    let mut session = Session::with_config(&[("config.yaml", ON)]);
    session.compositor_mut().light_the_night(&utc(2099, 23, 0));
    assert_eq!(session.compositor().night_kelvin(), 4000);
    session.compositor_mut().light_the_night(&utc(2099, 19, 10));
    assert_eq!(session.compositor().night_kelvin(), 5191, "on its way");
    session.compositor_mut().light_the_night(&utc(2099, 12, 0));
    assert_eq!(session.compositor().night_kelvin(), 6500);
}

#[test]
fn switched_off_by_hand_it_stays_off_until_the_day_and_comes_back_the_next_night() {
    let mut session = Session::with_config(&[("config.yaml", ON)]);
    let compositor = session.compositor_mut();
    compositor.light_the_night(&utc(2099, 23, 0));
    compositor.perform(&Action::NightLight);
    assert_eq!(compositor.night_kelvin(), 6500);
    compositor.light_the_night(&utc(2099, 3, 0));
    assert_eq!(compositor.night_kelvin(), 6500, "still off that night");
    compositor.light_the_night(&utc(2099, 12, 0));
    compositor.light_the_night(&utc(2099, 23, 0));
    assert_eq!(compositor.night_kelvin(), 4000, "on again the next");
}

#[test]
fn switched_on_by_hand_where_it_is_off_it_is_as_warm_as_it_says_until_switched_again() {
    let config = "night_light: { temperature: 3000 }\n";
    let mut session = Session::with_config(&[("config.yaml", config)]);
    let compositor = session.compositor_mut();
    assert_eq!(compositor.night_kelvin(), 6500, "off unless set");
    compositor.perform(&Action::NightLight);
    assert_eq!(compositor.night_kelvin(), 3000);
    compositor.light_the_night(&utc(2099, 12, 0));
    assert_eq!(compositor.night_kelvin(), 3000, "kept, as nothing turns");
    compositor.perform(&Action::NightLight);
    compositor.perform(&Action::NightLight);
    compositor.perform(&Action::NightLight);
    assert_eq!(compositor.night_kelvin(), 6500);
}

#[test]
fn the_schedule_is_asked_again_as_it_falls_due_and_not_before() {
    let mut session = Session::with_config(&[("config.yaml", ON)]);
    let compositor = session.compositor_mut();
    let (now, wall) = (Instant::now(), SystemTime::now());
    compositor.light_the_night(&utc(2099, 23, 0));
    assert!(compositor.night_due(now, wall) > Some(now), "not yet due");
    compositor.light_the_night_if_due(wall);
    assert_eq!(compositor.night_kelvin(), 4000, "not asked again");
    // Asked long ago, it is due, and asked again now.
    compositor.light_the_night(&utc(2020, 23, 0));
    assert_eq!(compositor.night_due(now, wall), Some(now));
    compositor.light_the_night_if_due(wall);
    assert!(compositor.night_due(now, wall) > Some(now));
}
