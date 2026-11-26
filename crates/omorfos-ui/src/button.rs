//! A question's button: its words in the middle of a box drawn as the
//! theme's `question-button` says, in the states it is in. The boxes that
//! ask, the lock screen and the welcome card all draw theirs with it, so
//! that a button looks and is sized alike wherever it is.

use omorfos_config::{Font, Part, States, Style, Theme};

use crate::{Area, Canvas, Text};

/// How large a button saying `label` is: as wide as its words and the
/// theme's padding either side, held to its `min-width` and `max-width`,
/// and as tall as the theme's `height`.
#[must_use]
pub fn button_size(text: &mut Text, theme: &Theme, label: &str) -> (u32, u32) {
    let style = theme.style(Part::QuestionButton, States::NONE);
    let font = theme.font(Part::QuestionButton, States::NONE);
    let wide = text
        .measure(label, font)
        .saturating_add(style.inset().across());
    (style.fit(wide), style.height)
}

/// How a button in `states` looks: `question-button`'s style and font in
/// them. Whatever has the keyboard is [`omorfos_config::State::Focused`],
/// which the theme rings; the one that goes on is
/// [`omorfos_config::State::Active`].
#[must_use]
pub fn button_look(theme: &Theme, states: States) -> (Style, &Font) {
    let style = theme.style(Part::QuestionButton, states);
    (style, theme.font(Part::QuestionButton, states))
}

/// Draws a button saying `label` over `area`, as `look` says, its words in
/// the middle.
pub fn button(
    canvas: &mut Canvas,
    text: &mut Text,
    (style, font): (&Style, &Font),
    (label, area): (&str, Area),
) {
    canvas.draw_box(area, style);
    text.draw_centered(canvas, label, area, style.color, font);
}

#[cfg(test)]
mod tests {
    use omorfos_config::State;

    use super::*;
    use crate::Scale;

    #[test]
    fn a_button_is_as_wide_as_its_words_and_padding_and_as_tall_as_the_theme_says() {
        let theme = Theme::default();
        let mut text = Text::new();
        let style = theme.style(Part::QuestionButton, States::NONE);
        let font = theme.font(Part::QuestionButton, States::NONE);
        let (wide, tall) = button_size(&mut text, &theme, "Shut down");
        assert_eq!(
            wide,
            text.measure("Shut down", font) + style.inset().across()
        );
        assert_eq!(tall, style.height);
        assert!(tall > 0 && style.inset().across() > 0, "the theme sizes it");
        let (longer, _) = button_size(&mut text, &theme, "Shut down the machine now");
        assert!(longer > wide);
        // A theme can hold it to a narrowest.
        let narrowest = Theme::parse("dialog.question > button { min-width: 300px; }").unwrap();
        assert_eq!(button_size(&mut text, &narrowest, "OK").0, 300);
    }

    #[test]
    fn a_button_is_its_box_with_its_words_in_the_middle_and_the_ring_where_it_has_the_keyboard() {
        let theme = Theme::default();
        let mut text = Text::new();
        let draw = |states: States, text: &mut Text| {
            let mut canvas = Canvas::scaled(160, 60, Scale::ONE);
            let (style, font) = button_look(&theme, states);
            button(
                &mut canvas,
                text,
                (&style, font),
                ("Cancel", (20, 12, 120, 36)),
            );
            canvas
        };
        let plain = draw(States::NONE, &mut text);
        let focused = draw(States::NONE.with(State::Focused), &mut text);
        let background = plain.at(25, 30);
        assert_ne!(background, 0, "its box");
        // Its words in the middle, none at its ends.
        let inked = |canvas: &Canvas, columns: std::ops::Range<u32>| {
            columns
                .flat_map(|x| (14..46).map(move |y| (x, y)))
                .filter(|(x, y)| canvas.at(*x, *y) != background)
                .count()
        };
        assert!(inked(&plain, 60..100) > 20, "words in the middle");
        assert_eq!(inked(&plain, 24..34), 0, "room at its start");
        // The ring, outside the box, only where it has the keyboard.
        assert_eq!(plain.at(17, 30), 0);
        assert_ne!(focused.at(17, 30), 0, "ringed");
    }
}
