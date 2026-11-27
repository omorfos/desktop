//! The time of day, as the panel and the lock screen show it.

use std::time::Duration;

use jiff::ZonedDateTime;
use jiff::civil::Date;

/// The time now where the user is, as it reads on the bar, how long until
/// it reads differently, and what day it is: all of one moment, so that the
/// time and the day agree at midnight too.
#[must_use]
pub fn now() -> (String, Duration, Date) {
    of(&ZonedDateTime::now())
}

/// How a moment reads on the bar, how long until it reads differently, and
/// the day it is in: what [`now`] says of the moment it reads.
fn of(moment: &ZonedDateTime) -> (String, Duration, Date) {
    let (time, changes) = reading(
        moment.hour(),
        moment.minute(),
        moment.second(),
        moment.millisecond(),
    );
    (time, changes, moment.date())
}

/// How the moment `seconds` after the Unix epoch reads on the bar, where
/// the user is: none for one no calendar has.
#[must_use]
pub fn at(seconds: u64) -> Option<String> {
    let seconds = i64::try_from(seconds).ok()?;
    let moment = jiff::Timestamp::from_second(seconds).ok()?;
    let zoned = moment.to_zoned_datetime(jiff::tz::TimeZone::system());
    Some(reading(zoned.hour(), zoned.minute(), 0, 0).0)
}

/// How a time of day reads on the bar, and how long until the minute after
/// it begins.
fn reading(hour: i8, minute: i8, second: i8, millisecond: i16) -> (String, Duration) {
    let into = i32::from(second) * 1000 + i32::from(millisecond);
    // A leap second, or a clock that says something odd, is no reason to
    // wait for less than nothing.
    let left = u64::from((60_000 - into).clamp(1, 60_000).unsigned_abs());
    (
        format!("{hour:02}:{minute:02}"),
        Duration::from_millis(left),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_time_reads_as_hours_and_minutes_of_the_day() {
        assert_eq!(reading(9, 5, 0, 0).0, "09:05");
        assert_eq!(reading(23, 59, 59, 999).0, "23:59");
        assert_eq!(reading(0, 0, 30, 0).0, "00:00");
    }

    #[test]
    fn it_reads_differently_when_the_next_minute_begins() {
        assert_eq!(reading(9, 5, 0, 0).1, Duration::from_secs(60));
        assert_eq!(reading(9, 5, 45, 250).1, Duration::from_millis(14_750));
        assert_eq!(reading(9, 5, 59, 999).1, Duration::from_millis(1));
        // A leap second is still a wait, however short.
        assert_eq!(reading(23, 59, 60, 500).1, Duration::from_millis(1));
    }

    #[test]
    fn a_moment_reads_as_a_time_of_day_if_there_is_one() {
        let read = at(1_761_400_800).unwrap();
        assert_eq!((read.len(), &read[2..3]), (5, ":"), "{read}");
        assert_eq!(at(u64::MAX), None, "past where time is counted");
        assert_eq!(at(1 << 40), None, "past where calendars go");
    }

    #[test]
    fn the_time_and_the_day_are_of_one_moment_across_midnight() {
        let utc = jiff::tz::TimeZone::UTC;
        let before = jiff::civil::date(2026, 11, 23)
            .at(23, 59, 59, 999_000_000)
            .to_zoned_datetime(utc)
            .unwrap();
        assert_eq!(
            of(&before),
            (
                "23:59".to_owned(),
                Duration::from_millis(1),
                jiff::civil::date(2026, 11, 23)
            )
        );
        let after = before
            .checked_add(jiff::Span::new().milliseconds(1))
            .unwrap();
        assert_eq!(
            of(&after),
            (
                "00:00".to_owned(),
                Duration::from_secs(60),
                jiff::civil::date(2026, 11, 24)
            )
        );
    }

    #[test]
    fn the_time_now_is_a_time_of_day() {
        let (time, wait, _) = now();
        let (hour, minute) = time.split_once(':').unwrap();
        assert!(hour.parse::<u8>().unwrap() < 24, "{time}");
        assert!(minute.parse::<u8>().unwrap() < 60, "{time}");
        assert_eq!(time.len(), 5, "{time}");
        assert!(wait <= Duration::from_secs(60));
    }
}
