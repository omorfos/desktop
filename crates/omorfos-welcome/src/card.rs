//! The card: a few steps, one at a time, from a welcome to the keys that
//! matter, light or dark, a wallpaper and the applications that open what;
//! which of its parts has the keyboard, how it is drawn and what a key or a
//! click does to it. Every step has Skip, and Back and Next; skipped or
//! finished, it keeps what was chosen.
//!
//! Nothing here knows of Wayland or of files.

use omorfos_config::{Color, Edges, Part, SchemeChoice, State, States, Style, Theme};
use omorfos_ui::{Area, Canvas, Image, Pressing, Scale, Text, inset};

/// The room between what stands side by side, and between a row of
/// pictures and what is under it.
const GAP: u32 = 10;

/// How wide the card is, whatever the theme says.
const WIDTH: u32 = 600;

/// How many rows of a list the room for a step's own has, a row as tall as
/// one of the launcher's.
const ROWS: u32 = 6;

/// How much of a picture's box is left around the picture, to show the
/// box's look through.
const FRAME: u32 = 4;

/// How tall the bar is under what is in use.
const BAR: u32 = 3;

/// How wide each of Back and Next is, and how tall all the buttons are.
/// How wide Back and Next are, alike so that the dots between them stay
/// where they are as Next becomes Finish; as tall as the theme makes a
/// question's button.
const BUTTON: u32 = 120;

/// How large the mark is on the first step.
const MARK: u32 = 96;

/// How large a dot is that says which step it is, and the room between.
const DOT: u32 = 8;

/// How many pictures stand side by side, the rest in rows under them.
const ACROSS: u32 = 3;

/// The schemes offered, in the order they stand.
pub const SCHEMES: [SchemeChoice; 3] =
    [SchemeChoice::Dark, SchemeChoice::Light, SchemeChoice::Auto];

/// A key, as the card understands keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// To the one on the left.
    Left,
    /// To the one on the right.
    Right,
    /// To the row above.
    Up,
    /// To the row below.
    Down,
    /// To the next of everything, round: Tab.
    Next,
    /// To the one before, round: Shift+Tab.
    Back,
    /// Do what has the keyboard.
    Enter,
    /// Close the card.
    Escape,
}

/// A step of the card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// The mark, and a word of welcome.
    Welcome,
    /// What the keys that matter do.
    Keys,
    /// Light, dark or with the day.
    Scheme,
    /// The wallpapers that come with omorfos.
    Wallpaper,
    /// The applications that open each kind of thing.
    Applications,
}

/// What on the card a key or a click can be on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// The wallpaper offered in this place.
    Wallpaper(usize),
    /// The scheme in this place of [`SCHEMES`].
    Scheme(usize),
    /// Back to the step before.
    Back,
    /// On to the next step, or done on the last.
    Next,
    /// Done, with what was chosen kept.
    Skip,
}

/// A choice made on the card, to be saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// The wallpaper offered in this place.
    Wallpaper(usize),
    /// A scheme.
    Scheme(SchemeChoice),
}

/// What comes of a key or a click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The card stays, perhaps looking different.
    Stay,
    /// This is to be saved, and the card stays.
    Choose(Choice),
    /// The card is done with.
    Close,
}

/// What the card says, in the user's language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Said {
    /// Its heading on the first step.
    pub title: String,
    /// Under it.
    pub greeting: String,
    /// The heading of the step of the keys.
    pub keys: String,
    /// The heading of the step that offers light or dark.
    pub scheme: String,
    /// The heading of the step that offers a wallpaper.
    pub wallpaper: String,
    /// The heading of the step of the applications.
    pub applications: String,
    /// Under the applications.
    pub chosen_elsewhere: String,
    /// In place of an application, where none is set.
    pub none: String,
    /// The schemes' names, as [`SCHEMES`] has them.
    pub schemes: [String; 3],
    /// On the button that closes it, keeping what was chosen.
    pub skip: String,
    /// On the button back.
    pub back: String,
    /// On the button on.
    pub next: String,
    /// On the button on, on the last step.
    pub finish: String,
}

/// An application that opens a kind of thing: what the kind is called, and
/// the application's name and icon, if one is set.
#[derive(Debug, Clone, PartialEq)]
pub struct Opener {
    /// The kind, as "The web".
    pub kind: String,
    /// The application's name, if one is set.
    pub name: Option<String>,
    /// Its icon, if it has one.
    pub icon: Option<Image>,
}

/// What the card offers and shows, besides its words.
#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    /// The mark, for the first step.
    pub mark: Option<Image>,
    /// Keys, as they are pressed, and what they do.
    pub rows: Vec<(String, String)>,
    /// A small picture of the desktop in each of [`SCHEMES`].
    pub schemes: Vec<Image>,
    /// The wallpapers offered, drawn small.
    pub wallpapers: Vec<Image>,
    /// What opens each kind of thing.
    pub openers: Vec<Opener>,
}

