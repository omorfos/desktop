//! Picking a program: what has been typed, which programs match, which of
//! them would be started, how all that is drawn and what a key does to it.
//!
//! Nothing here knows of Wayland or of sockets.

use omorfos_config::{Command, Edges, Launcher as Show, Part, State, States, Theme};
use omorfos_ipc::Request;
use omorfos_ui::{Area, Canvas, Motion, Scale, Text, inset};
use serde_json::json;

use crate::{App, Icons};

/// How many programs are listed at once.
const ROWS: usize = 8;

/// The most that can be typed, in characters. No program's name is longer.
const LONGEST: usize = 64;

/// The room between a program's icon, its name and what it says of itself.
const GAP: u32 = 10;

/// The narrowest the launcher is made, whatever the theme says.
const NARROWEST: u32 = 160;

/// A key, as the launcher understands keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    /// Something to add to what has been typed.
    Text(String),
    /// Take the last character back.
    Backspace,
    /// Choose the program above.
    Up,
    /// Choose the program below.
    Down,
    /// Start the chosen program.
    Enter,
    /// Go away without starting anything.
    Escape,
}

/// What comes of a key or a click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The launcher stays, perhaps looking different.
    Stay,
    /// The launcher is done with and nothing is started.
    Close,
    /// The compositor is asked for this, which starts the program, and the
    /// launcher is done with.
    Start(Request),
}

/// The launcher's state: the programs there are, what has been typed, and
/// which of those that match is chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picker {
    apps: Vec<App>,
    /// What is shown of each program besides its name.
    show: Show,
    query: String,
    /// The programs that match, best first, as places in `apps`.
    found: Vec<usize>,
    /// Which of `found` would be started.
    chosen: usize,
}

/// Where the launcher's boxes are, under a theme.
struct Layout {
    whole: Area,
    input: Area,
    /// The room the list has: its left, its top and its width.
    list: (i32, i32, u32),
    /// How far apart rows are, with the room the theme keeps around each.
    pitch: u32,
}

impl Layout {
    fn new(theme: &Theme) -> Self {
        let launcher = theme.style(Part::Launcher, States::NONE);
        let input = theme.style(Part::LauncherInput, States::NONE);
        let item = theme.style(Part::LauncherItem, States::NONE);
        let pitch = item.height.saturating_add(item.margin.down());
        let rows = u32::try_from(ROWS).unwrap_or(u32::MAX);
        let field = input.height.saturating_add(input.margin.down());
        let height = rows.saturating_mul(pitch).saturating_add(field);
        let height = height.saturating_add(launcher.inset().down());
        let whole = (0, 0, launcher.min_width.max(NARROWEST), height);
        let (left, top, width, _) = inset(whole, launcher.inset());
        let (x, y, wide, _) = inset((left, top, width, field), input.margin);
        Self {
            whole,
            input: (x, y, wide, input.height),
            list: (left, top.saturating_add_unsigned(field), width),
            pitch,
        }
    }

    /// The room for the row that is `place` rows down the list.
    fn row(&self, place: usize) -> Area {
        let (left, top, width) = self.list;
        let down = u32::try_from(place)
            .unwrap_or(u32::MAX)
            .saturating_mul(self.pitch);
        (left, top.saturating_add_unsigned(down), width, self.pitch)
    }
}

impl Picker {
    /// A launcher offering `apps`, in the order they are given, showing
    /// of each what `show` says.
    #[must_use]
    pub fn new(apps: Vec<App>, show: Show) -> Self {
        let found = (0..apps.len()).collect();
        Self {
            apps,
            show,
            query: String::new(),
            found,
            chosen: 0,
        }
    }

    /// How large the launcher is under `theme`, in units of layout. It
    /// does not depend on how many programs match.
    #[must_use]
    pub fn size(theme: &Theme) -> (u32, u32) {
        let (_, _, width, height) = Layout::new(theme).whole;
        (width, height)
    }

    /// What has been typed.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// The names of the programs that match what has been typed, best
    /// first.
    pub fn matches(&self) -> impl Iterator<Item = &str> {
        self.listed().map(|app| app.name.as_str())
    }

    /// The programs that match what has been typed, best first.
    fn listed(&self) -> impl Iterator<Item = &App> {
        self.found.iter().filter_map(|place| self.apps.get(*place))
    }

    /// The name of the program that would be started, if any matches.
    #[must_use]
    pub fn chosen(&self) -> Option<&str> {
        self.matches().nth(self.chosen)
    }

