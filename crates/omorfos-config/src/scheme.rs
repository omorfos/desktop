//! The two schemes the built-in theme comes in, and the accents it picks
//! things out with, as the design language has them (DESIGN.md, "Colour").
//!
//! Each is a few variables laid over the built-in theme's own, so that a
//! theme of the user's, laid over both, still has the last word.

use serde::Deserialize;

use crate::Color;

/// The variables of the light scheme, which replace the dark ones the
/// built-in theme is written with.
const LIGHT: &str = include_str!("light.css");

/// Whether the desktop is dark or light.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Scheme {
    /// Light writing on navy.
    #[default]
    Dark,
    /// Dark writing on a warm cream.
    Light,
}

/// The one colour that says what has focus and what will happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(try_from = "String")]
pub enum Accent {
    /// The brand's own.
    #[default]
    Cyan,
    /// Blue.
    Blue,
    /// Teal.
    Teal,
    /// Purple.
    Purple,
    /// Pink.
    Pink,
    /// Orange.
    Orange,
    /// Any colour, the same in either scheme.
    Own(Color),
}

/// The accents that come with omorfos, by the names they are written as.
pub(crate) const ACCENTS: [(&str, Accent); 6] = [
    ("cyan", Accent::Cyan),
    ("blue", Accent::Blue),
    ("teal", Accent::Teal),
    ("purple", Accent::Purple),
    ("pink", Accent::Pink),
    ("orange", Accent::Orange),
];

impl Accent {
    /// The colour it is in `scheme`. Each that comes with omorfos has one
    /// for each, as a colour that is clear on navy is too pale on cream.
    #[must_use]
    pub const fn color(self, scheme: Scheme) -> Color {
        let (dark, light) = match self {
            Self::Cyan => (Color::rgb(0x22, 0xd3, 0xee), Color::rgb(0x0e, 0x74, 0x90)),
            Self::Blue => (Color::rgb(0x5b, 0x9c, 0xf5), Color::rgb(0x25, 0x63, 0xd9)),
            Self::Teal => (Color::rgb(0x4f, 0xc3, 0xb5), Color::rgb(0x0f, 0x76, 0x6e)),
            Self::Purple => (Color::rgb(0xa7, 0x8b, 0xfa), Color::rgb(0x6d, 0x3f, 0xd4)),
            Self::Pink => (Color::rgb(0xf2, 0x7f, 0xb0), Color::rgb(0xc0, 0x2a, 0x72)),
            Self::Orange => (Color::rgb(0xf2, 0x9b, 0x5c), Color::rgb(0xb8, 0x4a, 0x0e)),
            Self::Own(color) => (color, color),
        };
        match scheme {
            Scheme::Dark => dark,
            Scheme::Light => light,
        }
    }

    /// The colour of what is written on it in `scheme`: the scheme's own
    /// for each that comes with omorfos, which is made to be read on them;
    /// and for any other, whichever of that and the scheme's text stands
    /// out from it more.
    #[must_use]
    pub fn on(self, scheme: Scheme) -> Color {
        let (on, text) = match scheme {
            Scheme::Dark => (Color::rgb(0x0c, 0x12, 0x26), Color::rgb(0xe6, 0xea, 0xf5)),
            Scheme::Light => (Color::rgb(0xff, 0xfd, 0xf4), Color::rgb(0x0c, 0x12, 0x26)),
        };
        let Self::Own(color) = self else {
            return on;
        };
        if color.contrast(text) > color.contrast(on) {
            text
        } else {
            on
        }
    }
}

impl TryFrom<String> for Accent {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        let named = ACCENTS.iter().find(|(name, _)| *name == text);
        if let Some((_, accent)) = named {
            return Ok(*accent);
        }
        let color = text.parse::<Color>().ok();
        let color = color.filter(|color| color.alpha == u8::MAX);
        color.map(Self::Own).ok_or_else(|| {
            format!(
                "invalid accent `{text}`: expected cyan, blue, teal, purple, pink or orange, \
                 or a colour that cannot be seen through, such as `#3584e4`"
            )
        })
    }
}

