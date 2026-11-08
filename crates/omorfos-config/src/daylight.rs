//! When the desktop is light and when it is dark, where `appearance.scheme`
//! is `auto`: from sunrise to sunset where the user says they are, and
//! otherwise between two hours of the day.
//!
//! Sunrise and sunset are worked out as NOAA's "General Solar Position
//! Calculations" do, which is within a minute or two of what an almanac
//! says, at the latitudes people live at.

use std::f64::consts::TAU;

use jiff::{Timestamp, Zoned};
use serde::Deserialize;

/// Where on Earth the user is, for when the sun is up there. Kept in
/// millionths of a degree, so that the configuration can be compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(try_from = "Degrees")]
pub struct Location {
    latitude: i32,
    longitude: i32,
}

/// A place as it is written: degrees north and east, south and west being
/// below 0.
#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct Degrees {
    /// From -90 at the South Pole to 90 at the North.
    latitude: f64,
    /// From -180 to 180, east of Greenwich being above 0.
    longitude: f64,
}

impl TryFrom<Degrees> for Location {
    type Error = String;

    fn try_from(degrees: Degrees) -> Result<Self, Self::Error> {
        let millionths = |degrees: f64, limit: f64| {
            #[expect(clippy::cast_possible_truncation, reason = "within 180 degrees")]
            let millionths = (degrees * 1e6).round() as i32;
            (-limit..=limit).contains(&degrees).then_some(millionths)
        };
        let latitude = millionths(degrees.latitude, 90.0);
        let longitude = millionths(degrees.longitude, 180.0);
        latitude
            .zip(longitude)
            .map(|(latitude, longitude)| Self {
                latitude,
                longitude,
            })
            .ok_or_else(|| {
                "a latitude is from -90 to 90 and a longitude from -180 to 180".to_owned()
            })
    }
}

/// The hours of the day the desktop is light where no location is given,
/// from `from` until `to`, in minutes after midnight, local time. One that
/// ends earlier than it begins runs through midnight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(try_from = "Hours")]
pub struct Day {
    from: u16,
    to: u16,
}

impl Default for Day {
    /// From seven in the morning to seven in the evening.
    fn default() -> Self {
        Self {
            from: 7 * 60,
            to: 19 * 60,
        }
    }
}

/// The hours of the day as they are written, as `"07:00"`.
#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct Hours {
    /// When the desktop turns light, as `"07:00"`.
    from: String,
    /// When it turns dark, as `"19:00"`.
    to: String,
}

impl TryFrom<Hours> for Day {
    type Error = String;

    fn try_from(hours: Hours) -> Result<Self, Self::Error> {
        // Two digits, as the hour and the minute are written.
        let number = |digits: &str| {
            let plain = digits.len() == 2 && digits.bytes().all(|byte| byte.is_ascii_digit());
            let digits = digits
                .bytes()
                .map(|byte| u16::from(byte.saturating_sub(b'0')));
            plain.then(|| digits.fold(0, |sum, digit| sum * 10 + digit))
        };
        let minutes = |text: &str| {
            let (hour, minute) = text.split_once(':')?;
            let (hour, minute) = (number(hour)?, number(minute)?);
            (hour < 24 && minute < 60).then_some(hour * 60 + minute)
        };
        let day = minutes(&hours.from).zip(minutes(&hours.to));
        let day = day.filter(|(from, to)| from != to);
        day.map(|(from, to)| Self { from, to }).ok_or_else(|| {
            format!(
                "`{}` to `{}` is not a day: each is a time as `07:00`, and they differ",
                hours.from, hours.to
            )
        })
    }
}