/// The card's state: what it shows, which step it is at, what is in use,
/// and what has the keyboard and the pointer.
#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    said: Said,
    shown: Shown,
    /// The steps, those with nothing to offer left out.
    steps: Vec<Step>,
    /// Which of them it is at.
    step: usize,
    /// Which wallpaper is in use, if one of those offered is.
    wallpaper: Option<usize>,
    /// The scheme in use.
    scheme: SchemeChoice,
    focus: Focus,
    hovered: Option<Focus>,
    /// What the pointer's button went down on, pressed as it is let go
    /// there (ADR 113).
    pressing: Pressing<Focus>,
    /// Why the last choice was not saved.
    problem: Option<String>,
}

/// Where the card's parts are, under a theme.
struct Layout {
    whole: Area,
    /// The heading of a step.
    heading: Area,
    /// The room for a step's own, and how tall a row of it is.
    body: Area,
    pitch: u32,
    /// Why a choice was not saved, over the buttons.
    problem: Area,
    /// Skip, the dots, Back and Next.
    skip: Area,
    dots: Area,
    back: Area,
    next: Area,
}

impl Layout {
    fn new(theme: &Theme) -> Self {
        let card = theme.style(Part::Launcher, States::NONE);
        let item = theme.style(Part::LauncherItem, States::NONE);
        let pitch = item.height.saturating_add(item.margin.down());
        let room = WIDTH.saturating_sub(card.inset().across());
        let (left, top) = (
            card.inset().left.cast_signed(),
            card.inset().top.cast_signed(),
        );
        let at = |down: u32| top.saturating_add_unsigned(down);
        let heading = (left, top, room, pitch);
        let body = (left, at(pitch), room, ROWS * pitch);
        let problem = (left, at(pitch + ROWS * pitch), room, pitch / 2);
        let foot = pitch + ROWS * pitch + pitch / 2 + GAP;
        let (wide, tall) = (
            BUTTON,
            theme.style(Part::QuestionButton, States::NONE).height,
        );
        let next = (
            left.saturating_add_unsigned(room - wide),
            at(foot),
            wide,
            tall,
        );
        let back = (
            left.saturating_add_unsigned(room - 2 * wide - GAP),
            at(foot),
            wide,
            tall,
        );
        let skip = (left, at(foot), wide, tall);
        let dots = (
            left.saturating_add_unsigned(wide + GAP),
            at(foot),
            room.saturating_sub(3 * wide + 2 * GAP),
            tall,
        );
        let height = (foot + tall).saturating_add(card.inset().down());
        Self {
            whole: (0, 0, WIDTH, height),
            heading,
            body,
            pitch,
            problem,
            skip,
            dots,
            back,
            next,
        }
    }

    /// The row of the body `place` rows down.
    fn row(&self, place: usize) -> Area {
        let (left, top, width, _) = self.body;
        let down = u32::try_from(place)
            .unwrap_or(u32::MAX)
            .saturating_mul(self.pitch);
        (left, top.saturating_add_unsigned(down), width, self.pitch)
    }

    /// `count` pictures across the body, [`ACROSS`] to a row, as wide as
    /// they can be with the room between them, as tall as a screen would
    /// make them, with room under each for its bar and its name.
    fn pictures(&self, count: usize) -> Vec<Area> {
        let (left, top, room, _) = self.body;
        let cell = room.saturating_sub(GAP * (ACROSS - 1)) / ACROSS;
        let tall = cell.saturating_sub(2 * FRAME) * 9 / 16 + 2 * FRAME;
        let down = tall + BAR + self.pitch / 2 + GAP;
        let places = 0..u32::try_from(count).unwrap_or(0);
        places
            .map(|place| {
                let (column, row) = (place % ACROSS, place / ACROSS);
                (
                    left.saturating_add_unsigned(column * (cell + GAP)),
                    top.saturating_add_unsigned(GAP + row * down),
                    cell,
                    tall,
                )
            })
            .collect()
    }
}

/// Whether `point` is in `area`.
fn inside((left, top, width, height): Area, (x, y): (i32, i32)) -> bool {
    (left..left.saturating_add_unsigned(width)).contains(&x)
        && (top..top.saturating_add_unsigned(height)).contains(&y)
}

impl Card {
    /// A card saying `said` and showing `shown`, with `wallpaper` and
    /// `scheme` in use, at its first step, Next having the keyboard. A step
    /// with nothing to offer, as the wallpapers with none installed, is
    /// left out.
    #[must_use]
    pub fn new(
        said: Said,
        shown: Shown,
        (wallpaper, scheme): (Option<usize>, SchemeChoice),
    ) -> Self {
        let steps = [
            (Step::Welcome, true),
            (Step::Keys, !shown.rows.is_empty()),
            (Step::Scheme, true),
            (Step::Wallpaper, !shown.wallpapers.is_empty()),
            (Step::Applications, !shown.openers.is_empty()),
        ];
        let steps = steps.into_iter().filter(|(_, any)| *any);
        Self {
            said,
            wallpaper: wallpaper.filter(|place| *place < shown.wallpapers.len()),
            steps: steps.map(|(step, _)| step).collect(),
            shown,
            step: 0,
            scheme,
            focus: Focus::Next,
            hovered: None,
            pressing: Pressing::new(),
            problem: None,
        }
    }

