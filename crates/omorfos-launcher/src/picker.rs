//! Picking a program: what has been typed, which programs match, which of
//! them would be started, how all that is drawn and what a key does to it.
//!
//! Nothing here knows of Wayland or of sockets.

use omorfos_config::{Command, Edges, Launcher as Show, Part, State, States, Theme};
use omorfos_ipc::Request;
use omorfos_ui::{Area, Canvas, Motion, Scale, Text, inset};
use serde_json::json;

use crate::{App, Icons, sums};

/// How many rows are listed at most until the launcher is told how tall
/// its screen is.
const ROWS: usize = 8;

/// The most that can be typed, in characters. No program's name is longer.
const LONGEST: usize = 64;

/// The room between a program's icon and its name.
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
    /// The compositor is asked for this, which starts the program or copies
    /// the answer, and the launcher is done with.
    Start(Request),
}

/// The launcher's state: the programs there are, what has been typed, the
/// answer to it if it is a sum, and which of the answer and the programs
/// that match is chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picker {
    apps: Vec<App>,
    /// What is shown of each program besides its name.
    show: Show,
    query: String,
    /// The answer to what has been typed, if it is a sum, and how it is
    /// written on its row.
    answer: Option<(String, String)>,
    /// The programs that match, best first, as places in `apps`.
    found: Vec<usize>,
    /// Which of the rows would be taken: the answer's first, if there is
    /// one, then the programs'.
    chosen: usize,
    /// How many rows are listed at most.
    room: usize,
}

/// A row of the launcher's list.
enum Row<'a> {
    /// The answer to a sum, to copy: as it is copied, and as it is shown.
    Answer(&'a str, &'a str),
    /// A program to start.
    App(&'a App),
}

