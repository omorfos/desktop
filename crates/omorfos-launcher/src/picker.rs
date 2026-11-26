//! Picking a program: what has been typed, which programs match, which of
//! them would be started, how all that is drawn and what a key does to it.
//!
//! Nothing here knows of Wayland or of sockets.

use omorfos_config::{Color, Edges, Launcher as Show, Mode, Part, State, States, Theme};
use omorfos_i18n::{Value, Words};
use omorfos_ipc::Request;
use omorfos_ui::field::{Edit, Field, Reach, Way};
use omorfos_ui::{Area, Canvas, Motion, Scale, Text, inset};
use serde_json::json;

use crate::sources::{Does, Found, Programs, Section, Source, Sums};
use crate::{App, Icons, Numbers};
use omorfos_icons::Sign;

/// How many rows are listed at most until the launcher is told how tall
/// its screen is.
const ROWS: usize = 8;

/// The most that can be typed, in characters. No program's name is longer.
const LONGEST: usize = 64;

/// The room between a program's icon and its name.
const GAP: u32 = 10;

/// The room between a mode's sign, its name and its digit, in the row of
/// modes, and between one mode and the next.
const BESIDE: u32 = 7;
const APART: u32 = 6;

/// The narrowest the launcher is made, whatever the theme says.
const NARROWEST: u32 = 160;

/// How many programs a line of the grid has.
const COLUMNS: usize = 6;

/// The room between a program's icon in the grid and its name under it.
const UNDER: u32 = 6;

/// A key, as the launcher understands keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    /// Something to type where the cursor is, in place of what is chosen.
    Text(String),
    /// Something else done to what is typed: the cursor moved, by a
    /// character, a word or to an end, choosing as it goes or not; a
    /// character or a word taken out; a change undone or done again.
    Edit(Edit),
    /// Copy what is chosen of what is typed.
    Copy,
    /// Copy what is chosen of what is typed, and take it out.
    Cut,
    /// Choose the program above.
    Up,
    /// Choose the program below.
    Down,
    /// Start the chosen program.
    Enter,
    /// Go away without starting anything.
    Escape,
    /// Choose all that is typed, for what is typed next to take its place.
    All,
    /// Go to the mode of this number, as Super and a digit do: the first
    /// and on in the configuration's order, of those built; 0 the generic
    /// one (ADR 109).
    Mode(u8),
}

/// What the launcher shows: everything it finds, or one mode's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Whatever matches, from every source.
    Generic,
    /// One mode's own, alone.
    Mode(Mode),
}

/// The modes built so far; the rest take no number until they are.
const BUILT: [Mode; 1] = [Mode::Programs];

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
    /// The compositor is asked for this, which copies what is chosen of what
    /// is typed, and the launcher stays.
    Ask(Request),
}

/// The launcher's state: what it finds things in, what has been typed,
/// what was found for it, and which of that is chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picker {
    /// The programs there are.
    programs: Programs,
    /// Sums, worked out.
    sums: Sums,
    /// What is shown of each program besides its name, and what else is
    /// done with what is typed.
    show: Show,
    /// What has been typed, where the cursor is in it, and what is chosen.
    field: Field,
    /// What was found for what is typed, best first, each with what it is
    /// shown under: a sum's answer, if there is one, first.
    found: Vec<(Section, Found)>,
    /// Which of the rows would be taken.
    chosen: usize,
    /// How many rows are listed at most.
    room: usize,
    /// The first row listed: as far down as the wheel has scrolled, or as
    /// the arrows have taken the chosen row.
    top: usize,
    /// Whether the pointer was pressed in the field and is still held,
    /// choosing what it moves over.
    dragging: bool,
    /// Everything, or one mode's own; the programs' is a grid.
    view: View,
    /// Whether the keyboard is on the programs, Down having taken it
    /// there, rather than on what is typed: the field has no cursor then,
    /// and every arrow walks the programs.
    listing: bool,
    /// Whether the pointer is over the launcher, which shows the row of
    /// modes while nothing is typed, and the mode in that row it is over.
    hovered: bool,
    pointed: Option<View>,
    /// How large it is drawn while it is on its way to its size: laid out
    /// as wide as that, and cut off as short.
    drawn: Option<(u32, u32)>,
}

/// Where the launcher's boxes are, under a theme.
struct Layout {
    /// How wide the launcher is.
    width: u32,
    /// How tall it is with no rows: its text field, and its border and
    /// padding.
    bare: u32,
    /// The room between the field and what is under it, if anything is.
    under: u32,
    /// How tall the row of modes is, with the room the theme keeps round
    /// it.
    modes: u32,
    input: Area,
    /// The room the list has: its left, its top and its width.
    list: (i32, i32, u32),
    /// How far apart lines are, with the room the theme keeps around each.
    pitch: u32,
    /// How many programs a line has: one in the list, more in the grid.
    columns: usize,
    /// How far apart a heading's line is from the next, and the top hit's.
    heading: u32,
    top: u32,
}

impl Layout {
    /// How the list is laid out under `theme`.
    #[cfg(test)]
    fn new(theme: &Theme) -> Self {
        Self::of(theme, false)
    }

    /// How the list, or the `grid`, is laid out under `theme`. The grid's
    /// lines are as tall as a program's tile, and the launcher as wide as
    /// a line of them, if that is wider than it would be.
    fn of(theme: &Theme, grid: bool) -> Self {
        Self::across(theme, grid, None)
    }

    /// The same, as wide as `width` if it is given, as the launcher is on
    /// its way from one width to another.
    fn across(theme: &Theme, grid: bool, width: Option<u32>) -> Self {
        let launcher = theme.style(Part::Launcher, States::NONE);
        let input = theme.style(Part::LauncherInput, States::NONE);
        let item = theme.style(Part::LauncherItem, States::NONE);
        let tile = theme.style(Part::LauncherTile, States::NONE);
        let heading = theme.style(Part::LauncherHeading, States::NONE);
        let best = theme.style(Part::LauncherTop, States::NONE);
        let modes = theme.style(Part::LauncherModes, States::NONE);
        let (columns, pitch, needed) = if grid {
            let wide = tile.min_width.saturating_add(tile.margin.across());
            let tall = tile_height(theme).saturating_add(tile.margin.down());
            let line = wide.saturating_mul(u32::try_from(COLUMNS).unwrap_or(u32::MAX));
            (
                COLUMNS,
                tall,
                line.saturating_add(launcher.inset().across()),
            )
        } else {
            (1, item.height.saturating_add(item.margin.down()), 0)
        };
        let field = input.height.saturating_add(input.margin.down());
        // The field's margin under it only where something is under it.
        let under = input.margin.bottom;
        let bare = field
            .saturating_sub(under)
            .saturating_add(launcher.inset().down());
        let natural = launcher.min_width.max(NARROWEST).max(needed);
        let width = width.unwrap_or(natural);
        let whole = (0, 0, width, bare);
        let (left, top, width, _) = inset(whole, launcher.inset());
        let (x, y, wide, _) = inset((left, top, width, field), input.margin);
        Self {
            width: whole.2,
            bare,
            under,
            modes: modes.height.saturating_add(modes.margin.down()),
            input: (x, y, wide, input.height),
            list: (left, top.saturating_add_unsigned(field), width),
            pitch,
            columns,
            heading: heading.height.saturating_add(heading.margin.down()),
            top: best.height.saturating_add(best.margin.down()),
        }
    }

    /// How wide each program's place in a line is.
    fn cell(&self) -> u32 {
        let columns = u32::try_from(self.columns).unwrap_or(u32::MAX);
        self.list.2 / columns.max(1)
    }

    /// The room for the program `column` places across the line that is
    /// `place` lines down.
    fn tile(&self, place: usize, column: usize) -> Area {
        let (left, top, _, pitch) = self.row(place);
        let across = u32::try_from(column).unwrap_or(u32::MAX);
        let across = across.saturating_mul(self.cell());
        (
            left.saturating_add_unsigned(across),
            top,
            self.cell(),
            pitch,
        )
    }

    /// The whole launcher, with `rows` rows, which is one at least, under
    /// the field's margin.
    fn whole(&self, rows: usize) -> Area {
        let rows = u32::try_from(rows).unwrap_or(u32::MAX);
        let list = rows.saturating_mul(self.pitch);
        let height = self.bare.saturating_add(self.under).saturating_add(list);
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

/// The sign of `view`, before the field and in the row of modes.
const fn sign_of(view: View) -> Sign {
    match view {
        View::Mode(Mode::Programs) => Sign::Programs,
        View::Generic | View::Mode(_) => Sign::Search,
    }
}

/// The message that names `view` in the row of modes.
const fn says_of(view: View) -> &'static str {
    match view {
        View::Mode(Mode::Programs) => "launcher-mode-programs",
        View::Generic | View::Mode(_) => "launcher-mode-all",
    }
}

/// How tall a program's tile in the grid is under `theme`: its icon, half
/// as wide as the room inside its padding, the room under that, and a line
/// for its name.
fn tile_height(theme: &Theme) -> u32 {
    let tile = theme.style(Part::LauncherTile, States::NONE);
    let font = theme.font(Part::LauncherTile, States::NONE);
    let room = tile.min_width.saturating_sub(tile.inset().across());
    let line = font.size.get().saturating_mul(5) / 3;
    let inside = (room / 2).saturating_add(UNDER).saturating_add(line);
    inside.saturating_add(tile.inset().down())
}

impl Picker {
    /// A launcher offering `apps`, in the order they are given, showing
    /// of each what `show` says, and reading sums as `numbers`, in the
    /// generic mode.
    #[must_use]
    pub fn new(apps: Vec<App>, show: Show, numbers: Numbers) -> Self {
        let mut picker = Self {
            programs: Programs::new(apps, show.descriptions),
            sums: Sums::new(numbers),
            show,
            field: Field::new(LONGEST),
            found: Vec::new(),
            chosen: 0,
            room: ROWS,
            top: 0,
            dragging: false,
            view: View::Generic,
            listing: false,
            hovered: false,
            pointed: None,
            drawn: None,
        };
        picker.search();
        picker
    }

    /// Shows `view`: everything, or one mode's own, the programs' as a
    /// grid, each program's icon over its name. Typing narrows either; in
    /// the grid the arrows walk it, and Left and Right move the cursor in
    /// what is typed once there is something typed to move in.
    pub fn set_view(&mut self, view: View) {
        self.view = view;
        self.listing = false;
        self.search();
    }

    /// What is shown: everything, or one mode's own.
    #[must_use]
    pub const fn view(&self) -> View {
        self.view
    }

    /// Whether what is found is shown as a grid, as the programs' mode is.
    const fn grid(&self) -> bool {
        matches!(self.view, View::Mode(Mode::Programs))
    }

    /// How the launcher is laid out under `theme`.
    fn layout(&self, theme: &Theme) -> Layout {
        Layout::across(theme, self.grid(), self.drawn.map(|(width, _)| width))
    }

    /// Draws it `size` large, as it is on its way to the size it is going
    /// to, or as large as it is with none.
    pub const fn draw_at(&mut self, size: Option<(u32, u32)>) {
        self.drawn = size;
    }

    /// The sources of what is shown: every one that is on, or the mode's.
    fn sources(&self) -> Vec<&dyn Source> {
        let sums = self.show.sums.then_some(&self.sums as &dyn Source);
        let programs = Some(&self.programs as &dyn Source);
        match self.view {
            View::Generic => sums.into_iter().chain(programs).collect(),
            View::Mode(_) => programs.into_iter().collect(),
        }
    }

    /// Goes to the mode numbered `number`, the first and on of those built
    /// in the configuration's order, or with 0 to the generic one; a number
    /// no mode has does nothing.
    fn go_to(&mut self, number: u8) {
        let view = self.views().nth(usize::from(number));
        if let Some(view) = view {
            self.set_view(view);
        }
    }

    /// Every view there is to go to, in the order of their numbers: the
    /// generic one, and those built in the configuration's order.
    fn views(&self) -> impl Iterator<Item = View> + '_ {
        let built = self.show.order.modes().filter(|mode| BUILT.contains(mode));
        std::iter::once(View::Generic).chain(built.map(View::Mode))
    }

    /// Whether the row of modes is shown: while the pointer is over the
    /// launcher and nothing is typed.
    fn modes_shown(&self) -> bool {
        self.hovered && self.field.text().is_empty()
    }

    /// Takes in where the pointer is over the launcher, or that it has
    /// left it, which shows or hides the row of modes, and marks the mode
    /// in it the pointer is over. `text` measures the modes' names.
    /// Returns whether that changed anything.
    pub fn hover(&mut self, over: Option<(i32, i32)>, theme: &Theme, text: &mut Text) -> bool {
        let was = (self.hovered, self.pointed);
        self.hovered = over.is_some();
        self.pointed = over.and_then(|(x, y)| self.mode_at(x, y, theme, text));
        was != (self.hovered, self.pointed)
    }

    /// The mode whose button in the row of modes a point is on, if the
    /// row is shown and the point is on one.
    fn mode_at(&self, x: i32, y: i32, theme: &Theme, text: &mut Text) -> Option<View> {
        let on = |(left, top, wide, tall): Area| {
            let across = x.checked_sub(left).and_then(|x| u32::try_from(x).ok());
            let down = y.checked_sub(top).and_then(|y| u32::try_from(y).ok());
            across.is_some_and(|x| x < wide) && down.is_some_and(|y| y < tall)
        };
        let mut buttons = self.mode_areas(theme, text).into_iter();
        buttons.find(|(_, area)| on(*area)).map(|(view, _)| view)
    }

    /// Where the button for `view` is in the row of modes, while the row
    /// is shown. `text` measures the modes' names.
    pub fn mode_area(&self, view: View, theme: &Theme, text: &mut Text) -> Option<Area> {
        let mut buttons = self.mode_areas(theme, text).into_iter();
        buttons
            .find(|(each, _)| *each == view)
            .map(|(_, area)| area)
    }

