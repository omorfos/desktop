//! A box that asks: what is asked, lines under it, room for whatever the
//! asker puts there, such as a field to type in, and its buttons along the
//! bottom at the right, the one that goes on last. Drawn as the theme's
//! `question`, `question-detail` and `question-button` say, and laid out
//! from the theme's padding and the buttons' own sizes, so that every box
//! that asks is alike.

use omorfos_config::{Part, States, Theme};

use crate::{Area, Canvas, Scale, Text, button, button_look, button_size};

/// How tall what is asked is, and each line under it.
const ASKED: u32 = 26;
const LINE: u32 = 20;
/// The room above the buttons, and between two of them.
const ABOVE: u32 = 16;
const BETWEEN: u32 = 8;

/// What a box asks, `N` buttons to answer it with.
#[derive(Debug, Clone, Copy)]
pub struct Asked<'a, const N: usize> {
    /// What is asked, on a line of its own.
    pub question: &'a str,
    /// The lines under it, as they are to be shown.
    pub lines: &'a [String],
    /// How tall the room for what the asker puts there is: none for a box
    /// with only words.
    pub room: u32,
    /// What each button says, in the order they stand.
    pub buttons: [&'a str; N],
}

/// Where everything of a box is, within it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Laid<const N: usize> {
    /// How large it is.
    pub size: (u32, u32),
    /// Where what is asked is.
    pub question: Area,
    /// Where each line is.
    pub lines: Vec<Area>,
    /// The room for what the asker puts there.
    pub room: Area,
    /// Where each button is.
    pub buttons: [Area; N],
}

/// Lays out a box `width` wide asking what `asked` says: within the
/// `question` part's padding, what is asked, then the lines, the room
/// asked for, and the buttons at the bottom, each as large as its words
/// make it, at the right.
#[must_use]
pub fn lay_out<const N: usize>(
    text: &mut Text,
    theme: &Theme,
    width: u32,
    asked: &Asked<'_, N>,
) -> Laid<N> {
    let inset = theme.style(Part::Question, States::NONE).inset();
    let room = width.saturating_sub(inset.across());
    let left = inset.left.cast_signed();
    let mut top = inset.top.cast_signed();
    let question = (left, top, room, ASKED);
    top += ASKED.cast_signed();
    let lines = asked
        .lines
        .iter()
        .map(|_| {
            let line = (left, top, room, LINE);
            top += LINE.cast_signed();
            line
        })
        .collect();
    let given = (left, top, room, asked.room);
    top += (asked.room + ABOVE).cast_signed();
    // From the right, the last first.
    let sizes = asked.buttons.map(|said| button_size(text, theme, said));
    let tall = sizes
        .iter()
        .map(|(_, tall)| *tall)
        .max()
        .unwrap_or_default();
    let mut end = width.saturating_sub(inset.right).cast_signed();
    let mut buttons = sizes.map(|(wide, _)| (0, top, wide, tall));
    for button in buttons.iter_mut().rev() {
        button.0 = end - button.2.cast_signed();
        end = button.0 - BETWEEN.cast_signed();
    }
    let height = top.unsigned_abs() + tall + inset.bottom;
    Laid {
        size: (width, height),
        question,
        lines,
        room: given,
        buttons,
    }
}

