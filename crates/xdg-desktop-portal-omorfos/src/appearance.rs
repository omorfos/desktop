//! What the desktop looks like, as applications are told it.

use omorfos_config::{Color, Part, State, States, Theme};

/// What applications are told of how the desktop looks, so that they can
/// look the same.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Appearance {
    /// Whether it is dark: light writing on a dark ground.
    pub dark: bool,
    /// The colour that picks things out, as red, green and blue from 0
    /// to 1.
    pub accent: (f64, f64, f64),
}

/// How bright a colour looks, weighing green most and blue least as the
/// eye does. Only for telling which of two is the brighter.
fn brightness(color: Color) -> u32 {
    2126 * u32::from(color.red) + 7152 * u32::from(color.green) + 722 * u32::from(color.blue)
}

impl Appearance {
    /// How a theme looks. It is dark if what it writes menus in is
    /// brighter than what it writes them on, and what picks things out is
    /// what it lights the row of a menu under the pointer with.
    #[must_use]
    pub fn of(theme: &Theme) -> Self {
        let menu = theme.style(Part::Menu, States::NONE);
        let picked = theme.style(Part::MenuItem, States::NONE.with(State::Hovered));
        let share = |channel: u8| f64::from(channel) / f64::from(u8::MAX);
        let accent = picked.background;
        Self {
            dark: brightness(menu.color) > brightness(menu.background),
            accent: (share(accent.red), share(accent.green), share(accent.blue)),
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
    use super::*;

    #[test]
    fn the_built_in_theme_is_dark_with_a_blue_accent() {
        let appearance = Appearance::of(&Theme::default());
        assert!(appearance.dark);
        assert_eq!(appearance.scheme(), 1);
        let (red, green, blue) = appearance.accent;
        assert!(blue > green && green > red, "{:?}", appearance.accent);
        assert!((blue - f64::from(0xf5_u8) / 255.0).abs() < 1e-9);
    }

    #[test]
    fn a_theme_that_writes_dark_on_light_is_light() {
        let theme = "menu { background: #ffffff; color: #101010; }\n\
                     menu-item:hover { background: #ff0000; }";
        let appearance = Appearance::of(&Theme::parse(theme).unwrap());
        assert!(!appearance.dark);
        assert_eq!(appearance.scheme(), 2);
        assert_eq!(appearance.accent, (1.0, 0.0, 0.0));
    }

    #[test]
    fn green_counts_for_more_than_blue_in_how_bright_a_colour_looks() {
        assert!(brightness(Color::rgb(0, 200, 0)) > brightness(Color::rgb(0, 0, 255)));
        assert!(brightness(Color::rgb(255, 255, 255)) > brightness(Color::rgb(254, 255, 255)));
        assert_eq!(brightness(Color::rgb(0, 0, 0)), 0);
    }
}
