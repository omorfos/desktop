//! `theme.yaml`: how the desktop looks.

use serde::Deserialize;

use crate::{Bounded, Color};

/// The contents of `theme.yaml`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    /// The font used for titlebars and menus.
    pub font: Font,
    /// Window borders.
    pub border: Border,
    /// Window titlebars.
    pub titlebar: Titlebar,
    /// The color palette.
    pub colors: Colors,
    /// Motion.
    pub animations: Animations,
}

/// The font used for titlebars and menus.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Font {
    /// A font family name, or a generic one such as `sans-serif`.
    pub family: String,
    /// Size in points.
    pub size: Bounded<6, 72>,
}

impl Default for Font {
    fn default() -> Self {
        Self {
            family: "sans-serif".to_owned(),
            size: Bounded::clamped(11),
        }
    }
}

/// Window borders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Border {
    /// Border thickness in pixels. `0` removes the border.
    pub width: Bounded<0, 16>,
    /// Corner radius in pixels. `0` gives square corners.
    pub radius: Bounded<0, 32>,
}

impl Default for Border {
    fn default() -> Self {
        Self {
            width: Bounded::clamped(1),
            radius: Bounded::clamped(8),
        }
    }
}

/// Window titlebars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Titlebar {
    /// Titlebar height in pixels.
    pub height: Bounded<16, 64>,
}

impl Default for Titlebar {
    fn default() -> Self {
        Self {
            height: Bounded::clamped(28),
        }
    }
}

/// The color palette. Colors are written as `"#rrggbb"` or `"#rrggbbaa"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Colors {
    /// The desktop background, shown where there is no wallpaper.
    pub background: Color,
    /// Highlights, such as the selected menu entry.
    pub accent: Color,
    /// Decoration of the focused window.
    pub focused: Decoration,
    /// Decoration of every other window.
    pub unfocused: Decoration,
}

impl Default for Colors {
    fn default() -> Self {
        let accent = Color::rgb(0x5b, 0x9c, 0xf5);
        Self {
            background: Color::rgb(0x14, 0x16, 0x1b),
            accent,
            focused: Decoration {
                border: accent,
                titlebar: Color::rgb(0x23, 0x26, 0x2e),
                text: Color::rgb(0xe6, 0xe8, 0xee),
            },
            unfocused: Decoration {
                border: Color::rgb(0x2c, 0x30, 0x3a),
                titlebar: Color::rgb(0x1b, 0x1d, 0x23),
                text: Color::rgb(0x8b, 0x90, 0xa0),
            },
        }
    }
}

/// The colors of one window's decoration. All three are required.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Decoration {
    /// The border around the window.
    pub border: Color,
    /// The titlebar background.
    pub titlebar: Color,
    /// The title text.
    pub text: Color,
}

/// Motion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Animations {
    /// Whether transitions are animated at all.
    pub enabled: bool,
    /// How long a transition takes, in milliseconds.
    pub duration: Bounded<0, 1000>,
}

impl Default for Animations {
    fn default() -> Self {
        Self {
            enabled: true,
            duration: Bounded::clamped(150),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(yaml: &str) -> Result<Theme, String> {
        serde_saphyr::from_str(yaml).map_err(|error| error.without_snippet().to_string())
    }

    #[test]
    fn the_default_theme_is_dark_with_rounded_corners_and_subtle_motion() {
        let theme = Theme::default();
        assert_eq!(theme.font.family, "sans-serif");
        assert_eq!(theme.font.size.get(), 11);
        assert_eq!(
            (theme.border.width.get(), theme.border.radius.get()),
            (1, 8)
        );
        assert_eq!(theme.titlebar.height.get(), 28);
        assert_eq!(theme.colors.background, Color::rgb(0x14, 0x16, 0x1b));
        assert_eq!(theme.colors.focused.border, theme.colors.accent);
        assert_ne!(theme.colors.focused, theme.colors.unfocused);
        assert_eq!(
            theme.animations,
            Animations {
                enabled: true,
                duration: Bounded::clamped(150)
            }
        );
    }

    #[test]
    fn a_partial_theme_overrides_only_what_it_names() {
        let theme = parse("border: { radius: 0 }\ncolors: { accent: '#ff0000' }\n").unwrap();
        assert_eq!(
            (theme.border.width.get(), theme.border.radius.get()),
            (1, 0)
        );
        assert_eq!(theme.colors.accent, Color::rgb(255, 0, 0));
        assert_eq!(theme.colors.background, Colors::default().background);
        assert_eq!(theme.font, Font::default());
    }

    #[test]
    fn every_token_can_be_overridden() {
        let yaml = "
font: { family: Inter, size: 12 }
border: { width: 2, radius: 4 }
titlebar: { height: 32 }
animations: { enabled: false, duration: 0 }
colors:
  background: '#000000'
  accent: '#ffffff80'
  focused: { border: '#010101', titlebar: '#020202', text: '#030303' }
  unfocused: { border: '#040404', titlebar: '#050505', text: '#060606' }
";
        let theme = parse(yaml).unwrap();
        assert_eq!(
            theme.font,
            Font {
                family: "Inter".to_owned(),
                size: Bounded::clamped(12)
            }
        );
        assert_eq!(theme.titlebar.height.get(), 32);
        assert_eq!(
            theme.animations,
            Animations {
                enabled: false,
                duration: Bounded::clamped(0)
            }
        );
        assert_eq!(theme.colors.accent, Color::rgba(255, 255, 255, 0x80));
        assert_eq!(theme.colors.focused.text, Color::rgb(3, 3, 3));
        assert_eq!(theme.colors.unfocused.border, Color::rgb(4, 4, 4));
    }

    #[test]
    fn sizes_outside_their_range_are_rejected() {
        let error = parse("font: { size: 200 }").unwrap_err();
        assert!(
            error.contains("expected a number between 6 and 72, got 200"),
            "{error}"
        );
    }

    #[test]
    fn a_decoration_must_name_all_of_its_colors() {
        let error = parse("colors: { focused: { border: '#000000' } }").unwrap_err();
        assert!(error.contains("missing field `titlebar`"), "{error}");
    }
}