    /// Each mode's button in the row of modes, left to right, as wide as
    /// its sign, its name and its digit side by side; none while the row
    /// is not shown.
    fn mode_areas(&self, theme: &Theme, text: &mut Text) -> Vec<(View, Area)> {
        if !self.modes_shown() {
            return Vec::new();
        }
        let ((left, top, _, tall), side) = self.modes_row(theme);
        let style = theme.style(Part::LauncherMode, States::NONE);
        let font = theme.font(Part::LauncherMode, States::NONE);
        let mut across = left;
        let mut areas = Vec::new();
        for (number, view) in self.views().enumerate() {
            let name = text.measure(says_of(view), font);
            let digit = (number > 0).then(|| text.measure(&number.to_string(), font));
            let digit = digit.map_or(0, |digit| BESIDE.saturating_add(digit));
            let wide = style.inset().across().saturating_add(side);
            let wide = wide.saturating_add(BESIDE).saturating_add(name);
            let wide = wide.saturating_add(digit);
            areas.push((view, (across, top, wide, tall)));
            across = across.saturating_add_unsigned(wide.saturating_add(APART));
        }
        areas
    }

    /// The room inside the row of modes, at the bottom, under what is
    /// listed, and how large each mode's sign is in it.
    fn modes_row(&self, theme: &Theme) -> (Area, u32) {
        let layout = self.layout(theme);
        let launcher = theme.style(Part::Launcher, States::NONE);
        let modes = theme.style(Part::LauncherModes, States::NONE);
        let style = theme.style(Part::LauncherMode, States::NONE);
        let (left, _, width) = layout.list;
        let above = layout.bare.saturating_sub(launcher.inset().bottom);
        let top = above.saturating_add(self.tall(&layout)).cast_signed();
        let row = inset((left, top, width, layout.modes), modes.margin);
        let inside = inset(row, modes.inset());
        let side = inside.3.saturating_sub(style.inset().down()) / 2;
        (inside, side)
    }

    /// Lists as many rows as fit in half of a screen `height` units tall,
    /// under `theme`, and one at least.
    pub fn fit(&mut self, theme: &Theme, height: u32) {
        let layout = self.layout(theme);
        let above = layout.bare.saturating_add(layout.under);
        let room = (height / 2).saturating_sub(above) / layout.pitch.max(1);
        self.room = usize::try_from(room).unwrap_or(usize::MAX).max(1);
        self.top = self.top.min(self.last_top());
    }

    /// How large the launcher is under `theme`, in units of layout: as
    /// tall as the rows it lists, up to as many as it has room for.
    #[must_use]
    pub fn size(&self, theme: &Theme) -> (u32, u32) {
        let layout = Layout::of(theme, self.grid());
        let modes = if self.modes_shown() { layout.modes } else { 0 };
        let tall = self.tall(&layout).saturating_add(modes);
        (layout.width, layout.bare.saturating_add(tall))
    }

    /// How tall what is shown under the field is: each line in sight, as
    /// tall as it is, or one to say that nothing matches.
    fn tall(&self, layout: &Layout) -> u32 {
        let shown = self.first()..self.first() + self.shown();
        let lines = shown.map(|line| self.line_height(layout, line));
        let tall = lines.fold(0, u32::saturating_add);
        let tall = if self.nothing() { layout.pitch } else { tall };
        // Parted from the field by its margin, if there is anything.
        if tall == 0 {
            0
        } else {
            tall.saturating_add(layout.under)
        }
    }

    /// How tall line `line` is: a heading's, the top hit's, or a row's.
    fn line_height(&self, layout: &Layout, line: usize) -> u32 {
        let (lead, top, _) = self.head();
        if self.is_heading(line) {
            layout.heading
        } else if top > 0 && line == lead {
            layout.top
        } else {
            layout.pitch
        }
    }

    /// The room for line `line`, of those in sight.
    fn line_area(&self, layout: &Layout, line: usize) -> Area {
        let (left, top, width) = layout.list;
        let above = (self.first()..line).map(|line| self.line_height(layout, line));
        let down = above.fold(0, u32::saturating_add);
        let tall = self.line_height(layout, line);
        (left, top.saturating_add_unsigned(down), width, tall)
    }

    /// How far from the top of a screen `height` units tall the launcher
    /// is put: where the list would be in the middle with every row it
    /// has room for, so that the text field stays where it is however
    /// many rows are listed, and in whichever mode.
    #[must_use]
    pub fn top(&self, theme: &Theme, height: u32) -> u32 {
        let list = Layout::of(theme, false);
        let above = list.bare.saturating_add(list.under);
        let rows = (height / 2).saturating_sub(above) / list.pitch.max(1);
        let rows = usize::try_from(rows).unwrap_or(usize::MAX).max(1);
        let (_, _, _, tallest) = list.whole(rows);
        height.saturating_sub(tallest) / 2
    }

    /// How many lines are shown: one, to say so, when nothing matches
    /// what is typed.
    fn shown(&self) -> usize {
        self.lines().min(self.room).max(usize::from(self.nothing()))
    }

    /// Whether something is typed, and nothing matches it.
    fn nothing(&self) -> bool {
        self.count() == 0 && !self.field.text().is_empty()
    }

    /// The furthest down the list can be scrolled: its last line at the
    /// bottom.
    fn last_top(&self) -> usize {
        self.lines().saturating_sub(self.room)
    }

    /// Scrolls the list `rows` lines down, or up if it is less than none,
    /// as far as it goes, the chosen program kept in sight: the same while
    /// it is in sight, and else the nearest that is. Says whether it moved.
    pub fn scroll(&mut self, rows: isize) -> bool {
        let top = self.top.saturating_add_signed(rows).min(self.last_top());
        let moved = top != self.top;
        self.top = top;
        // The program lit, if the programs have the keyboard, stays in
        // sight; with the field's, Enter still starts the first.
        if self.listing {
            let last = (self.top + self.room).min(self.lines()).saturating_sub(1);
            let (line, column) = self.line_of(self.chosen);
            let line = line.max(self.top).min(last);
            // Kept to a heading, it is the program beside it in sight.
            let line = match self.is_heading(line) {
                true if line == self.top => line + 1,
                true => line.saturating_sub(1),
                false => line,
            };
            self.chosen = self.nearest(line, column);
        }
        moved
    }

    /// How many programs a line has: one in the list, more in the grid.
    const fn columns(&self) -> usize {
        if self.grid() { COLUMNS } else { 1 }
    }

    /// How many lines there are: in the generic mode the heading over
    /// the top hit, the top hit's alone, and a heading over the rest if
    /// there are more; then the rest, as many to a line as there are
    /// columns.
    fn lines(&self) -> usize {
        let (lead, top, heading) = self.head();
        lead + top + heading + (self.found.len() - top).div_ceil(self.columns())
    }

    /// Which line the row `row` is on, and how far across it.
    fn line_of(&self, row: usize) -> (usize, usize) {
        let (lead, top, heading) = self.head();
        row.checked_sub(top).map_or((lead + row, 0), |rest| {
            let columns = self.columns();
            (lead + top + heading + rest / columns, rest % columns)
        })
    }

    /// The row `column` places across line `line`, if one is there: none
    /// on a heading. The top hit has a line to itself, all the way across.
    fn row_on(&self, line: usize, column: usize) -> Option<usize> {
        let (lead, top, heading) = self.head();
        let rest = line.checked_sub(lead + top + heading);
        let row = rest.map_or(0, |line| top + line * self.columns() + column);
        Some(row).filter(|row| !self.is_heading(line) && *row < self.count())
    }

    /// Whether line `line` is a heading's: over the top hit, or over what
    /// follows it.
    fn is_heading(&self, line: usize) -> bool {
        let (lead, top, heading) = self.head();
        (lead > 0 && line == 0) || (heading > 0 && line == lead + top)
    }

    /// The row on line `line` nearest `column` places across it: the
    /// last of a line that does not reach so far.
    fn nearest(&self, line: usize, column: usize) -> usize {
        let mut columns = (0..=column).rev();
        let found = columns.find_map(|column| self.row_on(line, column));
        found.unwrap_or(self.chosen)
    }

    /// Chooses the program `by` lines down, or up if less than none, as
    /// near the same place across as the line has one.
    fn move_lines(&mut self, by: isize) {
        // Past the last line there is nothing near, and the same stays
        // chosen.
        let (line, column) = self.line_of(self.chosen);
        let mut line = line.saturating_add_signed(by);
        // A heading is gone past, to what is beyond it.
        if self.is_heading(line) {
            line = line.saturating_add_signed(by);
        }
        self.chosen = self.nearest(line, column);
        self.follow();
    }

    /// Whether Left and Right walk the programs: when they have the
    /// keyboard, and in the grid while nothing is typed for them to move
    /// the cursor in.
    fn walks(&self) -> bool {
        self.listing || (self.grid() && self.field.text().is_empty())
    }

    /// What has been typed.
    #[must_use]
    pub fn query(&self) -> &str {
        self.field.text()
    }

    /// The names of the programs that match what has been typed, best
    /// first.
    pub fn matches(&self) -> impl Iterator<Item = &str> {
        let programs = self
            .found
            .iter()
            .filter(|(section, _)| *section == Section::Programs);
        programs.map(|(_, found)| found.name.as_str())
    }

    /// How many lines come before the rest: in the generic mode, the
    /// heading's over the top hit and the top hit's, the first found, a
    /// sum's answer if there is one; and the heading's over what is found
    /// after it, if anything is.
    fn head(&self) -> (usize, usize, usize) {
        let generic = matches!(self.view, View::Generic);
        let top = usize::from(generic && !self.found.is_empty());
        (top, top, usize::from(generic && self.found.len() > 1))
    }

    /// How many rows the list has.
    fn count(&self) -> usize {
        self.found.len()
    }

    /// The name of the program that would be started, or the answer that
    /// would be copied, if there is either.
    #[must_use]
    pub fn chosen(&self) -> Option<&str> {
        self.found
            .get(self.chosen)
            .map(|(_, found)| match &found.does {
                Does::Copy(answer) => answer.as_str(),
                Does::Spawn(_) => found.name.as_str(),
            })
    }

    /// Takes a key.
    pub fn key(&mut self, key: Key) -> Outcome {
        match key {
            Key::Edit(Edit::Move {
                way,
                reach: Reach::Character,
                choosing: false,
            }) if self.walks() => {
                // The first arrow from the field lights the first program.
                if self.listing {
                    let last = self.count().saturating_sub(1);
                    self.chosen = match way {
                        Way::Back => self.chosen.saturating_sub(1),
                        Way::On => (self.chosen + 1).min(last),
                    };
                    self.follow();
                } else {
                    self.listing = self.count() > 0;
                }
            }
            // Whatever is done to what is typed gives it the keyboard.
            Key::Text(text) => self.change(Edit::Type(text)),
            Key::Edit(edit) => self.change(edit),
            Key::All => self.change(Edit::All),
            Key::Copy => return self.copy(false),
            Key::Cut => return self.copy(true),
            // Up from the first line gives what is typed the keyboard
            // back.
            Key::Up if self.line_of(self.chosen).0 <= self.head().0 => {
                self.give_field_the_keyboard();
            }
            Key::Up => self.move_lines(-1),
            // Down from the field lights the first program, and goes on
            // from there.
            Key::Down if self.listing => self.move_lines(1),
            Key::Down => self.listing = self.count() > 0,
            Key::Enter => return self.start(),
            // In a mode Escape goes back to everything, and only there
            // closes the launcher.
            Key::Escape if self.view != View::Generic => self.set_view(View::Generic),
            Key::Escape => return Outcome::Close,
            Key::Mode(number) => self.go_to(number),
        }
        Outcome::Stay
    }

    /// Chooses the program whose row a point of the launcher is on, if it
    /// is on one, as when the pointer moves over it. Returns whether that
    /// changed anything.
    pub fn point(&mut self, x: i32, y: i32, theme: &Theme) -> bool {
        let over = self.row_at(x, y, theme);
        let changed = over.is_some_and(|row| !self.listing || row != self.chosen);
        self.chosen = over.unwrap_or(self.chosen);
        // Over a program, the programs have the keyboard, as after Down.
        self.listing |= over.is_some();
        changed
    }

    /// Gives what is typed the keyboard back: no program is lit, and the
    /// first that matches is the one Enter starts.
    const fn give_field_the_keyboard(&mut self) {
        self.listing = false;
        self.chosen = 0;
    }

    /// Takes a click at a point of the launcher: in the field it puts the
    /// cursor there, and chooses what the pointer is then dragged over
    /// until it is let go; on a program's row it starts that program; and
    /// anywhere else it does nothing. `text` measures what is typed.
    pub fn click(&mut self, x: i32, y: i32, theme: &Theme, text: &mut Text) -> Outcome {
        let (left, top, width, height) = self.layout(theme).input;
        let across = x.checked_sub(left).and_then(|x| u32::try_from(x).ok());
        let down = y.checked_sub(top).and_then(|y| u32::try_from(y).ok());
        self.dragging = across.is_some_and(|x| x < width) && down.is_some_and(|y| y < height);
        if self.dragging {
            self.give_field_the_keyboard();
            let at = self.place_at(x, text, theme);
            self.field.place(at, false);
            return Outcome::Stay;
        }
        if let Some(view) = self.mode_at(x, y, theme, text) {
            self.set_view(view);
            return Outcome::Stay;
        }
        let over = self.row_at(x, y, theme);
        self.chosen = over.unwrap_or(self.chosen);
        over.map_or(Outcome::Stay, |_| self.start())
    }