    /// How large the card is under `theme`, in units of layout: the same
    /// at every step.
    #[must_use]
    pub fn size(theme: &Theme) -> (u32, u32) {
        let (_, _, width, height) = Layout::new(theme).whole;
        (width, height)
    }

    /// The step it is at.
    #[must_use]
    pub fn step(&self) -> Step {
        self.steps[self.step]
    }

    /// What has the keyboard.
    #[must_use]
    pub const fn focus(&self) -> Focus {
        self.focus
    }

    /// Whether it is at its last step.
    const fn last(&self) -> bool {
        self.step + 1 >= self.steps.len()
    }

    /// The choices the step it is at offers, left to right.
    fn choices(&self) -> Vec<Focus> {
        match self.step() {
            Step::Scheme => (0..SCHEMES.len()).map(Focus::Scheme).collect(),
            Step::Wallpaper => (0..self.shown.wallpapers.len())
                .map(Focus::Wallpaper)
                .collect(),
            Step::Welcome | Step::Keys | Step::Applications => Vec::new(),
        }
    }

    /// Everything that can have the keyboard at this step, in the order Tab
    /// goes: its choices, Back but on the first, Next and Skip.
    fn focusable(&self) -> Vec<Focus> {
        let back = (self.step > 0).then_some(Focus::Back);
        let mut all = self.choices();
        all.extend(back);
        all.extend([Focus::Next, Focus::Skip]);
        all
    }

    /// Of the choices of the step it is at, the one in use.
    fn in_use(&self) -> Option<Focus> {
        let scheme = SCHEMES.iter().position(|scheme| *scheme == self.scheme);
        match self.step() {
            Step::Scheme => scheme.map(Focus::Scheme),
            Step::Wallpaper => self.wallpaper.map(Focus::Wallpaper),
            Step::Welcome | Step::Keys | Step::Applications => None,
        }
    }

    /// What a key comes to.
    pub fn key(&mut self, key: Key) -> Outcome {
        let all = self.focusable();
        let here = all
            .iter()
            .position(|focus| *focus == self.focus)
            .unwrap_or(0);
        let choices = self.choices();
        let along = choices.iter().position(|focus| *focus == self.focus);
        self.focus = match (key, along) {
            (Key::Escape, _) => return Outcome::Close,
            (Key::Enter, _) => return self.press(self.focus),
            (Key::Next, _) => all[(here + 1) % all.len()],
            (Key::Back, _) => all[(here + all.len() - 1) % all.len()],
            (Key::Left, Some(place)) => choices[place.saturating_sub(1)],
            (Key::Right, Some(place)) => choices[(place + 1).min(choices.len() - 1)],
            (Key::Down, Some(_)) => Focus::Next,
            (Key::Up, None) => {
                let first = choices.first().copied();
                self.in_use().or(first).unwrap_or(self.focus)
            }
            (Key::Left, None) if self.focus == Focus::Next && self.step > 0 => Focus::Back,
            (Key::Right, None) if self.focus == Focus::Back => Focus::Next,
            _ => self.focus,
        };
        Outcome::Stay
    }

    /// What pressing `focus` comes to, by a key or a click.
    fn press(&mut self, focus: Focus) -> Outcome {
        match focus {
            Focus::Skip => Outcome::Close,
            Focus::Next if self.last() => Outcome::Close,
            Focus::Next => self.go(self.step + 1),
            Focus::Back => self.go(self.step.saturating_sub(1)),
            Focus::Wallpaper(place) => {
                self.focus = focus;
                Outcome::Choose(Choice::Wallpaper(place))
            }
            Focus::Scheme(place) => {
                self.focus = focus;
                Outcome::Choose(Choice::Scheme(SCHEMES[place]))
            }
        }
    }

    /// Goes to the step `step`, Next having the keyboard.
    fn go(&mut self, step: usize) -> Outcome {
        self.step = step;
        self.focus = Focus::Next;
        self.hovered = None;
        self.problem = None;
        Outcome::Stay
    }

    /// What is at `x`, `y` on the card at this step, if anything a click
    /// means something on.
    fn at(&self, x: i32, y: i32, theme: &Theme) -> Option<Focus> {
        let areas = self.areas(&Layout::new(theme));
        let found = areas.into_iter().find(|(_, area)| inside(*area, (x, y)));
        found.map(|(focus, _)| focus)
    }