/// Whether it is light at `now`, and when that next changes, or the
/// latest it is to be asked again: by the next midnight, so that a day
/// the sun never sets is followed by one it does.
pub(crate) fn light_at(now: &Zoned, location: Option<Location>, day: Day) -> (bool, Timestamp) {
    let midnight = now.date().tomorrow().ok();
    let midnight = midnight.and_then(|date| date.to_zoned(now.time_zone().clone()).ok());
    let midnight = midnight.map_or(now.timestamp(), |midnight| midnight.timestamp());
    let (light, next) = match location {
        Some(location) => by_the_sun(&now.timestamp(), location),
        None => by_the_clock(now, day),
    };
    (light, next.map_or(midnight, |next| next.min(midnight)))
}

/// Seconds in a day.
const DAY: i64 = 24 * 60 * 60;

/// When the sun is up on the day of the year `of`, at `location`: from
/// rise to set, in minutes after midnight UTC, or all day or not at all.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Sun {
    Between(f64, f64),
    AllDay,
    Never,
}

fn sun(of: i16, location: Location) -> Sun {
    let angle = TAU / 365.0 * f64::from(of - 1);
    let (once, twice, thrice) = (angle, 2.0 * angle, 3.0 * angle);
    // How far the sun is ahead of noon by the clock, in minutes, and how
    // far north of the equator it is overhead.
    let early = 229.18
        * (0.000_075 + 0.001_868 * once.cos()
            - 0.032_077 * once.sin()
            - 0.014_615 * twice.cos()
            - 0.040_849 * twice.sin());
    let north = 0.006_918 - 0.399_912 * once.cos() + 0.070_257 * once.sin()
        - 0.006_758 * twice.cos()
        + 0.000_907 * twice.sin()
        - 0.002_697 * thrice.cos()
        + 0.001_48 * thrice.sin();
    let latitude = (f64::from(location.latitude) / 1e6).to_radians();
    let longitude = f64::from(location.longitude) / 1e6;
    // Up once its top edge clears the horizon, the air bending its light.
    let horizon = 90.833_f64.to_radians().cos();
    let hour = horizon / (latitude.cos() * north.cos()) - latitude.tan() * north.tan();
    if hour < -1.0 {
        return Sun::AllDay;
    }
    if hour > 1.0 {
        return Sun::Never;
    }
    let hour = hour.acos().to_degrees();
    let noon = 720.0 - 4.0 * longitude - early;
    Sun::Between(noon - 4.0 * hour, noon + 4.0 * hour)
}

/// Whether the sun is up at `now`, and when it next rises or sets, if it
/// does in the day either side.
fn by_the_sun(now: &Timestamp, location: Location) -> (bool, Option<Timestamp>) {
    let today = now.as_second().div_euclid(DAY) * DAY;
    let days = [today - DAY, today, today + DAY].map(|midnight| {
        let of = Timestamp::from_second(midnight).ok();
        let of = of.map_or(1, |of| of.to_zoned(jiff::tz::TimeZone::UTC).day_of_year());
        let at = |minutes: f64| {
            #[expect(clippy::cast_possible_truncation, reason = "within a day either side")]
            let seconds = (minutes * 60.0).round() as i64;
            Timestamp::from_second(midnight + seconds).ok()
        };
        match sun(of, location) {
            Sun::Between(rise, set) => (at(rise), at(set), false),
            // The whole day, and an hour either side, so that one such
            // day runs into the next.
            Sun::AllDay => (at(-60.0), at(25.0 * 60.0), true),
            Sun::Never => (None, None, false),
        }
    });
    let light = days.iter().any(|(rise, set, _)| {
        rise.zip(*set)
            .is_some_and(|(rise, set)| rise <= *now && *now < set)
    });
    let events = days.iter().filter(|(_, _, all)| !all);
    let events = events.flat_map(|(rise, set, _)| [*rise, *set]).flatten();
    (light, events.filter(|at| at > now).min())
}

