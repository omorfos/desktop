//! A tooltip: a few lines of text in a box, drawn as the theme's `tooltip`
//! says, where the pointer has rested. The compositor shows one over the
//! mark that says the screen is captured, and the shell's components over
//! what is on their bars.

use omorfos_config::{Part, States, Theme};

use crate::{Canvas, Scale, Text};

/// How tall each line of a tooltip is, in units of layout.
pub const ROW: u32 = 22;

/// Paints a tooltip saying `lines`, a line of text for each, in a box as
/// wide as the widest.
#[must_use]
pub fn tooltip(text: &mut Text, theme: &Theme, lines: &[String], scale: Scale) -> Canvas {
    let style = theme.style(Part::Tooltip, States::NONE);
    let font = theme.font(Part::Tooltip, States::NONE);
    let widest = lines
        .iter()
        .map(|line| text.measure(line, font))
        .max()
        .unwrap_or(0);
    let inset = style.inset();
    let width = widest.saturating_add(inset.across());
    let rows = u32::try_from(lines.len()).unwrap_or(u32::MAX);
    let height = rows.saturating_mul(ROW).saturating_add(inset.down());
    let whole = (0, 0, width, height);
    let mut canvas = Canvas::scaled(width, height, scale);
    canvas.draw_box(whole, &style);
    let (left, mut top, room, _) = crate::inset(whole, inset);
    for line in lines {
        text.draw(&mut canvas, line, (left, top, room, ROW), style.color, font);
        top += ROW.cast_signed();
    }
    canvas
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(said: &[&str]) -> Vec<String> {
        said.iter().map(|line| (*line).to_owned()).collect()
    }

    #[test]
    fn a_tooltip_is_a_row_a_line_and_as_wide_as_its_widest_line() {
        let theme = Theme::default();
        let mut text = Text::new();
        let one = tooltip(&mut text, &theme, &lines(&["Saturday"]), Scale::ONE);
        let two = tooltip(
            &mut text,
            &theme,
            &lines(&["Saturday", "14 November 2026, a long line"]),
            Scale::ONE,
        );
        let (one_wide, one_tall) = one.units();
        let (two_wide, two_tall) = two.units();
        assert_eq!(two_tall - one_tall, ROW, "a row more");
        assert!(two_wide > one_wide, "as wide as the wider line");
        let font = theme.font(Part::Tooltip, States::NONE);
        let inset = theme.style(Part::Tooltip, States::NONE).inset();
        assert_eq!(one_wide, text.measure("Saturday", font) + inset.across());
    }

    #[test]
    fn a_tooltip_is_its_background_with_its_text_on_it() {
        let theme = Theme::default();
        let style = theme.style(Part::Tooltip, States::NONE);
        let canvas = tooltip(
            &mut Text::new(),
            &theme,
            &lines(&["Wide words"]),
            Scale::ONE,
        );
        let (width, height) = canvas.units();
        // As a canvas holds it: each of red, green and blue as much of
        // itself as the colour, which is glass, is there at all.
        let color = style.background;
        let share = |channel: u8| {
            let shared = (u16::from(channel) * u16::from(color.alpha) + 127) / 255;
            u8::try_from(shared).unwrap()
        };
        let (red, green, blue) = (share(color.red), share(color.green), share(color.blue));
        let background = u32::from_be_bytes([color.alpha, red, green, blue]);
        let pixels = (0..width).flat_map(|x| (0..height).map(move |y| (x, y)));
        let all: Vec<u32> = pixels.map(|(x, y)| canvas.at(x, y)).collect();
        assert!(all.contains(&background), "its background");
        let inked = all.iter().filter(|pixel| **pixel != background).count();
        // More than its border and its rounded corners: the words.
        let border = 2 * (width + height) as usize;
        assert!(inked > border, "words on it");
    }
}