    /// Takes the pointer moving to a point of the launcher while it is
    /// held: pressed in the field, it chooses from where it was pressed to
    /// there. Returns whether that changed anything.
    pub fn drag(&mut self, x: i32, _: i32, theme: &Theme, text: &mut Text) -> bool {
        let at = self.dragging.then(|| self.place_at(x, text, theme));
        at.is_some_and(|at| self.field.place(at, true))
    }

    /// Takes the pointer being let go.
    pub const fn release(&mut self) {
        self.dragging = false;
    }

    /// Where what is typed is written: the field less its padding and its
    /// border.
    fn typed_area(&self, theme: &Theme) -> Area {
        let input = theme.style(Part::LauncherInput, States::NONE);
        let inside = inset(self.layout(theme).input, input.inset());
        // After the mode's sign, half as tall as the field, and the room
        // beside it.
        inset(inside, on_the_left(inside.3 / 2 + GAP))
    }

    /// The first place in what is typed that is shown: where what comes
    /// before the cursor fits in the field from, as far back as it goes,
    /// so that the cursor is always in sight.
    fn first_shown(&self, text: &mut Text, theme: &Theme) -> usize {
        let (_, _, room, _) = self.typed_area(theme);
        let font = theme.font(Part::LauncherInput, States::NONE);
        let (typed, cursor) = (self.field.text(), self.field.cursor());
        let mut places = self.field.boundaries().take_while(|at| *at <= cursor);
        let fits = places.find(|at| text.measure(&typed[*at..cursor], font) < room);
        fits.unwrap_or(cursor)
    }

    /// The place in what is typed nearest a point `x` across the launcher,
    /// as it is shown.
    fn place_at(&self, x: i32, text: &mut Text, theme: &Theme) -> usize {
        let (left, ..) = self.typed_area(theme);
        let font = theme.font(Part::LauncherInput, States::NONE);
        let first = self.first_shown(text, theme);
        let typed = self.field.text();
        let across = i64::from(x) - i64::from(left);
        let places = self.field.boundaries().filter(|at| *at >= first);
        let far = |at: &usize| {
            let to = i64::from(text.measure(&typed[first..*at], font));
            (to - across).abs()
        };
        places.min_by_key(far).unwrap_or(first)
    }

    /// Does `edit` to what is typed, and lists what matches it if that
    /// changed.
    fn change(&mut self, edit: Edit) {
        self.give_field_the_keyboard();
        let before = self.field.text().to_owned();
        self.field.edit(edit);
        if self.field.text() != before {
            self.search();
        }
    }

    /// Asks for what is chosen of what is typed to be copied, if anything
    /// is, and takes it out if `cut`.
    fn copy(&mut self, cut: bool) -> Outcome {
        let chosen = self.field.chosen_text().map(str::to_owned);
        if cut && chosen.is_some() {
            self.change(Edit::Remove {
                way: Way::Back,
                reach: Reach::Character,
            });
        }
        chosen.map_or(Outcome::Stay, |text| Outcome::Ask(Request::Copy { text }))
    }

    /// Draws the launcher, with `scale` pixels to a unit of layout, and the
    /// field's cursor if it is in the `cursor` half of its blink. The
    /// icons of the programs in sight are looked up in `icons`, if icons
    /// are shown. A row that has just been chosen, or has just stopped
    /// being, is as far on its way to its new colours as `motion` has it.
    /// When nothing matches what is typed, it says so, in `words`.
    #[must_use]
    pub fn paint(
        &self,
        (text, words): (&mut Text, &Words),
        (theme, scale, cursor): (&Theme, Scale, bool),
        icons: &mut Icons,
        motion: &mut Motion,
    ) -> Canvas {
        let layout = self.layout(theme);
        let (width, height) = self.drawn.unwrap_or_else(|| self.size(theme));
        let whole = (0, 0, width, height);
        let mut canvas = Canvas::scaled(width, height, scale);
        canvas.draw_box(whole, &theme.style(Part::Launcher, States::NONE));

        let drawing = (&mut *text, words, &mut *icons, scale);
        self.paint_field(&mut canvas, drawing, theme, (layout.input, cursor));

        // Under an answer, what Enter does with it, whatever is shown of
        // the programs.
        let copies = words.say("enter-copies", &[]);
        let described = self.show.descriptions;
        let sight = self.first()..self.first() + self.room;
        let mut drawing = (&mut canvas, &mut *text, &mut *icons, motion);
        for (row, (_, item)) in self.found.iter().enumerate() {
            let (line, column) = self.line_of(row);
            if !sight.contains(&line) {
                continue;
            }
            let place = line - self.first();
            // Lit only while the programs have the keyboard.
            let lit = self.listing && row == self.chosen;
            let states = States::NONE.when(State::Active, lit);
            // A row is the same row whichever program is in it.
            let key = u64::try_from(row).unwrap_or(u64::MAX);
            // The top hit is outlined as what Enter takes.
            let part = if line == self.head().0 && self.head().1 > 0 {
                Part::LauncherTop
            } else {
                Part::LauncherItem
            };
            let area = self.line_area(&layout, line);
            let theme = (theme, scale);
            let icon = item.icon.as_deref();
            match &item.does {
                Does::Copy(_) => {
                    let said = (item.name.as_str(), Some(copies.as_str()), None);
                    self.paint_row(&mut drawing, theme, (part, key, states), area, said);
                }
                Does::Spawn(_) if self.grid() => {
                    let area = layout.tile(place, column);
                    self.paint_tile(&mut drawing, theme, (key, states), area, item);
                }
                Does::Spawn(_) => {
                    let says = described.then_some(item.says.as_str());
                    let said = (item.name.as_str(), says, icon);
                    self.paint_row(&mut drawing, theme, (part, key, states), area, said);
                }
            }
        }
        // The headings in sight: over the top hit, and over what follows.
        let style = theme.style(Part::LauncherHeading, States::NONE);
        let font = theme.font(Part::LauncherHeading, States::NONE);
        for line in sight.filter(|line| self.is_heading(*line)) {
            let area = inset(self.line_area(&layout, line), style.inset());
            let says = if line == 0 {
                "launcher-top-hit"
            } else {
                "launcher-programs"
            };
            text.draw(&mut canvas, &words.say(says, &[]), area, style.color, font);
        }
        if self.nothing() {
            // Where the first program would be, as what a program says of
            // itself is written.
            let style = theme.style(Part::LauncherItem, States::NONE);
            let area = inset(inset(layout.row(0), style.margin), style.inset());
            let said = theme.style(Part::LauncherDescription, States::NONE);
            let font = theme.font(Part::LauncherDescription, States::NONE);
            let typed = [("typed", Value::Words(self.field.text()))];
            let nothing = words.say("nothing-matches", &typed);
            text.draw(&mut canvas, &nothing, area, said.color, font);
        }
        self.paint_modes(&mut canvas, (text, words, icons, scale), theme);
        canvas
    }

    /// Draws the row of modes, if it is shown: a hairline over it, and
    /// each mode's sign, name and digit, the one shown lit and the one
    /// under the pointer marked.
    fn paint_modes(
        &self,
        canvas: &mut Canvas,
        (text, words, icons, scale): (&mut Text, &Words, &mut Icons, Scale),
        theme: &Theme,
    ) {
        let buttons = self.mode_areas(theme, text);
        let modes = theme.style(Part::LauncherModes, States::NONE);
        let ((left, top, width, _), side) = self.modes_row(theme);
        if !buttons.is_empty() {
            let (left, top) = (
                left.saturating_sub_unsigned(modes.inset().left),
                top.saturating_sub_unsigned(modes.inset().top),
            );
            let row = (
                left,
                top,
                width.saturating_add(modes.inset().across()),
                self.layout(theme).modes,
            );
            canvas.draw_box(inset(row, modes.margin), &modes);
        }
        let dim = theme.style(Part::LauncherDescription, States::NONE).color;
        for (number, (view, area)) in buttons.into_iter().enumerate() {
            let shown = view == self.view;
            let states = States::NONE
                .when(State::Active, shown)
                .when(State::Hovered, self.pointed == Some(view));
            let style = theme.style(Part::LauncherMode, states);
            let font = theme.font(Part::LauncherMode, states);
            canvas.draw_box(area, &style);
            let (x, y, wide, tall) = inset(area, style.inset());
            let down = y.saturating_add_unsigned(tall.saturating_sub(side) / 2);
            let pixels = scale.pixels(side.cast_signed()).unsigned_abs();
            let drawn = icons.sign(sign_of(view), pixels, style.color).into_iter();
            drawn.for_each(|image| canvas.draw_image((x, down, side, side), image));
            let after = side.saturating_add(BESIDE);
            let name = words.say(says_of(view), &[]);
            let at = (
                x.saturating_add_unsigned(after),
                y,
                wide.saturating_sub(after),
                tall,
            );
            text.draw(canvas, &name, at, style.color, font);
            if number > 0 {
                let beyond = after.saturating_add(text.measure(&name, font));
                let beyond = beyond.saturating_add(BESIDE);
                let at = (
                    x.saturating_add_unsigned(beyond),
                    y,
                    wide.saturating_sub(beyond),
                    tall,
                );
                let color = if shown { style.color } else { dim };
                text.draw(canvas, &number.to_string(), at, color, font);
            }
        }
    }

    /// Draws a row of the list at `place`: `name`, what it `says` under
    /// it if anything, and its icon beside them if icons are shown, with
    /// the colours of `states`, as far on their way as the row `key`'s are.
    fn paint_row(
        &self,
        (canvas, text, icons, motion): &mut Drawing<'_>,
        (theme, scale): (&Theme, Scale),
        (part, key, states): (Part, u64, States),
        place: Area,
        (name, says, icon): (&str, Option<&str>, Option<&str>),
    ) {
        let style = motion.style(theme, part, key, states);
        let area = inset(place, style.margin);
        canvas.draw_box(area, &style);
        let mut room = inset(area, style.inset());
        if self.show.icons {
            // A square as tall as the row's two lines.
            let side = room.3;
            let square = (room.0, room.1, side, side);
            draw_icon(canvas, icons, (icon, square, scale), style.color);
            room = inset(room, on_the_left(side.saturating_add(GAP)));
        }
        let font = theme.font(part, states);
        if let Some(says) = says {
            // The name in the top half, what it says under it.
            let (left, top, width, height) = room;
            let half = height / 2;
            let under = (
                left,
                top.saturating_add_unsigned(half),
                width,
                height - half,
            );
            text.draw(canvas, name, (left, top, width, half), style.color, font);
            let said = motion.style(theme, Part::LauncherDescription, key, states);
            let ink = said.color;
            let font = theme.font(Part::LauncherDescription, states);
            text.draw(canvas, says, under, ink, font);
        } else {
            text.draw(canvas, name, room, style.color, font);
        }
    }

    /// Draws a program's tile in the grid at `place`: its icon, if icons
    /// are shown, over its name in the middle, with the colours of
    /// `states`, as far on their way as the tile `key`'s are.
    fn paint_tile(
        &self,
        (canvas, text, icons, motion): &mut Drawing<'_>,
        (theme, scale): (&Theme, Scale),
        (key, states): (u64, States),
        place: Area,
        found: &Found,
    ) {
        let style = motion.style(theme, Part::LauncherTile, key, states);
        let area = inset(place, style.margin);
        canvas.draw_box(area, &style);
        let (left, top, width, height) = inset(area, style.inset());
        let side = width / 2;
        if self.show.icons {
            let square = (left.saturating_add_unsigned(side / 2), top, side, side);
            let icon = found.icon.as_deref();
            draw_icon(canvas, icons, (icon, square, scale), style.color);
        }
        let below = side.saturating_add(UNDER);
        let line = (
            left,
            top.saturating_add_unsigned(below),
            width,
            height.saturating_sub(below),
        );
        let font = theme.font(Part::LauncherTile, states);
        text.draw_centered(canvas, &found.name, line, style.color, font);
    }

    /// Draws the field, at `field`, with what is typed in it as far as it
    /// is in sight, and what is chosen of it, or the cursor while it is in
    /// the `cursor` half of its blink.
    fn paint_field(
        &self,
        canvas: &mut Canvas,
        (text, words, icons, scale): (&mut Text, &Words, &mut Icons, Scale),
        theme: &Theme,
        (field, cursor): (Area, bool),
    ) {
        let input = theme.style(Part::LauncherInput, States::NONE);
        let font = theme.font(Part::LauncherInput, States::NONE);
        canvas.draw_box(field, &input);
        // The mode's sign before what is typed, in the middle of the field
        // down, and its words in the field while it is empty.
        let inside = inset(field, input.inset());
        let side = inside.3 / 2;
        let down = inside.1.saturating_add_unsigned((inside.3 - side) / 2);
        let square = (inside.0, down, side, side);
        let sign = sign_of(self.view);
        let says = match self.view {
            View::Mode(Mode::Programs) => "launcher-search-programs",
            View::Generic | View::Mode(_) => "launcher-search",
        };
        let pixels = scale.pixels(side.cast_signed()).unsigned_abs();
        let drawn = icons.sign(sign, pixels, input.color).into_iter();
        drawn.for_each(|image| canvas.draw_image(square, image));
        let typed = self.typed_area(theme);
        if self.field.text().is_empty() {
            let said = theme.style(Part::LauncherDescription, States::NONE);
            let words = words.say(says, &[]);
            text.draw(canvas, &words, typed, said.color, font);
        }
        let (left, top, room, tall) = typed;
        // From where the cursor is in sight, what comes before it cut off.
        let first = self.first_shown(text, theme);
        let typed_in = self.field.text();
        let across = |text: &mut Text, at: usize| {
            text.measure(&typed_in[first..at.max(first)], font)
                .min(room.saturating_sub(1))
        };
        // Cut at the field's edge, never short: it is what scrolls.
        text.draw_cut(
            canvas,
            &typed_in[first..],
            typed,
            0..room,
            input.color,
            font,
        );
        let chosen = self.field.chosen();
        if !chosen.is_empty() {
            // What is chosen on what the theme marks it with, and no line
            // where the next character goes, as it goes in its place.
            let marked = theme.style(Part::LauncherInput, States::NONE.with(State::Active));
            let (start, end) = (across(text, chosen.start), across(text, chosen.end));
            let mark = (
                left.saturating_add_unsigned(start),
                top,
                end.saturating_sub(start),
                tall,
            );
            canvas.fill(mark, marked.ground());
            // The same line again, as far across as the mark, so that
            // every letter stands where it stood.
            let shown = &typed_in[first..];
            text.draw_cut(canvas, shown, typed, start..end, marked.color, font);
        } else if cursor && !self.listing {
            // A line where the next character will go, which stays in the
            // box however much has been typed; none while the programs
            // have the keyboard, nor while it blinks off.
            let margin = tall / 5;
            let caret = (
                left.saturating_add_unsigned(across(text, self.field.cursor())),
                top.saturating_add_unsigned(margin),
                1,
                tall.saturating_sub(margin * 2),
            );
            canvas.fill(caret, input.color);
        }
    }