/// Whether it is between the hours of `day` at `now`, and when it next
/// turns one of them.
fn by_the_clock(now: &Zoned, day: Day) -> (bool, Option<Timestamp>) {
    let minutes = |minutes: u16| i32::from(minutes);
    let time = now.time();
    let at = i32::from(time.hour()) * 60 + i32::from(time.minute());
    let (from, to) = (minutes(day.from), minutes(day.to));
    let light = if from < to {
        from <= at && at < to
    } else {
        from <= at || at < to
    };
    let zone = now.time_zone();
    let turns = [now.date(), now.date().tomorrow().unwrap_or(now.date())];
    let turns = turns.into_iter().flat_map(|date| {
        [day.from, day.to].map(|minutes| {
            let (hour, minute) = (minutes / 60, minutes % 60);
            #[expect(clippy::cast_possible_truncation, reason = "an hour and a minute")]
            let time = jiff::civil::time(hour as i8, minute as i8, 0, 0);
            date.to_datetime(time).to_zoned(zone.clone()).ok()
        })
    });
    let next = turns.flatten().map(|turn| turn.timestamp());
    (light, next.filter(|turn| *turn > now.timestamp()).min())
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;
    use jiff::tz::TimeZone;

    use super::*;

    fn place(latitude: f64, longitude: f64) -> Location {
        Location::try_from(Degrees {
            latitude,
            longitude,
        })
        .unwrap()
    }

    /// A moment, written in UTC.
    fn utc(year: i16, month: i8, day: i8, hour: i8, minute: i8) -> Zoned {
        let moment = date(year, month, day).at(hour, minute, 0, 0);
        moment.to_zoned(TimeZone::UTC).unwrap()
    }

    /// When the sun is next to rise or set after `now`, as `"HH:MM"` UTC,
    /// and whether it is up.
    fn sun_at(now: &Zoned, location: Location) -> (bool, String) {
        let (light, next) = light_at(now, Some(location), Day::default());
        let next = next.to_zoned(TimeZone::UTC);
        (light, format!("{:02}:{:02}", next.hour(), next.minute()))
    }

    /// Within `minutes` of `"HH:MM"`, either way.
    fn near(said: &str, wanted: &str, minutes: i32) -> bool {
        let minutes_of = |text: &str| {
            let (hour, minute) = text.split_once(':').unwrap();
            hour.parse::<i32>().unwrap() * 60 + minute.parse::<i32>().unwrap()
        };
        (minutes_of(said) - minutes_of(wanted)).abs() <= minutes
    }

    // The times wanted are what the US Naval Observatory's tables of rise
    // and set give for the place and day, in UTC.

    #[test]
    fn in_warsaw_the_sun_rises_and_sets_as_the_almanac_says_in_summer_and_winter() {
        let warsaw = place(52.23, 21.01);
        let (dark, rise) = sun_at(&utc(2026, 6, 21, 1, 0), warsaw);
        assert!(!dark && near(&rise, "02:14", 2), "{rise}");
        let (light, set) = sun_at(&utc(2026, 6, 21, 12, 0), warsaw);
        assert!(light && near(&set, "19:01", 2), "{set}");
        let (light, set) = sun_at(&utc(2026, 12, 21, 12, 0), warsaw);
        assert!(light && near(&set, "14:25", 2), "{set}");
        let (dark, rise) = sun_at(&utc(2026, 12, 21, 3, 0), warsaw);
        assert!(!dark && near(&rise, "06:43", 2), "{rise}");
    }

    #[test]
    fn far_east_of_greenwich_the_morning_is_the_evening_before_by_utc() {
        let singapore = place(1.29, 103.85);
        let (light, set) = sun_at(&utc(2026, 3, 20, 6, 0), singapore);
        assert!(light && near(&set, "11:15", 2), "{set}");
        let (dark, rise) = sun_at(&utc(2026, 3, 20, 20, 0), singapore);
        assert!(!dark && near(&rise, "23:09", 2), "{rise}");
        let sydney = place(-33.87, 151.21);
        let (dark, rise) = sun_at(&utc(2026, 11, 8, 12, 0), sydney);
        assert!(!dark && near(&rise, "18:48", 2), "{rise}");
        let (light, set) = sun_at(&utc(2026, 11, 8, 0, 0), sydney);
        assert!(light && near(&set, "08:29", 2), "{set}");
    }

    #[test]
    fn where_the_sun_never_sets_or_never_rises_it_is_asked_again_by_midnight() {
        let longyearbyen = place(78.22, 15.65);
        for hour in [0, 6, 12, 18, 23] {
            let summer = sun_at(&utc(2026, 6, 21, hour, 30), longyearbyen);
            assert_eq!(summer, (true, "00:00".to_owned()), "at {hour}");
            let winter = sun_at(&utc(2026, 12, 21, hour, 30), longyearbyen);
            assert_eq!(winter, (false, "00:00".to_owned()), "at {hour}");
        }
        // And by the next midnight where the user is, not where UTC is.
        let oslo = TimeZone::fixed(jiff::tz::offset(2));
        let evening = utc(2026, 6, 21, 20, 0).with_time_zone(oslo);
        let (_, next) = light_at(&evening, Some(longyearbyen), Day::default());
        assert_eq!(next, utc(2026, 6, 21, 22, 0).timestamp());
    }

    #[test]
    fn without_a_place_it_is_light_between_the_hours_of_the_day() {
        let zone = TimeZone::fixed(jiff::tz::offset(1));
        let at = |hour: i8, minute: i8| {
            date(2026, 3, 10)
                .at(hour, minute, 0, 0)
                .to_zoned(zone.clone())
                .unwrap()
        };
        let local = |next: Timestamp| {
            let next = next.to_zoned(zone.clone());
            (next.day(), next.hour(), next.minute())
        };
        let day = Day::default();
        let (light, next) = light_at(&at(6, 59), None, day);
        assert_eq!((light, local(next)), (false, (10, 7, 0)));
        let (light, next) = light_at(&at(7, 0), None, day);
        assert_eq!((light, local(next)), (true, (10, 19, 0)));
        let (light, next) = light_at(&at(19, 0), None, day);
        assert_eq!((light, local(next)), (false, (11, 0, 0)), "by midnight");
        // A day through midnight.
        let night = Day {
            from: 22 * 60,
            to: 6 * 60,
        };
        let (light, next) = light_at(&at(23, 0), None, night);
        assert_eq!((light, local(next)), (true, (11, 0, 0)));
        let (light, next) = light_at(&at(3, 0), None, night);
        assert_eq!((light, local(next)), (true, (10, 6, 0)));
        let (light, next) = light_at(&at(12, 0), None, night);
        assert_eq!((light, local(next)), (false, (10, 22, 0)));
    }

    #[test]
    fn a_place_or_hours_that_cannot_be_are_refused() {
        let place = |latitude, longitude| {
            Location::try_from(Degrees {
                latitude,
                longitude,
            })
        };
        assert_eq!(
            place(-90.0, 180.0),
            Ok(Location {
                latitude: -90_000_000,
                longitude: 180_000_000
            })
        );
        for (latitude, longitude) in [(90.5, 0.0), (0.0, -180.5), (f64::NAN, 0.0)] {
            let message = place(latitude, longitude).unwrap_err();
            assert!(message.contains("from -90 to 90"), "{message}");
        }
        let day = |from: &str, to: &str| {
            Day::try_from(Hours {
                from: from.to_owned(),
                to: to.to_owned(),
            })
        };
        assert_eq!(
            day("06:30", "21:15"),
            Ok(Day {
                from: 390,
                to: 1275
            })
        );
        assert_eq!(day("23:59", "00:00"), Ok(Day { from: 1439, to: 0 }));
        for (from, to) in [
            ("07:00", "07:00"),
            ("24:00", "19:00"),
            ("07:60", "19:00"),
            ("7:00", "19:00"),
            ("07-00", "19:00"),
            ("07:0a", "19:00"),
            ("+7:00", "19:00"),
        ] {
            let message = day(from, to).unwrap_err();
            assert!(message.contains("is not a day"), "{from} {to}: {message}");
        }
    }
}