/// Paints a box laid out as `laid`, asking what `asked` says, each button
/// in the states `states` gives it, at `scale`. The room asked for is left
/// for the asker to draw on.
#[must_use]
pub fn paint_question<const N: usize>(
    text: &mut Text,
    theme: &Theme,
    (asked, laid): (&Asked<'_, N>, &Laid<N>),
    states: [States; N],
    scale: Scale,
) -> Canvas {
    let (width, height) = laid.size;
    let mut canvas = Canvas::scaled(width, height, scale);
    let style = theme.style(Part::Question, States::NONE);
    canvas.draw_box((0, 0, width, height), &style);
    let font = theme.font(Part::Question, States::NONE);
    text.draw(
        &mut canvas,
        asked.question,
        laid.question,
        style.color,
        font,
    );
    let detail = theme.style(Part::QuestionDetail, States::NONE);
    let detail_font = theme.font(Part::QuestionDetail, States::NONE);
    for (line, area) in asked.lines.iter().zip(&laid.lines) {
        text.draw(&mut canvas, line, *area, detail.color, detail_font);
    }
    let buttons = asked.buttons.iter().zip(laid.buttons).zip(states);
    for ((said, area), states) in buttons {
        let (style, font) = button_look(theme, states);
        button(&mut canvas, text, (&style, font), (said, area));
    }
    canvas
}

/// Which of `laid`'s buttons `point` is on, by its place among them.
#[must_use]
pub fn button_at<const N: usize>(laid: &Laid<N>, (x, y): (i32, i32)) -> Option<usize> {
    let on = |(left, top, width, height): &Area| {
        (*left..*left + width.cast_signed()).contains(&x)
            && (*top..*top + height.cast_signed()).contains(&y)
    };
    laid.buttons.iter().position(on)
}

#[cfg(test)]
mod tests {
    use omorfos_config::State;

    use super::*;

    fn asked(lines: &[String], room: u32) -> Asked<'_, 2> {
        Asked {
            question: "Shut down now?",
            lines,
            room,
            buttons: ["Cancel", "Shut down"],
        }
    }

    #[test]
    fn a_box_is_laid_out_within_its_padding_with_its_buttons_at_the_bottom_right() {
        let theme = Theme::default();
        let mut text = Text::new();
        let inset = theme.style(Part::Question, States::NONE).inset();
        let lines = vec!["One".to_owned(), "Two".to_owned()];
        let laid = lay_out(&mut text, &theme, 380, &asked(&lines, 0));
        let (left, top) = (inset.left.cast_signed(), inset.top.cast_signed());
        assert_eq!(laid.question, (left, top, 380 - inset.across(), ASKED));
        assert_eq!(laid.lines.len(), 2);
        assert_eq!(laid.lines[1].1, top + (ASKED + LINE).cast_signed());
        let [cancel, go_on] = laid.buttons;
        let (go_wide, tall) = button_size(&mut text, &theme, "Shut down");
        assert_eq!(
            go_on.0 + go_on.2.cast_signed(),
            (380 - inset.right).cast_signed(),
            "at the right"
        );
        assert_eq!((go_on.2, go_on.3), (go_wide, tall));
        assert_eq!(
            cancel.0 + cancel.2.cast_signed() + BETWEEN.cast_signed(),
            go_on.0,
            "before it"
        );
        assert_eq!(laid.size.1, go_on.1.unsigned_abs() + tall + inset.bottom);

        // Room asked for comes under the lines, and moves the buttons down.
        let roomy = lay_out(&mut text, &theme, 380, &asked(&lines, 50));
        assert_eq!(roomy.room.1, laid.lines[1].1 + LINE.cast_signed());
        assert_eq!(roomy.room.3, 50);
        assert_eq!(roomy.buttons[0].1, laid.buttons[0].1 + 50);
        assert_eq!(roomy.size.1, laid.size.1 + 50);
    }

    #[test]
    fn a_box_is_painted_with_its_words_and_its_buttons_and_says_which_is_where() {
        let theme = Theme::default();
        let mut text = Text::new();
        let lines = vec!["Whatever is not saved will be lost.".to_owned()];
        let asked = asked(&lines, 0);
        let laid = lay_out(&mut text, &theme, 380, &asked);
        let states = [States::NONE, States::NONE.with(State::Active)];
        let canvas = paint_question(&mut text, &theme, (&asked, &laid), states, Scale::ONE);
        assert_eq!(canvas.units(), laid.size);
        let [cancel, go_on] = laid.buttons;
        let (x, y) = (go_on.0 + 4, go_on.1 + 4);
        let accent = theme
            .style(Part::QuestionButton, States::NONE.with(State::Active))
            .background;
        let accent = u32::from_be_bytes([accent.alpha, accent.red, accent.green, accent.blue]);
        assert_eq!(
            canvas.at(x.unsigned_abs(), y.unsigned_abs()),
            accent,
            "the one that goes on"
        );
        assert_eq!(button_at(&laid, (x, y)), Some(1));
        assert_eq!(button_at(&laid, (cancel.0 + 1, cancel.1 + 1)), Some(0));
        assert_eq!(button_at(&laid, (cancel.0 - 2, cancel.1 + 1)), None);
        assert_eq!(button_at(&laid, (x, go_on.1 - 1)), None);
    }
}