    /// Takes a key.
    pub fn key(&mut self, key: Key) -> Outcome {
        match key {
            Key::Text(text) => {
                let room = LONGEST.saturating_sub(self.query.chars().count());
                let typed = text.chars().filter(|character| !character.is_control());
                self.query.extend(typed.take(room));
                self.search();
            }
            Key::Backspace => {
                self.query.pop();
                self.search();
            }
            Key::Up => self.chosen = self.chosen.saturating_sub(1),
            Key::Down => {
                let last = self.found.len().saturating_sub(1);
                self.chosen = (self.chosen + 1).min(last);
            }
            Key::Enter => return self.start(),
            Key::Escape => return Outcome::Close,
        }
        Outcome::Stay
    }

    /// Chooses the program whose row a point of the launcher is on, if it
    /// is on one, as when the pointer moves over it. Returns whether that
    /// changed anything.
    pub fn point(&mut self, x: i32, y: i32, theme: &Theme) -> bool {
        let over = self.row_at(x, y, theme).filter(|row| *row != self.chosen);
        self.chosen = over.unwrap_or(self.chosen);
        over.is_some()
    }

    /// Takes a click at a point of the launcher: on a program's row it
    /// starts that program, and anywhere else it does nothing.
    pub fn click(&mut self, x: i32, y: i32, theme: &Theme) -> Outcome {
        let over = self.row_at(x, y, theme);
        self.chosen = over.unwrap_or(self.chosen);
        over.map_or(Outcome::Stay, |_| self.start())
    }

    /// Draws the launcher, with `scale` pixels to a unit of layout. The
    /// icons of the programs in sight are looked up in `icons`, if icons
    /// are shown. A row that has just been chosen, or has just stopped
    /// being, is as far on its way to its new colours as `motion` has it.
    #[must_use]
    pub fn paint(
        &self,
        text: &mut Text,
        theme: &Theme,
        scale: Scale,
        icons: &mut Icons,
        motion: &mut Motion,
    ) -> Canvas {
        let layout = Layout::new(theme);
        let (_, _, width, height) = layout.whole;
        let mut canvas = Canvas::scaled(width, height, scale);
        canvas.draw_box(layout.whole, &theme.style(Part::Launcher, States::NONE));

        let input = theme.style(Part::LauncherInput, States::NONE);
        let font = theme.font(Part::LauncherInput, States::NONE);
        canvas.draw_box(layout.input, &input);
        let typed = inset(layout.input, input.inset());
        text.draw(&mut canvas, &self.query, typed, input.color, font);
        // A line where the next character will go, which stays in the box
        // however much has been typed.
        let (left, top, room, tall) = typed;
        let along = text.measure(&self.query, font).min(room.saturating_sub(1));
        let margin = tall / 5;
        let caret = (
            left.saturating_add_unsigned(along),
            top.saturating_add_unsigned(margin),
            1,
            tall.saturating_sub(margin * 2),
        );
        canvas.fill(caret, input.color);

        let listed = self.listed().enumerate().skip(self.first()).take(ROWS);
        for (place, (row, app)) in listed.enumerate() {
            let states = States::NONE.when(State::Active, row == self.chosen);
            // A row is the same row whichever program is in it.
            let key = u64::try_from(row).unwrap_or(u64::MAX);
            let style = motion.style(theme, Part::LauncherItem, key, states);
            let area = inset(layout.row(place), style.margin);
            canvas.draw_box(area, &style);
            let mut room = inset(area, style.inset());
            if self.show.icons {
                // A square somewhat shorter than the row, level with the
                // middle of it.
                let side = room.3 * 3 / 4;
                let down = (room.3 - side) / 2;
                let square = (room.0, room.1.saturating_add_unsigned(down), side, side);
                let pixels = scale.pixels(side.cast_signed()).unsigned_abs();
                if let Some(icon) = icons.get(&app.icon, pixels, style.color) {
                    canvas.draw_image(square, icon);
                }
                room = inset(room, on_the_left(side.saturating_add(GAP)));
            }
            let font = theme.font(Part::LauncherItem, states);
            text.draw(&mut canvas, &app.name, room, style.color, font);
            if self.show.descriptions {
                let named = text.measure(&app.name, font).saturating_add(GAP);
                let rest = inset(room, on_the_left(named.min(room.2)));
                let said = motion.style(theme, Part::LauncherDescription, key, states);
                let ink = said.color;
                let font = theme.font(Part::LauncherDescription, states);
                text.draw(&mut canvas, &app.description, rest, ink, font);
            }
        }
        canvas
    }

