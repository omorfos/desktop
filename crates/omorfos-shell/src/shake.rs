//! A field shaken to say that what was typed in it was not right, as the
//! prompt for a secret and the lock screen do.

use crate::whole;

/// How far the field goes each way as it shakes, and the room it has to on
/// each side of it.
pub const SHAKE: i32 = 8;

/// How long the field shakes for, in milliseconds: no longer than anything
/// moves (ADR 32).
pub const SHAKING: u32 = 300;

/// How long a frame is taken to be, in milliseconds, for the first of a
/// shake: how far into it the field is when it is first shown moved.
pub const FRAME: u32 = 16;

/// How far the field is from where it rests, `elapsed` milliseconds into
/// shaking: three times each way, less each time, and still again once it
/// has shaken for as long as it does.
#[must_use]
pub fn shaken(elapsed: u32) -> i32 {
    let through = f64::from(elapsed.min(SHAKING)) / f64::from(SHAKING);
    let swing = (through * 3.0 * std::f64::consts::TAU).sin() * (1.0 - through);
    whole(swing * f64::from(SHAKE) + 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_field_shakes_three_times_less_each_time_and_comes_to_rest() {
        assert_eq!(shaken(0), 0);
        assert_eq!(shaken(SHAKING), 0, "at rest once it has shaken");
        assert_eq!(shaken(u32::MAX), 0);
        let swings: Vec<i32> = (0..SHAKING).step_by(10).map(shaken).collect();
        let most = swings.iter().map(|swing| swing.abs()).max().unwrap();
        assert!(most > SHAKE / 2 && most <= SHAKE, "{swings:?}");
        assert!(swings.iter().any(|swing| *swing < 0), "both ways");
        // The first swing is wider than the last.
        let early = swings[..10].iter().map(|swing| swing.abs()).max().unwrap();
        let late = swings[20..].iter().map(|swing| swing.abs()).max().unwrap();
        assert!(early > late, "{swings:?}");
    }
}