    /// Where each of what can have the keyboard is, at this step.
    fn areas(&self, layout: &Layout) -> Vec<(Focus, Area)> {
        let choices = self.choices();
        let pictures = layout.pictures(choices.len());
        let mut areas: Vec<(Focus, Area)> = choices.into_iter().zip(pictures).collect();
        let back = (self.step > 0).then_some((Focus::Back, layout.back));
        areas.extend(back);
        areas.extend([(Focus::Next, layout.next), (Focus::Skip, layout.skip)]);
        areas
    }

    /// Takes in where the pointer is on the card, in units of layout, and
    /// says whether that changes how it looks.
    pub fn point(&mut self, x: i32, y: i32, theme: &Theme) -> bool {
        let over = self.at(x, y, theme);
        self.pressing.moved(over);
        let changed = over != self.hovered;
        self.hovered = over;
        changed
    }

    /// What a click at `x`, `y` on the card comes to: the button pressed
    /// and let go there.
    pub fn click(&mut self, x: i32, y: i32, theme: &Theme) -> Outcome {
        self.pointer_down(x, y, theme);
        self.pointer_up(Some((x, y)), theme)
    }

    /// Takes the pointer's button going down at `x`, `y` on the card:
    /// what is there is pressed as the button is let go on it (ADR 113).
    pub fn pointer_down(&mut self, x: i32, y: i32, theme: &Theme) {
        let over = self.at(x, y, theme);
        self.pressing.press(over);
    }

    /// Takes the pointer's button being let go `at` a place on the card,
    /// or off it: on what it went down on, that is pressed; anywhere else,
    /// nothing.
    pub fn pointer_up(&mut self, at: Option<(i32, i32)>, theme: &Theme) -> Outcome {
        let over = at.and_then(|(x, y)| self.at(x, y, theme));
        let held = self.pressing.release(over);
        held.map_or(Outcome::Stay, |focus| self.press(focus))
    }

    /// Takes in that `choice` was saved, and is in use.
    pub fn saved(&mut self, choice: Choice) {
        self.problem = None;
        match choice {
            Choice::Wallpaper(place) => self.wallpaper = Some(place),
            Choice::Scheme(scheme) => self.scheme = scheme,
        }
    }

    /// Takes in why a choice was not saved, to say it.
    pub fn refused(&mut self, why: String) {
        self.problem = Some(why);
    }

    /// The states `focus` is drawn in.
    fn states(&self, focus: Focus) -> States {
        States::NONE.when(State::Hovered, self.hovered == Some(focus))
    }

    /// Draws the card at `scale`, under `theme`.
    #[must_use]
    pub fn paint(&self, text: &mut Text, theme: &Theme, scale: Scale) -> Canvas {
        let layout = Layout::new(theme);
        let (_, _, width, height) = layout.whole;
        let mut canvas = Canvas::scaled(width, height, scale);
        canvas.draw_box(layout.whole, &theme.style(Part::Launcher, States::NONE));
        let item = theme.style(Part::LauncherItem, States::NONE);
        let heading = match self.step() {
            Step::Welcome => &self.said.title,
            Step::Keys => &self.said.keys,
            Step::Scheme => &self.said.scheme,
            Step::Wallpaper => &self.said.wallpaper,
            Step::Applications => &self.said.applications,
        };
        let font = theme.font(Part::LauncherInput, States::NONE);
        text.draw(&mut canvas, heading, layout.heading, item.color, font);
        match self.step() {
            Step::Welcome => self.paint_welcome(&mut canvas, text, theme, &layout),
            Step::Keys => self.paint_keys(&mut canvas, text, theme, &layout),
            Step::Scheme => {
                let names = self.said.schemes.iter().map(String::as_str);
                let pictures = self.shown.schemes.iter().zip(names).collect();
                self.paint_pictures(&mut canvas, (text, theme), &layout, pictures);
            }
            Step::Wallpaper => {
                let pictures = self.shown.wallpapers.iter().map(|picture| (picture, ""));
                let pictures = pictures.collect();
                self.paint_pictures(&mut canvas, (text, theme), &layout, pictures);
            }
            Step::Applications => self.paint_openers(&mut canvas, text, theme, &layout),
        }
        self.paint_foot(&mut canvas, text, theme, &layout);
        canvas
    }

    /// Draws the mark in the middle at the top, and the greeting under it.
    fn paint_welcome(&self, canvas: &mut Canvas, text: &mut Text, theme: &Theme, layout: &Layout) {
        let (left, top, room, _) = layout.body;
        let middle = left.saturating_add_unsigned(room.saturating_sub(MARK) / 2);
        let mark = (middle, top.saturating_add_unsigned(GAP), MARK, MARK);
        if let Some(image) = &self.shown.mark {
            canvas.draw_image(mark, image);
        }
        let said = theme.style(Part::LauncherDescription, States::NONE);
        let font = theme.font(Part::LauncherDescription, States::NONE);
        let lines = text.wrap(&self.said.greeting, room, font, 3);
        let (below, line) = (MARK + 2 * GAP, layout.pitch / 2);
        for (place, words) in (0_u32..).zip(&lines) {
            let down = top.saturating_add_unsigned(below + place * line);
            text.draw_centered(canvas, words, (left, down, room, line), said.color, font);
        }
    }