    /// The first program listed: the list moves down only as far as it
    /// takes to keep the chosen one in sight.
    fn first(&self) -> usize {
        (self.chosen + 1).saturating_sub(ROWS)
    }

    /// Which of the programs that match is listed at a point, if any is.
    fn row_at(&self, x: i32, y: i32, theme: &Theme) -> Option<usize> {
        let layout = Layout::new(theme);
        let (left, top, width) = layout.list;
        let across = u32::try_from(x.saturating_sub(left)).ok();
        let down = u32::try_from(y.saturating_sub(top)).ok();
        let inside = across.zip(down).filter(|(across, _)| *across < width);
        let place = inside.and_then(|(_, down)| down.checked_div(layout.pitch));
        let place = place.and_then(|place| usize::try_from(place).ok());
        let row = place
            .filter(|place| *place < ROWS)
            .map(|place| self.first() + place);
        row.filter(|row| *row < self.found.len())
    }

    /// Finds the programs that match what has been typed: those whose name
    /// begins with it first, then those that have it anywhere in their
    /// name, then, if what programs say of themselves is shown, those that
    /// have it there. Each lot is in the order the programs came in. The
    /// first of all is chosen.
    fn search(&mut self) {
        let query = self.query.to_lowercase();
        let described = self.show.descriptions;
        // How well each program matches: the lower the better.
        let rank = |app: &App| {
            let name = app.name.to_lowercase();
            let says = described && app.description.to_lowercase().contains(&query);
            let ranks = [name.starts_with(&query), name.contains(&query), says];
            ranks.into_iter().position(|matches| matches)
        };
        let ranked = self.apps.iter().enumerate();
        let ranked = ranked.filter_map(|(place, app)| rank(app).map(|rank| (rank, place)));
        let mut ranked: Vec<(usize, usize)> = ranked.collect();
        // The sort is stable, so each lot stays in the order it came in.
        ranked.sort_by_key(|(rank, _)| *rank);
        self.found = ranked.into_iter().map(|(_, place)| place).collect();
        self.chosen = 0;
    }

    /// Asks for the chosen program to be started, if there is one.
    fn start(&self) -> Outcome {
        let app = self
            .found
            .get(self.chosen)
            .and_then(|place| self.apps.get(*place));
        app.map_or(Outcome::Stay, |app| {
            // One that needs a terminal is started in the one the built-in
            // bindings start.
            let terminal = app.terminal.then(Command::terminal);
            let terminal = terminal.iter().map(Command::program);
            let command: Vec<&str> = terminal
                .chain(app.command.iter().map(String::as_str))
                .collect();
            Outcome::Start(Request::Perform {
                action: json!({ "spawn": command }),
            })
        })
    }
}

/// Room taken from the left of a box and from nowhere else.
const fn on_the_left(room: u32) -> Edges<u32> {
    Edges {
        left: room,
        ..Edges::NONE
    }
}

#[cfg(test)]
mod tests {
    use omorfos_config::Color;

    use super::*;

    fn app(name: &str) -> App {
        App {
            name: name.to_owned(),
            command: vec![name.to_lowercase(), "--go".to_owned()],
            terminal: false,
            description: format!("A {} of sorts", name.to_lowercase()),
            icon: name.to_lowercase(),
        }
    }

    fn picker(names: &[&str]) -> Picker {
        let apps = names.iter().map(|name| app(name)).collect();
        Picker::new(apps, Show::default())
    }

    /// The launcher's picture under the built-in theme.
    fn painted(picker: &Picker) -> Canvas {
        let (theme, mut icons) = (Theme::default(), Icons::default());
        still(picker, &theme, Scale::ONE, &mut icons)
    }

    /// The launcher's picture with nothing on its way anywhere.
    fn still(picker: &Picker, theme: &Theme, scale: Scale, icons: &mut Icons) -> Canvas {
        picker.paint(&mut Text::new(), theme, scale, icons, &mut Motion::new())
    }

    fn several() -> Picker {
        picker(&["Calculator", "Files", "Firefox", "Terminal", "Text Editor"])
    }

    fn typed(picker: &mut Picker, text: &str) {
        assert_eq!(picker.key(Key::Text(text.to_owned())), Outcome::Stay);
    }

    fn names_of(picker: &Picker) -> Vec<&str> {
        picker.matches().collect()
    }

