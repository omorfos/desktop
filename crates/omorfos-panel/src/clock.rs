//! The time of day, as the panel shows it.

use std::time::Duration;

use jiff::Zoned;

/// The time now where the user is, as it reads on the bar, and how long
/// until it reads differently.
pub(crate) fn now() -> (String, Duration) {
    let now = Zoned::now();
    reading(now.hour(), now.minute(), now.second(), now.millisecond())
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
    fn the_time_now_is_a_time_of_day() {
        let (time, wait) = now();
        let (hour, minute) = time.split_once(':').unwrap();
        assert!(hour.parse::<u8>().unwrap() < 24, "{time}");
        assert!(minute.parse::<u8>().unwrap() < 60, "{time}");
        assert_eq!(time.len(), 5, "{time}");
        assert!(wait <= Duration::from_secs(60));
    }
}
