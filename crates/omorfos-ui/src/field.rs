//! A line of text to type in: what is typed, where what is typed next goes,
//! what is chosen of it, and what can be undone, as a text field in GTK's
//! programs has them. Words and characters are as Unicode has them: an
//! accented letter written as two code points is one character, and a word
//! is what a word processor would jump over. Nothing here draws or knows of
//! keys.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

/// How far a move or a removal goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// A character, as the arrows go.
    Character,
    /// To the start or the end of a word, as Ctrl or Alt with an arrow goes.
    Word,
    /// To the start or the end of all of it, as Home and End go.
    Line,
}

/// Which way a move or a removal goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    /// Towards the start, as Left and Backspace go.
    Back,
    /// Towards the end, as Right and Delete go.
    On,
}

/// What can be done to a field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    /// Types this in place of what is chosen, or where the cursor is.
    Type(String),
    /// Moves the cursor `reach` one `way`; `choosing` what it goes over, as
    /// with Shift held, or else leaving nothing chosen.
    Move {
        /// Which way.
        way: Way,
        /// How far.
        reach: Reach,
        /// Whether what it goes over is chosen, with what was.
        choosing: bool,
    },
    /// Takes out what is chosen; or, with nothing chosen, as far as `reach`
    /// goes `way`, as Backspace and Delete do.
    Remove {
        /// Which way.
        way: Way,
        /// How far.
        reach: Reach,
    },
    /// Chooses all of it.
    All,
    /// Puts back how it was before the last change.
    Undo,
    /// Puts back the last change undone.
    Redo,
}

/// What is typed, where the cursor is in it, and how much is chosen: all
/// that can be undone and done again.
#[derive(Debug, Clone, PartialEq, Eq)]
struct State {
    text: String,
    /// Where the cursor is, in bytes, on the boundary of a character.
    cursor: usize,
    /// The other end of what is chosen, which is nothing where it is the
    /// cursor.
    anchor: usize,
}

/// A line of text being typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    now: State,
    /// The most characters it takes.
    longest: usize,
    /// How it was before each change, the last last.
    undone: Vec<State>,
    /// How it was after each change undone, the last undone last.
    redone: Vec<State>,
    /// Whether the last change was typing, which the next typing joins, so
    /// that a word typed is undone at once.
    typing: bool,
}

/// How many changes are kept to be undone.
const KEPT: usize = 100;

impl Field {
    /// An empty field taking at most `longest` characters.
    #[must_use]
    pub const fn new(longest: usize) -> Self {
        Self {
            now: State {
                text: String::new(),
                cursor: 0,
                anchor: 0,
            },
            longest,
            undone: Vec::new(),
            redone: Vec::new(),
            typing: false,
        }
    }