    fn spawn(command: &[&str]) -> Outcome {
        Outcome::Start(Request::Perform {
            action: json!({ "spawn": command }),
        })
    }

    fn argb(color: Color) -> u32 {
        u32::from_be_bytes([color.alpha, color.red, color.green, color.blue])
    }

    /// The colour a row is filled with when it is the chosen one.
    fn lit(theme: &Theme) -> u32 {
        let chosen = States::NONE.with(State::Active);
        argb(theme.style(Part::LauncherItem, chosen).background)
    }

    /// The middle of the row `place` rows down the list, under the
    /// built-in theme.
    fn row(place: i32) -> (i32, i32) {
        let layout = Layout::new(&Theme::default());
        let (left, top, width) = layout.list;
        let pitch = layout.pitch.cast_signed();
        (
            left + (width / 2).cast_signed(),
            top + pitch * place + pitch / 2,
        )
    }

    #[test]
    fn every_program_is_offered_until_something_is_typed() {
        let picker = several();
        assert_eq!(picker.query(), "");
        assert_eq!(names_of(&picker).len(), 5);
        assert_eq!(picker.chosen(), Some("Calculator"));
    }

    #[test]
    fn typing_leaves_the_programs_whose_names_have_what_was_typed() {
        let mut picker = several();
        typed(&mut picker, "t");
        // Those that begin with it, then those that have it.
        assert_eq!(names_of(&picker), ["Terminal", "Text Editor", "Calculator"]);
        typed(&mut picker, "E");
        assert_eq!(picker.query(), "tE");
        assert_eq!(
            names_of(&picker),
            ["Terminal", "Text Editor"],
            "whatever the case"
        );
        typed(&mut picker, "x");
        assert_eq!(names_of(&picker), ["Text Editor"]);
        typed(&mut picker, "q");
        assert_eq!(names_of(&picker), [""; 0]);
        assert_eq!(picker.chosen(), None);
    }

    #[test]
    fn backspace_takes_back_what_was_typed() {
        let mut picker = several();
        typed(&mut picker, "fi");
        assert_eq!(names_of(&picker), ["Files", "Firefox"]);
        typed(&mut picker, "r");
        assert_eq!(names_of(&picker), ["Firefox"]);
        assert_eq!(picker.key(Key::Backspace), Outcome::Stay);
        assert_eq!(names_of(&picker), ["Files", "Firefox"]);
        for _ in 0..5 {
            picker.key(Key::Backspace);
        }
        assert_eq!(picker.query(), "");
        assert_eq!(names_of(&picker).len(), 5);
    }

    #[test]
    fn only_so_much_can_be_typed_and_nothing_that_is_not_a_character() {
        let mut picker = several();
        typed(&mut picker, "a\u{8}b\n\u{1b}c");
        assert_eq!(picker.query(), "abc");
        typed(&mut picker, &"x".repeat(500));
        assert_eq!(picker.query().chars().count(), LONGEST);
        typed(&mut picker, "y");
        assert_eq!(picker.query().chars().count(), LONGEST);
    }

    #[test]
    fn the_arrows_choose_among_the_programs_that_match() {
        let mut picker = several();
        assert_eq!(picker.key(Key::Up), Outcome::Stay);
        assert_eq!(picker.chosen(), Some("Calculator"), "there is none above");
        picker.key(Key::Down);
        picker.key(Key::Down);
        assert_eq!(picker.chosen(), Some("Firefox"));
        picker.key(Key::Up);
        assert_eq!(picker.chosen(), Some("Files"));
        for _ in 0..10 {
            picker.key(Key::Down);
        }
        assert_eq!(picker.chosen(), Some("Text Editor"), "nor any below");
        // Typing starts again from the best match.
        typed(&mut picker, "f");
        assert_eq!(picker.chosen(), Some("Files"));
        // With nothing matching there is nothing to move among.
        typed(&mut picker, "zzz");
        picker.key(Key::Down);
        assert_eq!(picker.chosen(), None);
    }

    #[test]
    fn enter_asks_for_the_chosen_program_to_be_started() {
        let mut picker = several();
        typed(&mut picker, "fire");
        assert_eq!(picker.key(Key::Enter), spawn(&["firefox", "--go"]));
        let mut picker = several();
        picker.key(Key::Down);
        assert_eq!(picker.key(Key::Enter), spawn(&["files", "--go"]));
        // With nothing matching, Enter does nothing.
        typed(&mut picker, "zzz");
        assert_eq!(picker.key(Key::Enter), Outcome::Stay);
    }