    /// Draws the keys in a column as wide as the widest, and what each
    /// does after it.
    fn paint_keys(&self, canvas: &mut Canvas, text: &mut Text, theme: &Theme, layout: &Layout) {
        let item = theme.style(Part::LauncherItem, States::NONE);
        let font = theme.font(Part::LauncherItem, States::NONE);
        let told = theme.style(Part::LauncherDescription, States::NONE);
        let told_font = theme.font(Part::LauncherDescription, States::NONE);
        let rows = self.shown.rows.iter();
        let widest = rows.map(|(keys, _)| text.measure(keys, font)).max();
        let column = widest.unwrap_or(0).saturating_add(GAP * 2);
        for (place, (keys, does)) in self.shown.rows.iter().enumerate() {
            let area = inset(layout.row(place), item.inset());
            text.draw(canvas, keys, area, item.color, font);
            let after = Edges {
                left: column.min(area.2),
                ..Edges::NONE
            };
            text.draw(canvas, does, inset(area, after), told.color, told_font);
        }
    }

    /// Draws `pictures` side by side, each with its name under it and a
    /// bar under the one in use; and why the last choice was not saved.
    fn paint_pictures(
        &self,
        canvas: &mut Canvas,
        (text, theme): (&mut Text, &Theme),
        layout: &Layout,
        pictures: Vec<(&Image, &str)>,
    ) {
        let in_use = self.in_use();
        let places = layout.pictures(pictures.len());
        let told = theme.style(Part::LauncherDescription, States::NONE);
        let told_font = theme.font(Part::LauncherDescription, States::NONE);
        let accent = theme.style(Part::QuestionButton, States::NONE.with(State::Active));
        let each = places.iter().zip(pictures).zip(self.choices());
        for ((area, (picture, name)), focus) in each {
            let style = theme.style(Part::LauncherItem, self.states(focus));
            canvas.draw_box(*area, &ringed(style, theme, self.focus == focus));
            canvas.draw_image(inset(*area, Edges::all(FRAME)), picture);
            let (left, top, width, height) = *area;
            let under = top.saturating_add_unsigned(height + BAR + 2);
            let named = (left, under, width, layout.pitch / 2);
            text.draw_centered(canvas, name, named, told.color, told_font);
            if in_use == Some(focus) {
                let bar = (left, top.saturating_add_unsigned(height + 1), width, BAR);
                let middle = Edges {
                    left: width / 4,
                    right: width / 4,
                    ..Edges::NONE
                };
                canvas.fill(inset(bar, middle), accent.background);
            }
        }
        if let Some(problem) = &self.problem {
            text.draw(canvas, problem, layout.problem, told.color, told_font);
        }
    }

    /// Draws each kind of thing and the application that opens it, with
    /// its icon, or that none is set; and that they are chosen elsewhere.
    fn paint_openers(&self, canvas: &mut Canvas, text: &mut Text, theme: &Theme, layout: &Layout) {
        let item = theme.style(Part::LauncherItem, States::NONE);
        let font = theme.font(Part::LauncherItem, States::NONE);
        let told = theme.style(Part::LauncherDescription, States::NONE);
        let told_font = theme.font(Part::LauncherDescription, States::NONE);
        let openers = self.shown.openers.iter();
        let widest = openers
            .map(|opener| text.measure(&opener.kind, told_font))
            .max();
        let column = widest.unwrap_or(0).saturating_add(GAP * 2);
        // A row for each, and one for the note, as tall as they all fit.
        let (left, top, room, height) = layout.body;
        let count = u32::try_from(self.shown.openers.len()).unwrap_or(u32::MAX);
        let line = (height / count.saturating_add(1)).min(layout.pitch);
        let row = |place: u32| (left, top.saturating_add_unsigned(place * line), room, line);
        for (place, opener) in (0..).zip(&self.shown.openers) {
            let area = inset(row(place), item.inset());
            text.draw(canvas, &opener.kind, area, told.color, told_font);
            let after = Edges {
                left: column.min(area.2),
                ..Edges::NONE
            };
            let (left, top, width, height) = inset(area, after);
            let side = height.min(width);
            if let Some(icon) = &opener.icon {
                canvas.draw_image((left, top, side, side), icon);
            }
            let beside = side + GAP;
            let name = (
                left.saturating_add_unsigned(beside),
                top,
                width.saturating_sub(beside),
                height,
            );
            match &opener.name {
                Some(named) => text.draw(canvas, named, name, item.color, font),
                None => text.draw(canvas, &self.said.none, name, told.color, told_font),
            }
        }
        let note = inset(row(count), item.inset());
        let elsewhere = &self.said.chosen_elsewhere;
        text.draw(canvas, elsewhere, note, told.color, told_font);
    }