    /// The first row listed.
    const fn first(&self) -> usize {
        self.top
    }

    /// Scrolls the list only as far as it takes to keep the chosen
    /// program's line in sight, as the arrows move it.
    fn follow(&mut self) {
        let (line, _) = self.line_of(self.chosen);
        let lowest = (line + 1).saturating_sub(self.room);
        // The top hit with its heading over it.
        let highest = if line == self.head().0 { 0 } else { line };
        self.top = self.top.min(highest).max(lowest);
    }

    /// Which of the programs that match is shown at a point, if any is.
    fn row_at(&self, x: i32, y: i32, theme: &Theme) -> Option<usize> {
        let layout = self.layout(theme);
        let (left, top, width) = layout.list;
        let across = u32::try_from(x.saturating_sub(left)).ok();
        let down = u32::try_from(y.saturating_sub(top)).ok();
        let inside = across.zip(down).filter(|(across, _)| *across < width);
        // The line whose room reaches down past the point, of those in
        // sight, each as tall as it is.
        let sight = self.first()..self.first() + self.room;
        let line = inside.and_then(|(_, down)| {
            let mut reach = 0;
            sight.clone().find(|line| {
                reach += self.line_height(&layout, *line);
                down < reach
            })
        });
        // What is left over past the last column is the last column's.
        let last = self.columns() - 1;
        let column =
            inside.and_then(|(across, _)| usize::try_from(across / layout.cell().max(1)).ok());
        let place = line.zip(column.map(|column| column.min(last)));
        place.and_then(|(line, column)| self.row_on(line, column))
    }

    /// Finds what matches what has been typed in each source shown, a
    /// sum's answer first and then the programs, as each ranks them. The
    /// first of all is the one Enter takes.
    fn search(&mut self) {
        let query = self.field.text();
        // With nothing typed, the mode that finds everything shows nothing.
        let empty = matches!(self.view, View::Generic) && query.is_empty();
        let found = self.sources().into_iter().flat_map(|source| {
            let section = source.section();
            source
                .find(query)
                .into_iter()
                .map(move |found| (section, found))
        });
        self.found = if empty { Vec::new() } else { found.collect() };
        self.chosen = 0;
        self.top = 0;
    }

    /// Asks for the chosen program to be started, or the chosen answer to
    /// be copied, if there is either.
    fn start(&self) -> Outcome {
        match self.found.get(self.chosen).map(|(_, found)| &found.does) {
            Some(Does::Copy(answer)) => Outcome::Start(Request::Copy {
                text: answer.clone(),
            }),
            Some(Does::Spawn(command)) => Outcome::Start(Request::Perform {
                action: json!({ "spawn": command }),
            }),
            None => Outcome::Stay,
        }
    }
}

/// What the launcher is drawn on and with: the canvas, what draws text,
/// the icons there are, and how far each row is on its way to its colours.
type Drawing<'a> = (&'a mut Canvas, &'a mut Text, &'a mut Icons, &'a mut Motion);