    #[test]
    fn a_program_that_needs_a_terminal_is_started_in_one() {
        let top = App {
            terminal: true,
            ..app("top")
        };
        let mut picker = Picker::new(vec![top], Show::default());
        let terminal = Command::terminal();
        assert_eq!(
            picker.key(Key::Enter),
            spawn(&[terminal.program(), "top", "--go"])
        );
    }

    #[test]
    fn escape_goes_away_without_starting_anything() {
        let mut picker = several();
        typed(&mut picker, "fire");
        assert_eq!(picker.key(Key::Escape), Outcome::Close);
    }

    #[test]
    fn the_pointer_chooses_the_program_it_is_over() {
        let theme = Theme::default();
        let mut picker = several();
        let (x, y) = row(2);
        assert!(picker.point(x, y, &theme));
        assert_eq!(picker.chosen(), Some("Firefox"));
        assert!(!picker.point(x, y + 1, &theme), "it was already");
        // Not where there is no program, nor off the list.
        let (x, below) = row(6);
        assert!(!picker.point(x, below, &theme));
        assert!(!picker.point(x, 2, &theme), "on the text field");
        assert!(!picker.point(-5, y, &theme));
        assert!(!picker.point(5000, y, &theme));
        assert!(!picker.point(x, 5000, &theme));
        assert_eq!(picker.chosen(), Some("Firefox"));
    }

    #[test]
    fn a_click_on_a_program_starts_it() {
        let theme = Theme::default();
        let mut picker = several();
        let (x, y) = row(3);
        assert_eq!(picker.click(x, y, &theme), spawn(&["terminal", "--go"]));
        assert_eq!(
            picker.click(x, 2, &theme),
            Outcome::Stay,
            "on the text field"
        );
        let (x, below) = row(7);
        assert_eq!(picker.click(x, below, &theme), Outcome::Stay);
    }

    #[test]
    fn the_launcher_is_as_large_as_the_theme_makes_it_whatever_matches() {
        let theme = Theme::default();
        // A border and padding each side, the text field with the room
        // under it, and eight rows.
        assert_eq!(Picker::size(&theme), (480, 2 * 9 + 32 + 8 + 8 * 28));
        let wide = Theme::parse(
            "launcher { min-width: 600px; padding: 0; border: none; }\n\
             launcher-input { margin: 0; height: 40px; }\n\
             launcher-item { height: 20px; margin: 1px 0; }",
        );
        assert_eq!(Picker::size(&wide.unwrap()), (600, 40 + 8 * 22));
        let narrow = Theme::parse("launcher { min-width: 10px; }").unwrap();
        assert_eq!(Picker::size(&narrow).0, NARROWEST);

        let mut picker = several();
        let canvas = painted(&picker);
        assert_eq!((canvas.width(), canvas.height()), Picker::size(&theme));
        typed(&mut picker, "zzz");
        let canvas = still(&picker, &theme, Scale::new(2.0), &mut Icons::default());
        let (width, height) = Picker::size(&theme);
        assert_eq!((canvas.width(), canvas.height()), (width * 2, height * 2));
    }

    #[test]
    fn the_chosen_program_is_lit_and_no_other() {
        let theme = Theme::default();
        let mut picker = several();
        picker.key(Key::Down);
        let canvas = painted(&picker);
        let at = |place: i32| {
            let (x, y) = row(place);
            // Towards the right-hand end, clear of the name.
            canvas.at(x.unsigned_abs() + 200, y.unsigned_abs())
        };
        let surface = argb(theme.style(Part::Launcher, States::NONE).background);
        assert_eq!(at(1), lit(&theme));
        assert_eq!(at(0), surface);
        assert_eq!(at(2), surface);
        assert_eq!(at(6), surface, "no program there");
    }

    #[test]
    fn what_is_typed_is_shown_with_a_mark_where_the_next_character_goes() {
        let theme = Theme::default();
        let layout = Layout::new(&theme);
        let input = theme.style(Part::LauncherInput, States::NONE);
        let (left, top, width, height) = layout.input;
        let field = argb(input.background);
        let inked = |picker: &Picker| {
            let canvas = painted(picker);
            let columns = left.unsigned_abs()..left.unsigned_abs() + width;
            let middle = top.unsigned_abs() + height / 2;
            let inked = columns.filter(|x| canvas.at(*x, middle) != field);
            inked.collect::<Vec<u32>>()
        };
        let mut picker = several();
        let empty = inked(&picker);
        // Only the mark, at the start of the field's padding.
        assert_eq!(empty, [left.unsigned_abs() + input.padding.left]);
        typed(&mut picker, "fire");
        let some = inked(&picker);
        assert!(some.len() > 1 && some.last() > empty.last(), "{some:?}");
        // However much is typed, the mark stays in the field.
        typed(&mut picker, &"w".repeat(60));
        let full = inked(&picker);
        let end = left.unsigned_abs() + width - input.padding.right;
        assert!(full.last().unwrap() < &end, "{full:?} within {end}");
    }