/// Where the launcher's boxes are, under a theme.
struct Layout {
    /// How wide the launcher is.
    width: u32,
    /// How tall it is with no rows: its text field, and its border and
    /// padding.
    bare: u32,
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
        let field = input.height.saturating_add(input.margin.down());
        let bare = field.saturating_add(launcher.inset().down());
        let whole = (0, 0, launcher.min_width.max(NARROWEST), bare);
        let (left, top, width, _) = inset(whole, launcher.inset());
        let (x, y, wide, _) = inset((left, top, width, field), input.margin);
        Self {
            width: whole.2,
            bare,
            input: (x, y, wide, input.height),
            list: (left, top.saturating_add_unsigned(field), width),
            pitch,
        }
    }

    /// The whole launcher, with `rows` rows.
    fn whole(&self, rows: usize) -> Area {
        let rows = u32::try_from(rows).unwrap_or(u32::MAX);
        let height = self.bare.saturating_add(rows.saturating_mul(self.pitch));
        (0, 0, self.width, height)
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
            answer: None,
            found,
            chosen: 0,
            room: ROWS,
        }
    }

    /// Lists as many rows as fit in half of a screen `height` units tall,
    /// under `theme`, and one at least.
    pub fn fit(&mut self, theme: &Theme, height: u32) {
        let layout = Layout::new(theme);
        let room = (height / 2).saturating_sub(layout.bare) / layout.pitch.max(1);
        self.room = usize::try_from(room).unwrap_or(usize::MAX).max(1);
    }

    /// How large the launcher is under `theme`, in units of layout: as
    /// tall as the rows it lists, up to as many as it has room for.
    #[must_use]
    pub fn size(&self, theme: &Theme) -> (u32, u32) {
        let (_, _, width, height) = Layout::new(theme).whole(self.shown());
        (width, height)
    }

    /// How far from the top of a screen `height` units tall the launcher
    /// is put: where it would be in the middle with every row it has room
    /// for, so that the text field stays where it is however many rows
    /// are listed.
    #[must_use]
    pub fn top(&self, theme: &Theme, height: u32) -> u32 {
        let (_, _, _, tallest) = Layout::new(theme).whole(self.room);
        height.saturating_sub(tallest) / 2
    }

    /// How many rows are listed.
    fn shown(&self) -> usize {
        self.count().min(self.room)
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

    /// The rows of the list: the answer, if there is one, then the
    /// programs that match.
    fn rows(&self) -> impl Iterator<Item = Row<'_>> {
        let answer = self.answer.iter();
        let answer = answer.map(|(copied, shown)| Row::Answer(copied, shown));
        answer.chain(self.listed().map(Row::App))
    }

    /// How many rows the list has.
    fn count(&self) -> usize {
        usize::from(self.answer.is_some()) + self.found.len()
    }

    /// The name of the program that would be started, or the answer that
    /// would be copied, if there is either.
    #[must_use]
    pub fn chosen(&self) -> Option<&str> {
        self.rows().nth(self.chosen).map(|row| match row {
            Row::Answer(answer, _) => answer,
            Row::App(app) => app.name.as_str(),
        })
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
                let last = self.count().saturating_sub(1);
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
        let whole = layout.whole(self.shown());
        let (_, _, width, height) = whole;
        let mut canvas = Canvas::scaled(width, height, scale);
        canvas.draw_box(whole, &theme.style(Part::Launcher, States::NONE));

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

        let listed = self.rows().enumerate().skip(self.first()).take(self.room);
        for (place, (row, item)) in listed.enumerate() {
            let (name, says, icon) = match item {
                Row::Answer(_, shown) => (shown, self.query.as_str(), None),
                Row::App(app) => (app.name.as_str(), app.description.as_str(), Some(&app.icon)),
            };
            let states = States::NONE.when(State::Active, row == self.chosen);
            // A row is the same row whichever program is in it.
            let key = u64::try_from(row).unwrap_or(u64::MAX);
            let style = motion.style(theme, Part::LauncherItem, key, states);
            let area = inset(layout.row(place), style.margin);
            canvas.draw_box(area, &style);
            let mut room = inset(area, style.inset());
            if self.show.icons {
                // A square as tall as the row's two lines.
                let side = room.3;
                let square = (room.0, room.1, side, side);
                let pixels = scale.pixels(side.cast_signed()).unsigned_abs();
                let icon = icon.and_then(|icon| icons.get(icon, pixels, style.color));
                if let Some(icon) = icon {
                    canvas.draw_image(square, icon);
                }
                room = inset(room, on_the_left(side.saturating_add(GAP)));
            }
            let font = theme.font(Part::LauncherItem, states);
            if self.show.descriptions {
                // The name in the top half, what it says under it.
                let (left, top, width, height) = room;
                let half = height / 2;
                let under = (
                    left,
                    top.saturating_add_unsigned(half),
                    width,
                    height - half,
                );
                text.draw(
                    &mut canvas,
                    name,
                    (left, top, width, half),
                    style.color,
                    font,
                );
                let said = motion.style(theme, Part::LauncherDescription, key, states);
                let ink = said.color;
                let font = theme.font(Part::LauncherDescription, states);
                text.draw(&mut canvas, says, under, ink, font);
            } else {
                text.draw(&mut canvas, name, room, style.color, font);
            }
        }
        canvas
    }

    /// The first program listed: the list moves down only as far as it
    /// takes to keep the chosen one in sight.
    fn first(&self) -> usize {
        (self.chosen + 1).saturating_sub(self.room)
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
            .filter(|place| *place < self.room)
            .map(|place| self.first() + place);
        row.filter(|row| *row < self.count())
    }

    /// Finds the programs that match what has been typed: those whose name
    /// begins with it first, then those that have it anywhere in their
    /// name, then, if what programs say of themselves is shown, those that
    /// have it there. Each lot is in the order the programs came in. If it
    /// is a sum, its answer comes before them all. The first of all is
    /// chosen.
    fn search(&mut self) {
        let answer = sums::answer(&self.query);
        self.answer = answer.map(|answer| {
            let shown = format!("= {answer}");
            (answer, shown)
        });
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

    /// Asks for the chosen program to be started, or the chosen answer to
    /// be copied, if there is either.
    fn start(&self) -> Outcome {
        match self.rows().nth(self.chosen) {
            Some(Row::Answer(answer, _)) => Outcome::Start(Request::Copy {
                text: answer.to_owned(),
            }),
            Some(Row::App(app)) => {
                // One that needs a terminal is started in the one the
                // built-in bindings start.
                let terminal = app.terminal.then(Command::terminal);
                let terminal = terminal.iter().map(Command::program);
                let command: Vec<&str> = terminal
                    .chain(app.command.iter().map(String::as_str))
                    .collect();
                Outcome::Start(Request::Perform {
                    action: json!({ "spawn": command }),
                })
            }
            None => Outcome::Stay,
        }
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

    /// Names alone, which most of what is tested here is easiest seen in.
    const NAMES: Show = Show {
        icons: false,
        descriptions: false,
    };

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
        Picker::new(apps, NAMES)
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
        let mut picker = Picker::new(vec![top], NAMES);
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
    fn the_launcher_is_as_tall_as_what_it_lists_up_to_half_the_screen() {
        let theme = Theme::default();
        // A border and padding each side, and the text field with the
        // room under it.
        let bare = 2 * 13 + 32 + 8;
        let mut picker = several();
        // Five rows, each of two lines, with room for nine.
        picker.fit(&theme, 1080);
        assert_eq!(picker.size(&theme), (480, bare + 5 * 48));
        typed(&mut picker, "fi");
        assert_eq!(picker.size(&theme), (480, bare + 2 * 48));
        typed(&mut picker, "zzz");
        assert_eq!(picker.size(&theme), (480, bare), "the field alone");
        // A smaller screen has room for fewer.
        let mut picker = several();
        picker.fit(&theme, 480);
        assert_eq!(picker.size(&theme), (480, bare + 3 * 48));
        picker.fit(&theme, 100);
        assert_eq!(picker.size(&theme), (480, bare + 48), "one at least");
        // Shorter rows, more of them.
        let wide = Theme::parse(
            "launcher { min-width: 600px; padding: 0; border: none; }\n\
             launcher-input { margin: 0; height: 40px; }\n\
             launcher-item { height: 20px; margin: 1px 0; }",
        )
        .unwrap();
        let many: Vec<String> = (0..20).map(|n| format!("Program {n:02}")).collect();
        let mut long = self::picker(&many.iter().map(String::as_str).collect::<Vec<_>>());
        long.fit(&wide, 480);
        assert_eq!(long.size(&wide), (600, 40 + 9 * 22));
        let narrow = Theme::parse("launcher { min-width: 10px; }").unwrap();
        assert_eq!(long.size(&narrow).0, NARROWEST);

        let mut picker = several();
        let canvas = painted(&picker);
        assert_eq!((canvas.width(), canvas.height()), picker.size(&theme));
        typed(&mut picker, "zzz");
        let canvas = still(&picker, &theme, Scale::new(2.0), &mut Icons::default());
        let (width, height) = picker.size(&theme);
        assert_eq!((canvas.width(), canvas.height()), (width * 2, height * 2));
    }

    #[test]
    fn the_launcher_stands_where_it_would_in_the_middle_were_it_full() {
        let theme = Theme::default();
        let mut picker = several();
        picker.fit(&theme, 1080);
        // Nine rows at most: 498 tall in all.
        assert_eq!(picker.top(&theme, 1080), (1080 - 498) / 2);
        typed(&mut picker, "zzz");
        assert_eq!(picker.top(&theme, 1080), (1080 - 498) / 2, "however few");
        picker.fit(&theme, 100);
        assert_eq!(picker.top(&theme, 100), 0, "however small the screen");
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
        // Nor is there anything where there is no program: the launcher
        // ends under the last.
        let (_, last) = row(4);
        let (_, after) = row(5);
        assert!(canvas.height() > last.unsigned_abs());
        assert!(canvas.height() < after.unsigned_abs());
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
            ..NAMES
        };
        let mut described = Picker::new(names.iter().map(|name| app(name)).collect(), show);

        // Under each name there is something written, and nothing
        // without; the name is in the top half of its row with it, and in
        // the middle without.
        let (_, middle) = row(1);
        let layout = Layout::new(&Theme::default());
        let (left, _, width) = layout.list;
        let half = layout.pitch.cast_signed() / 2;
        let inked = |picker: &Picker, lines: std::ops::Range<i32>| {
            let canvas = painted(picker);
            let surface = canvas.at(canvas.width() - 20, middle.unsigned_abs());
            let columns = left.unsigned_abs()..left.unsigned_abs() + width;
            let points = columns.flat_map(|x| lines.clone().map(move |y| (x, y)));
            let inked = points.filter(|(x, y)| canvas.at(*x, y.unsigned_abs()) != surface);
            inked.count()
        };
        let (top, bottom) = (middle - half + 8..middle - 2, middle + 6..middle + half - 6);
        assert!(inked(&described, bottom.clone()) > 20);
        assert_eq!(inked(&plain, bottom), 0);
        assert!(inked(&described, top.clone()) > 20);
        assert!(inked(&plain, middle - 3..middle + 3) > 20);

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
            ..NAMES
        };
        let with = Picker::new(names.iter().map(|name| app(name)).collect(), show);
        let canvas = still(&with, &theme, Scale::ONE, &mut icons);
        let green = |canvas: &Canvas, place: i32| {
            let y = row(place).1.unsigned_abs();
            let columns = 0..canvas.width();
            columns.filter(|x| canvas.at(*x, y) == 0xff00_ff00).count()
        };
        // Files has an icon as tall as the row's two lines; Calculator has
        // none, and room is left where it would be.
        assert_eq!(green(&canvas, 1), 36);
        assert_eq!(green(&canvas, 0), 0);
        let layout = Layout::new(&theme);
        let item = theme.style(Part::LauncherItem, States::NONE);
        let start = layout.list.0.unsigned_abs() + item.padding.left;
        let y = row(0).1.unsigned_abs();
        let chosen = lit(&theme);
        let beside = start..start + 36 + GAP;
        assert!(beside.clone().all(|x| canvas.at(x, y) == chosen));

        // Without icons asked for, none is drawn and the name starts there.
        let without = picker(&names);
        let canvas = still(&without, &theme, Scale::ONE, &mut icons);
        assert_eq!(green(&canvas, 1), 0);
        assert!(beside.into_iter().any(|x| canvas.at(x, y) != chosen));
    }

    fn copy(text: &str) -> Outcome {
        Outcome::Start(Request::Copy {
            text: text.to_owned(),
        })
    }

    #[test]
    fn a_sum_is_answered_above_the_programs_and_enter_copies_the_answer() {
        let mut picker = picker(&["Calculator", "Counter 2"]);
        typed(&mut picker, "2");
        assert_eq!(picker.chosen(), Some("Counter 2"), "a number alone");
        typed(&mut picker, "+2");
        assert_eq!(picker.chosen(), Some("4"));
        assert_eq!(names_of(&picker), [""; 0], "no program is named so");
        assert_eq!(picker.key(Key::Enter), copy("4"));
        // The answer changes as the sum is typed, and goes once it is no
        // longer a sum.
        typed(&mut picker, "*3");
        assert_eq!(picker.chosen(), Some("8"));
        typed(&mut picker, "+");
        assert_eq!(picker.chosen(), None);
        assert_eq!(picker.key(Key::Enter), Outcome::Stay);
    }

    #[test]
    fn the_programs_that_match_a_sum_are_listed_under_its_answer() {
        let mut picker = picker(&["Calculator", "Counter 1+1"]);
        typed(&mut picker, "1+1");
        assert_eq!(picker.chosen(), Some("2"));
        assert_eq!(picker.key(Key::Down), Outcome::Stay);
        assert_eq!(picker.chosen(), Some("Counter 1+1"));
        picker.key(Key::Down);
        assert_eq!(picker.chosen(), Some("Counter 1+1"), "the last");
        assert_eq!(picker.key(Key::Enter), spawn(&["counter 1+1", "--go"]));
        picker.key(Key::Up);
        assert_eq!(picker.key(Key::Enter), copy("2"));

        // The pointer and clicks find the answer's row as a program's.
        let theme = Theme::default();
        let (x, y) = row(1);
        assert!(picker.point(x, y, &theme));
        assert_eq!(picker.chosen(), Some("Counter 1+1"));
        let (x, y) = row(0);
        assert_eq!(picker.click(x, y, &theme), copy("2"));
        let (x, below) = row(2);
        assert_eq!(picker.click(x, below, &theme), Outcome::Stay);
    }

    #[test]
    fn an_answer_is_written_on_its_row_with_the_sum_under_it() {
        let theme = Theme::default();
        let layout = Layout::new(&theme);
        let (left, _, width) = layout.list;
        let (_, middle) = row(0);
        let half = layout.pitch.cast_signed() / 2;
        let inked = |picker: &Picker, lines: std::ops::Range<i32>| {
            let canvas = painted(picker);
            let columns = left.unsigned_abs()..left.unsigned_abs() + width;
            let points = columns.flat_map(|x| lines.clone().map(move |y| (x, y)));
            let inked = points.filter(|(x, y)| canvas.at(*x, y.unsigned_abs()) != lit(&theme));
            inked.count()
        };
        let (top, bottom) = (middle - half + 8..middle - 2, middle + 6..middle + half - 6);
        let sum = |show: Show| {
            let mut picker = Picker::new(Vec::new(), show);
            typed(&mut picker, "6*7");
            picker
        };
        let described = sum(Show {
            descriptions: true,
            ..NAMES
        });
        assert!(inked(&described, top) > 20, "the answer");
        assert!(inked(&described, bottom.clone()) > 20, "the sum");
        let plain = sum(NAMES);
        assert!(inked(&plain, middle - 3..middle + 3) > 20, "the answer");
        assert_eq!(inked(&plain, bottom), 0);
        // With icons, it is written where the programs' names are.
        let iconed = sum(Show {
            icons: true,
            ..NAMES
        });
        let canvas = painted(&iconed);
        let item = theme.style(Part::LauncherItem, States::NONE);
        let start = left.unsigned_abs() + item.padding.left;
        let y = middle.unsigned_abs();
        let beside = start..start + 36 + GAP;
        assert!(beside.clone().all(|x| canvas.at(x, y) == lit(&theme)));
        let past = beside.end..beside.end + 40;
        assert!(past.into_iter().any(|x| canvas.at(x, y) != lit(&theme)));
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