/// Draws the icon called `icon` in `square`, at `scale`, in `ink` where it
/// is drawn in one colour, if it is there to draw.
fn draw_icon(
    canvas: &mut Canvas,
    icons: &mut Icons,
    (icon, square, scale): (Option<&str>, Area, Scale),
    ink: Color,
) {
    let pixels = scale.pixels(square.2.cast_signed()).unsigned_abs();
    let icon = icon.and_then(|icon| icons.get(icon, pixels, ink));
    if let Some(icon) = icon {
        canvas.draw_image(square, icon);
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

    use omorfos_config::Command;

    use super::*;

    /// Backspace, taking out the character before the cursor.
    const BACKSPACE: Key = Key::Edit(Edit::Remove {
        way: Way::Back,
        reach: Reach::Character,
    });

    /// Names alone, which most of what is tested here is easiest seen in,
    /// with sums worked out.
    const NAMES: Show = Show {
        icons: false,
        descriptions: false,
        sums: true,
        order: omorfos_config::ModeOrder::BUILT_IN,
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
        Picker::new(apps, NAMES, Numbers::Point)
    }

    /// The launcher's picture under the built-in theme.
    fn painted(picker: &Picker) -> Canvas {
        let (theme, mut icons) = (Theme::default(), Icons::default());
        still(picker, &theme, Scale::ONE, &mut icons)
    }

    /// The launcher's picture with nothing on its way anywhere.
    fn still(picker: &Picker, theme: &Theme, scale: Scale, icons: &mut Icons) -> Canvas {
        let words = Words::in_languages(&omorfos_i18n::LAUNCHER, &[]);
        let text = (&mut Text::new(), &words);
        picker.paint(text, (theme, scale, true), icons, &mut Motion::new())
    }

    /// Where what is typed begins, across, and the middle of its line,
    /// down, under the built-in theme.
    fn field_at(theme: &Theme) -> (i32, i32) {
        let (left, top, _, tall) = several().typed_area(theme);
        (left, top + tall.cast_signed() / 2)
    }

    /// Where the line where the next character goes is drawn, across the
    /// middle of the field: what is drawn with it and not without.
    fn cursor(picker: &Picker) -> Vec<u32> {
        let theme = Theme::default();
        let words = Words::in_languages(&omorfos_i18n::LAUNCHER, &[]);
        let paint = |on: bool| {
            let text = (&mut Text::new(), &words);
            let look = (&theme, Scale::ONE, on);
            picker.paint(text, look, &mut Icons::default(), &mut Motion::new())
        };
        let (on, off) = (paint(true), paint(false));
        let (left, top, wide, tall) = picker.layout(&theme).input;
        let middle = top.unsigned_abs() + tall / 2;
        let columns = left.unsigned_abs()..left.unsigned_abs() + wide;
        columns
            .filter(|x| on.at(*x, middle) != off.at(*x, middle))
            .collect()
    }

    fn several() -> Picker {
        picker(&["Calculator", "Files", "Firefox", "Terminal", "Text Editor"])
    }

    /// The five of [`several`], every one found: what each says of itself,
    /// that it is one of sorts, looked through for what is typed. The
    /// first is the top hit, the rest under their heading.
    fn every() -> Picker {
        let names = ["Calculator", "Files", "Firefox", "Terminal", "Text Editor"];
        let show = Show {
            descriptions: true,
            ..NAMES
        };
        let mut picker = Picker::new(
            names.iter().map(|name| app(name)).collect(),
            show,
            Numbers::Point,
        );
        typed(&mut picker, "sorts");
        picker
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

    /// A colour as a canvas holds it: each of red, green and blue as much
    /// of itself as the colour is there at all.
    fn argb(color: Color) -> u32 {
        let share = |channel: u8| {
            let shared = (u16::from(channel) * u16::from(color.alpha) + 127) / 255;
            u8::try_from(shared).unwrap()
        };
        let (red, green, blue) = (share(color.red), share(color.green), share(color.blue));
        u32::from_be_bytes([color.alpha, red, green, blue])
    }

    /// The colour a row is filled with when it is the chosen one.
    fn lit(theme: &Theme) -> u32 {
        let chosen = States::NONE.with(State::Active);
        argb(theme.style(Part::LauncherItem, chosen).background)
    }

    /// The middle of the row `place` rows down the list, under the
    /// built-in theme.
    /// The middle of the line `place` lines down what is found in the
    /// mode that finds everything, with several found: the heading over
    /// the top hit, the top hit, the heading over the rest, and the rest.
    fn row(place: i32) -> (i32, i32) {
        let layout = Layout::new(&Theme::default());
        let (left, top, width) = layout.list;
        let height = |line: i32| match line {
            0 | 2 => layout.heading,
            1 => layout.top,
            _ => layout.pitch,
        };
        let above: u32 = (0..place).map(height).sum();
        let middle = above + height(place) / 2;
        (left + (width / 2).cast_signed(), top + middle.cast_signed())
    }

    #[test]
    fn nothing_is_offered_until_something_is_typed_but_in_the_grid_everything() {
        let picker = several();
        assert_eq!(picker.query(), "");
        assert_eq!(names_of(&picker).len(), 0);
        assert_eq!(picker.chosen(), None);
        let mut picker = several();
        picker.set_view(View::Mode(Mode::Programs));
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
        assert_eq!(picker.key(BACKSPACE), Outcome::Stay);
        assert_eq!(names_of(&picker), ["Files", "Firefox"]);
        for _ in 0..5 {
            picker.key(BACKSPACE);
        }
        assert_eq!(picker.query(), "");
        assert_eq!(names_of(&picker).len(), 0, "nothing typed, nothing");
    }

    #[test]
    fn what_is_typed_is_edited_where_the_cursor_is_and_what_is_chosen_copied_or_cut() {
        let mut picker = several();
        typed(&mut picker, "fiefox");
        assert_eq!(names_of(&picker), Vec::<&str>::new());
        let left = Key::Edit(Edit::Move {
            way: Way::Back,
            reach: Reach::Character,
            choosing: false,
        });
        for _ in 0..4 {
            assert_eq!(picker.key(left.clone()), Outcome::Stay);
        }
        typed(&mut picker, "r");
        assert_eq!(picker.query(), "firefox");
        assert_eq!(names_of(&picker), ["Firefox"]);
        // Nothing chosen is nothing to copy or cut.
        assert_eq!(picker.key(Key::Copy), Outcome::Stay);
        assert_eq!(picker.key(Key::Cut), Outcome::Stay);
        assert_eq!(picker.query(), "firefox");
        let copy = Outcome::Ask(Request::Copy {
            text: "firefox".to_owned(),
        });
        picker.key(Key::All);
        assert_eq!(picker.key(Key::Copy), copy, "the launcher staying");
        assert_eq!(picker.query(), "firefox");
        assert_eq!(picker.key(Key::Cut), copy);
        assert_eq!(picker.query(), "");
        assert_eq!(names_of(&picker).len(), 0, "nothing again");
        picker.key(Key::Edit(Edit::Undo));
        assert_eq!(names_of(&picker), ["Firefox"]);
        // Moving the cursor alone lists nothing anew: the first is chosen
        // still, wherever the list was.
        picker.key(Key::Down);
        picker.key(left);
        assert_eq!(picker.chosen(), Some("Firefox"));
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
    fn all_that_is_typed_can_be_chosen_for_the_next_key_to_take_its_place() {
        let mut picker = several();
        typed(&mut picker, "fire");
        assert_eq!(picker.key(Key::All), Outcome::Stay);
        typed(&mut picker, "t");
        assert_eq!(picker.query(), "t", "typed in place of all of it");
        typed(&mut picker, "er");
        assert_eq!(picker.query(), "ter", "and then after it, as usual");
        picker.key(Key::All);
        picker.key(BACKSPACE);
        assert_eq!(picker.query(), "", "all of it taken out");
        assert_eq!(names_of(&picker).len(), 0);
        // With nothing typed there is nothing to choose.
        picker.key(Key::All);
        typed(&mut picker, "f");
        picker.key(BACKSPACE);
        assert_eq!(picker.query(), "");
        // The arrows leave it chosen.
        typed(&mut picker, "fi");
        picker.key(Key::All);
        picker.key(Key::Down);
        typed(&mut picker, "t");
        assert_eq!(picker.query(), "t");
    }

    /// Which columns of what is typed, in the upper half of its line, hold
    /// anything but `ground`, under the built-in theme.
    fn inked_columns(picker: &Picker, ground: impl Fn(u32) -> u32) -> Vec<bool> {
        let (left, top, room, tall) = several().typed_area(&Theme::default());
        let canvas = painted(picker);
        let (left, top) = (left.unsigned_abs(), top.unsigned_abs());
        let inked = |x: u32| (top..top + tall / 2).any(|y| canvas.at(x, y) != ground(x));
        (left..left + room).map(inked).collect()
    }

    #[test]
    fn text_wider_than_the_field_is_cut_at_its_edge_and_never_short_with_an_ellipsis() {
        let theme = Theme::default();
        let mut picker = picker(&["Files"]);
        // Each is tall, where the dots of an ellipsis are low.
        typed(&mut picker, &"W".repeat(60));
        let home = Edit::Move {
            way: Way::Back,
            reach: Reach::Line,
            choosing: false,
        };
        picker.key(Key::Edit(home));
        let inked = inked_columns(&picker, |_| {
            argb(
                Theme::default()
                    .style(Part::Launcher, States::NONE)
                    .background,
            )
        });
        let font = theme.font(Part::LauncherInput, States::NONE);
        let letter = usize::try_from(Text::new().measure("W", font)).unwrap();
        let last = &inked[inked.len() - letter..];
        assert!(last.contains(&true), "letters up to the edge");
    }

    #[test]
    fn choosing_a_part_of_what_is_typed_leaves_every_letter_where_it_was() {
        let theme = Theme::default();
        let marked = theme.style(Part::LauncherInput, States::NONE.with(State::Active));
        let (left, top, _, _) = several().typed_area(&theme);
        let (left, top) = (left.unsigned_abs(), top.unsigned_abs());
        // On the mark where it is chosen, and on the field elsewhere.
        let inked = |picker: &Picker| {
            let canvas = painted(picker);
            let on_mark = |x: u32| canvas.at(left + x, top) == argb(marked.background);
            // The field has no ground of its own: off the mark, the
            // launcher's glass.
            let glass = theme.style(Part::Launcher, States::NONE).background;
            let ground = |x: u32| {
                let ground = if on_mark(x - left) {
                    marked.background
                } else {
                    glass
                };
                argb(ground)
            };
            let marks = (0..200).filter(|x| on_mark(*x)).count();
            (inked_columns(picker, ground), marks)
        };
        // Letters that sit closer together than each is wide, in a font
        // that has them so.
        let mut picker = picker(&["Files"]);
        typed(&mut picker, "WAVE To AVA");
        picker.key(Key::All);
        let (all, whole) = inked(&picker);
        let step = |way, choosing| {
            Key::Edit(Edit::Move {
                way,
                reach: Reach::Character,
                choosing,
            })
        };
        picker.key(step(Way::Back, false));
        for _ in 0..2 {
            picker.key(step(Way::On, false));
        }
        for _ in 0..5 {
            picker.key(step(Way::On, true));
        }
        let (part, some) = inked(&picker);
        assert!(some > 5 && some < whole, "{some} of {whole} chosen");
        assert_eq!(part, all);
    }

    #[test]
    fn what_is_typed_when_all_of_it_is_chosen_is_drawn_on_the_themes_mark() {
        let theme = Theme::default();
        let layout = Layout::new(&theme);
        let input = theme.style(Part::LauncherInput, States::NONE);
        let marked = theme.style(Part::LauncherInput, States::NONE.with(State::Active));
        assert_ne!(input.background, marked.background);
        let (_, top, _, height) = layout.input;
        // Just inside where the text starts, past the mode's sign, half-way
        // down.
        let at = (
            several().typed_area(&theme).0.unsigned_abs() + 1,
            top.unsigned_abs() + height / 2,
        );
        let mut picker = several();
        typed(&mut picker, "fire");
        let ground = |picker: &Picker| {
            let canvas = painted(picker);
            let row = (at.0..at.0 + 30).map(|x| canvas.at(x, at.1));
            row.filter(|pixel| *pixel == argb(marked.background))
                .count()
        };
        assert_eq!(ground(&picker), 0);
        picker.key(Key::All);
        // Between the letters, as far across as they go.
        assert!(ground(&picker) > 5, "the mark behind the text");
        typed(&mut picker, "f");
        assert_eq!(ground(&picker), 0);
    }

    #[test]
    fn the_arrows_choose_among_the_programs_that_match() {
        let mut picker = every();
        assert_eq!(picker.key(Key::Up), Outcome::Stay);
        assert_eq!(picker.chosen(), Some("Calculator"), "there is none above");
        // The first Down lights the first, and the next go on from it.
        picker.key(Key::Down);
        assert_eq!(picker.chosen(), Some("Calculator"));
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
        picker.key(Key::All);
        typed(&mut picker, "f");
        assert_eq!(picker.chosen(), Some("Files"));
        // With nothing matching there is nothing to move among.
        typed(&mut picker, "zzz");
        picker.key(Key::Down);
        assert_eq!(picker.chosen(), None);
    }

    #[test]
    fn the_wheel_scrolls_the_list_and_the_pointer_chooses_in_it_without_moving_it() {
        let theme = Theme::default();
        // The heading, Calculator the top hit, the heading, Files, Firefox,
        // Terminal and Text Editor: seven lines, two in sight.
        let mut picker = every();
        picker.room = 2;
        // With the field's keyboard, what Enter starts is still the first.
        assert!(picker.scroll(1));
        assert_eq!(picker.chosen(), Some("Calculator"));
        assert!(picker.scroll(-1));
        picker.key(Key::Down);
        assert!(picker.scroll(1));
        assert_eq!(picker.first(), 1);
        assert_eq!(picker.chosen(), Some("Calculator"), "still in sight");
        // Out of sight, the chosen row is the nearest in sight, past the
        // heading over all but the first.
        assert!(picker.scroll(1));
        assert_eq!(picker.chosen(), Some("Files"));
        for _ in 0..3 {
            assert!(picker.scroll(1));
        }
        assert!(!picker.scroll(1), "no further than the last");
        assert_eq!(picker.first(), 5);
        assert_eq!(picker.chosen(), Some("Terminal"));
        // The pointer on the second row in sight chooses it, and the list
        // stays where it was scrolled to.
        let (x, y, _, tall) = Layout::new(&theme).row(1);
        assert!(picker.point(x + 1, y + tall.cast_signed() / 2, &theme));
        assert_eq!(picker.chosen(), Some("Text Editor"));
        assert_eq!(picker.first(), 5);
        // While it is in sight, the chosen row stays as the list scrolls.
        assert!(picker.scroll(-1));
        assert_eq!(picker.chosen(), Some("Terminal"), "the nearest in sight");
        picker.key(Key::Up);
        assert!(picker.scroll(-1));
        assert_eq!((picker.first(), picker.chosen()), (3, Some("Firefox")));
        // Scrolled to the top, the nearest in sight is the top hit, past
        // the heading.
        assert!(picker.scroll(-10));
        assert!(!picker.scroll(-1), "no further than the first");
        assert_eq!(picker.chosen(), Some("Calculator"));
        // The arrows bring the chosen row back into sight, and no further.
        for _ in 0..2 {
            picker.key(Key::Down);
        }
        assert_eq!((picker.first(), picker.chosen()), (3, Some("Firefox")));
        // With three in sight, scrolled back to the top, the last in sight
        // is the heading: the nearest program is the one over it.
        picker.room = 3;
        assert!(picker.scroll(-10));
        assert_eq!((picker.first(), picker.chosen()), (0, Some("Calculator")));
        // Typing lists from the top again.
        typed(&mut picker, "t");
        assert_eq!(picker.first(), 0);
    }

    #[test]
    fn when_nothing_matches_it_says_so_under_what_is_typed() {
        let theme = Theme::default();
        let layout = Layout::new(&theme);
        let mut launcher = several();
        typed(&mut launcher, "zzz");
        let (_, tall) = launcher.size(&theme);
        let under = layout.bare + layout.under;
        assert_eq!(tall, under + layout.pitch, "a row to say so");
        let canvas = painted(&launcher);
        let (left, top, width, height) = layout.row(0);
        let (left, top) = (left.unsigned_abs(), top.unsigned_abs());
        let mut colours: Vec<u32> = (top..top + height)
            .flat_map(|y| (left..left + width).map(move |x| (x, y)))
            .map(|(x, y)| canvas.at(x, y))
            .collect();
        colours.sort_unstable();
        colours.dedup();
        assert!(colours.len() > 2, "written");
        // With nothing typed there is nothing to say, even with nothing to
        // offer.
        let none = picker(&[]);
        assert_eq!(none.size(&theme).1, layout.bare);
        // And it is said in the user's language.
        let words = Words::in_languages(&omorfos_i18n::LAUNCHER, &["pl".to_owned()]);
        let said = words.say("nothing-matches", &[("typed", Value::Words("zzz"))]);
        assert!(
            said.contains("zzz") && !said.starts_with("Nothing"),
            "{said}"
        );
    }

    #[test]
    fn enter_asks_for_the_chosen_program_to_be_started() {
        let mut picker = several();
        typed(&mut picker, "fire");
        assert_eq!(picker.key(Key::Enter), spawn(&["firefox", "--go"]));
        let mut picker = every();
        picker.key(Key::Down);
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
        let mut picker = Picker::new(vec![top], NAMES, Numbers::Point);
        typed(&mut picker, "top");
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
        let mut picker = every();
        // Under the heading over all but the first, Files, then Firefox.
        let (x, y) = row(4);
        assert!(picker.point(x, y, &theme));
        assert_eq!(picker.chosen(), Some("Firefox"));
        assert!(!picker.point(x, y + 1, &theme), "it was already");
        // Not where there is no program, nor off the list.
        let (x, below) = row(7);
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
        let mut picker = every();
        // Under the heading over all but the first.
        let (x, y) = row(5);
        assert_eq!(
            picker.click(x, y, &theme, &mut Text::new()),
            spawn(&["terminal", "--go"])
        );
        assert_eq!(
            picker.click(x, 2, &theme, &mut Text::new()),
            Outcome::Stay,
            "on the text field"
        );
        let (x, below) = row(8);
        assert_eq!(
            picker.click(x, below, &theme, &mut Text::new()),
            Outcome::Stay
        );
    }

    #[test]
    fn the_launcher_is_as_tall_as_what_it_lists_up_to_half_the_screen() {
        let theme = Theme::default();
        // A border and padding each side, and the text field; and under
        // it, if anything is, the field's margin.
        let (bare, under, wide) = (2 * 13 + 56, 8, 480);
        // Nothing typed, the field alone.
        let mut picker = several();
        picker.fit(&theme, 1080);
        assert_eq!(picker.size(&theme), (wide, bare));
        // Five rows, each of two lines, and a heading of 24 over the top
        // hit and another over the rest, with room for nine.
        let mut picker = every();
        picker.fit(&theme, 1080);
        assert_eq!(picker.size(&theme), (wide, bare + under + 2 * 24 + 5 * 48));
        let mut picker = several();
        typed(&mut picker, "fi");
        assert_eq!(picker.size(&theme), (wide, bare + under + 2 * 24 + 2 * 48));
        typed(&mut picker, "zzz");
        assert_eq!(
            picker.size(&theme),
            (wide, bare + under + 48),
            "a row to say so"
        );
        // A smaller screen has room for fewer: three lines, two of them
        // headings.
        let mut picker = every();
        picker.fit(&theme, 480);
        assert_eq!(picker.size(&theme), (wide, bare + under + 2 * 24 + 48));
        picker.fit(&theme, 100);
        assert_eq!(
            picker.size(&theme),
            (wide, bare + under + 24),
            "one at least"
        );
        // Shorter rows, more of them.
        let wide = Theme::parse(
            "launcher { min-width: 600px; padding: 0; border: none; }\n\
             launcher-input { margin: 0; height: 40px; }\n\
             launcher-item { height: 20px; margin: 1px 0; }",
        )
        .unwrap();
        let many: Vec<String> = (0..20).map(|n| format!("Program {n:02}")).collect();
        let mut long = self::picker(&many.iter().map(String::as_str).collect::<Vec<_>>());
        typed(&mut long, "program");
        long.fit(&wide, 480);
        // Its two headings and its top hit as the built-in theme has them.
        assert_eq!(long.size(&wide), (600, 40 + 2 * 24 + 48 + 6 * 22));
        let narrow = Theme::parse("launcher { min-width: 10px; }").unwrap();
        assert_eq!(long.size(&narrow).0, NARROWEST);

        let mut picker = every();
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
        // Nine rows at most: 522 tall in all.
        assert_eq!(picker.top(&theme, 1080), (1080 - 522) / 2);
        typed(&mut picker, "zzz");
        assert_eq!(picker.top(&theme, 1080), (1080 - 522) / 2, "however few");
        picker.fit(&theme, 100);
        assert_eq!(picker.top(&theme, 100), 0, "however small the screen");
    }

    #[test]
    fn the_chosen_program_is_lit_and_no_other() {
        let theme = Theme::default();
        let surface = argb(theme.style(Part::Launcher, States::NONE).background);
        let mut picker = every();
        // While what is typed has the keyboard, no program is lit.
        let canvas = painted(&picker);
        let (x, y) = row(3);
        assert_eq!(canvas.at(x.unsigned_abs() + 200, y.unsigned_abs()), surface);
        picker.key(Key::Down);
        picker.key(Key::Down);
        let canvas = painted(&picker);
        let at = |place: i32| {
            let (x, y) = row(place);
            // Towards the right-hand end, clear of the name.
            canvas.at(x.unsigned_abs() + 200, y.unsigned_abs())
        };
        // The second, under the heading over all but the first.
        assert_eq!(at(3), lit(&theme));
        assert_eq!(at(1), surface);
        assert_eq!(at(4), surface);
        // Nor is there anything where there is no program: the launcher
        // ends under the last.
        let (_, last) = row(6);
        let (_, after) = row(7);
        assert!(canvas.height() > last.unsigned_abs());
        assert!(canvas.height() < after.unsigned_abs());
    }

    #[test]
    fn what_is_typed_is_shown_with_a_mark_where_the_next_character_goes() {
        let theme = Theme::default();
        let layout = Layout::new(&theme);
        let input = theme.style(Part::LauncherInput, States::NONE);
        let (left, _, width, _) = layout.input;
        let mut picker = several();
        // At the start of where the text goes, past the mode's sign.
        let start = picker.typed_area(&theme).0.unsigned_abs();
        assert_eq!(cursor(&picker), [start]);
        typed(&mut picker, "fire");
        let some = cursor(&picker);
        assert!(some.len() == 1 && some[0] > start, "{some:?}");
        // However much is typed, the mark stays in the field.
        typed(&mut picker, &"w".repeat(60));
        let full = cursor(&picker);
        let end = left.unsigned_abs() + width - input.padding.right;
        assert!(full.last().unwrap() < &end, "{full:?} within {end}");
    }

    #[test]
    fn what_programs_say_of_themselves_is_shown_and_searched_only_if_asked_for() {
        let names = ["Calculator", "Files", "Sorter"];
        // Calculator and Files found, Files under the heading.
        let mut plain = picker(&names);
        typed(&mut plain, "l");
        let show = Show {
            descriptions: true,
            ..NAMES
        };
        let mut described = Picker::new(
            names.iter().map(|name| app(name)).collect(),
            show,
            Numbers::Point,
        );
        typed(&mut described, "l");

        // Under each name there is something written, and nothing
        // without; the name is in the top half of its row with it, and in
        // the middle without: the second, under the heading.
        let (_, middle) = row(3);
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
        described.key(Key::All);
        typed(&mut described, "sort");
        assert_eq!(names_of(&described), ["Sorter", "Calculator", "Files"]);
        plain.key(Key::All);
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
        let mut with = Picker::new(
            names.iter().map(|name| app(name)).collect(),
            show,
            Numbers::Point,
        );
        // Both found, the first lit, to see where the icon's room is
        // against it.
        typed(&mut with, "l");
        with.key(Key::Down);
        let canvas = still(&with, &theme, Scale::ONE, &mut icons);
        let green = |canvas: &Canvas, place: i32| {
            let y = row(place).1.unsigned_abs();
            let columns = 0..canvas.width();
            columns.filter(|x| canvas.at(*x, y) == 0xff00_ff00).count()
        };
        // Files, under the heading, has an icon as tall as the row's two
        // lines; Calculator has none, and room is left where it would be.
        assert_eq!(green(&canvas, 3), 36);
        assert_eq!(green(&canvas, 1), 0);
        // Calculator is the top hit, outlined: its room for an icon is
        // inside its border too.
        let layout = Layout::new(&theme);
        let best = theme.style(Part::LauncherTop, States::NONE);
        let start = layout.list.0.unsigned_abs() + best.inset().left;
        let side = best.height - best.inset().down();
        let y = row(1).1.unsigned_abs();
        let chosen = lit(&theme);
        let beside = start..start + side + GAP;
        assert!(beside.clone().all(|x| canvas.at(x, y) == chosen));

        // Without icons asked for, none is drawn and the name starts there.
        let mut without = picker(&names);
        typed(&mut without, "l");
        without.key(Key::Down);
        let canvas = still(&without, &theme, Scale::ONE, &mut icons);
        assert_eq!(green(&canvas, 3), 0);
        assert!(beside.into_iter().any(|x| canvas.at(x, y) != chosen));
    }

    fn copy(text: &str) -> Outcome {
        Outcome::Start(Request::Copy {
            text: text.to_owned(),
        })
    }

    #[test]
    fn a_click_in_the_field_puts_the_cursor_there_and_a_drag_chooses() {
        let theme = Theme::default();
        let font = theme.font(Part::LauncherInput, States::NONE).clone();
        let mut text = Text::new();
        let mut picker = picker(&["Files", "Lens"]);
        typed(&mut picker, "files");
        let (left, y) = field_at(&theme);
        // Before the first letter, and between the second and the third.
        assert_eq!(picker.click(left, y, &theme, &mut text), Outcome::Stay);
        picker.release();
        typed(&mut picker, "x");
        assert_eq!(picker.query(), "xfiles");
        let between = left + text.measure("xfi", &font).cast_signed();
        picker.click(between, y, &theme, &mut text);
        picker.release();
        typed(&mut picker, "y");
        assert_eq!(picker.query(), "xfiyles");
        // Pressed at the start and dragged past the end: all of it chosen,
        // for what is typed next to take its place.
        picker.click(left, y, &theme, &mut text);
        assert!(picker.drag(left + 400, y, &theme, &mut text));
        assert!(!picker.drag(left + 400, y, &theme, &mut text), "no further");
        picker.release();
        typed(&mut picker, "l");
        assert_eq!(picker.query(), "l");
        // Once let go, moving chooses nothing; nor does a press elsewhere.
        assert!(!picker.drag(left, y, &theme, &mut text));
        picker.click(left, y + 200, &theme, &mut text);
        assert!(!picker.drag(left, y, &theme, &mut text));
    }

    #[test]
    fn text_wider_than_the_field_is_shown_from_where_the_cursor_can_be_seen() {
        let theme = Theme::default();
        let mut text = Text::new();
        let mut picker = picker(&["Files"]);
        assert_eq!(picker.first_shown(&mut text, &theme), 0, "nothing typed");
        typed(&mut picker, &"w".repeat(60));
        let first = picker.first_shown(&mut text, &theme);
        assert!(first > 0, "{first}");
        // The cursor is at the end, in sight: what comes before it, from
        // there, fits, and from a character sooner it would not.
        let (_, _, room, _) = several().typed_area(&theme);
        let font = theme.font(Part::LauncherInput, States::NONE).clone();
        assert!(text.measure(&picker.query()[first..], &font) < room);
        assert!(text.measure(&picker.query()[first - 1..], &font) >= room);
        // A click at the start of the field puts the cursor where it is
        // shown from.
        let (left, y) = field_at(&theme);
        picker.click(left, y, &theme, &mut text);
        picker.release();
        typed(&mut picker, "x");
        assert_eq!(picker.query().find('x'), Some(first));
        // With the cursor at the start, it is shown from the start.
        let home = Edit::Move {
            way: Way::Back,
            reach: Reach::Line,
            choosing: false,
        };
        picker.key(Key::Edit(home));
        assert_eq!(picker.first_shown(&mut text, &theme), 0);
        let canvas = painted(&picker);
        assert!(canvas.width() > 0);
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
        // The answer changes as the sum is typed.
        typed(&mut picker, "*3");
        assert_eq!(picker.chosen(), Some("8"));
        // Another sign keeps the answer so far, to go on from.
        typed(&mut picker, "+");
        assert_eq!(picker.chosen(), Some("8"));
        typed(&mut picker, "2");
        assert_eq!(picker.chosen(), Some("10"));
        // Taken back to a number alone, it is no sum.
        for _ in 0..5 {
            picker.key(BACKSPACE);
        }
        assert_eq!(picker.query(), "2+");
        assert_eq!(picker.chosen(), None);
        assert_eq!(picker.key(Key::Enter), Outcome::Stay);
    }

    #[test]
    fn a_sum_is_read_and_answered_as_numbers_are_written_where_the_user_is() {
        let apps = ["Calculator"].map(app).to_vec();
        let mut picker = Picker::new(apps, NAMES, Numbers::Comma);
        typed(&mut picker, "1 000,5+1");
        assert_eq!(picker.chosen(), Some("1001,5"));
        assert_eq!(picker.key(Key::Enter), copy("1001,5"));
    }

    #[test]
    fn sums_turned_off_are_looked_for_among_the_programs_as_anything_typed() {
        let show = Show {
            sums: false,
            ..NAMES
        };
        let apps = ["Calculator", "Counter 1+1"].map(app).to_vec();
        let mut picker = Picker::new(apps, show, Numbers::Point);
        typed(&mut picker, "1+1");
        assert_eq!(picker.chosen(), Some("Counter 1+1"));
        assert_eq!(names_of(&picker), ["Counter 1+1"]);
        typed(&mut picker, "0");
        assert_eq!(picker.chosen(), None);
    }

    #[test]
    fn the_programs_that_match_a_sum_are_listed_under_its_answer() {
        let mut picker = picker(&["Calculator", "Counter 1+1"]);
        typed(&mut picker, "1+1");
        assert_eq!(picker.chosen(), Some("2"));
        assert_eq!(picker.key(Key::Down), Outcome::Stay);
        assert_eq!(picker.chosen(), Some("2"), "lit");
        picker.key(Key::Down);
        assert_eq!(picker.chosen(), Some("Counter 1+1"));
        picker.key(Key::Down);
        assert_eq!(picker.chosen(), Some("Counter 1+1"), "the last");
        assert_eq!(picker.key(Key::Enter), spawn(&["counter 1+1", "--go"]));
        picker.key(Key::Up);
        assert_eq!(picker.key(Key::Enter), copy("2"));

        // The pointer and clicks find the answer's row as a program's; the
        // program is under the heading.
        let theme = Theme::default();
        let (x, y) = row(3);
        assert!(picker.point(x, y, &theme));
        assert_eq!(picker.chosen(), Some("Counter 1+1"));
        let (x, y) = row(1);
        assert_eq!(picker.click(x, y, &theme, &mut Text::new()), copy("2"));
        let (x, below) = row(4);
        assert_eq!(
            picker.click(x, below, &theme, &mut Text::new()),
            Outcome::Stay
        );
    }

    #[test]
    fn an_answer_is_written_on_its_row_with_how_to_copy_it_under_it() {
        let theme = Theme::default();
        let layout = Layout::new(&theme);
        let (left, _, width) = layout.list;
        let (_, middle) = row(1);
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
            let mut picker = Picker::new(Vec::new(), show, Numbers::Point);
            typed(&mut picker, "6*7");
            // Lit, to see where its room is against it.
            picker.key(Key::Down);
            picker
        };
        let described = sum(Show {
            descriptions: true,
            ..NAMES
        });
        assert!(inked(&described, top.clone()) > 20, "the answer");
        assert!(inked(&described, bottom.clone()) > 20, "how to copy it");
        // Whether or not what programs say of themselves is shown.
        let plain = sum(NAMES);
        assert!(inked(&plain, top.clone()) > 20, "the answer");
        assert!(inked(&plain, bottom) > 20, "how to copy it");
        // With icons, it is written where the programs' names are.
        let iconed = sum(Show {
            icons: true,
            ..NAMES
        });
        let canvas = painted(&iconed);
        let best = theme.style(Part::LauncherTop, States::NONE);
        let start = left.unsigned_abs() + best.inset().left;
        let lines = (middle - half + 4..middle + half - 4).map(i32::unsigned_abs);
        let lit_at = |x: u32| lines.clone().all(|y| canvas.at(x, y) == lit(&theme));
        let beside = start..start + best.height - best.inset().down() + GAP;
        assert!(beside.clone().all(lit_at));
        let past = beside.end..beside.end + 40;
        assert!(!past.into_iter().all(lit_at));
    }

    #[test]
    fn a_long_list_moves_to_keep_the_chosen_program_in_sight() {
        let theme = Theme::default();
        let many: Vec<String> = (0..20).map(|n| format!("Program {n:02}")).collect();
        let many: Vec<&str> = many.iter().map(String::as_str).collect();
        let mut picker = picker(&many);
        typed(&mut picker, "program");
        // The first Down lights the first, the top hit, and the next goes
        // past the heading.
        for _ in 0..12 {
            picker.key(Key::Down);
        }
        assert_eq!(picker.chosen(), Some("Program 11"));
        // It is the last of the eight in sight.
        // Scrolled past the headings, every line in sight is a row's.
        let canvas = painted(&picker);
        let row = |place: usize| {
            let (x, y, _, tall) = Layout::new(&theme).row(place);
            (x + 200, y + tall.cast_signed() / 2)
        };
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

    /// Eight programs, more than a line of the grid has.
    const EIGHT: [&str; 8] = [
        "Archive", "Books", "Calendar", "Clock", "Files", "Maps", "Music", "Photos",
    ];

    /// The arrows Left and Right, as the shell's typing gives them.
    const fn across(way: Way) -> Key {
        Key::Edit(Edit::Move {
            way,
            reach: Reach::Character,
            choosing: false,
        })
    }

    fn grid(names: &[&str]) -> Picker {
        let mut picker = picker(names);
        picker.set_view(View::Mode(Mode::Programs));
        picker
    }

    /// The middle of the tile `column` places across the line `line`
    /// lines down the grid, under the built-in theme.
    fn tile(line: usize, column: usize) -> (i32, i32) {
        let (left, top, width, height) = Layout::of(&Theme::default(), true).tile(line, column);
        (
            left + (width / 2).cast_signed(),
            top + (height / 2).cast_signed(),
        )
    }

    #[test]
    fn the_grid_has_six_programs_to_a_line_and_the_arrows_walk_it() {
        let mut picker = grid(&EIGHT);
        let mut press = |key: Key| {
            assert_eq!(picker.key(key), Outcome::Stay);
            picker.chosen().map(str::to_owned)
        };
        // The first arrow lights the first; the next go on from it.
        assert_eq!(press(across(Way::On)).as_deref(), Some("Archive"));
        assert_eq!(press(across(Way::On)).as_deref(), Some("Books"));
        assert_eq!(press(Key::Down).as_deref(), Some("Photos"), "a line down");
        assert_eq!(press(Key::Up).as_deref(), Some("Books"));
        assert_eq!(press(across(Way::Back)).as_deref(), Some("Archive"));
        assert_eq!(press(across(Way::Back)).as_deref(), Some("Archive"));
        assert_eq!(press(Key::Up).as_deref(), Some("Archive"));
        // From the fifth across, down is the last of a shorter line.
        for _ in 0..4 {
            press(across(Way::On));
        }
        assert_eq!(press(Key::Down).as_deref(), Some("Photos"));
        assert_eq!(press(Key::Down).as_deref(), Some("Photos"));
        assert_eq!(press(across(Way::On)).as_deref(), Some("Photos"));
        // The list has a program to a line, and its Left and Right are
        // the field's.
        let mut list = Picker::new(
            EIGHT.iter().map(|name| app(name)).collect(),
            NAMES,
            Numbers::Point,
        );
        // All but Archive, Calendar and Clock have an s, Books first.
        typed(&mut list, "s");
        list.key(across(Way::On));
        assert_eq!(list.chosen(), Some("Books"));
        list.key(Key::Down);
        list.key(Key::Down);
        assert_eq!(list.chosen(), Some("Files"));
        // Without icons, a tile keeps their room, and nothing is in it.
        let canvas = painted(&grid(&EIGHT));
        let layout = Layout::of(&Theme::default(), true);
        let style = Theme::default().style(Part::LauncherTile, States::NONE);
        let (left, top, width, _) = inset(layout.tile(0, 1), style.margin);
        let (left, top) = (left.unsigned_abs(), top.unsigned_abs());
        let ground = canvas.at(left + 2, top + 2);
        let room = (left + width / 4..left + width * 3 / 4).map(|x| canvas.at(x, top + 20));
        assert!(room.into_iter().all(|pixel| pixel == ground));
    }

    #[test]
    fn once_something_is_typed_the_grid_narrows_and_left_and_right_move_the_cursor() {
        let mut picker = grid(&EIGHT);
        typed(&mut picker, "m");
        assert_eq!(names_of(&picker), ["Maps", "Music"]);
        picker.key(across(Way::Back));
        picker.key(across(Way::On));
        assert_eq!(picker.chosen(), Some("Maps"), "not walked");
        // The cursor went back and on again, to the end.
        typed(&mut picker, "u");
        assert_eq!(names_of(&picker), ["Music"]);
        // Emptied, the arrows walk it again, the first lighting the first.
        picker.key(BACKSPACE);
        picker.key(BACKSPACE);
        picker.key(across(Way::On));
        picker.key(across(Way::On));
        assert_eq!(picker.chosen(), Some("Books"));
    }

    #[test]
    fn the_grid_is_as_wide_as_a_line_of_tiles_and_scrolls_by_lines() {
        let theme = Theme::default();
        let names: Vec<String> = (0..20).map(|n| format!("Program {n:02}")).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let mut picker = grid(&names);
        let layout = Layout::of(&theme, true);
        let tile = theme.style(Part::LauncherTile, States::NONE);
        let launcher = theme.style(Part::Launcher, States::NONE);
        let wide = 6 * tile.min_width + launcher.inset().across();
        assert_eq!(picker.size(&theme).0, wide);
        assert!(wide > Layout::new(&theme).width);
        // Room for two lines of the four.
        let two = layout.bare + layout.under + 2 * layout.pitch;
        picker.fit(&theme, 2 * two);
        assert_eq!(picker.size(&theme).1, two);
        picker.key(Key::Down);
        assert!(picker.scroll(1));
        assert_eq!(picker.chosen(), Some("Program 06"), "kept in sight");
        assert!(picker.scroll(5));
        assert!(!picker.scroll(1), "the last line at the bottom");
        assert_eq!(picker.chosen(), Some("Program 12"));
        // The arrows take the view with them.
        picker.key(Key::Up);
        picker.key(Key::Up);
        assert_eq!(picker.chosen(), Some("Program 00"));
        assert!(picker.scroll(1));
    }

    #[test]
    fn the_pointer_chooses_the_tile_it_is_over_and_a_click_starts_it() {
        let theme = Theme::default();
        let mut picker = grid(&EIGHT);
        let (x, y) = tile(0, 2);
        assert!(picker.point(x, y, &theme));
        assert_eq!(picker.chosen(), Some("Calendar"));
        let (x, y) = tile(1, 1);
        assert!(picker.point(x, y, &theme));
        assert_eq!(picker.chosen(), Some("Photos"));
        // Where no program is, nothing is chosen.
        let (x, y) = tile(1, 3);
        assert!(!picker.point(x, y, &theme));
        assert_eq!(picker.chosen(), Some("Photos"));
        let (x, y) = tile(0, 1);
        let mut text = Text::new();
        let started = picker.click(x, y, &theme, &mut text);
        assert_eq!(started, spawn(&["books", "--go"]));
        // A launcher wider than a line of tiles leaves what is over past
        // the last column to the last column, not to the next line.
        let wide = Theme::parse("launcher { min-width: 1000px; }").unwrap();
        let layout = Layout::of(&wide, true);
        let (left, top, width) = layout.list;
        assert!(width > 6 * layout.cell(), "something left over");
        let y = top + (layout.pitch / 2).cast_signed();
        assert!(picker.point(left + width.cast_signed() - 1, y, &wide));
        assert_eq!(picker.chosen(), Some("Maps"));
    }

    #[test]
    fn each_tile_has_its_icon_over_its_name_and_the_chosen_one_is_lit() {
        let root = tempfile::TempDir::new().unwrap();
        let green = "<svg xmlns='http://www.w3.org/2000/svg' width='8' height='8'>\
                     <rect width='8' height='8' fill='#00ff00'/></svg>";
        std::fs::write(root.path().join("books.svg"), green).unwrap();
        let mut icons = Icons::new(vec![root.path().to_owned()]);
        let theme = Theme::default();
        let show = Show {
            icons: true,
            ..NAMES
        };
        let apps = EIGHT.iter().map(|name| app(name)).collect();
        let mut picker = Picker::new(apps, show, Numbers::Point);
        picker.set_view(View::Mode(Mode::Programs));
        // The first lit, as Down from the field lights it.
        picker.key(Key::Down);
        let canvas = still(&picker, &theme, Scale::ONE, &mut icons);
        let layout = Layout::of(&theme, true);
        let style = theme.style(Part::LauncherTile, States::NONE);
        let inside = |line, column| {
            inset(
                inset(layout.tile(line, column), style.margin),
                style.inset(),
            )
        };
        // Books has its icon, half as wide as the tile's room, in the
        // middle at its top; Archive has none.
        let green_in = |(left, top, width, height): Area| {
            let xs = left.unsigned_abs()..(left.unsigned_abs() + width);
            let ys = top.unsigned_abs()..(top.unsigned_abs() + height);
            let all = xs.flat_map(|x| ys.clone().map(move |y| (x, y)));
            all.filter(|(x, y)| canvas.at(*x, *y) == 0xff00_ff00)
                .count()
        };
        let books = inside(0, 1);
        let side = books.2 / 2;
        assert_eq!(green_in(books), (side * side) as usize);
        let square = (books.0 + (side / 2).cast_signed(), books.1, side, side);
        assert_eq!(green_in(square), (side * side) as usize);
        assert_eq!(green_in(inside(0, 0)), 0);
        // Under the icon, the name: ink in its line, of the tile's colour.
        let ink = |(left, top, width, height): Area, color: u32| {
            let below = top + (side + UNDER).cast_signed();
            let xs = left.unsigned_abs()..(left.unsigned_abs() + width);
            let ys = below.unsigned_abs()..(top.unsigned_abs() + height);
            let mut all = xs.flat_map(|x| ys.clone().map(move |y| (x, y)));
            all.any(|(x, y)| canvas.at(x, y) == color)
        };
        let on_accent = theme
            .style(Part::LauncherTile, States::NONE.with(State::Active))
            .color;
        assert!(ink(inside(0, 0), argb(on_accent)), "the chosen one's name");
        assert!(ink(inside(0, 1), argb(style.color)), "another's");
        // The chosen one is lit, and no other.
        let lit_tile = argb(
            theme
                .style(Part::LauncherTile, States::NONE.with(State::Active))
                .background,
        );
        let corner = |(left, top, _, _): Area| canvas.at(left.unsigned_abs(), top.unsigned_abs());
        assert_eq!(corner(inside(0, 0)), lit_tile);
        assert_ne!(corner(inside(0, 1)), lit_tile);
    }

    #[test]
    fn down_gives_the_programs_the_keyboard_and_every_arrow_walks_them() {
        let mut picker = several();
        typed(&mut picker, "f");
        assert_eq!(names_of(&picker), ["Files", "Firefox"]);
        // Down lights the first, rather than going past it.
        picker.key(Key::Down);
        assert_eq!(picker.chosen(), Some("Files"));
        picker.key(across(Way::On));
        assert_eq!(picker.chosen(), Some("Firefox"), "walked, not the cursor");
        picker.key(across(Way::Back));
        assert_eq!(picker.chosen(), Some("Files"));
        picker.key(across(Way::On));
        assert_eq!(picker.chosen(), Some("Firefox"));
        assert_eq!(picker.query(), "f");
        // Up from the first gives the field the keyboard back, and Left
        // moves its cursor again.
        picker.key(Key::Up);
        picker.key(Key::Up);
        assert_eq!(picker.chosen(), Some("Files"));
        picker.key(across(Way::Back));
        typed(&mut picker, "x");
        assert_eq!(picker.query(), "xf");
        // Typing gives it back too.
        let mut picker = several();
        typed(&mut picker, "f");
        picker.key(Key::Down);
        typed(&mut picker, "i");
        picker.key(across(Way::Back));
        typed(&mut picker, "x");
        assert_eq!(picker.query(), "fxi");
        // With nothing to walk, Down leaves the keyboard where it is.
        let mut none = several();
        typed(&mut none, "zz");
        none.key(Key::Down);
        none.key(across(Way::Back));
        typed(&mut none, "y");
        assert_eq!(none.query(), "zyz");
    }

    #[test]
    fn in_the_grid_down_gives_its_programs_the_keyboard_once_something_is_typed() {
        let mut picker = grid(&EIGHT);
        typed(&mut picker, "m");
        picker.key(Key::Down);
        picker.key(across(Way::On));
        assert_eq!(picker.chosen(), Some("Music"));
        // A click in the field gives the field the keyboard back.
        let theme = Theme::default();
        let (x, top, _, tall) = picker.typed_area(&theme);
        let mut text = Text::new();
        picker.click(x, top + tall.cast_signed() / 2, &theme, &mut text);
        picker.release();
        // With the field's keyboard, Enter's is the first again, and Right
        // moves the cursor rather than to the next program.
        assert_eq!(picker.chosen(), Some("Maps"));
        picker.key(across(Way::On));
        assert_eq!(picker.chosen(), Some("Maps"), "the cursor moved");
    }

    #[test]
    fn the_field_has_no_cursor_while_the_programs_have_the_keyboard() {
        let theme = Theme::default();
        // How many columns the cursor takes: drawn while it blinks on, and
        // never while it blinks off, as `cursor` compares.
        let inked = |picker: &Picker| cursor(picker).len();
        // In the grid, where every program is found with nothing typed.
        let mut picker = several();
        picker.set_view(View::Mode(Mode::Programs));
        assert_eq!(inked(&picker), 1, "the cursor alone");
        picker.key(Key::Down);
        assert_eq!(inked(&picker), 0);
        picker.key(Key::Up);
        assert_eq!(inked(&picker), 1);
        // The pointer over a program, even the first, lights it and takes
        // the keyboard to the programs as Down does.
        let (x, y) = tile(0, 0);
        assert!(picker.point(x, y, &theme));
        assert_eq!(inked(&picker), 0);
        assert!(!picker.point(x, y, &theme), "nothing new");
        let canvas = painted(&picker);
        let tiled = theme.style(Part::LauncherTile, States::NONE.with(State::Active));
        assert_eq!(
            canvas.at(x.unsigned_abs(), (y - 30).unsigned_abs()),
            argb(tiled.background)
        );
    }

    #[test]
    fn super_and_a_digit_go_to_the_modes_shown_and_super_and_0_back() {
        let mut picker = several();
        assert_eq!(picker.view(), View::Generic);
        assert_eq!(picker.key(Key::Mode(1)), Outcome::Stay);
        assert_eq!(picker.view(), View::Mode(Mode::Programs));
        // No other mode is built yet: the second is nothing.
        picker.key(Key::Mode(2));
        assert_eq!(picker.view(), View::Mode(Mode::Programs));
        picker.key(Key::Mode(0));
        assert_eq!(picker.view(), View::Generic);
        // Escape in a mode goes back, and only then closes.
        picker.key(Key::Mode(1));
        assert_eq!(picker.key(Key::Escape), Outcome::Stay);
        assert_eq!(picker.view(), View::Generic);
        assert_eq!(picker.key(Key::Escape), Outcome::Close);
        // Going to a mode gives what is typed the keyboard, nothing lit:
        // the first arrow lights the first again.
        picker.key(Key::Down);
        picker.key(Key::Mode(1));
        picker.key(across(Way::On));
        assert_eq!(picker.chosen(), Some("Calculator"));
        // Ordered otherwise, the programs are on their own digit.
        let order = omorfos_config::ModeOrder::try_from(vec![Mode::Actions, Mode::Programs]);
        let show = Show {
            order: order.unwrap(),
            ..NAMES
        };
        let apps = ["Files", "Firefox"].iter().map(|name| app(name)).collect();
        let mut ordered = Picker::new(apps, show, Numbers::Point);
        // Actions are not built yet, and take no number: the programs are
        // still the first.
        ordered.key(Key::Mode(1));
        assert_eq!(ordered.view(), View::Mode(Mode::Programs));
    }

    #[test]
    fn the_programs_mode_is_the_grid_and_finds_no_sums() {
        let mut picker = picker(&["1+1 Trainer", "Files"]);
        picker.key(Key::Mode(1));
        let wide = picker.size(&Theme::default()).0;
        assert_eq!(wide, Layout::of(&Theme::default(), true).width);
        typed(&mut picker, "1+1");
        assert_eq!(
            picker.chosen(),
            Some("1+1 Trainer"),
            "no answer in the grid"
        );
        picker.key(Key::Mode(0));
        assert_eq!(
            picker.size(&Theme::default()).0,
            Layout::new(&Theme::default()).width
        );
        assert_eq!(picker.chosen(), Some("2"), "the answer in the generic mode");
    }

    #[test]
    fn the_generic_mode_shows_a_top_hit_and_the_rest_under_their_heading() {
        let mut picker = several();
        typed(&mut picker, "fi");
        assert_eq!(names_of(&picker), ["Files", "Firefox"]);
        // The top hit under its heading, the other heading, and the rest
        // under it.
        assert!(picker.is_heading(0) && picker.is_heading(2));
        assert_eq!(picker.line_of(0), (1, 0));
        assert_eq!(picker.line_of(1), (3, 0));
        assert_eq!(picker.lines(), 4);
        assert_eq!(picker.key(Key::Enter), spawn(&["files", "--go"]));
        // One found, a top hit and no heading over nothing.
        typed(&mut picker, "r");
        assert!(picker.is_heading(0) && !picker.is_heading(2));
        assert_eq!(picker.lines(), 2);
        // A mode has no top hit and no heading.
        let tiles = grid(&["Files", "Firefox"]);
        assert!(!tiles.is_heading(0));
        assert_eq!(tiles.line_of(1), (0, 1));
    }

    #[test]
    fn a_sum_is_the_top_hit_and_the_programs_follow() {
        let mut picker = picker(&["Calculator", "Counter 1+1"]);
        typed(&mut picker, "1+1");
        assert_eq!(picker.chosen(), Some("2"));
        assert!(picker.is_heading(2));
        assert_eq!(picker.line_of(1), (3, 0));
        assert_eq!(picker.key(Key::Enter), copy("2"));
    }

    #[test]
    fn the_arrows_and_the_pointer_never_stop_on_a_heading() {
        let theme = Theme::default();
        let mut picker = several();
        typed(&mut picker, "fi");
        picker.key(Key::Down);
        assert_eq!(picker.chosen(), Some("Files"), "the top hit lit");
        picker.key(Key::Down);
        assert_eq!(picker.chosen(), Some("Firefox"), "past the heading");
        picker.key(Key::Up);
        assert_eq!(picker.chosen(), Some("Files"));
        // The pointer over the heading lights nothing.
        let (x, y) = row(2);
        assert!(!picker.point(x, y, &theme));
        assert_eq!(picker.chosen(), Some("Files"));
        let (x, y) = row(3);
        assert!(picker.point(x, y, &theme));
        assert_eq!(picker.chosen(), Some("Firefox"));
    }

    #[test]
    fn a_heading_is_drawn_in_the_launchers_words_and_never_lit() {
        let theme = Theme::default();
        let mut picker = several();
        typed(&mut picker, "fi");
        picker.key(Key::Down);
        let canvas = painted(&picker);
        let (left, _, width) = Layout::new(&theme).list;
        let (_, y) = row(2);
        let ground = argb(theme.style(Part::Launcher, States::NONE).background);
        let across = left.unsigned_abs()..left.unsigned_abs() + width;
        let inked = across
            .clone()
            .filter(|x| canvas.at(*x, y.unsigned_abs()) != ground);
        assert!(inked.count() > 5, "its words");
        assert!(
            across
                .into_iter()
                .all(|x| canvas.at(x, y.unsigned_abs()) != lit(&theme))
        );
    }

    #[test]
    fn nothing_typed_shows_nothing_and_nothing_found_one_line_with_no_gap() {
        let theme = Theme::default();
        let layout = Layout::new(&theme);
        let mut picker = several();
        assert_eq!(picker.lines(), 0);
        assert_eq!(picker.size(&theme).1, layout.bare, "the field alone");
        assert_eq!(picker.key(Key::Enter), Outcome::Stay);
        typed(&mut picker, "zzz");
        let under = layout.bare + layout.under;
        assert_eq!(picker.size(&theme).1, under + layout.pitch);
        // Headings are as tall as theirs, not a row's.
        let mut picker = several();
        typed(&mut picker, "fi");
        let tall = 2 * layout.heading + layout.top + layout.pitch;
        assert_eq!(picker.size(&theme).1, under + tall);
    }

    #[test]
    fn the_top_hit_is_outlined_and_the_headings_written_dim() {
        let theme = Theme::default();
        let mut picker = several();
        typed(&mut picker, "fi");
        let canvas = painted(&picker);
        let layout = Layout::new(&theme);
        // The top hit's outline, along its top, in its border's colour.
        let (left, top, width, _) = picker.line_area(&layout, 1);
        let style = theme.style(Part::LauncherTop, States::NONE);
        let (left, top) = (left.unsigned_abs(), top.unsigned_abs());
        let glass = canvas.at(left + 1, top.saturating_sub(4));
        let edge = (left + 20..left + width - 20).map(|x| canvas.at(x, top));
        assert!(
            edge.into_iter().all(|pixel| pixel != glass),
            "{:?}",
            style.border.top
        );
        // A heading's words have ink, in its colour, not lit.
        let (left, top, width, tall) = picker.line_area(&layout, 0);
        let (left, top) = (left.unsigned_abs(), top.unsigned_abs());
        let points = (left..left + width).flat_map(|x| (top..top + tall).map(move |y| (x, y)));
        let ground = canvas.at(left + width - 2, top + 1);
        let inked = points.filter(|(x, y)| canvas.at(*x, *y) != ground).count();
        assert!(inked > 10, "the heading's words");
    }

    #[test]
    fn the_field_has_the_modes_sign_before_it_and_its_words_while_empty() {
        let theme = Theme::default();
        let layout = Layout::new(&theme);
        let (left, top, _, tall) = layout.input;
        let glass = argb(theme.style(Part::Launcher, States::NONE).background);
        let inked = |canvas: &Canvas, xs: std::ops::Range<u32>| {
            let ys = top.unsigned_abs()..top.unsigned_abs() + tall;
            xs.flat_map(|x| ys.clone().map(move |y| (x, y)))
                .filter(|(x, y)| canvas.at(*x, *y) != glass)
                .count()
        };
        // The sign in the field's start, before where the text begins.
        let mut picker = several();
        let text_left = picker.typed_area(&theme).0.unsigned_abs();
        let sign = left.unsigned_abs()..text_left;
        let generic = painted(&picker);
        assert!(
            inked(&generic, sign.clone()) > 20,
            "the generic mode's sign"
        );
        // The mode's words, while nothing is typed: well along the field.
        let words = text_left + 60..text_left + 120;
        assert!(inked(&generic, words.clone()) > 20, "its words");
        typed(&mut picker, "f");
        assert_eq!(
            inked(&painted(&picker), words.clone()),
            0,
            "gone as one types"
        );
        // Another mode, another sign and other words.
        let mut grid = several();
        grid.set_view(View::Mode(Mode::Programs));
        let tiles = painted(&grid);
        let (tile_left, _, _, _) = grid.typed_area(&theme);
        let tile_sign = Layout::of(&theme, true).input.0.unsigned_abs()..tile_left.unsigned_abs();
        let picture = |canvas: &Canvas, xs: std::ops::Range<u32>, y: u32| {
            xs.map(|x| canvas.at(x, y)).collect::<Vec<_>>()
        };
        let middle = top.unsigned_abs() + tall / 2;
        assert_ne!(
            picture(&generic, sign.clone(), middle),
            picture(&tiles, tile_sign, middle),
            "another sign"
        );
    }

    #[test]
    fn the_empty_launcher_has_as_much_room_under_the_field_as_over_it() {
        let theme = Theme::default();
        let launcher = theme.style(Part::Launcher, States::NONE);
        let input = theme.style(Part::LauncherInput, States::NONE);
        let edge = launcher.inset().top;
        assert_eq!(edge, launcher.inset().bottom);
        let empty = several();
        assert_eq!(empty.size(&theme).1, 2 * edge + input.height);
        let (_, top, _, tall) = Layout::new(&theme).input;
        assert_eq!(top.unsigned_abs(), edge);
        assert_eq!(empty.size(&theme).1 - (top.unsigned_abs() + tall), edge);
        // With something under it, the field's margin parts the two.
        let mut found = several();
        typed(&mut found, "zzz");
        let under = input.margin.bottom + Layout::new(&theme).pitch;
        assert_eq!(found.size(&theme).1, 2 * edge + input.height + under);
    }

    #[test]
    fn every_mode_puts_the_field_at_the_same_height_on_the_screen() {
        let theme = Theme::default();
        for screen in [1080, 720, 480, 100] {
            let mut picker = several();
            picker.fit(&theme, screen);
            let generic = picker.top(&theme, screen);
            picker.set_view(View::Mode(Mode::Programs));
            picker.fit(&theme, screen);
            assert_eq!(picker.top(&theme, screen), generic, "on {screen}");
            // And the grid, as tall as it can be, still on the screen.
            let (_, _, _, tallest) = picker.layout(&theme).whole(picker.room);
            assert!(generic + tallest <= screen.max(tallest), "on {screen}");
        }
    }

    /// The middle of the button for `view` in the row of modes, while it
    /// is shown.
    fn mode_middle(picker: &Picker, view: View) -> Option<(i32, i32)> {
        let theme = Theme::default();
        let area = picker.mode_area(view, &theme, &mut Text::new());
        area.map(|(left, top, wide, tall)| {
            let middle = |at: i32, across: u32| at.saturating_add_unsigned(across / 2);
            (middle(left, wide), middle(top, tall))
        })
    }

    #[test]
    fn the_modes_are_offered_at_the_bottom_while_the_pointer_is_over_an_empty_launcher() {
        let theme = Theme::default();
        let modes = theme.style(Part::LauncherModes, States::NONE);
        let row = modes.height + modes.margin.down();
        let text = &mut Text::new();
        let mut picker = several();
        let (_, alone) = picker.size(&theme);
        assert_eq!(
            mode_middle(&picker, View::Generic),
            None,
            "not until hovered"
        );
        // The pointer over the field: the row under it, at the bottom.
        let (x, y) = field_at(&theme);
        assert!(picker.hover(Some((x, y)), &theme, text));
        assert_eq!(picker.size(&theme).1, alone + row);
        let (_, down) = mode_middle(&picker, View::Generic).unwrap_or_default();
        assert!(down.unsigned_abs() > alone - modes.inset().bottom, "{down}");
        assert!(!picker.hover(Some((x + 1, y)), &theme, text), "nothing new");
        // Gone while anything is typed, and back once it is taken back.
        typed(&mut picker, "f");
        assert_eq!(mode_middle(&picker, View::Generic), None);
        let found = picker.size(&theme).1;
        picker.key(BACKSPACE);
        assert_eq!(picker.size(&theme).1, alone + row);
        // Under what is found, in a mode that lists it with nothing typed.
        picker.set_view(View::Mode(Mode::Programs));
        let (_, grid) = mode_middle(&picker, View::Generic).unwrap_or_default();
        let (_, tall) = picker.size(&theme);
        assert!(
            grid.unsigned_abs() > tall - row,
            "{grid} under the grid of {tall}"
        );
        // And gone as the pointer leaves.
        assert!(picker.hover(None, &theme, text));
        assert_eq!(mode_middle(&picker, View::Generic), None);
        picker.set_view(View::Generic);
        assert_eq!(picker.size(&theme).1, alone);
        assert!(found > alone);
    }

    #[test]
    fn a_click_on_a_mode_goes_to_it_and_the_one_shown_is_lit() {
        let theme = Theme::default();
        let text = &mut Text::new();
        let mut picker = several();
        let (x, y) = field_at(&theme);
        picker.hover(Some((x, y)), &theme, text);
        let lit = argb(
            theme
                .style(Part::LauncherMode, States::NONE.with(State::Active))
                .background,
        );
        // Near the button's left, half-way down, clear of its sign and its
        // words.
        let at = |picker: &Picker, view| {
            let (_, y) = mode_middle(picker, view).unwrap_or_default();
            let area = picker.mode_area(view, &theme, &mut Text::new());
            let (left, ..) = area.unwrap_or_default();
            painted(picker).at(left.unsigned_abs() + 3, y.unsigned_abs())
        };
        assert_eq!(at(&picker, View::Generic), lit, "everything, shown");
        let programs = View::Mode(Mode::Programs);
        let unmarked = at(&picker, programs);
        assert_ne!(unmarked, lit);
        // The pointer over a mode marks it, in the theme's see-through
        // tint over the launcher's glass.
        let (x, y) = mode_middle(&picker, programs).unwrap_or_default();
        assert!(picker.hover(Some((x, y)), &theme, text));
        let marked = at(&picker, programs);
        assert!(![unmarked, lit].contains(&marked), "{marked:#x}");
        // A click goes to it.
        assert_eq!(picker.click(x, y, &theme, text), Outcome::Stay);
        assert_eq!(picker.view(), programs);
        assert_eq!(at(&picker, programs), lit);
        assert_ne!(at(&picker, View::Generic), lit);
        let (x, y) = mode_middle(&picker, View::Generic).unwrap_or_default();
        assert_eq!(picker.click(x, y, &theme, text), Outcome::Stay);
        assert_eq!(picker.view(), View::Generic);
        // Each mode has its name, and its digit after it but everything.
        let words = Words::in_languages(&omorfos_i18n::LAUNCHER, &[]);
        assert_eq!(words.say("launcher-mode-all", &[]), "All");
        assert_eq!(words.say("launcher-mode-programs", &[]), "Programs");
        let all = picker
            .mode_area(View::Generic, &theme, text)
            .unwrap_or_default();
        let grid = picker.mode_area(programs, &theme, text).unwrap_or_default();
        assert!(grid.0 > all.0 + all.2.cast_signed(), "side by side");
    }

    #[test]
    fn drawn_on_its_way_to_another_size_the_field_keeps_to_its_edge() {
        let theme = Theme::default();
        let mut picker = several();
        let natural = picker.size(&theme);
        let start = picker.typed_area(&theme).0;
        let (_, _, field, _) = picker.layout(&theme).input;
        // Wider and taller on its way: drawn as large as it is so far, the
        // field as much wider, and the sign and the text as far from the
        // left edge.
        let on_its_way = (natural.0 + 200, natural.1 + 40);
        picker.draw_at(Some(on_its_way));
        assert_eq!(picker.size(&theme), natural, "where it is going");
        let canvas = painted(&picker);
        assert_eq!((canvas.width(), canvas.height()), on_its_way);
        assert_eq!(picker.typed_area(&theme).0, start);
        assert_eq!(picker.layout(&theme).input.2, field + 200);
        picker.draw_at(None);
        assert_eq!(painted(&picker).width(), natural.0);
    }
}
