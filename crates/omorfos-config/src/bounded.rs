//! Integers constrained to a range.

use serde::Deserialize;

/// An integer guaranteed to lie between `MIN` and `MAX`, inclusive.
///
/// Out-of-range values are rejected when the configuration is read, so code
/// that consumes a `Bounded` never has to clamp or re-validate it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u32")]
pub struct Bounded<const MIN: u32, const MAX: u32>(u32);

impl<const MIN: u32, const MAX: u32> Bounded<MIN, MAX> {
    /// Creates a value, clamping `value` into the range.
    #[must_use]
    pub const fn clamped(value: u32) -> Self {
        if value < MIN {
            Self(MIN)
        } else if value > MAX {
            Self(MAX)
        } else {
            Self(value)
        }
    }

    /// Returns the underlying integer.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl<const MIN: u32, const MAX: u32> TryFrom<u32> for Bounded<MIN, MAX> {
    type Error = String;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        if (MIN..=MAX).contains(&value) {
            Ok(Self(value))
        } else {
            Err(format!(
                "expected a number between {MIN} and {MAX}, got {value}"
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Percent = Bounded<10, 100>;

    #[test]
    fn accepts_values_at_both_ends_of_the_range() {
        assert_eq!(Percent::try_from(10).map(Percent::get), Ok(10));
        assert_eq!(Percent::try_from(100).map(Percent::get), Ok(100));
    }

    #[test]
    fn rejects_values_outside_the_range_and_says_what_is_allowed() {
        let expected = Err("expected a number between 10 and 100, got 9".to_owned());
        assert_eq!(Percent::try_from(9), expected);
        assert!(Percent::try_from(101).is_err());
    }

    #[test]
    fn clamped_pulls_values_into_the_range() {
        assert_eq!(Percent::clamped(0).get(), 10);
        assert_eq!(Percent::clamped(50).get(), 50);
        assert_eq!(Percent::clamped(500).get(), 100);
    }
}
