//! Colors.

use std::str::FromStr;

use serde::Deserialize;

/// An sRGB color with straight (non-premultiplied) alpha.
///
/// Written in configuration as `"#rrggbb"` or `"#rrggbbaa"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct Color {
    /// Red channel.
    pub red: u8,
    /// Green channel.
    pub green: u8,
    /// Blue channel.
    pub blue: u8,
    /// Opacity, where `255` is fully opaque.
    pub alpha: u8,
}

impl Color {
    /// Creates a fully opaque color.
    #[must_use]
    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self::rgba(red, green, blue, u8::MAX)
    }

    /// Creates a color with the given opacity.
    #[must_use]
    pub const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }
}

impl FromStr for Color {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let channels = text.strip_prefix('#').and_then(|hex| {
            hex.as_bytes()
                .chunks(2)
                .map(hex_byte)
                .collect::<Option<Vec<u8>>>()
        });
        match channels.as_deref() {
            Some(&[red, green, blue]) => Ok(Self::rgb(red, green, blue)),
            Some(&[red, green, blue, alpha]) => Ok(Self::rgba(red, green, blue, alpha)),
            _ => Err(format!(
                "invalid color `{text}`: expected `#rrggbb` or `#rrggbbaa`"
            )),
        }
    }
}

impl TryFrom<String> for Color {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

fn hex_byte(pair: &[u8]) -> Option<u8> {
    match *pair {
        [high, low] => Some(hex_digit(high)? << 4 | hex_digit(low)?),
        _ => None,
    }
}

const fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_digits_parse_as_an_opaque_color() {
        assert_eq!("#1a2B3c".parse(), Ok(Color::rgb(0x1a, 0x2b, 0x3c)));
        assert_eq!(Color::rgb(1, 2, 3).alpha, 255);
    }

    #[test]
    fn eight_digits_include_alpha() {
        assert_eq!("#000000Ff".parse(), Ok(Color::rgba(0, 0, 0, 255)));
        assert_eq!("#09afAF80".parse(), Ok(Color::rgba(0x09, 0xaf, 0xaf, 0x80)));
    }

    #[test]
    fn malformed_colors_are_rejected_with_the_expected_format() {
        for text in [
            "",
            "red",
            "112233",
            "#",
            "#12345",
            "#1234567",
            "#1122334455",
            "#gg0000",
            "#00gg00",
            "#0g0000",
            "#+12345",
            "#ééé",
        ] {
            assert_eq!(
                text.parse::<Color>(),
                Err(format!(
                    "invalid color `{text}`: expected `#rrggbb` or `#rrggbbaa`"
                )),
            );
        }
    }

    #[test]
    fn converts_from_an_owned_string() {
        assert_eq!(
            Color::try_from("#ffffff".to_owned()),
            Ok(Color::rgb(255, 255, 255))
        );
    }
}