    /// What is typed.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.now.text
    }

    /// Where the cursor is, in bytes from the start.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.now.cursor
    }

    /// What is chosen, in bytes from the start: empty where nothing is.
    #[must_use]
    pub fn chosen(&self) -> Range<usize> {
        let State { cursor, anchor, .. } = self.now;
        cursor.min(anchor)..cursor.max(anchor)
    }

    /// The text chosen, if any is.
    #[must_use]
    pub fn chosen_text(&self) -> Option<&str> {
        let chosen = self.chosen();
        (!chosen.is_empty()).then(|| &self.now.text[chosen])
    }

    /// Does `edit`, and says whether anything changed, the cursor or what
    /// is chosen among it.
    pub fn edit(&mut self, edit: Edit) -> bool {
        let before = self.now.clone();
        match edit {
            Edit::Type(text) => self.type_in(&text),
            Edit::Move {
                way,
                reach,
                choosing,
            } => self.travel(way, reach, choosing),
            Edit::Remove { way, reach } => self.remove(way, reach),
            Edit::All => {
                self.now.anchor = 0;
                self.now.cursor = self.now.text.len();
                self.typing = false;
            }
            Edit::Undo => self.back_to(Way::Back),
            Edit::Redo => self.back_to(Way::On),
        }
        self.now != before
    }

    /// Takes out what is chosen, and says what it was: as Ctrl and X do.
    pub fn cut(&mut self) -> Option<String> {
        let taken = self.chosen_text().map(str::to_owned);
        let cut = Edit::Remove {
            way: Way::Back,
            reach: Reach::Character,
        };
        taken.inspect(|_| {
            self.edit(cut);
        })
    }

    /// Puts the cursor at `at` bytes from the start, or the nearest
    /// boundary of a character before it, as a click does; or, if
    /// `choosing`, chooses from where the cursor was to there, as a drag
    /// does.
    pub fn place(&mut self, at: usize, choosing: bool) -> bool {
        let before = self.now.clone();
        self.now.cursor = self.boundary(at);
        if !choosing {
            self.now.anchor = self.now.cursor;
        }
        self.typing = false;
        self.now != before
    }

    /// Where each character begins, in bytes from the start, and where the
    /// last ends: the places the cursor can be.
    pub fn boundaries(&self) -> impl Iterator<Item = usize> + '_ {
        let starts = self.now.text.grapheme_indices(true).map(|(at, _)| at);
        starts.chain([self.now.text.len()])
    }

    /// The boundary of a character at `at` bytes from the start, or the
    /// nearest before it.
    fn boundary(&self, at: usize) -> usize {
        let before = self.boundaries().take_while(|start| *start <= at);
        before.last().unwrap_or(0)
    }

    /// Types `text` in place of what is chosen, as much of it as there is
    /// room for, and without what cannot be shown.
    fn type_in(&mut self, text: &str) {
        let shown = text.chars().filter(|character| !character.is_control());
        let chosen = self.chosen();
        let kept = self.now.text.chars().count() - self.now.text[chosen.clone()].chars().count();
        let typed: String = shown.take(self.longest.saturating_sub(kept)).collect();
        if typed.is_empty() && chosen.is_empty() {
            return;
        }
        if !self.typing || !chosen.is_empty() {
            self.keep();
        }
        self.now.text.replace_range(chosen.clone(), &typed);
        self.now.cursor = chosen.start + typed.len();
        self.now.anchor = self.now.cursor;
        self.typing = true;
    }

    /// Moves the cursor, choosing as it goes or not.
    fn travel(&mut self, way: Way, reach: Reach, choosing: bool) {
        let chosen = self.chosen();
        // With something chosen, an arrow alone goes to its end that way,
        // as GTK's fields have it.
        let to = if !choosing && !chosen.is_empty() && reach == Reach::Character {
            match way {
                Way::Back => chosen.start,
                Way::On => chosen.end,
            }
        } else {
            self.reached(way, reach)
        };
        self.now.cursor = to;
        if !choosing {
            self.now.anchor = to;
        }
        self.typing = false;
    }

    /// Takes out what is chosen, or else as far as `reach` goes `way`.
    fn remove(&mut self, way: Way, reach: Reach) {
        let chosen = self.chosen();
        let taken = if chosen.is_empty() {
            let to = self.reached(way, reach);
            to.min(self.now.cursor)..to.max(self.now.cursor)
        } else {
            chosen
        };
        if !taken.is_empty() {
            self.keep();
            self.now.text.replace_range(taken.clone(), "");
            self.now.cursor = taken.start;
            self.now.anchor = taken.start;
        }
        self.typing = false;
    }

    /// Where the cursor would go, `reach` one `way` from where it is.
    fn reached(&self, way: Way, reach: Reach) -> usize {
        let (text, cursor) = (&self.now.text, self.now.cursor);
        let word = |part: &str| part.chars().any(char::is_alphanumeric);
        match (way, reach) {
            (Way::Back, Reach::Character) => {
                let before = text[..cursor].grapheme_indices(true).next_back();
                before.map_or(0, |(at, _)| at)
            }
            (Way::On, Reach::Character) => {
                let after = text[cursor..].graphemes(true).next();
                after.map_or(cursor, |character| cursor + character.len())
            }
            // To the start of the word the cursor is in or after.
            (Way::Back, Reach::Word) => {
                let parts = text[..cursor].split_word_bound_indices();
                let mut words = parts.filter(|(_, part)| word(part));
                words.next_back().map_or(0, |(at, _)| at)
            }
            // To the end of the word the cursor is in or before.
            (Way::On, Reach::Word) => {
                let parts = text[cursor..].split_word_bound_indices();
                let mut words = parts.filter(|(_, part)| word(part));
                let next = words.next();
                next.map_or(text.len(), |(at, part)| cursor + at + part.len())
            }
            (Way::Back, Reach::Line) => 0,
            (Way::On, Reach::Line) => text.len(),
        }
    }

    /// Keeps how it is now, to be undone to, and forgets what was undone.
    fn keep(&mut self) {
        if self.undone.len() == KEPT {
            self.undone.remove(0);
        }
        self.undone.push(self.now.clone());
        self.redone.clear();
    }

    /// Goes back to how it was before the last change, `Way::Back`, or on
    /// to how it was after the last one undone.
    fn back_to(&mut self, way: Way) {
        let (from, to) = match way {
            Way::Back => (&mut self.undone, &mut self.redone),
            Way::On => (&mut self.redone, &mut self.undone),
        };
        if let Some(state) = from.pop() {
            to.push(std::mem::replace(&mut self.now, state));
        }
        self.typing = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(text: &str) -> Field {
        let mut field = Field::new(200);
        field.edit(Edit::Type(text.to_owned()));
        field
    }

    fn going(way: Way, reach: Reach) -> Edit {
        Edit::Move {
            way,
            reach,
            choosing: false,
        }
    }

    fn choosing(way: Way, reach: Reach) -> Edit {
        Edit::Move {
            way,
            reach,
            choosing: true,
        }
    }

    fn removing(way: Way, reach: Reach) -> Edit {
        Edit::Remove { way, reach }
    }

    /// The text with `|` where the cursor is, and `[` and `]` round what is
    /// chosen.
    fn shown(field: &Field) -> String {
        let chosen = field.chosen();
        let mut shown = field.text().to_owned();
        if chosen.is_empty() {
            shown.insert(field.cursor(), '|');
        } else {
            shown.insert(chosen.end, ']');
            shown.insert(chosen.start, '[');
        }
        shown
    }

    #[test]
    fn typing_goes_where_the_cursor_is_and_the_arrows_move_it_a_character_at_a_time() {
        let mut field = field("firfox");
        assert_eq!(shown(&field), "firfox|");
        assert!(field.edit(going(Way::Back, Reach::Character)));
        field.edit(going(Way::Back, Reach::Character));
        field.edit(going(Way::Back, Reach::Character));
        field.edit(Edit::Type("e".to_owned()));
        assert_eq!(shown(&field), "fire|fox");
        field.edit(going(Way::On, Reach::Line));
        assert!(!field.edit(going(Way::On, Reach::Character)), "at the end");
        field.edit(going(Way::Back, Reach::Line));
        assert!(
            !field.edit(going(Way::Back, Reach::Character)),
            "at the start"
        );
        assert_eq!(shown(&field), "|firefox");
    }

    #[test]
    fn a_character_is_one_however_many_code_points_it_is_written_with() {
        // An e with an accent written after it, and a flag of two letters.
        let mut field = field("ae\u{301}b 🇵🇱");
        field.edit(removing(Way::Back, Reach::Character));
        assert_eq!(field.text(), "ae\u{301}b ");
        for _ in 0..3 {
            field.edit(going(Way::Back, Reach::Character));
        }
        assert_eq!(field.cursor(), 1, "before the accented e, whole");
        field.edit(removing(Way::On, Reach::Character));
        assert_eq!(field.text(), "ab ");
        // A click in the middle of one is before it.
        let mut flag = self::field("🇵🇱");
        flag.place(4, false);
        assert_eq!(flag.cursor(), 0);
    }

    #[test]
    fn ctrl_or_alt_with_an_arrow_or_backspace_goes_by_words() {
        let mut field = field("open the  file.txt now");
        field.edit(going(Way::Back, Reach::Word));
        assert_eq!(shown(&field), "open the  file.txt |now");
        field.edit(going(Way::Back, Reach::Word));
        assert_eq!(
            shown(&field),
            "open the  |file.txt now",
            "a name with a dot is a word"
        );
        field.edit(going(Way::Back, Reach::Word));
        assert_eq!(shown(&field), "open |the  file.txt now", "over spaces");
        field.edit(going(Way::On, Reach::Word));
        assert_eq!(shown(&field), "open the|  file.txt now");
        field.edit(going(Way::On, Reach::Line));
        field.edit(removing(Way::Back, Reach::Word));
        assert_eq!(shown(&field), "open the  file.txt |");
        field.edit(going(Way::Back, Reach::Line));
        field.edit(removing(Way::On, Reach::Word));
        assert_eq!(shown(&field), "| the  file.txt ");
        // With no word left that way, as far as it goes.
        field.edit(going(Way::Back, Reach::Word));
        assert_eq!(field.cursor(), 0);
        let mut spaces = self::field("a   ");
        spaces.edit(going(Way::Back, Reach::Line));
        spaces.edit(going(Way::On, Reach::Character));
        spaces.edit(going(Way::On, Reach::Word));
        assert_eq!(shown(&spaces), "a   |", "to the end, past no word");
    }

    #[test]
    fn shift_chooses_what_the_cursor_goes_over_and_typing_takes_its_place() {
        let mut field = field("hello world");
        field.edit(choosing(Way::Back, Reach::Word));
        assert_eq!(shown(&field), "hello [world]");
        assert_eq!(field.chosen_text(), Some("world"));
        field.edit(choosing(Way::Back, Reach::Character));
        assert_eq!(shown(&field), "hello[ world]");
        field.edit(Edit::Type("!".to_owned()));
        assert_eq!(shown(&field), "hello!|");
        assert_eq!(field.chosen_text(), None);
        // An arrow alone goes to the end of what is chosen, that way.
        field.edit(Edit::All);
        assert_eq!(shown(&field), "[hello!]");
        field.edit(going(Way::Back, Reach::Character));
        assert_eq!(shown(&field), "|hello!");
        field.edit(Edit::All);
        field.edit(going(Way::On, Reach::Character));
        assert_eq!(shown(&field), "hello!|");
        // And by a word, from the cursor.
        field.edit(choosing(Way::Back, Reach::Character));
        field.edit(going(Way::Back, Reach::Word));
        assert_eq!(shown(&field), "|hello!");
    }

    #[test]
    fn backspace_and_delete_take_out_what_is_chosen() {
        let mut field = field("one two three");
        field.edit(going(Way::Back, Reach::Word));
        field.edit(choosing(Way::Back, Reach::Word));
        field.edit(choosing(Way::Back, Reach::Character));
        assert_eq!(field.chosen_text(), Some(" two "));
        field.edit(removing(Way::On, Reach::Word));
        assert_eq!(shown(&field), "one|three");
        // Nothing to take out takes nothing.
        field.edit(going(Way::Back, Reach::Line));
        assert!(!field.edit(removing(Way::Back, Reach::Character)));
        // Home and End with Shift, and Backspace by all of it.
        field.edit(choosing(Way::On, Reach::Line));
        assert_eq!(field.chosen_text(), Some("onethree"));
        field.edit(going(Way::On, Reach::Line));
        field.edit(removing(Way::Back, Reach::Line));
        assert_eq!(field.text(), "");
    }

    #[test]
    fn undo_puts_back_each_change_and_redo_does_it_again() {
        let mut field = Field::new(200);
        for character in ["c", "a", "t"] {
            field.edit(Edit::Type(character.to_owned()));
        }
        field.edit(Edit::Type(" ".to_owned()));
        field.edit(going(Way::Back, Reach::Character));
        field.edit(Edit::Type("s".to_owned()));
        assert_eq!(field.text(), "cats ");
        assert!(field.edit(Edit::Undo));
        assert_eq!(
            shown(&field),
            "cat| ",
            "typing after a move is a change of its own"
        );
        assert!(field.edit(Edit::Undo));
        assert_eq!(shown(&field), "|", "what was typed together goes together");
        assert!(!field.edit(Edit::Undo), "nothing before that");
        field.edit(Edit::Redo);
        field.edit(Edit::Redo);
        assert_eq!(field.text(), "cats ");
        assert!(!field.edit(Edit::Redo), "nothing undone");
        // A change after an undo forgets what was undone.
        field.edit(Edit::Undo);
        field.edit(removing(Way::Back, Reach::Character));
        assert!(!field.edit(Edit::Redo));
        assert_eq!(field.text(), "ca ");
    }

    #[test]
    fn only_so_many_changes_are_kept_to_undo() {
        let mut field = Field::new(1000);
        for _ in 0..KEPT + 5 {
            field.edit(Edit::Type("x".to_owned()));
            field.edit(going(Way::Back, Reach::Line));
        }
        let mut undone = 0;
        while field.edit(Edit::Undo) {
            undone += 1;
        }
        assert_eq!(undone, KEPT);
        assert_eq!(field.text().len(), 5);
    }

    #[test]
    fn it_takes_no_more_than_it_has_room_for_nor_what_cannot_be_shown() {
        let mut field = Field::new(5);
        field.edit(Edit::Type("ab\u{7}c\ndefgh".to_owned()));
        assert_eq!(field.text(), "abcde");
        assert!(!field.edit(Edit::Type("x".to_owned())), "full");
        assert!(!field.edit(Edit::Type("\u{7}".to_owned())));
        // In place of what is chosen, as much as was chosen and no more.
        field.edit(choosing(Way::Back, Reach::Character));
        field.edit(choosing(Way::Back, Reach::Character));
        field.edit(Edit::Type("XYZ".to_owned()));
        assert_eq!(field.text(), "abcXY");
        // Nothing typable in place of what is chosen takes it out.
        field.edit(Edit::All);
        assert!(field.edit(Edit::Type("\u{7}".to_owned())));
        assert_eq!(field.text(), "");
    }

    #[test]
    fn a_click_puts_the_cursor_and_a_drag_chooses() {
        let mut field = field("launcher");
        assert!(field.place(3, false));
        assert_eq!(shown(&field), "lau|ncher");
        assert!(field.place(6, true));
        assert_eq!(field.chosen_text(), Some("nch"));
        assert!(!field.place(6, true), "where it was");
        field.place(99, false);
        assert_eq!(field.cursor(), 8, "no further than the end");
        assert_eq!(field.boundaries().count(), 9);
    }

    #[test]
    fn cut_takes_out_what_is_chosen_and_says_what() {
        let mut field = field("copy this");
        assert_eq!(field.cut(), None, "nothing chosen");
        field.edit(choosing(Way::Back, Reach::Word));
        assert_eq!(field.cut().as_deref(), Some("this"));
        assert_eq!(field.text(), "copy ");
        field.edit(Edit::Undo);
        assert_eq!(field.text(), "copy this");
    }
}
