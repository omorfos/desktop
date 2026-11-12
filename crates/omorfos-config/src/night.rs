//! Night light: the screens made warmer after dark, by the day and night
//! that `scheme: auto` goes by.

use std::time::SystemTime;

use jiff::{SignedDuration, Zoned};
use serde::Deserialize;

use crate::bounded::Bounded;
use crate::daylight;
use crate::general::Appearance;

/// The temperature the screens are at by day, and with night light off: as
/// they are, white as daylight.
pub const DAYLIGHT: u16 = 6500;

/// How long the screens take to turn warm once it is dark, and to turn
/// back before it is light, in minutes.
const EASING: i64 = 20;

/// How often the screens are made warmer, or less warm, while they turn.
const STEP: SignedDuration = SignedDuration::from_secs(30);

/// The screens made warmer after dark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct NightLight {
    /// Whether the screens are made warmer after dark: from sunset to
    /// sunrise where `appearance.location` says, or outside the hours of
    /// `appearance.day`, as `scheme: auto` goes by, turning warm over the
    /// 20 minutes after it is dark and back over the 20 before it is light.
    pub enabled: bool,
    /// How warm the screens are at night, as the colour of a light of this
    /// temperature, in kelvin: 6500 is daylight, as screens are, and lower
    /// is warmer.
    pub temperature: Bounded<1900, 6500>,
}

impl Default for NightLight {
    fn default() -> Self {
        Self {
            enabled: false,
            temperature: Bounded::clamped(4000),
        }
    }
}

impl NightLight {
    /// The temperature the screens are at, at `now`, by the day and night
    /// `appearance` goes by, in kelvin; and, while night light is on, when
    /// that is next to be asked again.
    #[must_use]
    pub fn at(&self, appearance: &Appearance, now: &Zoned) -> (u16, Option<SystemTime>) {
        if !self.enabled {
            return (DAYLIGHT, None);
        }
        let place = (appearance.location, appearance.day);
        let zone = now.time_zone().clone();
        let system = zone.iana_name();
        let light = |minutes: i64| {
            let at = now
                .timestamp()
                .saturating_add(SignedDuration::from_mins(minutes));
            let at = at.unwrap_or(now.timestamp()).to_zoned(zone.clone());
            daylight::light_at(&at, place, system)
        };
        // How much of the last twenty minutes, and of the next, it is dark,
        // a minute at a time: the less of either, the less warm.
        let dark = |minutes: std::ops::RangeInclusive<i64>| {
            let dark = minutes.filter(|minutes| !light(*minutes).0).count();
            u32::try_from(dark).unwrap_or(u32::MAX)
        };
        let samples = u32::try_from(EASING + 1).unwrap_or(u32::MAX);
        let warmth = dark(-EASING..=0).min(dark(0..=EASING));
        let warmest = u32::from(DAYLIGHT) - self.temperature.get().min(u32::from(DAYLIGHT));
        let kelvin = u32::from(DAYLIGHT) - warmest * warmth / samples;
        let kelvin = u16::try_from(kelvin).unwrap_or(DAYLIGHT);
        let (_, turns) = light(0);
        let soon = now
            .timestamp()
            .saturating_add(STEP)
            .unwrap_or(now.timestamp());
        let next = if warmth == 0 {
            // By day, until it is dark.
            turns
        } else if warmth == samples {
            // At night, until the screens are to turn back before it is
            // light, or what is asked again by midnight.
            let turning = turns.saturating_sub(SignedDuration::from_mins(EASING));
            turning.unwrap_or(turns).max(soon)
        } else {
            soon
        };
        (kelvin, Some(SystemTime::from(next)))
    }
}

/// What the screens' red, green and blue are made, each from 0 to 1, to
/// look as a light of `kelvin` does beside daylight: by Tanner Helland's
/// fit to the colours of a black body, made relative to daylight's, so
/// that 6500 K leaves the screens as they are.
#[must_use]
pub fn tint(kelvin: u16) -> [f64; 3] {
    let daylight = black_body(DAYLIGHT);
    let warm = black_body(kelvin.min(DAYLIGHT));
    [0, 1, 2].map(|channel| (warm[channel] / daylight[channel]).clamp(0.0, 1.0))
}

/// The colour of a black body at `kelvin`, as red, green and blue from 0 to
/// 255, by Tanner Helland's fit, for 1000 K to 40000 K.
fn black_body(kelvin: u16) -> [f64; 3] {
    let hundreds = f64::from(kelvin) / 100.0;
    let red = if hundreds <= 66.0 {
        255.0
    } else {
        329.698_727_446 * (hundreds - 60.0).powf(-0.133_204_759_2)
    };
    let green = if hundreds <= 66.0 {
        99.470_802_586_1 * hundreds.ln() - 161.119_568_166_1
    } else {
        288.122_169_528_3 * (hundreds - 60.0).powf(-0.075_514_849_2)
    };
    let blue = if hundreds >= 66.0 {
        255.0
    } else if hundreds <= 19.0 {
        0.0
    } else {
        138.517_731_223_1 * (hundreds - 10.0).ln() - 305.044_792_730_7
    };
    [red, green, blue].map(|value| value.clamp(0.0, 255.0))
}

