//! What the desktop looks like, as applications are told it.

use omorfos_config::{Cursor, Icons, Part, State, States, Theme};

/// What applications are told of how the desktop looks, so that they can
/// look the same.
#[derive(Debug, Clone, PartialEq)]
pub struct Appearance {
    /// Whether it is dark: light writing on a dark ground.
    pub dark: bool,
    /// The colour that picks things out, as red, green and blue from 0
    /// to 1.
    pub accent: (f64, f64, f64),
    /// The pointer: the name of its theme, and how large it is, in units
    /// of layout, for toolkits that draw it themselves.
    pub pointer: (String, i32),
    /// The font most text is written in, as GTK writes one.
    pub font: String,
    /// The name of the icon theme.
    pub icons: String,
}

impl Appearance {
    /// How a theme looks: dark as [`Theme::dark`] says, and what picks
    /// things out is what it fills the row chosen in the launcher with.
    /// A row under the pointer is not the accent but raised, as the design
    /// language has it.
    /// The pointer is `cursor`, and the icons are `icons`.
    #[must_use]
    pub fn of(theme: &Theme, cursor: &Cursor, icons: &Icons) -> Self {
        let picked = theme.style(Part::LauncherItem, States::NONE.with(State::Active));
        let share = |channel: u8| f64::from(channel) / f64::from(u8::MAX);
        let accent = picked.ground();
        let size = i32::try_from(cursor.size.get()).unwrap_or(i32::MAX);
        Self {
            dark: theme.dark(),
            accent: (share(accent.red), share(accent.green), share(accent.blue)),
            pointer: (cursor.theme.clone(), size),
            font: theme.font_name(),
            icons: icons.theme.clone(),
        }
    }

    /// Light or dark, by the number the portal has for each: 1 for dark
    /// and 2 for light.
    #[must_use]
    pub const fn scheme(&self) -> u32 {
        if self.dark { 1 } else { 2 }
    }
}

#[cfg(test)]
mod tests {
    use omorfos_config::{Accent, Scheme};

    use super::*;

    #[test]
    fn the_built_in_theme_is_dark_with_a_cyan_accent() {
        let appearance = Appearance::of(&Theme::default(), &Cursor::default(), &Icons::default());
        assert!(appearance.dark);
        assert_eq!(appearance.scheme(), 1);
        let (red, green, blue) = appearance.accent;
        assert!(blue > green && green > red, "{:?}", appearance.accent);
        assert!((blue - f64::from(0xee_u8) / 255.0).abs() < 1e-9);
    }

    #[test]
    fn the_light_scheme_is_told_as_light_with_its_deeper_accent() {
        let theme = Theme::parse_in("", Scheme::Light, Accent::Cyan).unwrap();
        let appearance = Appearance::of(&theme, &Cursor::default(), &Icons::default());
        assert!(!appearance.dark);
        assert_eq!(appearance.scheme(), 2);
        let share = |channel: u8| f64::from(channel) / 255.0;
        assert_eq!(appearance.accent, (share(0x0e), share(0x74), share(0x90)));
    }

    #[test]
    fn a_theme_that_writes_dark_on_light_is_light() {
        let theme = "menu { background: #ffffff; color: #101010; }\n\
                     launcher-item:active { background: #ff0000; }";
        let appearance = Appearance::of(
            &Theme::parse(theme).unwrap(),
            &Cursor::default(),
            &Icons::default(),
        );
        assert!(!appearance.dark);
        assert_eq!(appearance.scheme(), 2);
        assert_eq!(appearance.accent, (1.0, 0.0, 0.0));
    }
}