    #[test]
    fn what_programs_say_of_themselves_is_shown_and_searched_only_if_asked_for() {
        let names = ["Calculator", "Files", "Sorter"];
        let plain = picker(&names);
        let show = Show {
            descriptions: true,
            ..Show::default()
        };
        let mut described = Picker::new(names.iter().map(|name| app(name)).collect(), show);

        // After each name there is more written than without.
        let y = row(1).1.unsigned_abs();
        let inked = |picker: &Picker| {
            let canvas = painted(picker);
            let surface = canvas.at(canvas.width() - 20, y);
            let columns = 0..canvas.width();
            columns
                .filter(|column| canvas.at(*column, y) != surface)
                .count()
        };
        assert!(inked(&described) > inked(&plain) + 20);

        // "sort" is in what every one of them says, and in one's name,
        // which comes first.
        typed(&mut described, "sort");
        assert_eq!(names_of(&described), ["Sorter", "Calculator", "Files"]);
        let mut plain = plain;
        typed(&mut plain, "sort");
        assert_eq!(names_of(&plain), ["Sorter"]);
    }

    #[test]
    fn icons_are_shown_beside_the_names_only_if_asked_for() {
        let root = tempfile::TempDir::new().unwrap();
        let green = "<svg xmlns='http://www.w3.org/2000/svg' width='8' height='8'>\
                     <rect width='8' height='8' fill='#00ff00'/></svg>";
        std::fs::write(root.path().join("files.svg"), green).unwrap();
        let mut icons = Icons::new(vec![root.path().to_owned()]);
        let theme = Theme::default();
        let names = ["Calculator", "Files"];
        let show = Show {
            icons: true,
            ..Show::default()
        };
        let with = Picker::new(names.iter().map(|name| app(name)).collect(), show);
        let canvas = still(&with, &theme, Scale::ONE, &mut icons);
        let green = |canvas: &Canvas, place: i32| {
            let y = row(place).1.unsigned_abs();
            let columns = 0..canvas.width();
            columns.filter(|x| canvas.at(*x, y) == 0xff00_ff00).count()
        };
        // Files has an icon, three quarters of the row across; Calculator
        // has none, and room is left where it would be.
        assert_eq!(green(&canvas, 1), 21);
        assert_eq!(green(&canvas, 0), 0);
        let layout = Layout::new(&theme);
        let item = theme.style(Part::LauncherItem, States::NONE);
        let start = layout.list.0.unsigned_abs() + item.padding.left;
        let y = row(0).1.unsigned_abs();
        let chosen = lit(&theme);
        let beside = start..start + 21 + GAP;
        assert!(beside.clone().all(|x| canvas.at(x, y) == chosen));

        // Without icons asked for, none is drawn and the name starts there.
        let without = picker(&names);
        let canvas = still(&without, &theme, Scale::ONE, &mut icons);
        assert_eq!(green(&canvas, 1), 0);
        assert!(beside.into_iter().any(|x| canvas.at(x, y) != chosen));
    }

    #[test]
    fn a_long_list_moves_to_keep_the_chosen_program_in_sight() {
        let theme = Theme::default();
        let many: Vec<String> = (0..20).map(|n| format!("Program {n:02}")).collect();
        let many: Vec<&str> = many.iter().map(String::as_str).collect();
        let mut picker = picker(&many);
        for _ in 0..11 {
            picker.key(Key::Down);
        }
        assert_eq!(picker.chosen(), Some("Program 11"));
        // It is the last of the eight in sight.
        let canvas = painted(&picker);
        let (x, y) = row(7);
        assert_eq!(
            canvas.at(x.unsigned_abs() + 200, y.unsigned_abs()),
            lit(&theme)
        );
        // The pointer on the first row is on the program four above.
        let (x, y) = row(0);
        assert!(picker.point(x, y, &theme));
        assert_eq!(picker.chosen(), Some("Program 04"));
    }
}