/// A gamma table of `length` steps for each of red, green and blue, as a
/// screen takes one, that tints what it shows as `tint` says.
#[must_use]
pub fn ramps(tint: [f64; 3], length: usize) -> [Vec<u16>; 3] {
    let last = length.saturating_sub(1).max(1);
    tint.map(|share| {
        (0..length)
            .map(|step| {
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "a table of at most some thousands"
                )]
                let level = step as f64 / last as f64;
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "within 0 to 65535 by the clamp"
                )]
                let value = (level * share * 65535.0).round().clamp(0.0, 65535.0) as u16;
                value
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use jiff::Timestamp;
    use jiff::civil::date;
    use jiff::tz::TimeZone;

    use super::*;

    /// A moment, written in UTC, where the day is from seven to seven.
    fn utc(hour: i8, minute: i8) -> Zoned {
        let moment = date(2026, 11, 12).at(hour, minute, 0, 0);
        moment.to_zoned(TimeZone::UTC).unwrap()
    }

    fn on() -> NightLight {
        NightLight {
            enabled: true,
            temperature: Bounded::clamped(4000),
        }
    }

    /// The temperature at `hour:minute`, and when it is asked again, as
    /// `"HH:MM:SS"` UTC.
    fn at(light: NightLight, hour: i8, minute: i8) -> (u16, Option<String>) {
        let (kelvin, next) = light.at(&Appearance::default(), &utc(hour, minute));
        let next = next.map(|next| {
            let next = Timestamp::try_from(next).unwrap().to_zoned(TimeZone::UTC);
            format!(
                "{:02}:{:02}:{:02}",
                next.hour(),
                next.minute(),
                next.second()
            )
        });
        (kelvin, next)
    }

    #[test]
    fn off_the_screens_are_as_they_are_and_nothing_is_asked_again() {
        assert_eq!(at(NightLight::default(), 23, 0), (DAYLIGHT, None));
    }

    #[test]
    fn by_day_the_screens_are_as_they_are_until_dark() {
        assert_eq!(at(on(), 12, 0), (DAYLIGHT, Some("19:00:00".to_owned())));
    }

    #[test]
    fn after_dark_the_screens_turn_warm_over_twenty_minutes_a_step_at_a_time() {
        // At dusk, one minute of the twenty-one; ten minutes on, eleven.
        assert_eq!(at(on(), 19, 0), (6381, Some("19:00:30".to_owned())));
        assert_eq!(at(on(), 19, 10), (5191, Some("19:10:30".to_owned())));
        // Warm, until twenty minutes before it is light, or by midnight.
        assert_eq!(at(on(), 19, 20), (4000, Some("23:40:00".to_owned())));
        assert_eq!(at(on(), 3, 0), (4000, Some("06:40:00".to_owned())));
    }

    #[test]
    fn before_it_is_light_the_screens_turn_back_and_are_as_they_were_by_dawn() {
        assert_eq!(at(on(), 6, 50), (5310, Some("06:50:30".to_owned())));
        assert_eq!(at(on(), 7, 0), (DAYLIGHT, Some("19:00:00".to_owned())));
    }

    #[test]
    fn daylight_leaves_the_screens_as_they_are_and_warmer_takes_away_blue_most() {
        assert_eq!(tint(DAYLIGHT), [1.0, 1.0, 1.0]);
        assert_eq!(tint(9000), [1.0, 1.0, 1.0], "never colder than daylight");
        let [red, green, blue] = tint(4000);
        assert!((red - 1.0).abs() < f64::EPSILON, "{red}");
        assert!(
            0.8 < green && green < 0.9 && 0.6 < blue && blue < 0.7,
            "{green} {blue}"
        );
        let [red, green, blue] = tint(1900);
        assert!(red > green && green > blue, "{red} {green} {blue}");
        assert!(blue < 0.1, "{blue}");
    }

    #[test]
    fn a_ramp_goes_from_black_to_the_tint_of_white() {
        let [red, green, blue] = ramps([1.0, 0.5, 0.0], 3);
        assert_eq!(red, [0, 32768, 65535]);
        assert_eq!(green, [0, 16384, 32768]);
        assert_eq!(blue, [0, 0, 0]);
        assert_eq!(ramps([1.0; 3], 1)[0], [0], "a table of one");
    }

    #[test]
    fn a_black_body_hotter_than_daylight_is_bluer() {
        let [red, green, blue] = black_body(9000);
        assert!(red < 255.0 && green < 255.0 && (blue - 255.0).abs() < f64::EPSILON);
    }
}