    /// Draws Skip, the dots that say which step it is, Back but on the
    /// first step, and Next or Finish.
    fn paint_foot(&self, canvas: &mut Canvas, text: &mut Text, theme: &Theme, layout: &Layout) {
        // Skip is only words, not to be taken for going on.
        let told = theme.style(Part::LauncherDescription, self.states(Focus::Skip));
        let told_font = theme.font(Part::LauncherDescription, States::NONE);
        let skip = &self.said.skip;
        text.draw_centered(canvas, skip, layout.skip, told.color, told_font);
        if self.focus == Focus::Skip {
            // The ring alone, round its words.
            let mut ring = theme.style(Part::QuestionButton, States::NONE.with(State::Focused));
            ring.background = Color::rgba(0, 0, 0, 0);
            ring.gradient = None;
            canvas.draw_box(layout.skip, &ring);
        }
        let mut button = |canvas: &mut Canvas, focus: Focus, area: Area, said: &str| {
            let states = self
                .states(focus)
                .when(State::Active, focus == Focus::Next)
                .when(State::Focused, self.focus == focus);
            let (style, font) = omorfos_ui::button_look(theme, states);
            omorfos_ui::button(canvas, text, (&style, font), (said, area));
        };
        if self.step > 0 {
            button(canvas, Focus::Back, layout.back, &self.said.back);
        }
        let on = if self.last() {
            &self.said.finish
        } else {
            &self.said.next
        };
        button(canvas, Focus::Next, layout.next, on);

        // A dot for each step, the one it is at in the accent.
        let (left, top, room, tall) = layout.dots;
        let count = u32::try_from(self.steps.len()).unwrap_or(0);
        let wide = count.saturating_mul(DOT * 2).saturating_sub(DOT);
        let from = left.saturating_add_unsigned(room.saturating_sub(wide) / 2);
        let down = top.saturating_add_unsigned(tall.saturating_sub(DOT) / 2);
        let accent = theme.style(Part::QuestionButton, States::NONE.with(State::Active));
        // The others in the colour of a line between things, which the
        // hairline round the card, being seen through, is too faint for.
        let quiet = theme.style(Part::MenuSeparator, States::NONE);
        for (place, dot) in (0..count).zip(0..) {
            let background = if dot == self.step {
                accent.background
            } else {
                quiet.color
            };
            let at = from.saturating_add_unsigned(place * DOT * 2);
            let style = Style {
                background,
                gradient: None,
                radius: DOT / 2,
                ..accent
            };
            canvas.draw_box((at, down, DOT, DOT), &style);
        }
    }
}

/// `style`, with the ring round what has the keyboard if `focused`, as a
/// question's buttons have it.
fn ringed(style: Style, theme: &Theme, focused: bool) -> Style {
    let ring = theme.style(Part::QuestionButton, States::NONE.with(State::Focused));
    if focused {
        Style {
            outline: ring.outline,
            outline_offset: ring.outline_offset,
            ..style
        }
    } else {
        style
    }
}

#[cfg(test)]
mod tests {
    use omorfos_config::{Accent, Scheme};

    use super::*;

    fn theme() -> Theme {
        Theme::built_in(Scheme::Dark, Accent::default())
    }

    fn said() -> Said {
        let words = |word: &str| word.to_owned();
        Said {
            title: words("Welcome"),
            greeting: words("A few steps to make it yours."),
            keys: words("Keys"),
            scheme: words("Light or dark"),
            wallpaper: words("Wallpaper"),
            applications: words("What opens what"),
            chosen_elsewhere: words("Chosen in Settings"),
            none: words("None"),
            schemes: ["Dark", "Light", "With the day"].map(words),
            skip: words("Skip"),
            back: words("Back"),
            next: words("Next"),
            finish: words("Finish"),
        }
    }

    fn picture(colour: u8) -> Image {
        Image::new(16, 9, [colour, 0, 0, 255].repeat(16 * 9)).unwrap()
    }

    fn shown(wallpapers: usize, openers: usize) -> Shown {
        Shown {
            mark: Some(picture(255)),
            rows: vec![("Super+Return".to_owned(), "Open a terminal".to_owned())],
            schemes: vec![picture(10), picture(20), picture(30)],
            wallpapers: (0..wallpapers).map(|_| picture(40)).collect(),
            openers: (0..openers)
                .map(|place| Opener {
                    kind: format!("Kind {place}"),
                    name: (place % 2 == 0).then(|| format!("App {place}")),
                    icon: (place == 0).then(|| picture(50)),
                })
                .collect(),
        }
    }