/// What `scheme` and `accent` lay over the built-in theme: the variables
/// of the scheme, and the accent and what is written on it.
pub(crate) fn sheet(scheme: Scheme, accent: Accent) -> String {
    let hex = |color: Color| format!("#{:02x}{:02x}{:02x}", color.red, color.green, color.blue);
    let colors = match scheme {
        Scheme::Dark => "",
        Scheme::Light => LIGHT,
    };
    format!(
        "{colors}\n:root {{ --accent: {}; --on-accent: {}; }}\n",
        hex(accent.color(scheme)),
        hex(accent.on(scheme)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn accent(text: &str) -> Result<Accent, String> {
        Accent::try_from(text.to_owned())
    }

    #[test]
    fn an_accent_is_one_that_comes_with_omorfos_or_any_solid_colour() {
        assert_eq!(accent("cyan"), Ok(Accent::Cyan));
        assert_eq!(accent("orange"), Ok(Accent::Orange));
        assert_eq!(
            accent("#3584e4"),
            Ok(Accent::Own(Color::rgb(0x35, 0x84, 0xe4)))
        );
        for wrong in ["Cyan", "green", "#3584e480", "3584e4", ""] {
            let message = accent(wrong).unwrap_err();
            assert!(message.contains(&format!("`{wrong}`")), "{message}");
            assert!(message.contains("purple"), "{message}");
        }
    }

    #[test]
    fn each_accent_is_deeper_in_the_light_scheme_and_a_colour_of_ones_own_is_not() {
        for (name, accent) in ACCENTS {
            let (dark, light) = (accent.color(Scheme::Dark), accent.color(Scheme::Light));
            let navy = Color::rgb(0x0c, 0x12, 0x26);
            assert!(
                dark.contrast(navy) > light.contrast(navy),
                "{name} is paler on navy than on cream"
            );
        }
        let own = Accent::Own(Color::rgb(1, 2, 3));
        assert_eq!(own.color(Scheme::Dark), Color::rgb(1, 2, 3));
        assert_eq!(own.color(Scheme::Light), Color::rgb(1, 2, 3));
    }

    #[test]
    fn what_is_written_on_an_accent_of_ones_own_is_whichever_stands_out_more() {
        let (navy, cream) = (Color::rgb(0x0c, 0x12, 0x26), Color::rgb(0xff, 0xfd, 0xf4));
        let pale = Accent::Own(Color::rgb(0xf0, 0xe0, 0x90));
        let deep = Accent::Own(Color::rgb(0x40, 0x10, 0x60));
        assert_eq!(pale.on(Scheme::Dark), navy);
        assert_eq!(pale.on(Scheme::Light), navy);
        assert_eq!(deep.on(Scheme::Dark), Color::rgb(0xe6, 0xea, 0xf5));
        assert_eq!(deep.on(Scheme::Light), cream);
        // The ones that come with omorfos have the scheme's own.
        assert_eq!(Accent::Blue.on(Scheme::Dark), navy);
        assert_eq!(Accent::Blue.on(Scheme::Light), cream);
    }

    #[test]
    fn the_light_scheme_lays_its_colours_and_the_accent_over_the_theme() {
        let dark = sheet(Scheme::Dark, Accent::Cyan);
        assert!(!dark.contains("--background"), "{dark}");
        assert!(
            dark.contains("--accent: #22d3ee; --on-accent: #0c1226;"),
            "{dark}"
        );
        let light = sheet(Scheme::Light, Accent::Own(Color::rgb(0xab, 0xcd, 0xef)));
        assert!(light.contains("--background: #fcf7e3;"), "{light}");
        assert!(light.contains("--accent: #abcdef;"), "{light}");
    }
}