    fn card(wallpapers: usize, openers: usize) -> Card {
        let in_use = (Some(1), SchemeChoice::Auto);
        Card::new(said(), shown(wallpapers, openers), in_use)
    }

    fn middle((left, top, width, height): Area) -> (i32, i32) {
        (
            left + (width / 2).cast_signed(),
            top + (height / 2).cast_signed(),
        )
    }

    #[test]
    fn next_goes_through_the_steps_back_returns_and_the_last_finishes() {
        let mut card = card(3, 2);
        assert_eq!(card.step(), Step::Welcome);
        assert_eq!(card.focus(), Focus::Next);
        let steps = [
            Step::Keys,
            Step::Scheme,
            Step::Wallpaper,
            Step::Applications,
        ];
        for step in steps {
            assert_eq!(card.key(Key::Enter), Outcome::Stay);
            assert_eq!(card.step(), step);
        }
        assert_eq!(card.key(Key::Enter), Outcome::Close, "finished");
        card.key(Key::Left);
        assert_eq!(card.focus(), Focus::Back);
        card.key(Key::Right);
        assert_eq!(card.focus(), Focus::Next);
        card.key(Key::Left);
        card.key(Key::Enter);
        assert_eq!(card.step(), Step::Wallpaper);
        assert_eq!(
            card.focus(),
            Focus::Next,
            "Next has the keyboard at each step"
        );
    }

    #[test]
    fn a_step_with_nothing_to_offer_is_left_out() {
        let shown = Shown {
            rows: Vec::new(),
            ..shown(0, 0)
        };
        let mut card = Card::new(said(), shown, (None, SchemeChoice::Dark));
        card.key(Key::Enter);
        assert_eq!(card.step(), Step::Scheme);
        assert_eq!(card.key(Key::Enter), Outcome::Close, "the last");
        // On the first step there is no going back.
        let mut first = self::card(1, 1);
        first.key(Key::Left);
        assert_eq!(first.focus(), Focus::Next);
    }

    #[test]
    fn skip_and_escape_close_it_at_any_step_and_nothing_else_does() {
        let mut card = card(2, 1);
        card.key(Key::Enter);
        assert_eq!(card.key(Key::Escape), Outcome::Close);
        // Tab goes Next, Skip, and round: there is no button in the corner
        // that closes it, Skip being the one that does.
        card.key(Key::Next);
        assert_eq!(card.focus(), Focus::Skip);
        assert_eq!(card.key(Key::Enter), Outcome::Close);
        card.key(Key::Next);
        assert_eq!(card.focus(), Focus::Back, "round to the first");
        card.key(Key::Back);
        assert_eq!(card.focus(), Focus::Skip);
    }

    #[test]
    fn the_arrows_go_along_the_choices_and_enter_chooses() {
        let mut card = card(3, 0);
        card.key(Key::Enter);
        card.key(Key::Enter);
        assert_eq!(card.step(), Step::Scheme);
        // Up from the buttons goes to the scheme in use.
        card.key(Key::Up);
        assert_eq!(card.focus(), Focus::Scheme(2));
        card.key(Key::Right);
        assert_eq!(card.focus(), Focus::Scheme(2), "no further");
        card.key(Key::Left);
        assert_eq!(
            card.key(Key::Enter),
            Outcome::Choose(Choice::Scheme(SchemeChoice::Light))
        );
        card.saved(Choice::Scheme(SchemeChoice::Light));
        card.key(Key::Down);
        assert_eq!(card.focus(), Focus::Next);
        card.key(Key::Up);
        assert_eq!(card.focus(), Focus::Scheme(1), "the one in use now");
        card.key(Key::Down);
        card.key(Key::Enter);
        assert_eq!(card.step(), Step::Wallpaper);
        card.key(Key::Up);
        assert_eq!(card.focus(), Focus::Wallpaper(1));
        card.key(Key::Left);
        card.key(Key::Left);
        assert_eq!(card.focus(), Focus::Wallpaper(0), "no further");
        assert_eq!(card.key(Key::Enter), Outcome::Choose(Choice::Wallpaper(0)));
        card.saved(Choice::Wallpaper(0));
        // Up on a choice stays.
        card.key(Key::Up);
        assert_eq!(card.focus(), Focus::Wallpaper(0));
    }

    #[test]
    fn up_where_nothing_is_in_use_goes_to_the_first_and_without_choices_stays() {
        let mut card = Card::new(said(), shown(2, 0), (None, SchemeChoice::Dark));
        for _ in 0..3 {
            card.key(Key::Enter);
        }
        assert_eq!(card.step(), Step::Wallpaper);
        card.key(Key::Up);
        assert_eq!(card.focus(), Focus::Wallpaper(0));
        let mut first = self::card(1, 1);
        first.key(Key::Up);
        assert_eq!(first.focus(), Focus::Next);
    }

    #[test]
    fn a_click_does_what_it_is_on_and_the_pointer_lights_it() {
        let theme = theme();
        let layout = Layout::new(&theme);
        let mut card = card(2, 1);
        let (x, y) = middle(layout.next);
        assert!(card.point(x, y, &theme));
        assert!(!card.point(x + 1, y, &theme), "still on it");
        assert_eq!(card.click(x, y, &theme), Outcome::Stay);
        assert_eq!(card.step(), Step::Keys);
        assert!(
            !card.point(-1, -1, &theme),
            "nothing lit on the step gone to"
        );
        let (x, y) = middle(layout.back);
        card.click(x, y, &theme);
        assert_eq!(card.step(), Step::Welcome);
        // Back is not there on the first step.
        assert_eq!(card.click(x, y, &theme), Outcome::Stay);
        assert_eq!(card.step(), Step::Welcome);
        card.key(Key::Enter);
        card.key(Key::Enter);
        let (x, y) = middle(layout.pictures(3)[0]);
        assert_eq!(
            card.click(x, y, &theme),
            Outcome::Choose(Choice::Scheme(SchemeChoice::Dark))
        );
        let (x, y) = middle(layout.skip);
        assert_eq!(card.click(x, y, &theme), Outcome::Close);
        // In the corner where a button that closed it was, a click does
        // nothing.
        let (left, top, wide, _) = layout.heading;
        let corner = (left + wide.cast_signed() - 7, top + 7);
        assert_eq!(card.click(corner.0, corner.1, &theme), Outcome::Stay);
        assert_eq!(card.click(-5, -5, &theme), Outcome::Stay);
    }

    #[test]
    fn a_choice_not_saved_is_said_until_the_next_is_or_the_step_changes() {
        let mut card = card(2, 0);
        card.refused("read only".to_owned());
        assert_eq!(card.problem.as_deref(), Some("read only"));
        card.saved(Choice::Wallpaper(0));
        assert_eq!(card.problem, None);
        card.refused("read only".to_owned());
        card.key(Key::Enter);
        assert_eq!(card.problem, None);
    }

    #[test]
    fn a_wallpaper_said_to_be_in_use_that_is_not_offered_is_none() {
        let card = Card::new(said(), shown(1, 0), (Some(5), SchemeChoice::Dark));
        assert_eq!(card.wallpaper, None);
    }

    /// How many pixels of `area` of `canvas` are not the card's own colour.
    fn inked(canvas: &Canvas, (left, top, width, height): Area) -> usize {
        // As a canvas holds it: each of red, green and blue as much of
        // itself as the colour, which is glass, is there at all.
        let color = theme().style(Part::Launcher, States::NONE).background;
        let share = |channel: u8| {
            let shared = (u16::from(channel) * u16::from(color.alpha) + 127) / 255;
            u8::try_from(shared).unwrap()
        };
        let (red, green, blue) = (share(color.red), share(color.green), share(color.blue));
        let ground = u32::from_be_bytes([color.alpha, red, green, blue]);
        let (left, top) = (left.unsigned_abs(), top.unsigned_abs());
        (top..top + height)
            .flat_map(|y| (left..left + width).map(move |x| (x, y)))
            .filter(|(x, y)| canvas.at(*x, *y) != ground)
            .count()
    }

    #[test]
    fn each_step_is_drawn_with_its_own_and_every_one_with_its_buttons() {
        let theme = theme();
        let layout = Layout::new(&theme);
        let mut card = card(2, 6);
        assert_eq!(Card::size(&theme).0, WIDTH);
        let mut text = Text::new();
        for step in 0..5 {
            card.refused("why".to_owned());
            let canvas = card.paint(&mut text, &theme, Scale::ONE);
            assert!(inked(&canvas, layout.body) > 50, "step {step}");
            assert!(inked(&canvas, layout.next) > 50, "Next, at step {step}");
            let (left, top, wide, tall) = layout.heading;
            let corner = (left + wide.cast_signed() - 20, top, 20, tall);
            assert_eq!(inked(&canvas, corner), 0, "nothing in the corner");
            assert_eq!(inked(&canvas, layout.back) > 0, step > 0, "Back, at {step}");
            card.key(Key::Enter);
        }
        // Skip with the keyboard is ringed.
        let mut ringed = self::card(0, 0);
        let plain = inked(&ringed.paint(&mut text, &theme, Scale::ONE), layout.skip);
        ringed.key(Key::Next);
        let focused = inked(&ringed.paint(&mut text, &theme, Scale::ONE), layout.skip);
        assert!(focused > plain, "{focused} {plain}");
        // Without a mark, the first step still says hello.
        let shown = Shown {
            mark: None,
            ..shown(0, 0)
        };
        let markless = Card::new(said(), shown, (None, SchemeChoice::Dark));
        let canvas = markless.paint(&mut text, &theme, Scale::ONE);
        assert!(inked(&canvas, layout.body) > 50);
    }
}
