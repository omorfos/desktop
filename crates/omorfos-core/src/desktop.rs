//! The state of the session: windows, tab groups, workspaces and focus.

use crate::decoration::{Decoration, Metrics};
use crate::geometry::{Point, Rect, Size};
use crate::placement::{CASCADE_STEP, Placement, place};
use crate::rules::{Rule, resolve};

/// Identifies a window for as long as it exists. Never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowId(u64);

impl WindowId {
    /// The identifier as a plain number, for showing to users and sending
    /// to other programs.
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }

    /// Rebuilds an identifier from a number another program sent back.
    ///
    /// Nothing guarantees such a window exists; the desktop ignores
    /// identifiers it does not know.
    #[must_use]
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }
}

/// A window together with where it is, as listed by [`Desktop::windows`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Listed<'a> {
    /// The window.
    pub window: &'a Window,
    /// The output whose set of workspaces it is in, counted in the order
    /// the outputs were given. Zero while there is no output.
    pub output: usize,
    /// The zero-based workspace it is on, among that output's.
    pub workspace: usize,
    /// Whether its frame is minimized.
    pub minimized: bool,
    /// Whether it is the tab its frame is showing.
    pub shown: bool,
    /// Whether it has keyboard focus.
    pub focused: bool,
    /// How its frame stands apart from the others, as the user asked.
    pub standing: Standing,
}

/// An application window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    /// The window's identifier.
    pub id: WindowId,
    /// The application the window belongs to.
    pub app_id: String,
    /// The window's title.
    pub title: String,
    /// Whether the window gets a titlebar and border.
    pub decorated: bool,
    /// The size it says it can be no smaller than, without its frame:
    /// nothing either way where it has not said.
    pub least: Size,
}

/// An output: a monitor, or the window a nested compositor draws into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Screen {
    /// The whole output, in the global space.
    pub area: Rect,
    /// The part of it left for windows once panels and docks have reserved
    /// their edges. New windows are placed in it and maximized windows fill
    /// it; a fullscreen window covers the whole area.
    pub usable: Rect,
}

impl From<Rect> for Screen {
    /// An output with nothing reserved: all of it is usable.
    fn from(area: Rect) -> Self {
        Self { area, usable: area }
    }
}

/// How a window that rules open maximized or fullscreen should first be
/// told to draw itself, as answered by [`Desktop::opening`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Opening {
    /// The size the window itself will have.
    pub size: Size,
    /// Whether it opens maximized.
    pub maximized: bool,
    /// Whether it opens fullscreen.
    pub fullscreen: bool,
}

/// Behaviour that the user can configure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    /// Whether moving the pointer onto a window focuses it. When `false`, a
    /// window has to be clicked.
    pub focus_follows_pointer: bool,
    /// Whether a window is raised when the pointer or a click focuses it.
    pub raise_on_focus: bool,
    /// Where new windows appear.
    pub placement: Placement,
    /// The thickness of the titlebar and borders around decorated windows.
    pub decoration: Metrics,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            focus_follows_pointer: false,
            raise_on_focus: true,
            placement: Placement::default(),
            decoration: Metrics::default(),
        }
    }
}

/// A frame on screen: one or more windows sharing a position as tabs.
///
/// This is a read-only view produced by [`Desktop::frames`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame<'a> {
    /// The windows in the frame, in tab order. Never empty.
    pub tabs: &'a [Window],
    /// The index in `tabs` of the window being shown.
    pub active: usize,
    /// Where the frame is on screen, decoration included, taking maximized
    /// and fullscreen into account.
    pub geometry: Rect,
    /// The layout of the titlebar and borders, if the frame has them. A
    /// frame is decorated when its shown window is, or when it holds
    /// several tabs, which need a titlebar to be reachable. A fullscreen
    /// frame never is.
    pub decoration: Option<Decoration>,
    /// Whether the frame fills its output.
    pub maximized: bool,
    /// Whether the frame covers its output without decorations.
    pub fullscreen: bool,
    /// What kind of workspace it is on.
    pub home: Home,
    /// Whether the frame has keyboard focus.
    pub focused: bool,
    /// How it stands apart from the others, as the user asked.
    pub standing: Standing,
}

/// How a frame stands apart from the others, each as the user asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Standing {
    /// Rolled up into its titlebar, which is all that shows of it. A
    /// frame without a titlebar cannot be.
    pub shaded: bool,
    /// Kept above the frames that are not, whichever has focus.
    pub above: bool,
    /// On whichever workspace its output is showing.
    pub sticky: bool,
}

impl Frame<'_> {
    /// The window being shown.
    #[must_use]
    pub fn window(&self) -> &Window {
        &self.tabs[self.active]
    }

    /// The part of the frame the window itself occupies.
    #[must_use]
    pub fn client(&self) -> Rect {
        // A titlebar a window alone on its workspace is given is laid over
        // it, and takes no room from it.
        let over = self.fullscreen && self.home == Home::Solo;
        let around = self.decoration.filter(|_| !over);
        around.map_or(self.geometry, |decoration| decoration.client())
    }
}

/// The kind of workspace a frame is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Home {
    /// A desktop, where frames are under the panels.
    Desktop,
    /// A workspace made for one fullscreen window: that window, or a
    /// dialog of it that opened over it. These are over the panels.
    Solo,
    /// A workspace made for windows to share, each filling a part of it,
    /// and what opened over them. These are over the panels too.
    Shared,
}

/// One or more windows sharing a position as tabs.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Group {
    /// Creation order, which is also the order focus cycles in.
    serial: u64,
    tabs: Vec<Window>,
    active: usize,
    /// The position when neither maximized nor fullscreen.
    geometry: Rect,
    maximized: bool,
    fullscreen: bool,
    minimized: bool,
    standing: Standing,
}

impl Group {
    /// What it can be no smaller than in a part of a workspace windows
    /// share, where its titlebar is over its window and it has no border:
    /// as large as the largest any of its tabs says, whichever is shown.
    fn least(&self, titlebar: u32) -> Size {
        let width = self.tabs.iter().map(|tab| tab.least.width).max();
        let height = self.tabs.iter().map(|tab| tab.least.height).max();
        let over = if self.decorated() { titlebar } else { 0 };
        let height = height.unwrap_or(0).saturating_add(over);
        Size::new(width.unwrap_or(0), height)
    }

    /// Where it is among the frames of its workspace, whatever order they
    /// were raised in: those kept above are over those that are not. A
    /// fullscreen frame is under whatever else is on the workspace made
    /// for it, which is its own program's dialogs.
    fn level(&self) -> u8 {
        u8::from(!self.fullscreen) * (1 + u8::from(self.standing.above))
    }

    /// Whether the frame has a titlebar and borders when not fullscreen.
    fn decorated(&self) -> bool {
        self.tabs.len() > 1 || self.tabs[self.active].decorated
    }

    fn remove(&mut self, tab: usize) -> Window {
        let window = self.tabs.remove(tab);
        // Keep showing the same window; if it was the one removed, show its
        // right-hand neighbour, or the new last tab if there is none.
        if tab < self.active || self.active == self.tabs.len() {
            self.active = self.active.saturating_sub(1);
        }
        window
    }
}

/// What a workspace is known by for as long as there is one: its place
/// among the others changes as they come and go, and this does not. It is
/// never used again in a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WorkspaceId(u64);

impl WorkspaceId {
    /// The identifier as a number, for those who have to write it down.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// The identifier that was written down as `number`. It may be of no
    /// workspace there is, or ever was.
    #[must_use]
    pub const fn from_raw(number: u64) -> Self {
        Self(number)
    }
}

/// One of an output's workspaces, as those who show them are told of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    /// What it is known by.
    pub id: WorkspaceId,
    /// The output it belongs to, counted in the order the outputs were
    /// given.
    pub output: usize,
    /// Its place among that output's workspaces, from zero.
    pub index: usize,
    /// Whether it is always there, as those the configuration names are.
    /// One that is not goes when it is empty and another is shown.
    pub kept: bool,
    /// Whether its output is showing it.
    pub shown: bool,
    /// Which of its output's desktops it is, from zero, as a key or a rule
    /// would count to it. A workspace made for one fullscreen window is
    /// not a desktop and has none.
    pub desktop: Option<usize>,
    /// The windows it was made for, in the order of its parts, if it was
    /// made for windows to fill between them. A desktop has none.
    pub tiles: Vec<WindowId>,
    /// Whether another window can join those it was made for: it has a
    /// part with nothing in it, or is one window's, which can give up
    /// half.
    pub room: bool,
}

/// A half of an output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Half {
    /// The left half.
    Left,
    /// The right half.
    Right,
    /// The top half.
    Top,
    /// The bottom half.
    Bottom,
}

/// What a split is out of: a first part with this much has everything.
const WHOLE: u32 = 1 << 16;

/// A line between windows that share a workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Line {
    /// Which line of its workspace it is: 0 for the one that cuts the
    /// whole output in two, 1 and 2 for the one that cuts the first or
    /// the second of those two again.
    pub which: usize,
    /// Where it is: a hair wide or a hair tall, and as long as what it
    /// cuts.
    pub at: Rect,
    /// Whether it lies across, between a window above and one below, and
    /// is dragged up and down.
    pub across: bool,
}

/// A window of a desktop offered in a part of a workspace that has nothing
/// in it, to be put there by a click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Offer {
    /// The window offered.
    pub window: WindowId,
    /// Where its picture is: the shape of the window, and no larger.
    pub at: Rect,
    /// Where its title is written, under its picture, as wide as its box
    /// inside its padding.
    pub title: Rect,
    /// The box round both, which is what is pointed at and chosen. Those
    /// of a row are of one height.
    pub whole: Rect,
}

/// The room, in units of layout, left round each window offered in a part
/// with nothing in it, and between one and the next.
const OFFER_ROOM: u32 = 24;

/// How tall the line is that a window offered has its title on, and how
/// far under its picture.
const OFFER_TITLE: u32 = 20;
const OFFER_GAP: u32 = 4;

/// The room inside the box of a window offered, round its picture and its
/// title.
const OFFER_PAD: u32 = 8;

/// Where in `area` each of the windows of `sizes` is shown small, in rows
/// and columns as near to square as there can be, each in a box as wide as
/// its share of the area and as tall as the tallest of its row needs, with
/// its title under it: each as large as its share lets it be, and no larger
/// than it is. The pictures of a row stand on one line, and their titles
/// are on another under it.
fn offered(area: Rect, sizes: &[Size]) -> Vec<Placed> {
    let count = u32::try_from(sizes.len()).unwrap_or(u32::MAX);
    // As many columns as the square root, rounded up, and one at least.
    let square = count.isqrt();
    let columns = (square + u32::from(square * square < count)).max(1);
    let rows = count.div_ceil(columns);
    let share = |along: u32, many: u32| {
        let room = OFFER_ROOM.saturating_mul(many + 1);
        along.saturating_sub(room) / many.max(1)
    };
    let cell = Size::new(
        share(area.size.width, columns),
        share(area.size.height, rows),
    );
    let step = |many: u32, cell: u32| (OFFER_ROOM + cell).saturating_mul(many).cast_signed();
    // What the picture has of its box, inside its padding and with its
    // title under it.
    let under = OFFER_GAP + OFFER_TITLE;
    let room = Size::new(
        cell.width.saturating_sub(OFFER_PAD * 2),
        cell.height.saturating_sub(under + OFFER_PAD * 2),
    );
    let fitted: Vec<Size> = sizes
        .iter()
        .map(|size| {
            let (width, height) = (size.width.max(1), size.height.max(1));
            // As wide as its room, unless that makes it too tall.
            let wide = u64::from(width) * u64::from(room.height)
                >= u64::from(height) * u64::from(room.width);
            if wide {
                let across = width.min(room.width);
                Size::new(across, scaled(height, across, width))
            } else {
                let down = height.min(room.height);
                Size::new(scaled(width, down, height), down)
            }
        })
        .collect();
    let across = usize::try_from(columns).unwrap_or(usize::MAX);
    let tallest: Vec<u32> = fitted
        .chunks(across)
        .map(|row| row.iter().map(|size| size.height).max().unwrap_or(0))
        .collect();
    fitted
        .iter()
        .zip(0..)
        .zip(
            tallest
                .iter()
                .flat_map(|tallest| std::iter::repeat_n(*tallest, across)),
        )
        .map(|((size, index), tallest)| {
            let corner = area.origin.offset(
                OFFER_ROOM.cast_signed() + step(index % columns, cell.width),
                OFFER_ROOM.cast_signed() + step(index / columns, cell.height),
            );
            // The box in the middle of the cell, and the picture standing
            // on the line its row's titles are under.
            let tall = tallest.saturating_add(under + OFFER_PAD * 2);
            let top = corner.offset(0, (cell.height.saturating_sub(tall) / 2).cast_signed());
            let line = top.offset(0, (OFFER_PAD + tallest).cast_signed());
            let left = (cell.width.saturating_sub(size.width) / 2).cast_signed();
            let picture = line.offset(left, -size.height.cast_signed());
            let title = line.offset(OFFER_PAD.cast_signed(), OFFER_GAP.cast_signed());
            Placed {
                picture: Rect::new(picture, *size),
                title: Rect::new(title, Size::new(room.width, OFFER_TITLE)),
                whole: Rect::new(top, Size::new(cell.width, tall)),
            }
        })
        .collect()
}

/// Where a window offered is shown: its picture, its title, and the box
/// round both.
struct Placed {
    picture: Rect,
    title: Rect,
    whole: Rect,
}

/// `length` made smaller by as much as `to` is than `from`.
fn scaled(length: u32, to: u32, from: u32) -> u32 {
    let scaled = u64::from(length) * u64::from(to) / u64::from(from.max(1));
    u32::try_from(scaled).unwrap_or(length)
}

/// A way for up to four windows to share a workspace: how it is cut into
/// parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Arrangement {
    /// Two, side by side.
    SideBySide,
    /// Two, one above the other.
    Stacked,
    /// One on the left, and two on the right, one above the other.
    OneBesideTwo,
    /// Two on the left, one above the other, and one on the right.
    TwoBesideOne,
    /// One across the top, and two side by side under it.
    OneAboveTwo,
    /// Two side by side, and one across the bottom.
    TwoAboveOne,
    /// Four, a quarter each.
    Quarters,
}

impl Arrangement {
    /// Every arrangement, from the fewest parts to the most.
    pub const ALL: [Self; 7] = [
        Self::SideBySide,
        Self::Stacked,
        Self::OneBesideTwo,
        Self::TwoBesideOne,
        Self::OneAboveTwo,
        Self::TwoAboveOne,
        Self::Quarters,
    ];

    /// Whether its two sides are one above the other, and which of them
    /// is cut again.
    const fn cuts(self) -> (bool, [bool; 2]) {
        match self {
            Self::SideBySide => (false, [false, false]),
            Self::Stacked => (true, [false, false]),
            Self::OneBesideTwo => (false, [false, true]),
            Self::TwoBesideOne => (false, [true, false]),
            Self::OneAboveTwo => (true, [false, true]),
            Self::TwoAboveOne => (true, [true, false]),
            Self::Quarters => (false, [true, true]),
        }
    }

    /// Where its parts are on a rectangle, in the order they are read in:
    /// down the first side, then down the second.
    #[must_use]
    pub fn parts(self, area: Rect) -> Vec<Rect> {
        let cut = Tiling::arranged(WorkspaceId(0), self, None);
        let parts = cut.slots().filter(|(_, part)| *part != Part::None);
        parts.map(|(at, _)| cut.rect(at, area)).collect()
    }
}

/// A part of a workspace made for windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    /// There is no such part: the workspace is cut into fewer.
    None,
    /// A part with nothing in it yet.
    Empty,
    /// The frame that fills it.
    Tile(u64),
}

/// One of the two sides a workspace made for windows is cut into: one
/// part, or two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Side {
    /// How much of the side its first part has, out of [`WHOLE`].
    split: u32,
    /// The first is only [`Part::None`] if there is no such side.
    parts: [Part; 2],
}

impl Side {
    /// No side at all: the other has everything.
    const NONE: Self = Self::of(Part::None);

    /// A side that is one part.
    const fn of(part: Part) -> Self {
        Self {
            split: WHOLE / 2,
            parts: [part, Part::None],
        }
    }

    /// A side cut in two, with `first` before `second`.
    const fn cut(first: Part, second: Part) -> Self {
        Self {
            split: WHOLE / 2,
            parts: [first, second],
        }
    }
}

/// Cuts a rectangle in two, the first having `split` of it out of
/// [`WHOLE`]: side by side, or with `rows` one above the other.
fn halves(area: Rect, rows: bool, split: u32) -> [Rect; 2] {
    let (width, height) = (area.size.width, area.size.height);
    let along = if rows { height } else { width };
    let first = u64::from(along) * u64::from(split) / u64::from(WHOLE);
    let first = u32::try_from(first).unwrap_or(along);
    let rest = along - first;
    if rows {
        let below = area.origin.offset(0, first.cast_signed());
        [
            Rect::new(area.origin, Size::new(width, first)),
            Rect::new(below, Size::new(width, rest)),
        ]
    } else {
        let beside = area.origin.offset(first.cast_signed(), 0);
        [
            Rect::new(area.origin, Size::new(first, height)),
            Rect::new(beside, Size::new(rest, height)),
        ]
    }
}

/// What each of `groups` can be no smaller than in a part of a workspace
/// windows share, by its serial: nothing, for one that is not there.
fn least_of(groups: &[Group], titlebar: u32) -> impl Fn(u64) -> Size + '_ {
    move |serial| {
        let group = groups.iter().find(|group| group.serial == serial);
        group.map_or_else(Size::default, |group| group.least(titlebar))
    }
}

/// The split at which the first of two parts has `first` of a length of
/// `along`, or as near that as leaves each a sixth of it and as much as
/// it says, in `least`, that it can be no less than. Where there is not
/// room for both, the second has what it says; and neither says more than
/// leaves the other its sixth.
fn split_at(first: u32, along: u32, least: [u32; 2]) -> u32 {
    let sixth = along / 6;
    let least = least.map(|least| least.max(sixth).min(along - sixth));
    let first = first.max(least[0]).min(along - least[1]);
    // Rounded up, so that the part is the length asked for and not one
    // short of it.
    let split = (u64::from(first) * u64::from(WHOLE)).div_ceil(u64::from(along.max(1)));
    u32::try_from(split).unwrap_or(WHOLE / 2)
}

/// What makes a workspace one that was made for windows to fill between
/// them, and how it is cut up: in two, side by side or one above the
/// other, each side of which is one part or is cut again the other way.
/// So there are four parts at most, in eight arrangements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Tiling {
    /// The desktop the frames came from, and go back to.
    origin: WorkspaceId,
    /// Whether the two sides are one above the other, and not side by
    /// side.
    stacked: bool,
    /// How much of the output the first side has, out of [`WHOLE`].
    split: u32,
    /// The second is [`Side::NONE`] where the first has everything.
    sides: [Side; 2],
}

impl Tiling {
    /// A workspace for one frame, which has all of it.
    const fn alone(origin: WorkspaceId, frame: u64) -> Self {
        Self {
            origin,
            stacked: false,
            split: WHOLE / 2,
            sides: [Side::of(Part::Tile(frame)), Side::NONE],
        }
    }

    /// A workspace cut up as an arrangement has it, with nothing in its
    /// parts but, if one is given, a frame in the part so far along in
    /// the order they are read in.
    fn arranged(origin: WorkspaceId, arrangement: Arrangement, tile: Option<(usize, u64)>) -> Self {
        let (stacked, cuts) = arrangement.cuts();
        let side = |cut: bool| {
            if cut {
                Side::cut(Part::Empty, Part::Empty)
            } else {
                Side::of(Part::Empty)
            }
        };
        let empty = Self {
            origin,
            stacked,
            split: WHOLE / 2,
            sides: cuts.map(side),
        };
        let mut along = 0;
        empty.with(|_, part| {
            let here = along;
            along += usize::from(part != Part::None);
            match tile {
                Some((at, frame)) if part != Part::None && at == here => Part::Tile(frame),
                _ => part,
            }
        })
    }

    /// Every part there could be, in the order they are read in: where it
    /// is, as a side and a place on that side, and what is in it.
    fn slots(self) -> impl Iterator<Item = ((usize, usize), Part)> {
        let places = [(0, 0), (0, 1), (1, 0), (1, 1)];
        places
            .into_iter()
            .map(move |(side, place)| ((side, place), self.sides[side].parts[place]))
    }

    /// The frames that fill its parts, in the order of the parts.
    fn tiles(self) -> impl Iterator<Item = u64> {
        self.slots().filter_map(|(_, part)| match part {
            Part::Tile(frame) => Some(frame),
            _ => None,
        })
    }

    /// Whether it is cut into more than one part.
    fn shared(self) -> bool {
        self.slots().filter(|(_, part)| *part != Part::None).count() > 1
    }

    /// The same with each part changed by `change`.
    fn with(self, mut change: impl FnMut((usize, usize), Part) -> Part) -> Self {
        let mut sides = self.sides;
        for (at, part) in self.slots() {
            sides[at.0].parts[at.1] = change(at, part);
        }
        Self { sides, ..self }
    }

    /// The same put in order after parts have gone: a part that is left
    /// has the room of the one that went.
    fn closed_up(self) -> Self {
        let pack = |side: Side| {
            let gone = side.parts[0] == Part::None;
            let parts = if gone {
                [side.parts[1], Part::None]
            } else {
                side.parts
            };
            Side { parts, ..side }
        };
        let [first, second] = self.sides.map(pack);
        let (first, second) = if first.parts[0] == Part::None {
            (second, first)
        } else {
            (first, second)
        };
        // One side cut in two is the whole cut in two the other way.
        if second.parts[0] == Part::None && first.parts[1] != Part::None {
            let sides = first.parts.map(Side::of);
            return Self {
                stacked: !self.stacked,
                split: first.split,
                sides,
                ..self
            };
        }
        Self {
            sides: [first, second],
            ..self
        }
    }

    /// The same without the frames that are not `here` any longer. One
    /// that leaves gives its part up, and the frame beside it has the
    /// room.
    fn kept(self, here: impl Fn(u64) -> bool) -> Self {
        let kept = self.with(|_, part| match part {
            Part::Tile(frame) if !here(frame) => Part::None,
            other => other,
        });
        kept.closed_up()
    }

    /// The same with `frame` in the first part that had nothing in it.
    fn filled(self, frame: u64) -> Self {
        let mut room = true;
        self.with(|_, part| {
            let fill = part == Part::Empty && std::mem::take(&mut room);
            if fill { Part::Tile(frame) } else { part }
        })
    }

    /// The same with room for another frame, if it can be made to have:
    /// as it is, where a part has nothing in it; and with its one frame
    /// in the left half where it is one frame's. A workspace whose parts
    /// are all taken makes no more.
    fn roomy(self) -> Option<Self> {
        let only = self.tiles().next().filter(|_| !self.shared());
        let halved = only.map(|frame| self.halved((0, 0), frame, Half::Left));
        Some(self).filter(|tiling| tiling.has_room()).or(halved)
    }

    /// Whether it has a part with nothing in it.
    fn has_room(self) -> bool {
        self.slots().any(|(_, part)| part == Part::Empty)
    }

    /// Where a part is on an output of `area`.
    fn rect(self, (side, place): (usize, usize), area: Rect) -> Rect {
        let whole = self.sides[1].parts[0] == Part::None;
        let sides = halves(area, self.stacked, self.split);
        let within = if whole { area } else { sides[side] };
        let alone = self.sides[side].parts[1] == Part::None;
        let parts = halves(within, !self.stacked, self.sides[side].split);
        if alone { within } else { parts[place] }
    }

    /// Where the part that `frame` fills is.
    fn at(self, frame: u64) -> Option<(usize, usize)> {
        let mut slots = self.slots();
        slots.find_map(|(at, part)| (part == Part::Tile(frame)).then_some(at))
    }

    /// Where on an output of `area` the part filled by `frame` is.
    fn part_of(self, frame: u64, area: Rect) -> Rect {
        self.at(frame).map_or(area, |at| self.rect(at, area))
    }

    /// The same with `frame` taking a half of the part it fills, and the
    /// other half left with nothing in it for another. A part can be
    /// halved if it is the whole workspace, or a whole side of it halved
    /// the other way than the workspace is; otherwise nothing changes.
    fn halved(self, (side, place): (usize, usize), frame: u64, half: Half) -> Self {
        let rows = matches!(half, Half::Top | Half::Bottom);
        let first = matches!(half, Half::Left | Half::Top);
        let tile = Part::Tile(frame);
        let (before, after) = if first {
            (tile, Part::Empty)
        } else {
            (Part::Empty, tile)
        };
        if !self.shared() {
            return Self {
                stacked: rows,
                split: WHOLE / 2,
                sides: [Side::of(before), Side::of(after)],
                ..self
            };
        }
        let whole = self.sides[side].parts[1] == Part::None;
        if rows == self.stacked {
            // It has a half that way already, or the other: asked for the
            // other, the two sides change places.
            let other = (side == 0) != first;
            let [one, two] = self.sides;
            let sides = if other { [two, one] } else { [one, two] };
            return Self { sides, ..self };
        }
        let mut sides = self.sides;
        if !whole {
            // Likewise within a side that is cut already: its two parts
            // change places.
            let other = (place == 0) != first;
            let [one, two] = sides[side].parts;
            sides[side].parts = if other { [two, one] } else { [one, two] };
            return Self { sides, ..self };
        }
        sides[side] = Side::cut(before, after);
        Self { sides, ..self }
    }

    /// The lines between its parts on an output of `area`.
    fn lines(self, area: Rect) -> Vec<Line> {
        let hair = |second: Rect, across: bool| {
            let size = if across {
                Size::new(second.size.width, 1)
            } else {
                Size::new(1, second.size.height)
            };
            Rect::new(second.origin, size)
        };
        let sides = halves(area, self.stacked, self.split);
        let whole = self.sides[1].parts[0] == Part::None;
        let root = (!whole).then(|| Line {
            which: 0,
            at: hair(sides[1], self.stacked),
            across: self.stacked,
        });
        let within = (0..2).filter_map(|side| {
            let cut = self.sides[side].parts[1] != Part::None;
            let parts = halves(sides[side], !self.stacked, self.sides[side].split);
            cut.then(|| Line {
                which: side + 1,
                at: hair(parts[1], !self.stacked),
                across: !self.stacked,
            })
        });
        root.into_iter().chain(within).collect()
    }

    /// The same with one of its lines at `point` on an output of `area`,
    /// or as near it as leaves each part a sixth of the output, and each
    /// frame the size `least` says it can be no smaller than; or with
    /// `None` in the middle of what it cuts.
    fn cut(
        self,
        which: usize,
        point: Option<Point>,
        area: Rect,
        least: impl Fn(u64) -> Size,
    ) -> Self {
        let across = (which == 0) == self.stacked;
        let (from, along) = if across {
            (area.origin.y, area.size.height)
        } else {
            (area.origin.x, area.size.width)
        };
        let wanted = point.map_or(along / 2, |point| {
            let to = if across { point.y } else { point.x };
            to.saturating_sub(from).max(0).unsigned_abs()
        });
        self.cut_at(which, wanted, area, least)
    }

    /// The same with one of its lines `wanted` from where what it cuts
    /// begins, or as near it as `cut` allows.
    fn cut_at(self, which: usize, wanted: u32, area: Rect, least: impl Fn(u64) -> Size) -> Self {
        let across = (which == 0) == self.stacked;
        let along = if across {
            area.size.height
        } else {
            area.size.width
        };
        // What each part can be no less than the way the line goes; a side
        // cut in two the other way, what the larger of its two can be.
        let part = |part: Part| match part {
            Part::Tile(frame) if across => least(frame).height,
            Part::Tile(frame) => least(frame).width,
            Part::None | Part::Empty => 0,
        };
        let side = |side: Side| part(side.parts[0]).max(part(side.parts[1]));
        let least = match which.checked_sub(1) {
            None => self.sides.map(side),
            Some(side) => self.sides[side.min(1)].parts.map(part),
        };
        let split = split_at(wanted, along, least);
        let mut cut = self;
        match which.checked_sub(1) {
            None => cut.split = split,
            Some(side) => cut.sides[side.min(1)].split = split,
        }
        cut
    }

    /// The same on an output of `area` with each part as large as the
    /// frame in it says it can be no smaller than, where there is room: a
    /// line goes only as far as that takes, and back to where it was put
    /// once it no longer does, since this is not kept. `groups` are the
    /// frames of the workspace.
    fn with_room(self, area: Rect, groups: &[Group], titlebar: u32) -> Self {
        let least = least_of(groups, titlebar);
        self.lines(area).iter().fold(self, |tiling, line| {
            let across = (line.which == 0) == tiling.stacked;
            let along = if across {
                area.size.height
            } else {
                area.size.width
            };
            let split = line.which.checked_sub(1);
            let split = split.map_or(tiling.split, |side| tiling.sides[side.min(1)].split);
            let first = u64::from(along) * u64::from(split) / u64::from(WHOLE);
            let first = u32::try_from(first).unwrap_or(along);
            tiling.cut_at(line.which, first, area, &least)
        })
    }

    /// The part beside another, on the side of it that `toward` says: the
    /// one that shares most of that edge with it.
    fn beside(self, at: (usize, usize), toward: Half, area: Rect) -> Option<(usize, usize)> {
        let mine = self.rect(at, area);
        let shared = |other: Rect| {
            let (near, lengthwise) = match toward {
                Half::Left => (other.right() == i64::from(mine.origin.x), false),
                Half::Right => (i64::from(other.origin.x) == mine.right(), false),
                Half::Top => (other.bottom() == i64::from(mine.origin.y), true),
                Half::Bottom => (i64::from(other.origin.y) == mine.bottom(), true),
            };
            let span = |rect: Rect| {
                if lengthwise {
                    (i64::from(rect.origin.x), rect.right())
                } else {
                    (i64::from(rect.origin.y), rect.bottom())
                }
            };
            let (one, other) = (span(mine), span(other));
            let overlap = one.1.min(other.1) - one.0.max(other.0);
            (near && overlap > 0).then_some(overlap)
        };
        let others = self
            .slots()
            .filter(|(other, part)| *other != at && *part != Part::None);
        let beside = others.filter_map(|(other, _)| Some((shared(self.rect(other, area))?, other)));
        // The first of those that share the most, in the order they are
        // read in.
        let most = |best: Option<(i64, (usize, usize))>, next: (i64, (usize, usize))| {
            let better = best.is_none_or(|best| next.0 > best.0);
            if better { Some(next) } else { best }
        };
        beside.fold(None, most).map(|(_, other)| other)
    }

    /// The same with what is in two parts changed over.
    fn swapped(self, one: (usize, usize), other: (usize, usize)) -> Self {
        let (first, second) = (
            self.sides[one.0].parts[one.1],
            self.sides[other.0].parts[other.1],
        );
        self.with(|at, part| {
            if at == one {
                second
            } else if at == other {
                first
            } else {
                part
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Workspace {
    id: WorkspaceId,
    /// Always there. One that is not kept goes when it is empty and
    /// another is shown.
    kept: bool,
    /// Set for a workspace made for frames to fill between them. The
    /// others are desktops.
    solo: Option<Tiling>,
    /// Bottom to top.
    groups: Vec<Group>,
    focused: Option<u64>,
}

impl Workspace {
    const fn new(id: WorkspaceId, kept: bool) -> Self {
        Self {
            id,
            kept,
            solo: None,
            groups: Vec::new(),
            focused: None,
        }
    }

    /// The frame that fills a part, while it is here and still does.
    fn tile(&self, frame: u64) -> Option<&Group> {
        let tile = |group: &&Group| group.serial == frame && group.fullscreen;
        self.groups.iter().find(tile)
    }

    /// The frames this was made for that are still here, in the order of
    /// its parts.
    fn tiles(&self) -> Vec<&Group> {
        let parts = self.solo.into_iter().flat_map(Tiling::tiles);
        parts.filter_map(|frame| self.tile(frame)).collect()
    }

    /// Drops emptied groups and, if the focused group is gone or hidden,
    /// focuses the topmost one that can take focus.
    fn repair(&mut self) {
        self.groups.retain(|group| !group.tabs.is_empty());
        let focusable = |group: &&Group| !group.minimized;
        let focused = self.focused;
        if !self
            .groups
            .iter()
            .filter(focusable)
            .any(|group| Some(group.serial) == focused)
        {
            self.focused = self
                .groups
                .iter()
                .rev()
                .find(focusable)
                .map(|group| group.serial);
        }
    }

    fn focus(&mut self, group: usize, raise: bool) {
        self.groups[group].minimized = false;
        self.focused = Some(self.groups[group].serial);
        if raise {
            let raised = self.groups.remove(group);
            self.groups.push(raised);
        }
    }
}

/// The workspaces of one output, and which of them it is showing.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Set {
    /// Never empty, and never without a desktop.
    workspaces: Vec<Workspace>,
    active: usize,
    /// The desktop last shown: where the frames that are on every
    /// workspace wait while a workspace made for a fullscreen frame is
    /// shown.
    desk: WorkspaceId,
    /// Set while a fullscreen window's titlebar is asked for: how far
    /// down from the top of the output it is laid over the window.
    revealed: Option<u32>,
    /// How far right the workspace shown is drawn, in units of layout,
    /// while the workspaces are slid across: see [`Desktop::slide`].
    slid: i32,
}

impl Set {
    /// Where each desktop is among the workspaces, in order. Never empty.
    fn desktops(&self) -> Vec<usize> {
        let workspaces = self.workspaces.iter().enumerate();
        let desktops = workspaces.filter(|(_, workspace)| workspace.solo.is_none());
        desktops.map(|(index, _)| index).collect()
    }

    /// Where the workspace known by `id` is among the others.
    fn place(&self, id: WorkspaceId) -> usize {
        let known = |workspace: &Workspace| workspace.id == id;
        self.workspaces.iter().position(known).unwrap_or(0)
    }

    /// Takes on everything in `other`: its desktops one by one onto the
    /// desktops here, on top of what is there, and those it has more of
    /// whole, as ones that are not kept. Workspaces made for fullscreen
    /// frames come over as they are.
    fn absorb(&mut self, other: Self) {
        let desktops = self.desktops();
        let mut merged = Vec::new();
        let mut solos = Vec::new();
        let mut count = 0;
        for mut workspace in other.workspaces {
            match (workspace.solo, desktops.get(count)) {
                (Some(solo), _) => solos.push((workspace, solo)),
                (None, Some(onto)) => {
                    let onto = &mut self.workspaces[*onto];
                    merged.push((workspace.id, onto.id));
                    onto.groups.extend(workspace.groups);
                    onto.repair();
                    count += 1;
                }
                (None, None) => {
                    workspace.kept = false;
                    self.workspaces.push(workspace);
                    count += 1;
                }
            }
        }
        for (mut workspace, solo) in solos {
            // Its frame goes back to wherever the desktop it came from is
            // now.
            let now = merged.iter().find(|(was, _)| *was == solo.origin);
            let origin = now.map_or(solo.origin, |(_, now)| *now);
            workspace.solo = Some(Tiling { origin, ..solo });
            self.workspaces.push(workspace);
        }
        self.settle();
    }

    /// Puts right what an operation may have left: a workspace made for a
    /// fullscreen frame that has closed or left fullscreen is done away
    /// with, whatever else was on it going to the desktop the frame came
    /// from; frames that are on every workspace come to the desktop being
    /// shown; and workspaces that are not kept, are empty and are not
    /// needed go.
    fn settle(&mut self) {
        let mut shown = self.workspaces[self.active].id;
        // A frame that has closed or left gives its part up. One that left
        // and is still here goes back to the desktop it came from, and is
        // followed there if it was being looked at.
        for index in 0..self.workspaces.len() {
            let workspace = &mut self.workspaces[index];
            let Some(tiling) = workspace.solo else {
                continue;
            };
            let parts: Vec<u64> = tiling.tiles().collect();
            let left = |group: &mut Group| parts.contains(&group.serial) && !group.fullscreen;
            let leaving: Vec<Group> = workspace.groups.extract_if(.., left).collect();
            workspace.repair();
            let here = |frame: u64| workspace.tile(frame).is_some();
            workspace.solo = Some(tiling.kept(here));
            let from = workspace.id;
            if let Some(last) = leaving.last().map(|group| group.serial) {
                let origin = self.place(tiling.origin);
                let origin = &mut self.workspaces[origin];
                origin.groups.extend(leaving);
                origin.focused = Some(last);
                origin.repair();
                if shown == from {
                    shown = origin.id;
                }
            }
        }
        let over = |workspace: &Workspace| workspace.solo.filter(|_| workspace.tiles().is_empty());
        let ended = |workspaces: &[Workspace]| {
            let mut workspaces = workspaces.iter().enumerate();
            workspaces.find_map(|(index, workspace)| over(workspace).map(|solo| (index, solo)))
        };
        while let Some((index, solo)) = ended(&self.workspaces) {
            let ending = &mut self.workspaces[index];
            ending.solo = None;
            let (id, focused) = (ending.id, ending.focused.take());
            let groups = std::mem::take(&mut ending.groups);
            let origin = self.place(solo.origin);
            let origin = &mut self.workspaces[origin];
            origin.groups.extend(groups);
            origin.focused = focused.or(origin.focused);
            origin.repair();
            if shown == id {
                shown = origin.id;
            }
        }
        // A frame on every workspace follows to the desktop now shown. One
        // of them that had focus keeps it.
        let showing = self.place(shown);
        let (desk, to) = (self.place(self.desk), &self.workspaces[showing]);
        if to.solo.is_none() && to.id != self.desk {
            self.desk = to.id;
            let leaving = &mut self.workspaces[desk];
            let focused = leaving.focused;
            let carried = leaving.groups.extract_if(.., |group| group.standing.sticky);
            let carried: Vec<Group> = carried.collect();
            leaving.repair();
            let arriving = &mut self.workspaces[showing];
            let mut kept = carried.iter().map(|group| group.serial);
            let kept = kept.find(|serial| Some(*serial) == focused);
            arriving.groups.extend(carried);
            arriving.focused = kept.or(arriving.focused);
            arriving.repair();
        }
        let solos = self
            .workspaces
            .iter()
            .filter_map(|workspace| workspace.solo);
        let origins: Vec<WorkspaceId> = solos.map(|solo| solo.origin).collect();
        self.workspaces.retain(|workspace| {
            let needed = workspace.id == shown || origins.contains(&workspace.id);
            workspace.kept || !workspace.groups.is_empty() || needed
        });
        self.active = self.place(shown);
    }
}

/// A window dropped on a part of the workspace it shares: where the
/// window is, how the workspace is cut, the window's own part and the one
/// it was dropped on, and where on the output that one is.
type Dropped = (Location, Tiling, [(usize, usize); 2], Rect);

/// Where a window is: its output's set, its workspace there, its group and
/// its tab.
#[derive(Clone, Copy)]
struct Location {
    set: usize,
    workspace: usize,
    group: usize,
    tab: usize,
}

/// The state of the session.
///
/// Windows live in frames (tab groups of one or more) and frames live on
/// workspaces. Each output has a set of workspaces of its own and shows one
/// of them, so switching workspace on one screen leaves the others as they
/// are. One output is the current one: its focused frame has the keyboard,
/// and it is the one workspace switching applies to.
///
/// Frames are positioned in the one space all the outputs show. A frame
/// belongs to an output's set, and follows that output if it moves.
/// Methods that take a [`WindowId`] do nothing if the window is unknown, so
/// a late event for a window that has just closed is harmless.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Desktop {
    settings: Settings,
    rules: Vec<Rule>,
    outputs: Vec<Screen>,
    /// One set of workspaces for each output, in the same order, or a
    /// single set while there is no output. Never empty.
    sets: Vec<Set>,
    /// The set that has the keyboard.
    current: usize,
    /// The output that comes first: the one frames of an output that goes
    /// move to.
    primary: usize,
    /// How many of each output's workspaces are always there.
    kept: usize,
    next_serial: u64,
}

impl Desktop {
    /// The most workspaces an output can have. Asking for one more does
    /// nothing.
    pub const MOST_WORKSPACES: usize = 32;

    /// Creates an empty desktop with `workspaces` workspaces that are
    /// always there (at least one). More are made as they are asked for.
    #[must_use]
    pub fn new(settings: Settings, workspaces: usize) -> Self {
        let mut desktop = Self {
            settings,
            rules: Vec::new(),
            outputs: Vec::new(),
            sets: Vec::new(),
            current: 0,
            primary: 0,
            kept: workspaces.clamp(1, Self::MOST_WORKSPACES),
            next_serial: 0,
        };
        let first = desktop.set();
        desktop.sets.push(first);
        desktop
    }

    /// A set of workspaces for an output that has none yet: the ones that
    /// are always there.
    fn set(&mut self) -> Set {
        let workspaces = (0..self.kept).map(|_| Workspace::new(WorkspaceId(self.serial()), true));
        let workspaces: Vec<Workspace> = workspaces.collect();
        Set {
            desk: workspaces[0].id,
            workspaces,
            active: 0,
            revealed: None,
            slid: 0,
        }
    }

    // --- Configuration -----------------------------------------------------

    /// Replaces the configurable behaviour.
    pub const fn set_settings(&mut self, settings: Settings) {
        self.settings = settings;
    }

    /// Replaces the rules applied to windows added from now on.
    pub fn set_rules(&mut self, rules: Vec<Rule>) {
        self.rules = rules;
    }

    /// Replaces the outputs. A plain rectangle stands for an output that
    /// is usable in full.
    ///
    /// Outputs are told apart by their place in the list: the first given
    /// is the first there was, and so on. One more than before is a new
    /// output, with an empty set of workspaces. Fewer means the last ones
    /// have gone; to take away another, say which with
    /// [`Desktop::remove_output`] first.
    ///
    /// A frame keeps its place on its output if the output moves, and one
    /// left entirely off its output is moved onto it.
    pub fn set_outputs<S: Into<Screen>>(&mut self, outputs: impl IntoIterator<Item = S>) {
        let outputs: Vec<Screen> = outputs.into_iter().map(Into::into).collect();
        while self.sets.len() > outputs.len().max(1) {
            self.remove_set(self.sets.len() - 1);
        }
        while self.sets.len() < outputs.len() {
            let set = self.set();
            self.sets.push(set);
        }
        for (index, (set, output)) in self.sets.iter_mut().zip(&outputs).enumerate() {
            // Where the output was, if it was anywhere.
            let before = self.outputs.get(index).map(|output| output.area.origin);
            let moved = before.map_or((0, 0), |before| {
                let now = output.area.origin;
                (
                    now.x.saturating_sub(before.x),
                    now.y.saturating_sub(before.y),
                )
            });
            let groups = set.workspaces.iter_mut().flat_map(|on| &mut on.groups);
            for group in groups {
                group.geometry.origin = group.geometry.origin.offset(moved.0, moved.1);
                if output.area.intersection(group.geometry).is_none() {
                    group.geometry.origin = output.usable.origin;
                }
            }
        }
        self.outputs = outputs;
    }

    /// Makes the output at `index`, counted in the order the outputs were
    /// given, the one that comes first: the frames of one that goes move to
    /// it. While there is no such output, the first is.
    pub fn set_primary(&mut self, index: usize) {
        self.primary = index;
    }

    /// Takes away the output at `index`, counted in the order the outputs
    /// were given. Its frames move to the one that comes first of those
    /// that remain, which is the first unless another is said to be, each
    /// to the workspace of the same number, and its place in the list is
    /// closed up. Does nothing if there is no such output.
    pub fn remove_output(&mut self, index: usize) {
        if index < self.outputs.len() {
            self.outputs.remove(index);
            self.remove_set(index);
            // The frames that moved are placed by the next `set_outputs`,
            // which the outputs' new positions will come with. Until then
            // they are on their new output or off every one.
            let outputs = std::mem::take(&mut self.outputs);
            self.set_outputs(outputs);
        }
    }

    /// Removes a set of workspaces, giving its frames to the one of the
    /// output that comes first of those that remain. The last set there is
    /// stays.
    fn remove_set(&mut self, index: usize) {
        if self.sets.len() > 1 {
            let gone = self.sets.remove(index);
            // Counted among those that remain; the first, if it was this.
            let after = |at: usize| match at {
                at if at == index => 0,
                at if at > index => at - 1,
                at => at,
            };
            self.primary = after(self.primary).min(self.sets.len() - 1);
            self.sets[self.primary].absorb(gone);
            self.current = match self.current {
                current if current == index => self.primary,
                current => after(current),
            };
        }
    }

    /// Changes how many of each output's workspaces are always there (at
    /// least one): its first so many.
    ///
    /// Nothing moves. A workspace that is no longer kept stays for as long
    /// as it has a frame on it or is being shown, and then goes.
    pub fn set_workspace_count(&mut self, count: usize) {
        self.kept = count.clamp(1, Self::MOST_WORKSPACES);
        for set in 0..self.sets.len() {
            while self.sets[set].desktops().len() < self.kept {
                let id = WorkspaceId(self.serial());
                self.sets[set].workspaces.push(Workspace::new(id, true));
            }
            let desktops = self.sets[set].desktops().into_iter().enumerate();
            for (count, index) in desktops {
                self.sets[set].workspaces[index].kept = count < self.kept;
            }
            self.sets[set].settle();
        }
    }

    // --- Windows -----------------------------------------------------------

    /// Adds a new window in a frame of its own and returns its identifier.
    ///
    /// `size` is the size of the window itself; the frame is larger by its
    /// decoration. `wants_decoration` says whether the window expects the
    /// compositor to draw its titlebar and border; a rule overrides it
    /// either way. It is placed on the output under `pointer`, on the
    /// workspace that output is showing unless a rule names another of its
    /// workspaces, and becomes the focused, topmost frame there. That
    /// output becomes the current one.
    pub fn add_window(
        &mut self,
        app_id: impl Into<String>,
        title: impl Into<String>,
        size: Size,
        pointer: Point,
        wants_decoration: bool,
    ) -> WindowId {
        self.add_window_of(None, app_id, title, size, pointer, wants_decoration)
    }

    /// Adds a new window as [`Desktop::add_window`] does, that says it
    /// belongs to another: a dialog of `parent`, say.
    ///
    /// It matters on a workspace made for a fullscreen window. What
    /// belongs to a window there opens over it, where it is. Anything else
    /// opening while such a workspace is shown, with no part left for it,
    /// opens on the desktop the fullscreen window came from without that
    /// being shown: a second window of the same program as much as another
    /// program's. One the user asked for is then [activated], which shows
    /// it.
    ///
    /// [activated]: Desktop::activate
    pub fn add_window_of(
        &mut self,
        parent: Option<WindowId>,
        app_id: impl Into<String>,
        title: impl Into<String>,
        size: Size,
        pointer: Point,
        wants_decoration: bool,
    ) -> WindowId {
        let (app_id, title) = (app_id.into(), title.into());
        let effects = resolve(&self.rules, &app_id, &title);
        let alone = |at: &Location| self.sets[at.set].workspaces[at.workspace].solo.is_some();
        let beside = parent.and_then(|parent| self.locate(parent)).filter(alone);
        let set = beside.map_or_else(|| self.set_at(pointer), |at| at.set);
        let shown = self.sets[set].active;
        let showing = &self.sets[set].workspaces[shown];
        // Nobody's dialog and sent nowhere by a rule, it is for where the
        // user is: a part with nothing in it, if there is one, and failing
        // that the desktop.
        let free = beside.is_none() && effects.workspace.is_none();
        let made = showing.solo.filter(|_| free);
        let room = made.filter(|tiling| tiling.has_room());
        // Failing that, the desktop it was made from, which is not shown
        // for it.
        let home = made.filter(|_| room.is_none());
        let home = home.map(|solo| self.sets[set].place(solo.origin));
        let workspace = beside.map_or(home.unwrap_or(shown), |at| at.workspace);
        let workspace = effects
            .workspace
            .map_or(workspace, |on| self.reach(set, on));
        let decorated = effects.decorations.unwrap_or(wants_decoration);
        let area = self.output_at(pointer).usable;
        let fills = effects.maximized == Some(true) || effects.fullscreen == Some(true);
        let size = match (fills, decorated) {
            // The window drew itself to fill the output, so its own idea of
            // a size was never seen. When restored it gets a moderate one.
            (true, _) => Size::new(area.size.width / 3 * 2, area.size.height / 3 * 2),
            (false, true) => self.settings.decoration.outer_size(size),
            (false, false) => size,
        };
        let index = self.sets[set].workspaces[workspace].groups.len();
        let origin = place(self.settings.placement, size, area, pointer, index);
        let id = WindowId(self.serial());
        let window = Window {
            id,
            app_id,
            title,
            decorated,
            least: Size::default(),
        };
        let group = Group {
            serial: self.serial(),
            tabs: vec![window],
            active: 0,
            geometry: Rect::new(origin, size),
            maximized: effects.maximized.unwrap_or(false),
            fullscreen: room.is_some(),
            minimized: false,
            // Only a frame with a titlebar can be rolled up into it.
            standing: Standing {
                shaded: effects.shaded.unwrap_or(false) && decorated,
                above: effects.above.unwrap_or(false),
                sticky: effects.sticky.unwrap_or(false),
            },
        };
        let filled = room.map(|tiling| tiling.filled(group.serial));
        self.push_focused(set, workspace, group);
        self.current = set;
        if filled.is_some() {
            self.sets[set].workspaces[workspace].solo = filled;
        }
        if effects.fullscreen == Some(true) {
            self.set_fullscreen(id, true);
        }
        id
    }

    /// Says how a window about to be added should first draw itself, if
    /// rules open it maximized or fullscreen; `None` if it may choose its
    /// own size.
    ///
    /// Asking before the window has drawn anything lets it appear at its
    /// final size, instead of at its natural size for one frame.
    #[must_use]
    pub fn opening(
        &self,
        app_id: &str,
        title: &str,
        pointer: Point,
        wants_decoration: bool,
    ) -> Option<Opening> {
        let effects = resolve(&self.rules, app_id, title);
        let fullscreen = effects.fullscreen == Some(true);
        let maximized = effects.maximized == Some(true);
        let screen = self.output_at(pointer);
        let decorated = effects.decorations.unwrap_or(wants_decoration);
        let size = match (fullscreen, decorated) {
            (true, _) => screen.area.size,
            (false, true) => {
                Decoration::new(screen.usable, self.settings.decoration, 1)
                    .client()
                    .size
            }
            (false, false) => screen.usable.size,
        };
        (fullscreen || maximized).then_some(Opening {
            size,
            maximized,
            fullscreen,
        })
    }

    /// Removes a window, for example because it closed.
    pub fn remove_window(&mut self, id: WindowId) {
        let Some(at) = self.locate(id) else { return };
        self.sets[at.set].workspaces[at.workspace].groups[at.group].remove(at.tab);
        self.sets[at.set].workspaces[at.workspace].repair();
        self.sets[at.set].settle();
    }

    /// Updates a window's title.
    pub fn set_title(&mut self, id: WindowId, title: impl Into<String>) {
        let Some(at) = self.locate(id) else { return };
        self.sets[at.set].workspaces[at.workspace].groups[at.group].tabs[at.tab].title =
            title.into();
    }

    /// Looks a window up.
    #[must_use]
    pub fn window(&self, id: WindowId) -> Option<&Window> {
        self.locate(id)
            .map(|at| &self.sets[at.set].workspaces[at.workspace].groups[at.group].tabs[at.tab])
    }

    /// Whether a window is to be left out of pictures taken of the
    /// screen: a rule says so of it, or of a window it shares a frame
    /// with, whose title is on the same titlebar. It is asked of the
    /// windows as they are now, so a title that comes to match hides one.
    #[must_use]
    pub fn private(&self, id: WindowId) -> bool {
        let frame = self.locate(id);
        let tabs =
            frame.map(|at| &self.sets[at.set].workspaces[at.workspace].groups[at.group].tabs);
        let said = |window: &Window| resolve(&self.rules, &window.app_id, &window.title).private;
        tabs.is_some_and(|tabs| tabs.iter().any(|window| said(window) == Some(true)))
    }

    /// Every window on every workspace of every output, in the order frames
    /// were created and then tab order: a stable order suited to a task
    /// list.
    pub fn windows(&self) -> impl Iterator<Item = Listed<'_>> {
        let workspaces = self.sets.iter().enumerate().flat_map(|(output, set)| {
            let on = set.workspaces.iter().enumerate();
            on.map(move |(index, workspace)| (output, index, workspace))
        });
        let mut groups: Vec<(usize, usize, &Workspace, &Group)> = workspaces
            .flat_map(|(output, index, workspace)| {
                let groups = workspace.groups.iter();
                groups.map(move |group| (output, index, workspace, group))
            })
            .collect();
        groups.sort_by_key(|(_, _, _, group)| group.serial);
        let current = (self.current, self.sets[self.current].active);
        groups
            .into_iter()
            .flat_map(move |(output, index, workspace, group)| {
                let focused = (output, index) == current && workspace.focused == Some(group.serial);
                group
                    .tabs
                    .iter()
                    .enumerate()
                    .map(move |(tab, window)| Listed {
                        window,
                        output,
                        workspace: index,
                        minimized: group.minimized,
                        shown: tab == group.active,
                        focused: focused && tab == group.active,
                        standing: group.standing,
                    })
            })
    }

    /// The zero-based workspace a window is on.
    #[must_use]
    pub fn workspace_of(&self, id: WindowId) -> Option<usize> {
        self.locate(id).map(|at| at.workspace)
    }

    /// Sets whether a window gets a titlebar and border, for example
    /// because it turned out to draw its own. The frame keeps its place.
    pub fn set_decorated(&mut self, id: WindowId, decorated: bool) {
        let Some(at) = self.locate(id) else { return };
        self.sets[at.set].workspaces[at.workspace].groups[at.group].tabs[at.tab].decorated =
            decorated;
    }

    // --- Frame state -------------------------------------------------------

    /// Records that a window resized itself to `size`, and resizes its frame
    /// to fit. Ignored while the frame is maximized or fullscreen, when its
    /// size is not the window's to choose, and for a tab that is not shown.
    pub fn set_client_size(&mut self, id: WindowId, size: Size) {
        let Some(at) = self.locate(id) else { return };
        let metrics = self.settings.decoration;
        let group = &mut self.sets[at.set].workspaces[at.workspace].groups[at.group];
        if group.active == at.tab && !group.maximized && !group.fullscreen {
            group.geometry.size = if group.decorated() {
                metrics.outer_size(size)
            } else {
                size
            };
        }
    }

    /// Records the size a window says it can be no smaller than, without
    /// its frame. A part of a workspace windows share that its frame fills
    /// is made as large as that, where there is room.
    pub fn set_least(&mut self, id: WindowId, least: Size) {
        let Some(at) = self.locate(id) else { return };
        let group = &mut self.sets[at.set].workspaces[at.workspace].groups[at.group];
        group.tabs[at.tab].least = least;
    }

    /// Moves and resizes a window's frame, leaving maximized and fullscreen.
    ///
    /// A frame moved so that most of it is on another output now belongs
    /// there: it joins the workspace that output is showing, and that
    /// output becomes the current one.
    pub fn set_geometry(&mut self, id: WindowId, geometry: Rect) {
        let Some(at) = self.locate(id) else { return };
        let at = self.grounded(id, at);
        let group = &mut self.sets[at.set].workspaces[at.workspace].groups[at.group];
        group.geometry = geometry;
        group.maximized = false;
        let onto = self.set_for(geometry).unwrap_or(at.set);
        if onto != at.set {
            self.rehome(at, onto);
        }
    }

    /// Maximizes a window's frame, or restores it.
    pub fn set_maximized(&mut self, id: WindowId, maximized: bool) {
        let Some(group) = self.group_mut(id) else {
            return;
        };
        group.maximized = maximized;
    }

    /// Switches a window's frame between maximized and restored.
    pub fn toggle_maximized(&mut self, id: WindowId) {
        let Some(group) = self.group_mut(id) else {
            return;
        };
        group.maximized = !group.maximized;
    }

    /// Makes a window's frame fullscreen, or returns it to how it was.
    ///
    /// A fullscreen frame is alone on a workspace made for it, directly
    /// after the one it was on, which is shown if that one was. Leaving
    /// fullscreen takes it back to the desktop it came from and does away
    /// with the workspace.
    pub fn set_fullscreen(&mut self, id: WindowId, fullscreen: bool) {
        let Some(at) = self.locate(id) else { return };
        let on = &self.sets[at.set].workspaces[at.workspace];
        match (on.groups[at.group].fullscreen, fullscreen) {
            (true, false) => {
                self.grounded(id, at);
            }
            (false, true) => {
                let made = WorkspaceId(self.serial());
                self.solo(at, made, Tiling::alone);
            }
            _ => {}
        }
    }

    /// Has a window's frame take a half of where it is, leaving the other
    /// half with nothing in it until a window opens there.
    ///
    /// From a desktop, where it is becomes a workspace made for it,
    /// directly after the one it was on, which is shown if that one was.
    /// A frame that is fullscreen takes a half of its workspace. One that
    /// fills a part of a workspace takes a half of that part, if the part
    /// is a whole side of the workspace and the half is the other way
    /// than the workspace is cut: nothing is cut more than twice. Where
    /// it cannot take a half, it goes to the half asked for instead: its
    /// side of the workspace changes places with the other, or it changes
    /// places with the part that shares its side.
    pub fn tile(&mut self, id: WindowId, half: Half) {
        let Some(at) = self.locate(id) else { return };
        let on = &mut self.sets[at.set].workspaces[at.workspace];
        let frame = on.groups[at.group].serial;
        // One that fills a part already takes a half of that part.
        let tile = on.groups[at.group].fullscreen;
        let part = on.solo.filter(|_| tile);
        let part = part.and_then(|tiling| tiling.at(frame).map(|at| (tiling, at)));
        if let Some((tiling, part)) = part {
            on.solo = Some(tiling.halved(part, frame, half));
            return;
        }
        let made = WorkspaceId(self.serial());
        self.solo(at, made, |origin, frame| {
            Tiling::alone(origin, frame).halved((0, 0), frame, half)
        });
    }

    /// Puts a window's frame in a workspace made for it and cut up as an
    /// arrangement has it, in the part so far along in the order they are
    /// read in, or the last if there are not so many. The other parts
    /// have nothing in them until windows open there. The workspace is
    /// directly after the one the window was on, and is shown if that one
    /// was. A frame that is fullscreen, or fills a part elsewhere, leaves
    /// that first.
    pub fn arrange(&mut self, id: WindowId, arrangement: Arrangement, part: usize) {
        let Some(at) = self.locate(id) else { return };
        let at = self.grounded(id, at);
        let made = WorkspaceId(self.serial());
        let (_, cuts) = arrangement.cuts();
        let last = 1 + usize::from(cuts[0]) + usize::from(cuts[1]);
        self.solo(at, made, |origin, frame| {
            Tiling::arranged(origin, arrangement, Some((part.min(last), frame)))
        });
    }

    /// Where the part of a workspace made for windows that `id` fills is,
    /// with how the workspace is cut and the output it is cut from.
    fn tiled(&self, id: WindowId) -> Option<(Location, Tiling, (usize, usize), Rect)> {
        let at = self.locate(id)?;
        let on = &self.sets[at.set].workspaces[at.workspace];
        let tiling = on.solo?;
        let part = tiling.at(on.groups[at.group].serial)?;
        // With no output there is nothing to measure but the cut itself.
        let nominal = Rect::new(Point::new(0, 0), Size::new(WHOLE, WHOLE));
        let area = self
            .outputs
            .get(at.set)
            .map_or(nominal, |output| output.area);
        Some((at, tiling, part, area))
    }

    /// Puts a window's frame in a workspace that was made for windows,
    /// beside those it has, and shows that workspace: in a part with
    /// nothing in it, or in the right half where one window had the whole.
    /// The workspace is one of the window's own output's. Does nothing if
    /// there is no such workspace, its parts are all taken, or the window
    /// fills one of them already.
    pub fn join(&mut self, id: WindowId, workspace: WorkspaceId) {
        let Some(at) = self.locate(id) else { return };
        let set = &self.sets[at.set];
        let wanted =
            |on: &&Workspace| on.id == workspace && on.id != set.workspaces[at.workspace].id;
        let roomy = set.workspaces.iter().find(wanted);
        let Some(roomy) = roomy.and_then(|on| on.solo?.roomy()) else {
            return;
        };
        // It leaves what it filled before, and may be somewhere else for
        // it.
        let at = self.grounded(id, at);
        let set = &mut self.sets[at.set];
        let mut group = set.workspaces[at.workspace].groups.remove(at.group);
        set.workspaces[at.workspace].repair();
        group.fullscreen = true;
        group.minimized = false;
        let to = set.place(workspace);
        let onto = &mut set.workspaces[to];
        onto.solo = Some(roomy.filled(group.serial));
        onto.focused = Some(group.serial);
        onto.groups.push(group);
        set.active = to;
        set.settle();
    }

    /// The part of a workspace made for windows that is at `point`, with
    /// how the workspace is cut and where `id`'s own part is: another part
    /// than the one `id` fills.
    fn part_at(&self, id: WindowId, point: Point) -> Option<Dropped> {
        let (at, tiling, own, area) = self.tiled(id)?;
        let groups = &self.sets[at.set].workspaces[at.workspace].groups;
        let roomy = tiling.with_room(area, groups, self.settings.decoration.titlebar);
        let mut others = tiling
            .slots()
            .filter(|(other, part)| *other != own && *part != Part::None);
        let (other, place) = others.find_map(|(other, _)| {
            let place = roomy.rect(other, area);
            place.contains(point).then_some((other, place))
        })?;
        Some((at, tiling, [own, other], place))
    }

    /// Where the part is that a window would change places with if it
    /// were dropped at `point`: another part of the workspace it shares,
    /// whether a window fills it or nothing does.
    #[must_use]
    pub fn part_under(&self, id: WindowId, point: Point) -> Option<Rect> {
        self.part_at(id, point).map(|(.., place)| place)
    }

    /// Changes a window over with whatever is in the part of its workspace
    /// at `point`: another window, or nothing. Does nothing for a window
    /// that does not fill a part, or at a point on its own part or none.
    pub fn swap_at(&mut self, id: WindowId, point: Point) {
        if let Some((at, tiling, [own, other], _)) = self.part_at(id, point) {
            let on = &mut self.sets[at.set].workspaces[at.workspace];
            on.solo = Some(tiling.swapped(own, other));
        }
    }

    /// Changes a window over with whatever is in the part beside its own,
    /// on the side `toward` says: another window, or nothing. Does nothing
    /// for a window that does not fill a part, or has nothing that side.
    pub fn swap_toward(&mut self, id: WindowId, toward: Half) {
        let Some((at, tiling, part, area)) = self.tiled(id) else {
            return;
        };
        if let Some(other) = tiling.beside(part, toward, area) {
            let on = &mut self.sets[at.set].workspaces[at.workspace];
            on.solo = Some(tiling.swapped(part, other));
        }
    }

    /// Gives focus to the window in the part beside the focused one's, on
    /// the side `toward` says. Does nothing unless the window with focus
    /// fills a part and there is a window that side.
    pub fn focus_toward(&mut self, toward: Half) {
        let found = self.focused().and_then(|id| self.tiled(id));
        let Some((at, tiling, part, area)) = found else {
            return;
        };
        let other = tiling.beside(part, toward, area);
        let frame = other.and_then(|other| match tiling.sides[other.0].parts[other.1] {
            Part::Tile(frame) => Some(frame),
            _ => None,
        });
        let on = &mut self.sets[at.set].workspaces[at.workspace];
        let group =
            frame.and_then(|frame| on.groups.iter().position(|group| group.serial == frame));
        if let Some(group) = group {
            on.focus(group, false);
        }
    }

    /// Switches a window's frame in or out of fullscreen.
    pub fn toggle_fullscreen(&mut self, id: WindowId) {
        let now = self.group_mut(id).map(|group| group.fullscreen);
        if let Some(now) = now {
            self.set_fullscreen(id, !now);
        }
    }

    /// Puts the frame at `at` on a workspace made for it, known by `made`
    /// and cut up as `cut` says for a frame from a desktop.
    fn solo(
        &mut self,
        at: Location,
        made: WorkspaceId,
        cut: impl FnOnce(WorkspaceId, u64) -> Tiling,
    ) {
        let set = &mut self.sets[at.set];
        let from = &mut set.workspaces[at.workspace];
        let mut group = from.groups.remove(at.group);
        from.repair();
        group.fullscreen = true;
        // Made fullscreen, it is to be seen.
        group.minimized = false;
        // From a workspace made for another's fullscreen window, it goes
        // back where that one would.
        let origin = from.solo.map_or(from.id, |solo| solo.origin);
        let mut solo = Workspace::new(made, false);
        solo.solo = Some(cut(origin, group.serial));
        solo.focused = Some(group.serial);
        solo.groups.push(group);
        set.workspaces.insert(at.workspace + 1, solo);
        // What was being shown still is, unless it is where the frame came
        // from: then the frame is.
        if set.active >= at.workspace {
            set.active += 1;
        }
        set.settle();
    }

    /// Has a frame leave fullscreen, if it is, and says where its window
    /// is afterwards: back on the desktop it came from.
    fn grounded(&mut self, id: WindowId, at: Location) -> Location {
        self.sets[at.set].workspaces[at.workspace].groups[at.group].fullscreen = false;
        self.sets[at.set].settle();
        self.locate(id).unwrap_or(at)
    }

    /// Rolls a window's frame up into its titlebar, or lets it down again.
    pub fn toggle_shaded(&mut self, id: WindowId) {
        self.stand(id, |standing| standing.shaded = !standing.shaded);
    }

    /// Keeps a window's frame above those that are not kept so, or stops.
    pub fn toggle_above(&mut self, id: WindowId) {
        self.stand(id, |standing| standing.above = !standing.above);
    }

    /// Has a window's frame on whichever workspace its output is showing,
    /// or leaves it on the one it is on.
    pub fn toggle_sticky(&mut self, id: WindowId) {
        self.stand(id, |standing| standing.sticky = !standing.sticky);
    }

    /// Changes how a window's frame stands apart from the others.
    fn stand(&mut self, id: WindowId, change: impl FnOnce(&mut Standing)) {
        if let Some(group) = self.group_mut(id) {
            change(&mut group.standing);
        }
    }

    /// Hides a window's frame. Focus passes to the topmost remaining frame.
    /// [`Desktop::activate`] brings it back.
    pub fn minimize(&mut self, id: WindowId) {
        let Some(at) = self.locate(id) else { return };
        let set = &mut self.sets[at.set];
        let on = &mut set.workspaces[at.workspace];
        on.groups[at.group].minimized = true;
        on.repair();
        // With the frame a workspace was made for put away, there is
        // nothing there to look at.
        let shown = set.active == at.workspace;
        let dark = on.tiles().iter().all(|tile| tile.minimized) && shown;
        if let Some(solo) = on.solo.filter(|_| dark) {
            set.active = set.place(solo.origin);
            set.settle();
        }
    }

    // --- Focus -------------------------------------------------------------

    /// Brings a window to the user's attention: switches to its workspace,
    /// restores it if minimized, shows its tab, focuses and raises it.
    pub fn activate(&mut self, id: WindowId) {
        let Some(at) = self.locate(id) else { return };
        self.turn(at.set, at.workspace);
        self.current = at.set;
        // It may have been carried along with what its output is showing,
        // and is then somewhere else than it was found.
        let at = self.locate(id).unwrap_or(at);
        self.show(at, true);
    }

    /// Reports a click on a window, which focuses it and shows its tab.
    pub fn clicked(&mut self, id: WindowId) {
        let Some(at) = self.locate(id) else { return };
        self.current = at.set;
        self.show(at, self.settings.raise_on_focus);
    }

    /// Reports the pointer moving onto a window, which focuses it when focus
    /// follows the pointer.
    pub fn pointer_entered(&mut self, id: WindowId) {
        let Some(at) = self.locate(id) else { return };
        if self.settings.focus_follows_pointer {
            self.current = at.set;
            self.sets[at.set].workspaces[at.workspace]
                .focus(at.group, self.settings.raise_on_focus);
        }
    }

    /// Focuses and raises the frame created after the focused one, wrapping
    /// around.
    pub fn focus_next(&mut self) {
        self.cycle_focus(false);
    }

    /// Focuses and raises the frame created before the focused one, wrapping
    /// around.
    pub fn focus_previous(&mut self) {
        self.cycle_focus(true);
    }

    /// The window with keyboard focus: the shown tab of the focused frame on
    /// the workspace the current output is showing.
    #[must_use]
    pub fn focused(&self) -> Option<WindowId> {
        self.frames()
            .find(|frame| frame.focused)
            .map(|frame| frame.window().id)
    }

    // --- Tabs --------------------------------------------------------------

    /// Shows the tab after the current one in a window's frame, wrapping.
    pub fn next_tab(&mut self, id: WindowId) {
        let Some(group) = self.group_mut(id) else {
            return;
        };
        group.active = (group.active + 1) % group.tabs.len();
    }

    /// Shows the tab before the current one in a window's frame, wrapping.
    pub fn previous_tab(&mut self, id: WindowId) {
        let Some(group) = self.group_mut(id) else {
            return;
        };
        group.active = (group.active + group.tabs.len() - 1) % group.tabs.len();
    }

    /// Moves a window into `target`'s frame as its last and shown tab, and
    /// focuses that frame.
    pub fn attach(&mut self, id: WindowId, target: WindowId) {
        let (Some(from), Some(to)) = (self.locate(id), self.locate(target)) else {
            return;
        };
        if (from.set, from.workspace, from.group) == (to.set, to.workspace, to.group) {
            return;
        }
        let window =
            self.sets[from.set].workspaces[from.workspace].groups[from.group].remove(from.tab);
        let group = &mut self.sets[to.set].workspaces[to.workspace].groups[to.group];
        group.tabs.push(window);
        group.active = group.tabs.len() - 1;
        self.sets[to.set].workspaces[to.workspace].focus(to.group, true);
        self.sets[from.set].workspaces[from.workspace].repair();
        self.sets[from.set].settle();
    }

    /// Moves a window out of a shared frame into a frame of its own, just
    /// off the corner of the old one, and focuses it.
    pub fn detach(&mut self, id: WindowId) {
        let Some(at) = self.locate(id) else { return };
        let serial = self.serial();
        let source = &mut self.sets[at.set].workspaces[at.workspace].groups[at.group];
        if source.tabs.len() < 2 {
            return;
        }
        let step = CASCADE_STEP.cast_signed();
        let group = Group {
            serial,
            tabs: vec![source.remove(at.tab)],
            active: 0,
            geometry: Rect::new(
                source.geometry.origin.offset(step, step),
                source.geometry.size,
            ),
            maximized: false,
            fullscreen: false,
            minimized: false,
            standing: Standing::default(),
        };
        self.push_focused(at.set, at.workspace, group);
    }

    // --- Workspaces --------------------------------------------------------

    /// The number of desktops the current output has. At least one. A
    /// workspace made for a fullscreen window is not counted.
    #[must_use]
    pub fn workspace_count(&self) -> usize {
        self.sets[self.current].desktops().len()
    }

    /// Every workspace there is, output by output and in order.
    pub fn workspaces(&self) -> impl Iterator<Item = Place> {
        self.sets.iter().enumerate().flat_map(|(output, set)| {
            let desktops = set.desktops();
            let workspaces = set.workspaces.iter().enumerate();
            workspaces.map(move |(index, workspace)| Place {
                id: workspace.id,
                output,
                index,
                kept: workspace.kept,
                shown: index == set.active,
                desktop: desktops.iter().position(|desktop| *desktop == index),
                tiles: {
                    let tiles = workspace.tiles().into_iter();
                    tiles.map(|tile| tile.tabs[tile.active].id).collect()
                },
                room: workspace
                    .solo
                    .is_some_and(|tiling| tiling.roomy().is_some()),
            })
        })
    }

    /// The workspace that is an output's desktop `wanted`, from zero: that
    /// one, or a new one made at the end if `wanted` is past the last and
    /// there is room for another, or else the last.
    fn reach(&mut self, set: usize, wanted: usize) -> usize {
        let desktops = self.sets[set].desktops();
        if wanted >= desktops.len() && desktops.len() < Self::MOST_WORKSPACES {
            let id = WorkspaceId(self.serial());
            self.sets[set].workspaces.push(Workspace::new(id, false));
            return self.sets[set].workspaces.len() - 1;
        }
        desktops[wanted.min(desktops.len() - 1)]
    }

    /// Makes a workspace at the end of the current output's and shows it.
    /// It goes again once it is empty and another is shown.
    pub fn new_workspace(&mut self) {
        let workspace = self.reach(self.current, usize::MAX);
        self.turn(self.current, workspace);
    }

    /// Asks for the titlebar of whatever fullscreen window an output is
    /// showing to be laid over it, `down` from the top of the output, or
    /// with `None` for it to be taken away again. Counted in the order the
    /// outputs were given; an output that is not there is not asked.
    ///
    /// The window is not told to be any other size. A window that draws
    /// its own titlebar is given none.
    pub fn reveal(&mut self, output: usize, down: Option<u32>) {
        if let Some(set) = self.sets.get_mut(output) {
            set.revealed = down;
        }
    }

    /// The lines between the parts of the workspace an output is showing,
    /// if it is showing one that windows share. Outputs are counted in
    /// the order they were given.
    #[must_use]
    pub fn lines(&self, output: usize) -> Vec<Line> {
        let titlebar = self.settings.decoration.titlebar;
        let cut = self.sets.get(output).zip(self.outputs.get(output));
        let cut = cut.and_then(|(set, output)| {
            let shown = &set.workspaces[set.active];
            Some((shown.solo?, &shown.groups, output.area))
        });
        cut.map_or_else(Vec::new, |(tiling, groups, area)| {
            tiling.with_room(area, groups, titlebar).lines(area)
        })
    }

    /// The windows offered in each part with nothing in it of the workspace
    /// an output is showing, if it is one that windows share: those of the
    /// desktop it was made from, the one on top first, each where its
    /// picture is. Outputs are counted in the order they were given.
    #[must_use]
    pub fn offers(&self, output: usize) -> Vec<Offer> {
        let Some((set, screen)) = self.sets.get(output).zip(self.outputs.get(output)) else {
            return Vec::new();
        };
        let shown = &set.workspaces[set.active];
        let Some(tiling) = shown.solo else {
            return Vec::new();
        };
        let metrics = self.settings.decoration;
        let from = set.workspaces.iter().filter(|on| on.solo.is_none());
        let from = from.filter(|on| on.id == tiling.origin);
        let groups = from.flat_map(|on| on.groups.iter().rev());
        let (windows, sizes): (Vec<WindowId>, Vec<Size>) = groups
            .map(|group| {
                let size = group.geometry.size;
                let inner = Size::new(
                    size.width.saturating_sub(metrics.border.saturating_mul(2)),
                    size.height
                        .saturating_sub(metrics.border)
                        .saturating_sub(metrics.titlebar),
                );
                let size = if group.decorated() { inner } else { size };
                (group.tabs[group.active].id, size)
            })
            .unzip();
        let roomy = tiling.with_room(screen.area, &shown.groups, metrics.titlebar);
        let empty = tiling.slots().filter(|(_, part)| *part == Part::Empty);
        empty
            .flat_map(|(at, _)| {
                let places = offered(roomy.rect(at, screen.area), &sizes);
                windows.iter().zip(places).map(|(window, placed)| Offer {
                    window: *window,
                    at: placed.picture,
                    title: placed.title,
                    whole: placed.whole,
                })
            })
            .collect()
    }

    /// Puts a window offered in a part with nothing in it in the part at
    /// `point`, on the workspace its output is showing. Does nothing where
    /// there is no such part.
    pub fn choose(&mut self, id: WindowId, point: Point) {
        let Some(at) = self.locate(id) else { return };
        let set = &self.sets[at.set];
        let shown = set.workspaces[set.active].id;
        self.join(id, shown);
        self.swap_at(id, point);
    }

    /// Moves one of the lines of the workspace an output is showing to
    /// `point`, or as near it as leaves each part a sixth of the output;
    /// or with `None` puts it back in the middle. Does nothing where there
    /// is no such line.
    ///
    /// Nor does it leave a window less room than it says it can be no
    /// smaller than, where there is room for that: the line stops.
    pub fn move_line(&mut self, output: usize, which: usize, point: Option<Point>) {
        if !self.lines(output).iter().any(|line| line.which == which) {
            return;
        }
        let area = self.outputs[output].area;
        let titlebar = self.settings.decoration.titlebar;
        let set = &mut self.sets[output];
        let shown = &mut set.workspaces[set.active];
        let frame = least_of(&shown.groups, titlebar);
        shown.solo = shown
            .solo
            .map(|tiling| tiling.cut(which, point, area, frame));
    }

    /// Makes the output at `point` the current one: the one whose focused
    /// frame has the keyboard, and which switching workspace applies to. A
    /// point on no output leaves things as they are.
    pub fn turn_to(&mut self, point: Point) {
        let containing = |output: &Screen| output.area.contains(point);
        if let Some(index) = self.outputs.iter().position(containing) {
            self.current = index;
        }
    }

    /// The output that is current, counted in the order the outputs were
    /// given. Zero while there is none.
    #[must_use]
    pub const fn current_output(&self) -> usize {
        self.current
    }

    /// The zero-based workspace the current output is showing.
    #[must_use]
    pub fn active_workspace(&self) -> usize {
        self.sets[self.current].active
    }

    /// The zero-based workspace each output is showing, in the order the
    /// outputs were given. One entry while there is no output.
    pub fn shown_workspaces(&self) -> impl Iterator<Item = usize> {
        self.sets.iter().map(|set| set.active)
    }

    /// Shows a workspace on the current output. One past the last is made
    /// and shown; further than that does nothing.
    pub fn switch_workspace(&mut self, workspace: usize) {
        if workspace <= self.workspace_count() {
            let workspace = self.reach(self.current, workspace);
            self.turn(self.current, workspace);
        }
    }

    /// Draws the workspaces of the current output slid across by `offset`,
    /// in units of layout, as fingers on a touchpad carry them: the one
    /// shown moved right by it, and beside it the one that comes in on the
    /// side it leaves, the next from the right or the one before from the
    /// left. Frames that are on every workspace stay where they are, and
    /// what is shown and has the keyboard does not change. `0` ends it.
    pub fn slide(&mut self, offset: i32) {
        self.sets[self.current].slid = offset;
    }

    /// How far right the workspace the current output shows is slid.
    #[must_use]
    pub fn slid(&self) -> i32 {
        self.sets[self.current].slid
    }

    /// The workspace that comes in beside the one a set shows while it is
    /// slid, and how far right it is drawn; none if it is not slid, or if
    /// there is no other.
    fn beside(&self, set: usize) -> Option<(usize, i32)> {
        let shown = &self.sets[set];
        let width = self
            .outputs
            .get(set)
            .map_or(0, |output| output.area.size.width);
        let width = width.cast_signed();
        let count = shown.workspaces.len();
        let (workspace, shift) = match shown.slid.signum() {
            -1 => ((shown.active + 1) % count, shown.slid + width),
            1 => ((shown.active + count - 1) % count, shown.slid - width),
            _ => return None,
        };
        (workspace != shown.active).then_some((workspace, shift))
    }

    /// Shows the following workspace on the current output, wrapping
    /// around.
    pub fn next_workspace(&mut self) {
        let count = self.sets[self.current].workspaces.len();
        let next = (self.sets[self.current].active + 1) % count;
        self.turn(self.current, next);
    }

    /// Shows the preceding workspace on the current output, wrapping
    /// around.
    pub fn previous_workspace(&mut self) {
        let count = self.sets[self.current].workspaces.len();
        let previous = (self.sets[self.current].active + count - 1) % count;
        self.turn(self.current, previous);
    }

    /// Sends a window's whole frame to another workspace of its output,
    /// where it becomes the focused, topmost frame. One past the last is
    /// made for it; further than that does nothing. The workspace being
    /// shown does not change.
    pub fn move_to_workspace(&mut self, id: WindowId, workspace: usize) {
        let Some(at) = self.locate(id) else { return };
        if workspace <= self.sets[at.set].desktops().len() {
            self.send(id, at, workspace);
        }
    }

    /// Sends a window's whole frame to a workspace made for it, after the
    /// last of its output's. The workspace being shown does not change.
    pub fn move_to_new_workspace(&mut self, id: WindowId) {
        let Some(at) = self.locate(id) else { return };
        self.send(id, at, usize::MAX);
    }

    /// Sends the frame of a window at `at` to a desktop of its output,
    /// making one at the end if `workspace` is past the last. A fullscreen
    /// frame leaves fullscreen to go.
    fn send(&mut self, id: WindowId, at: Location, workspace: usize) {
        let at = self.grounded(id, at);
        let workspace = self.reach(at.set, workspace);
        if workspace == at.workspace {
            return;
        }
        let mut group = self.sets[at.set].workspaces[at.workspace]
            .groups
            .remove(at.group);
        // Sent to one workspace, it is no longer on all of them.
        group.standing.sticky = false;
        self.sets[at.set].workspaces[at.workspace].repair();
        self.push_focused(at.set, workspace, group);
        self.sets[at.set].settle();
    }

    /// Sends a window's whole frame to the next output, wrapping around,
    /// keeping its place relative to the output's corner where that fits.
    /// It joins the workspace that output is showing, and that output
    /// becomes the current one. Does nothing with fewer than two outputs.
    pub fn move_to_next_output(&mut self, id: WindowId) {
        let Some(at) = self.locate(id) else { return };
        let onto = (at.set + 1) % self.sets.len();
        let (Some(from), Some(to)) = (self.outputs.get(at.set), self.outputs.get(onto)) else {
            return;
        };
        if onto == at.set {
            return;
        }
        let (from, to) = (from.usable, *to);
        let at = self.grounded(id, at);
        let group = &mut self.sets[at.set].workspaces[at.workspace].groups[at.group];
        let within = (
            group.geometry.origin.x.saturating_sub(from.origin.x),
            group.geometry.origin.y.saturating_sub(from.origin.y),
        );
        group.geometry.origin = to.usable.origin.offset(within.0, within.1);
        if to.area.intersection(group.geometry).is_none() {
            group.geometry.origin = to.usable.origin;
        }
        self.rehome(at, onto);
    }

    // --- What to show ------------------------------------------------------

    /// The frames to draw, bottom to top: those on the workspace each
    /// output is showing. Minimized frames are left out, fullscreen frames
    /// are above all others, and the current output's are above the rest
    /// where frames of two outputs overlap.
    pub fn frames(&self) -> impl Iterator<Item = Frame<'_>> {
        // The current set last, so that its frames are on top.
        let order = (0..self.sets.len())
            .filter(|index| *index != self.current)
            .chain([self.current]);
        // While a set is slid, the workspace beside the one it shows is
        // drawn too, moved across with it.
        let shown = move |level: u8| {
            order.clone().flat_map(move |index| {
                let set = &self.sets[index];
                let here = [(set.active, set.slid, true)].into_iter();
                let beside = self.beside(index).map(|(at, shift)| (at, shift, false));
                here.chain(beside).flat_map(move |(at, shift, shown)| {
                    let workspace = &set.workspaces[at];
                    let groups = workspace.groups.iter();
                    groups
                        .filter(move |group| !group.minimized && group.level() == level)
                        .map(move |group| {
                            let shift = if group.standing.sticky { 0 } else { shift };
                            (index, workspace, group, (shift, shown))
                        })
                })
            })
        };
        (0..3)
            .flat_map(shown)
            .map(|(index, workspace, group, (shift, shown))| {
                // A frame fills its own output; with none, it has only
                // itself to fill.
                let own = self.outputs.get(index).copied();
                let screen = own.unwrap_or_else(|| Screen::from(group.geometry));
                let part = workspace.solo.filter(|_| group.fullscreen);
                let part = part.map(|tiling| {
                    let titlebar = self.settings.decoration.titlebar;
                    let roomy = tiling.with_room(screen.area, &workspace.groups, titlebar);
                    roomy.part_of(group.serial, screen.area)
                });
                let mut geometry = match (part, group.maximized) {
                    (Some(part), _) => part,
                    (None, true) => screen.usable,
                    (None, false) => group.geometry,
                };
                geometry.origin = geometry.origin.offset(shift, 0);
                let home = workspace.solo.map_or(Home::Desktop, |tiling| {
                    if tiling.shared() {
                        Home::Shared
                    } else {
                        Home::Solo
                    }
                });
                let decorated = group.decorated() && !group.fullscreen;
                // Rolled up, a frame is as tall as its titlebar and no more.
                let shaded = group.standing.shaded && decorated;
                let titlebar = self.settings.decoration.titlebar;
                let rolled = geometry.size.height.min(titlebar);
                geometry.size.height = if shaded { rolled } else { geometry.size.height };
                Frame {
                    tabs: &group.tabs,
                    active: group.active,
                    geometry,
                    decoration: decorated
                        .then_some((geometry, self.settings.decoration))
                        .or(self.laid_over(index, group, geometry, home == Home::Shared))
                        .map(|(within, metrics)| {
                            Decoration::new(within, metrics, group.tabs.len())
                        }),
                    maximized: group.maximized,
                    fullscreen: group.fullscreen,
                    home,
                    focused: index == self.current
                        && shown
                        && workspace.focused == Some(group.serial),
                    standing: Standing {
                        shaded,
                        ..group.standing
                    },
                }
            })
    }

    // --- Internals ---------------------------------------------------------

    /// Where the titlebar goes of a frame that fills a workspace or a part
    /// of one, and with what sizes: across the top of what it fills, with
    /// no borders.
    ///
    /// One that shares its workspace always has its titlebar, and its
    /// window the room under it. One that is alone has it only when it is
    /// asked for, laid over the window so far down from the top.
    fn laid_over(
        &self,
        set: usize,
        group: &Group,
        geometry: Rect,
        shared: bool,
    ) -> Option<(Rect, Metrics)> {
        let asked = self.sets[set].revealed;
        let down = if shared { Some(0) } else { asked };
        let down = down.filter(|_| group.fullscreen && group.decorated())?;
        let size = Size::new(
            geometry.size.width,
            geometry.size.height.saturating_sub(down),
        );
        let within = Rect::new(geometry.origin.offset(0, down.cast_signed()), size);
        let metrics = Metrics {
            border: 0,
            ..self.settings.decoration
        };
        Some((within, metrics))
    }

    const fn serial(&mut self) -> u64 {
        self.next_serial += 1;
        self.next_serial
    }

    fn locate(&self, id: WindowId) -> Option<Location> {
        self.sets.iter().enumerate().find_map(|(set, in_set)| {
            let workspaces = in_set.workspaces.iter().enumerate();
            workspaces.into_iter().find_map(|(workspace, on)| {
                on.groups.iter().enumerate().find_map(|(group, in_group)| {
                    let tab = in_group.tabs.iter().position(|window| window.id == id)?;
                    Some(Location {
                        set,
                        workspace,
                        group,
                        tab,
                    })
                })
            })
        })
    }

    fn group_mut(&mut self, id: WindowId) -> Option<&mut Group> {
        self.locate(id)
            .map(|at| &mut self.sets[at.set].workspaces[at.workspace].groups[at.group])
    }

    fn push_focused(&mut self, set: usize, workspace: usize, group: Group) {
        let workspace = &mut self.sets[set].workspaces[workspace];
        workspace.focused = Some(group.serial);
        workspace.groups.push(group);
        // A minimized frame can be moved here; it must not hold focus.
        workspace.repair();
    }

    /// Has an output show another of its workspaces. The frames that are on
    /// every workspace go along to a desktop, on top of what is there, and
    /// one of them that had focus keeps it.
    fn turn(&mut self, set: usize, to: usize) {
        let set = &mut self.sets[set];
        set.active = to;
        set.settle();
    }

    /// Gives a frame to another output's set, on the desktop that output
    /// is showing, focused and on top, and makes that output current. If
    /// that output is showing a workspace made for a fullscreen window,
    /// the frame goes to the desktop that window came from, which is then
    /// shown.
    fn rehome(&mut self, at: Location, onto: usize) {
        let from = &mut self.sets[at.set].workspaces[at.workspace];
        let group = from.groups.remove(at.group);
        from.repair();
        let showing = &self.sets[onto].workspaces[self.sets[onto].active];
        let home = showing.solo.map(|solo| self.sets[onto].place(solo.origin));
        let workspace = home.unwrap_or(self.sets[onto].active);
        self.push_focused(onto, workspace, group);
        self.turn(onto, workspace);
        self.current = onto;
        self.sets[at.set].settle();
    }

    /// The set of the output at `point`, or failing that the first.
    fn set_at(&self, point: Point) -> usize {
        let containing = |output: &Screen| output.area.contains(point);
        self.outputs.iter().position(containing).unwrap_or(0)
    }

    /// The set of the output most of a frame at `geometry` lies on, if it
    /// lies on any.
    fn set_for(&self, geometry: Rect) -> Option<usize> {
        let overlap = |output: &Screen| {
            let shared = output.area.intersection(geometry);
            shared.map_or(0, |shared| shared.size.area())
        };
        // `max_by_key` keeps the last of equal maxima; reversing makes ties
        // resolve to the first output.
        let most = self.outputs.iter().enumerate().rev();
        most.max_by_key(|(_, output)| overlap(output))
            .filter(|(_, output)| overlap(output) > 0)
            .map(|(index, _)| index)
    }

    fn show(&mut self, at: Location, raise: bool) {
        let workspace = &mut self.sets[at.set].workspaces[at.workspace];
        workspace.groups[at.group].active = at.tab;
        workspace.focus(at.group, raise);
    }

    fn cycle_focus(&mut self, backwards: bool) {
        let set = &mut self.sets[self.current];
        let workspace = &mut set.workspaces[set.active];
        let mut serials: Vec<u64> = workspace
            .groups
            .iter()
            .filter(|group| !group.minimized)
            .map(|group| group.serial)
            .collect();
        serials.sort_unstable();
        if backwards {
            serials.reverse();
        }
        let focused = workspace.focused;
        let after_focused = serials
            .iter()
            .skip_while(|serial| Some(**serial) != focused)
            .nth(1);
        let next = after_focused.or(serials.first()).copied();
        if let Some(group) = workspace
            .groups
            .iter()
            .position(|group| Some(group.serial) == next)
        {
            workspace.focus(group, true);
        }
    }

    /// The output containing `point`, or failing that the first one.
    fn output_at(&self, point: Point) -> Screen {
        let containing = self
            .outputs
            .iter()
            .find(|output| output.area.contains(point));
        containing
            .or(self.outputs.first())
            .copied()
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{Effects, Pattern};

    const SCREEN: Rect = Rect::new(Point::new(0, 0), Size::new(1920, 1080));
    const SECOND_SCREEN: Rect = Rect::new(Point::new(1920, 0), Size::new(1280, 1024));
    const SIZE: Size = Size::new(800, 600);
    const POINTER: Point = Point::new(10, 10);

    fn desktop() -> Desktop {
        let mut desktop = Desktop::new(Settings::default(), 3);
        desktop.set_outputs(vec![SCREEN]);
        desktop
    }

    fn open(desktop: &mut Desktop, app_id: &str) -> WindowId {
        desktop.add_window(app_id, format!("{app_id} window"), SIZE, POINTER, true)
    }

    /// What a window that says nothing of how small it can be can be.
    /// The line that cuts the workspace an output is showing in two, if it
    /// is cut.
    fn line(desktop: &Desktop, output: usize) -> Option<Line> {
        desktop.lines(output).first().copied()
    }

    /// Where a window's frame is.
    fn part(desktop: &Desktop, id: WindowId) -> (i32, i32, u32, u32) {
        let at = frame(desktop, id).geometry;
        (at.origin.x, at.origin.y, at.size.width, at.size.height)
    }

    /// Four windows sharing a workspace, a quarter each: `[a, b, c, d]`
    /// for the top left, the bottom left, the top right and the bottom
    /// right.
    fn quarters(desktop: &mut Desktop) -> [WindowId; 4] {
        let a = open(desktop, "a");
        desktop.tile(a, Half::Left);
        let c = open(desktop, "c");
        desktop.tile(a, Half::Top);
        let b = open(desktop, "b");
        desktop.tile(c, Half::Top);
        let d = open(desktop, "d");
        [a, b, c, d]
    }

    const HALVES: [Half; 4] = [Half::Left, Half::Right, Half::Top, Half::Bottom];

    /// What the first workspace made for windows is known by.
    fn desktop_made(desktop: &Desktop) -> WorkspaceId {
        let mut made = desktop.workspaces().filter(|place| !place.tiles.is_empty());
        made.next()
            .map_or(WorkspaceId::from_raw(0), |place| place.id)
    }

    /// Opens a window that says it belongs to `parent`.
    fn open_of(desktop: &mut Desktop, parent: WindowId) -> WindowId {
        desktop.add_window_of(Some(parent), "dialog", "dialog", SIZE, POINTER, true)
    }

    /// The shown window of each frame on the current workspace, bottom to top.
    fn stack(desktop: &Desktop) -> Vec<WindowId> {
        desktop.frames().map(|frame| frame.window().id).collect()
    }

    fn frame(desktop: &Desktop, id: WindowId) -> Frame<'_> {
        desktop
            .frames()
            .find(|frame| frame.tabs.iter().any(|tab| tab.id == id))
            .unwrap()
    }

    fn rule(app_id: &str, effects: Effects) -> Rule {
        Rule {
            app_id: Some(Pattern::new(app_id)),
            title: None,
            effects,
        }
    }

    // --- Construction ------------------------------------------------------

    #[test]
    fn a_new_desktop_is_empty_and_shows_the_first_workspace() {
        let desktop = desktop();
        assert_eq!(desktop.workspace_count(), 3);
        assert_eq!(desktop.active_workspace(), 0);
        assert_eq!(stack(&desktop), []);
        assert_eq!(desktop.focused(), None);
    }

    #[test]
    fn there_is_always_at_least_one_workspace() {
        assert_eq!(Desktop::new(Settings::default(), 0).workspace_count(), 1);
    }

    // --- Windows -----------------------------------------------------------

    #[test]
    fn a_new_window_is_focused_and_on_top() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");
        assert_eq!(stack(&desktop), [first, second]);
        assert_eq!(desktop.focused(), Some(second));
        assert!(frame(&desktop, second).focused);
        assert!(!frame(&desktop, first).focused);
    }

    #[test]
    fn a_new_window_is_decorated_normal_and_alone_in_its_frame() {
        let mut desktop = desktop();
        let id = desktop.add_window("foot", "~", SIZE, POINTER, true);
        let frame = frame(&desktop, id);
        assert_eq!(
            frame.tabs,
            [Window {
                id,
                app_id: "foot".to_owned(),
                title: "~".to_owned(),
                decorated: true,
                least: Size::default(),
            }]
        );
        assert_eq!(
            (frame.active, frame.maximized, frame.fullscreen),
            (0, false, false)
        );
        assert_eq!(desktop.workspace_of(id), Some(0));
    }

    #[test]
    fn new_windows_follow_the_placement_setting() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");
        assert_eq!(
            frame(&desktop, first).geometry,
            Rect::new(Point::new(0, 0), SIZE)
        );
        assert_eq!(
            frame(&desktop, second).geometry,
            Rect::new(Point::new(32, 32), SIZE)
        );

        desktop.set_settings(Settings {
            placement: Placement::Center,
            ..Settings::default()
        });
        let third = open(&mut desktop, "third");
        assert_eq!(
            frame(&desktop, third).geometry,
            Rect::new(Point::new(560, 240), SIZE)
        );
    }

    #[test]
    fn a_new_window_opens_on_the_output_under_the_pointer() {
        let mut desktop = desktop();
        desktop.set_outputs(vec![SCREEN, SECOND_SCREEN]);
        let id = desktop.add_window("app", "", SIZE, Point::new(2000, 500), true);
        assert_eq!(frame(&desktop, id).geometry.origin, SECOND_SCREEN.origin);
    }

    #[test]
    fn a_new_window_opens_on_the_first_output_when_the_pointer_is_on_none() {
        let mut desktop = desktop();
        desktop.set_outputs(vec![SECOND_SCREEN, SCREEN]);
        let id = desktop.add_window("app", "", SIZE, Point::new(-500, -500), true);
        assert_eq!(frame(&desktop, id).geometry.origin, SECOND_SCREEN.origin);
    }

    #[test]
    fn a_window_can_be_added_before_any_output_exists() {
        let mut desktop = Desktop::new(Settings::default(), 1);
        let id = open(&mut desktop, "early");
        assert_eq!(
            frame(&desktop, id).geometry,
            Rect::new(Point::new(0, 0), SIZE)
        );
    }

    #[test]
    fn rules_open_windows_shaded_kept_above_or_on_every_workspace() {
        let mut desktop = desktop();
        let kept = Effects {
            shaded: Some(true),
            above: Some(true),
            sticky: Some(true),
            ..Effects::default()
        };
        desktop.set_rules(vec![rule("notes", kept), rule("bare", kept)]);
        let notes = open(&mut desktop, "notes");
        let standing = frame(&desktop, notes).standing;
        assert!(standing.shaded && standing.above && standing.sticky);
        // Without a titlebar there is nothing to roll up into.
        let bare = desktop.add_window("bare", "bare window".to_owned(), SIZE, POINTER, false);
        let standing = frame(&desktop, bare).standing;
        assert!(!standing.shaded && standing.above && standing.sticky);
        let other = open(&mut desktop, "foot");
        assert_eq!(frame(&desktop, other).standing, Standing::default());
    }

    #[test]
    fn rules_set_the_initial_state_of_matching_windows() {
        let mut desktop = desktop();
        desktop.set_rules(vec![rule(
            "mpv",
            Effects {
                workspace: None,
                maximized: Some(true),
                fullscreen: Some(true),
                decorations: Some(false),
                ..Effects::default()
            },
        )]);
        let player = open(&mut desktop, "mpv");
        let frame = frame(&desktop, player);
        assert!(frame.maximized && frame.fullscreen && !frame.window().decorated);
        let other = open(&mut desktop, "foot");
        desktop.activate(other);
        assert!(!self::frame(&desktop, other).maximized);
    }

    #[test]
    fn a_window_that_rules_leave_alone_chooses_its_own_size() {
        let desktop = decorated_desktop();
        assert_eq!(desktop.opening("foot", "", POINTER, true), None);
    }

    #[test]
    fn a_window_a_rule_maximizes_is_told_the_size_it_will_fill() {
        let mut desktop = decorated_desktop();
        desktop.set_rules(vec![
            rule(
                "editor",
                Effects {
                    maximized: Some(true),
                    ..Effects::default()
                },
            ),
            rule(
                "bare",
                Effects {
                    maximized: Some(true),
                    decorations: Some(false),
                    ..Effects::default()
                },
            ),
        ]);

        let framed = desktop.opening("editor", "", POINTER, true).unwrap();
        assert_eq!(
            framed,
            Opening {
                size: Size::new(1916, 1058),
                maximized: true,
                fullscreen: false
            }
        );
        let bare = desktop.opening("bare", "", POINTER, true).unwrap();
        assert_eq!(bare.size, SCREEN.size, "no frame to leave room for");
        let own_titlebar = desktop.opening("editor", "", POINTER, false).unwrap();
        assert_eq!(own_titlebar.size, SCREEN.size);
    }

    #[test]
    fn a_window_a_rule_makes_fullscreen_is_told_the_whole_output() {
        let mut desktop = decorated_desktop();
        let usable = Rect::new(Point::new(0, 30), Size::new(1920, 1050));
        desktop.set_outputs([Screen {
            area: SCREEN,
            usable,
        }]);
        desktop.set_rules(vec![rule(
            "mpv",
            Effects {
                fullscreen: Some(true),
                ..Effects::default()
            },
        )]);

        let opening = desktop.opening("mpv", "", POINTER, true).unwrap();
        assert_eq!(
            opening,
            Opening {
                size: SCREEN.size,
                maximized: false,
                fullscreen: true
            }
        );
    }

    #[test]
    fn a_window_opened_filling_the_output_restores_to_a_moderate_size() {
        let mut desktop = desktop();
        desktop.set_rules(vec![rule(
            "editor",
            Effects {
                maximized: Some(true),
                ..Effects::default()
            },
        )]);
        let id = desktop.add_window("editor", "", SCREEN.size, POINTER, true);
        assert_eq!(frame(&desktop, id).geometry, SCREEN);

        desktop.set_maximized(id, false);
        assert_eq!(
            frame(&desktop, id).geometry.size,
            Size::new(1280, 720),
            "two thirds of the output"
        );
    }

    #[test]
    fn a_rule_can_open_a_window_on_another_workspace_without_showing_it() {
        let mut desktop = desktop();
        desktop.set_rules(vec![rule(
            "chat",
            Effects {
                workspace: Some(1),
                ..Effects::default()
            },
        )]);
        let here = open(&mut desktop, "foot");
        let chat = open(&mut desktop, "chat");

        assert_eq!(stack(&desktop), [here]);
        assert_eq!(desktop.focused(), Some(here));
        assert_eq!(desktop.workspace_of(chat), Some(1));

        desktop.switch_workspace(1);
        assert_eq!(desktop.focused(), Some(chat));
    }

    #[test]
    fn a_rule_naming_a_workspace_past_the_last_makes_one_at_the_end() {
        let mut desktop = desktop();
        desktop.set_rules(vec![rule(
            "*",
            Effects {
                workspace: Some(99),
                ..Effects::default()
            },
        )]);
        let id = open(&mut desktop, "app");
        assert_eq!(desktop.workspace_of(id), Some(3));
        assert_eq!(desktop.workspace_count(), 4);
        assert_eq!(desktop.active_workspace(), 0, "and does not show it");
    }

    #[test]
    fn removing_the_focused_window_focuses_the_topmost_remaining_frame() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");
        let third = open(&mut desktop, "third");

        desktop.remove_window(third);
        assert_eq!(stack(&desktop), [first, second]);
        assert_eq!(desktop.focused(), Some(second));

        desktop.remove_window(second);
        desktop.remove_window(first);
        assert_eq!(desktop.focused(), None);
        assert_eq!(desktop.window(first), None);
    }

    #[test]
    fn removing_another_window_leaves_focus_alone() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");
        desktop.remove_window(first);
        assert_eq!(desktop.focused(), Some(second));
    }

    #[test]
    fn a_title_can_change() {
        let mut desktop = desktop();
        let id = open(&mut desktop, "editor");
        desktop.set_title(id, "notes.txt");
        assert_eq!(
            desktop.window(id).map(|window| window.title.as_str()),
            Some("notes.txt")
        );
    }

    #[test]
    fn every_window_is_listed_with_where_it_is() {
        let mut desktop = desktop();
        let a = open(&mut desktop, "a");
        let b = open(&mut desktop, "b");
        let c = open(&mut desktop, "c");
        let d = open(&mut desktop, "d");
        desktop.attach(b, a);
        desktop.minimize(c);
        desktop.move_to_workspace(d, 2);

        let listed: Vec<_> = desktop
            .windows()
            .map(|entry| {
                (
                    entry.window.id,
                    entry.workspace,
                    entry.minimized,
                    entry.shown,
                    entry.focused,
                )
            })
            .collect();
        assert_eq!(
            listed,
            [
                (a, 0, false, false, false),
                (b, 0, false, true, true),
                (c, 0, true, true, false),
                (d, 2, false, true, false),
            ]
        );
    }

    #[test]
    fn a_window_focused_on_a_hidden_workspace_does_not_have_keyboard_focus() {
        let mut desktop = desktop();
        open(&mut desktop, "here");
        desktop.switch_workspace(1);
        assert!(desktop.windows().all(|entry| !entry.focused));
    }

    #[test]
    fn identifiers_survive_a_round_trip_through_a_number() {
        let mut desktop = desktop();
        let id = open(&mut desktop, "app");
        assert_eq!(WindowId::from_raw(id.raw()), id);
        desktop.activate(WindowId::from_raw(9999));
        assert_eq!(desktop.focused(), Some(id), "an unknown number is ignored");
    }

    #[test]
    fn window_identifiers_are_never_reused() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        desktop.remove_window(first);
        assert_ne!(open(&mut desktop, "second"), first);
    }

    // --- Decorations -------------------------------------------------------

    const METRICS: Metrics = Metrics {
        border: 2,
        titlebar: 20,
        buttons: [0; 3],
    };

    fn decorated_desktop() -> Desktop {
        let mut desktop = desktop();
        desktop.set_settings(Settings {
            decoration: METRICS,
            ..Settings::default()
        });
        desktop
    }

    #[test]
    fn a_frame_is_larger_than_its_window_by_the_decoration() {
        let mut desktop = decorated_desktop();
        let id = open(&mut desktop, "app");
        let frame = frame(&desktop, id);
        assert_eq!(
            frame.geometry,
            Rect::new(Point::new(0, 0), Size::new(804, 622))
        );
        assert_eq!(frame.client(), Rect::new(Point::new(2, 20), SIZE));
        assert_eq!(
            frame.decoration.map(|decoration| decoration.tabs()),
            Some(1)
        );
    }

    #[test]
    fn a_window_is_private_for_as_long_as_a_rule_says_so_of_it_or_its_frame() {
        let mut desktop = decorated_desktop();
        let private = Effects {
            private: Some(true),
            ..Effects::default()
        };
        let mut by_title = rule("", private);
        by_title.app_id = None;
        by_title.title = Some(Pattern::new("*Bank*"));
        desktop.set_rules(vec![rule("wallet", private), by_title]);

        let wallet = open(&mut desktop, "wallet");
        let browser = open(&mut desktop, "browser");
        assert!(desktop.private(wallet));
        assert!(!desktop.private(browser));
        // A title that comes to match hides the window, and one that no
        // longer does shows it again.
        desktop.set_title(browser, "My Bank - statement");
        assert!(desktop.private(browser));
        desktop.set_title(browser, "News");
        assert!(!desktop.private(browser));

        // A window that shares a frame with a private one is hidden with
        // it: the titlebar says what both are.
        desktop.attach(browser, wallet);
        assert!(desktop.private(browser));
        // A later rule can say that one is not private after all.
        let shown = Effects {
            private: Some(false),
            ..Effects::default()
        };
        desktop.set_rules(vec![rule("wallet", private), rule("wallet", shown)]);
        assert!(!desktop.private(wallet));
        desktop.remove_window(wallet);
        assert!(!desktop.private(wallet), "nor is one that has gone");
    }

    #[test]
    fn a_window_a_rule_leaves_undecorated_fills_its_frame() {
        let mut desktop = decorated_desktop();
        desktop.set_rules(vec![rule(
            "dock",
            Effects {
                decorations: Some(false),
                ..Effects::default()
            },
        )]);
        let id = open(&mut desktop, "dock");
        let frame = frame(&desktop, id);
        assert_eq!(frame.decoration, None);
        assert_eq!(frame.client(), frame.geometry);
        assert_eq!(frame.geometry.size, SIZE);
    }

    #[test]
    fn a_window_that_draws_its_own_titlebar_is_left_alone_unless_a_rule_insists() {
        let mut desktop = decorated_desktop();
        desktop.set_rules(vec![rule(
            "forced",
            Effects {
                decorations: Some(true),
                ..Effects::default()
            },
        )]);
        let own = desktop.add_window("gtk", "", SIZE, POINTER, false);
        let forced = desktop.add_window("forced", "", SIZE, POINTER, false);
        assert_eq!(frame(&desktop, own).decoration, None);
        assert!(frame(&desktop, forced).decoration.is_some());
    }

    #[test]
    fn a_window_can_gain_or_lose_its_decoration_later() {
        let mut desktop = decorated_desktop();
        let id = open(&mut desktop, "app");
        let outer = frame(&desktop, id).geometry;

        desktop.set_decorated(id, false);
        assert_eq!(frame(&desktop, id).decoration, None);
        assert_eq!(
            frame(&desktop, id).client(),
            outer,
            "the frame keeps its place"
        );

        desktop.set_decorated(id, true);
        assert_eq!(frame(&desktop, id).client().size, SIZE);
    }

    #[test]
    fn a_frame_with_several_tabs_is_decorated_even_if_its_windows_are_not() {
        let mut desktop = decorated_desktop();
        desktop.set_rules(vec![rule(
            "*",
            Effects {
                decorations: Some(false),
                ..Effects::default()
            },
        )]);
        let a = open(&mut desktop, "a");
        let b = open(&mut desktop, "b");
        desktop.attach(b, a);
        assert_eq!(
            frame(&desktop, a)
                .decoration
                .map(|decoration| decoration.tabs()),
            Some(2)
        );
    }

    #[test]
    fn a_maximized_frame_keeps_its_decoration_and_a_fullscreen_one_does_not() {
        let mut desktop = decorated_desktop();
        let id = open(&mut desktop, "app");

        desktop.set_maximized(id, true);
        let inside = Rect::new(Point::new(2, 20), Size::new(1916, 1058));
        assert_eq!(frame(&desktop, id).client(), inside);

        desktop.set_fullscreen(id, true);
        assert_eq!(frame(&desktop, id).decoration, None);
        assert_eq!(frame(&desktop, id).client(), SCREEN);
    }

    #[test]
    fn a_window_that_resizes_itself_resizes_its_frame() {
        let mut desktop = decorated_desktop();
        let id = open(&mut desktop, "terminal");
        desktop.set_client_size(id, Size::new(640, 480));
        assert_eq!(frame(&desktop, id).client().size, Size::new(640, 480));
        assert_eq!(frame(&desktop, id).geometry.size, Size::new(644, 502));

        desktop.set_decorated(id, false);
        desktop.set_client_size(id, Size::new(100, 100));
        assert_eq!(frame(&desktop, id).geometry.size, Size::new(100, 100));
    }

    #[test]
    fn a_window_cannot_resize_a_maximized_or_fullscreen_frame_or_one_showing_another_tab() {
        let mut desktop = decorated_desktop();
        let id = open(&mut desktop, "app");
        let normal = frame(&desktop, id).geometry;

        desktop.set_maximized(id, true);
        desktop.set_client_size(id, Size::new(10, 10));
        desktop.set_maximized(id, false);
        desktop.set_fullscreen(id, true);
        desktop.set_client_size(id, Size::new(10, 10));
        desktop.set_fullscreen(id, false);
        assert_eq!(frame(&desktop, id).geometry, normal);

        let other = open(&mut desktop, "other");
        desktop.attach(other, id);
        desktop.set_client_size(id, Size::new(10, 10));
        assert_eq!(
            frame(&desktop, id).geometry,
            normal,
            "`other` is the shown tab"
        );
    }

    #[test]
    fn a_fullscreen_frame_is_alone_on_a_workspace_made_for_it() {
        let mut desktop = desktop();
        let video = open(&mut desktop, "video");
        let chat = open(&mut desktop, "chat");
        desktop.set_fullscreen(video, true);

        assert_eq!(stack(&desktop), [video], "nothing else is there");
        assert_eq!(desktop.focused(), Some(video));
        assert_eq!(frame(&desktop, video).home, Home::Solo);
        assert_eq!(desktop.active_workspace(), 1, "directly after its own");
        assert_eq!(desktop.workspace_count(), 3, "and is not a desktop");
        let places: Vec<Place> = desktop.workspaces().collect();
        let desktops: Vec<Option<usize>> = places.iter().map(|place| place.desktop).collect();
        assert_eq!(desktops, [Some(0), None, Some(1), Some(2)]);
        assert_eq!(places[1].tiles, [video]);
        assert_eq!(places[0].tiles, []);
        assert!(!places[1].kept && places[1].shown);

        // Asked for again, nothing more happens.
        let before = desktop.clone();
        desktop.set_fullscreen(video, true);
        desktop.set_fullscreen(chat, false);
        assert_eq!(desktop, before);
    }

    #[test]
    fn a_fullscreen_frame_has_its_titlebar_laid_over_it_when_asked() {
        let mut desktop = decorated_desktop();
        let video = open(&mut desktop, "video");
        let bare = desktop.add_window("bare", "bare", SIZE, POINTER, false);
        desktop.set_fullscreen(video, true);
        assert_eq!(frame(&desktop, video).decoration, None);

        desktop.reveal(0, Some(30));
        desktop.reveal(7, Some(30));
        let shown = frame(&desktop, video);
        let decoration = shown.decoration.unwrap();
        let bar = Rect::new(Point::new(0, 30), Size::new(1920, METRICS.titlebar));
        assert_eq!(decoration.titlebar(), bar, "across the top, so far down");
        assert_eq!(shown.geometry, SCREEN);
        assert_eq!(shown.client(), SCREEN, "and takes no room from the window");

        desktop.reveal(0, None);
        assert_eq!(frame(&desktop, video).decoration, None);

        // Asked for where nothing is fullscreen, it changes nothing; and a
        // window that draws its own titlebar is given none.
        desktop.set_fullscreen(video, false);
        desktop.reveal(0, Some(0));
        let whole = frame(&desktop, video).decoration.unwrap();
        assert_eq!(whole.outer(), frame(&desktop, video).geometry);
        desktop.set_fullscreen(bare, true);
        assert_eq!(frame(&desktop, bare).decoration, None);
    }

    #[test]
    fn a_window_is_put_in_a_half_of_a_workspace_made_for_it() {
        let mut desktop = desktop();
        let editor = open(&mut desktop, "editor");
        let chat = open(&mut desktop, "chat");
        let (half, tall) = (Size::new(960, 1080), Size::new(1920, 540));
        let halves = [
            (Half::Left, Rect::new(Point::new(0, 0), half)),
            (Half::Right, Rect::new(Point::new(960, 0), half)),
            (Half::Top, Rect::new(Point::new(0, 0), tall)),
            (Half::Bottom, Rect::new(Point::new(0, 540), tall)),
        ];
        for (half, part) in halves {
            let mut desktop = desktop.clone();
            desktop.tile(editor, half);
            let frame = frame(&desktop, editor);
            assert_eq!(frame.geometry, part, "{half:?}");
            assert_eq!(frame.client(), part, "it fills it, with no borders");
            assert_eq!(frame.home, Home::Shared);
            assert!(frame.fullscreen);
            assert_eq!(stack(&desktop), [editor], "and is all there is there");
            assert_eq!(desktop.active_workspace(), 1);
            assert_eq!(desktop.workspaces().count(), 4);
        }
        assert_eq!(desktop.workspace_of(chat), Some(0));
        desktop.tile(WindowId(999), Half::Left);
    }

    #[test]
    fn the_next_window_to_open_fills_the_other_half() {
        let mut desktop = desktop();
        let editor = open(&mut desktop, "editor");
        desktop.tile(editor, Half::Right);

        let browser = open(&mut desktop, "browser");
        let half = Size::new(960, 1080);
        assert_eq!(desktop.active_workspace(), 1, "where the user is");
        assert_eq!(
            frame(&desktop, browser).geometry,
            Rect::new(Point::new(0, 0), half)
        );
        assert_eq!(
            frame(&desktop, editor).geometry,
            Rect::new(Point::new(960, 0), half)
        );
        assert_eq!(frame(&desktop, browser).home, Home::Shared);
        assert_eq!(desktop.focused(), Some(browser));
        let places: Vec<Place> = desktop.workspaces().collect();
        assert_eq!(
            places[1].tiles,
            [browser, editor],
            "in the order of the parts"
        );

        // With both halves taken, the next is for the desktop, unseen.
        let third = open(&mut desktop, "third");
        assert_eq!(desktop.workspace_of(third), Some(0));
        assert_eq!(desktop.focused(), Some(browser), "the keyboard stays");
        desktop.activate(third);
        assert_eq!(desktop.active_workspace(), 0);
        assert_eq!(stack(&desktop), [third]);
    }

    #[test]
    fn the_line_between_two_windows_is_moved_to_give_one_more_room() {
        let mut desktop = desktop();
        let editor = open(&mut desktop, "editor");
        assert_eq!(line(&desktop, 0), None, "a desktop has none");
        let nowhere = Desktop::new(Settings::default(), 1);
        assert_eq!(line(&nowhere, 0), None, "nor is there one with no output");
        desktop.set_fullscreen(editor, true);
        assert_eq!(line(&desktop, 0), None, "nor a window alone");
        desktop.move_line(0, 0, Some(Point::new(100, 100)));
        desktop.tile(editor, Half::Left);
        let browser = open(&mut desktop, "browser");
        let tall = |x: i32| Rect::new(Point::new(x, 0), Size::new(1, 1080));
        let first = line(&desktop, 0).unwrap();
        assert_eq!((first.at, first.across, first.which), (tall(960), false, 0));
        assert_eq!(line(&desktop, 7), None);

        desktop.move_line(0, 0, Some(Point::new(1201, 300)));
        assert_eq!(
            line(&desktop, 0).unwrap().at,
            tall(1201),
            "to the very pixel"
        );
        let widths = |desktop: &Desktop| {
            let of = |id| frame(desktop, id).geometry;
            (
                of(editor).size.width,
                of(browser).origin.x,
                of(browser).size.width,
            )
        };
        assert_eq!(widths(&desktop), (1201, 1201, 719));

        // Neither is left with less than a sixth of the screen.
        desktop.move_line(0, 0, Some(Point::new(5, 0)));
        assert_eq!(widths(&desktop), (320, 320, 1600));
        desktop.move_line(0, 0, Some(Point::new(-50, 0)));
        assert_eq!(widths(&desktop), (320, 320, 1600));
        desktop.move_line(0, 0, Some(Point::new(5000, 0)));
        assert_eq!(widths(&desktop), (1600, 1600, 320));

        // And it goes back to the middle when asked.
        desktop.move_line(0, 0, None);
        assert_eq!(widths(&desktop), (960, 960, 960));
    }

    #[test]
    fn a_line_stops_where_a_window_says_it_can_be_no_smaller() {
        let mut desktop = desktop();
        let editor = open(&mut desktop, "editor");
        desktop.tile(editor, Half::Left);
        let browser = open(&mut desktop, "browser");
        desktop.set_least(editor, Size::new(700, 2000));
        desktop.set_least(browser, Size::new(500, 100));
        let width = |desktop: &Desktop| frame(desktop, editor).geometry.size.width;
        desktop.move_line(0, 0, Some(Point::new(400, 0)));
        assert_eq!(width(&desktop), 700, "the editor's least");
        desktop.move_line(0, 0, Some(Point::new(1800, 0)));
        assert_eq!(width(&desktop), 1420, "the browser's least");
        desktop.move_line(0, 0, Some(Point::new(1000, 0)));
        assert_eq!(width(&desktop), 1000, "between them, where it is put");

        // Where there is not room for both, the second has what it needs.
        desktop.set_least(editor, Size::new(1200, 0));
        desktop.set_least(browser, Size::new(1200, 0));
        desktop.move_line(0, 0, None);
        assert_eq!(width(&desktop), 720);

        // And none can ask for so much that the other has less than its
        // sixth of the screen.
        desktop.set_least(editor, Size::default());
        desktop.set_least(browser, Size::new(u32::MAX, 0));
        desktop.move_line(0, 0, None);
        assert_eq!(width(&desktop), 320);

        // A window that shares a workspace has its titlebar over it, and
        // a frame of tabs needs room for the largest of them; a window
        // that draws its own titlebar is given none.
        let mut desktop = self::desktop();
        let top = open(&mut desktop, "top");
        desktop.tile(top, Half::Top);
        let bottom = open(&mut desktop, "bottom");
        let tab = open(&mut desktop, "tab");
        desktop.attach(tab, bottom);
        desktop.activate(bottom);
        let titlebar = Settings::default().decoration.titlebar;
        for (id, height) in [(top, 300), (bottom, 300), (tab, 600)] {
            desktop.set_least(id, Size::new(0, height));
        }
        let height = |desktop: &Desktop| frame(desktop, top).geometry.size.height;
        desktop.move_line(0, 0, Some(Point::new(0, 1000)));
        assert_eq!(height(&desktop), 1080 - 600 - titlebar);
        desktop.move_line(0, 0, Some(Point::new(0, 0)));
        assert_eq!(height(&desktop), 300 + titlebar);
        desktop.set_decorated(top, false);
        desktop.move_line(0, 0, Some(Point::new(0, 0)));
        assert_eq!(height(&desktop), 300);
    }

    #[test]
    fn a_line_beside_a_part_with_nothing_in_it_stops_only_for_the_window() {
        let mut desktop = desktop();
        let editor = open(&mut desktop, "editor");
        desktop.tile(editor, Half::Left);
        desktop.set_least(editor, Size::new(1200, 0));
        desktop.move_line(0, 0, Some(Point::new(1700, 0)));
        assert_eq!(part(&desktop, editor), (0, 0, 1600, 1080));
        desktop.move_line(0, 0, Some(Point::new(100, 0)));
        assert_eq!(part(&desktop, editor), (0, 0, 1200, 1080));
    }

    #[test]
    fn a_window_that_says_it_can_be_no_smaller_is_given_room_without_the_line_moved() {
        let mut desktop = desktop();
        let editor = open(&mut desktop, "editor");
        desktop.tile(editor, Half::Left);
        let browser = open(&mut desktop, "browser");
        desktop.move_line(0, 0, Some(Point::new(1201, 0)));
        let width = |desktop: &Desktop| frame(desktop, editor).geometry.size.width;
        let at = |desktop: &Desktop| line(desktop, 0).map(|line| line.at.origin.x);

        // Said later, the line goes as far as it takes, and is drawn there.
        desktop.set_least(browser, Size::new(1000, 0));
        assert_eq!(width(&desktop), 920);
        assert_eq!(at(&desktop), Some(920));
        // And it goes back where it was put once that no longer holds.
        desktop.set_least(browser, Size::default());
        assert_eq!(width(&desktop), 1201);
        assert_eq!(at(&desktop), Some(1201));
        desktop.set_least(WindowId::from_raw(999), Size::new(5000, 5000));
        assert_eq!(width(&desktop), 1201, "nor does a window there is not");

        // Said before, the part is made as large as it needs.
        let mut desktop = self::desktop();
        let editor = open(&mut desktop, "editor");
        desktop.set_least(editor, Size::new(1300, 0));
        desktop.tile(editor, Half::Left);
        assert_eq!(part(&desktop, editor), (0, 0, 1300, 1080));
    }

    #[test]
    fn the_line_within_a_side_makes_room_as_well() {
        let mut desktop = desktop();
        let top = open(&mut desktop, "top");
        desktop.tile(top, Half::Left);
        let right = open(&mut desktop, "right");
        desktop.tile(top, Half::Top);
        let bottom = open(&mut desktop, "bottom");
        assert_eq!(frame(&desktop, right).geometry.size.width, 960);
        let height = |desktop: &Desktop| frame(desktop, top).geometry.size.height;
        let before = height(&desktop);
        desktop.set_least(bottom, Size::new(0, 900));
        let titlebar = Settings::default().decoration.titlebar;
        assert_eq!(height(&desktop), 1080 - 900 - titlebar);
        assert!(height(&desktop) < before);
    }

    #[test]
    fn the_windows_of_the_desktop_are_offered_in_a_part_with_nothing_in_it() {
        let mut desktop = desktop();
        let a = open(&mut desktop, "a");
        let b = open(&mut desktop, "b");
        let c = open(&mut desktop, "c");
        assert_eq!(desktop.offers(0), [], "a desktop has no parts");
        desktop.tile(c, Half::Left);

        // The one on top first, side by side, each its own shape in a box as
        // wide as its share, with its title under it.
        let offers = desktop.offers(0);
        let offered = |window, x: i32| Offer {
            window,
            at: Rect::new(Point::new(x + 8, 367), Size::new(428, 321)),
            title: Rect::new(Point::new(x + 8, 692), Size::new(428, 20)),
            whole: Rect::new(Point::new(x, 359), Size::new(444, 361)),
        };
        assert_eq!(offers, [offered(b, 984), offered(a, 1452)]);

        // Chosen, it fills the part, and nothing more is offered.
        desktop.choose(a, Point::new(1500, 500));
        assert_eq!(part(&desktop, a), (960, 0, 960, 1080));
        assert_eq!(desktop.offers(0), []);
        assert_eq!(desktop.offers(1), [], "nor on an output there is not");
    }

    #[test]
    fn a_window_offered_is_shown_no_larger_than_it_is() {
        let mut desktop = desktop();
        // Tall, and with no titlebar of ours to take from its size.
        let tall = desktop.add_window("tall", "tall", Size::new(200, 900), POINTER, false);
        let other = open(&mut desktop, "other");
        desktop.tile(other, Half::Left);
        let only = Offer {
            window: tall,
            at: Rect::new(Point::new(1340, 78), Size::new(200, 900)),
            title: Rect::new(Point::new(992, 982), Size::new(896, 20)),
            whole: Rect::new(Point::new(984, 70), Size::new(912, 940)),
        };
        assert_eq!(desktop.offers(0), [only]);
    }

    #[test]
    fn the_windows_offered_in_a_row_have_their_titles_on_one_line() {
        let mut desktop = desktop();
        desktop.add_window("tall", "tall", Size::new(200, 900), POINTER, false);
        open(&mut desktop, "wide");
        let other = open(&mut desktop, "other");
        desktop.tile(other, Half::Left);
        let offers = desktop.offers(0);
        let [wide, tall] = [offers[0], offers[1]];
        assert_ne!(wide.at.size.height, tall.at.size.height);
        assert_eq!(
            wide.title.origin.y, tall.title.origin.y,
            "titles on one line"
        );
        assert_eq!(
            wide.at.bottom(),
            tall.at.bottom(),
            "and pictures on another"
        );
        assert_eq!(
            wide.whole.size.height, tall.whole.size.height,
            "in boxes alike"
        );
        for offer in [wide, tall] {
            assert!(
                offer.whole.contains(offer.at.origin) && offer.whole.contains(offer.title.origin)
            );
        }
    }

    #[test]
    fn a_window_chosen_goes_to_the_part_it_was_chosen_in() {
        let mut desktop = desktop();
        let waiting = open(&mut desktop, "waiting");
        let a = open(&mut desktop, "a");
        desktop.tile(a, Half::Left);
        desktop.tile(a, Half::Top);
        // Below it and to the right are both offered it.
        let offers = desktop.offers(0);
        assert_eq!(offers.len(), 2);
        assert!(offers[0].at.origin.y > 540 && offers[0].at.origin.x < 960);
        assert!(offers[1].at.origin.x > 960);

        let before = desktop.clone();
        desktop.choose(WindowId::from_raw(99), Point::new(1500, 500));
        assert_eq!(desktop, before, "one that is not there is not chosen");
        desktop.choose(waiting, Point::new(1500, 500));
        assert_eq!(part(&desktop, waiting), (960, 0, 960, 1080));
    }

    #[test]
    fn the_line_between_one_window_and_another_below_it_goes_up_and_down() {
        let mut desktop = desktop();
        let editor = open(&mut desktop, "editor");
        desktop.tile(editor, Half::Bottom);
        let first = line(&desktop, 0).unwrap();
        let wide = |y: i32| Rect::new(Point::new(0, y), Size::new(1920, 1));
        assert_eq!((first.at, first.across), (wide(540), true));

        desktop.move_line(0, 0, Some(Point::new(700, 300)));
        assert_eq!(line(&desktop, 0).unwrap().at, wide(300));
        assert_eq!(frame(&desktop, editor).geometry.origin.y, 300);
        assert_eq!(frame(&desktop, editor).geometry.size.height, 780);
    }

    #[test]
    fn a_side_of_a_shared_workspace_is_halved_again_for_a_third_window() {
        let mut desktop = desktop();
        let editor = open(&mut desktop, "editor");
        desktop.tile(editor, Half::Left);
        let browser = open(&mut desktop, "browser");

        // The one on the right takes the bottom of the right, and the
        // next to open has the top.
        desktop.tile(browser, Half::Bottom);
        assert_eq!(part(&desktop, browser), (960, 540, 960, 540));
        assert_eq!(part(&desktop, editor), (0, 0, 960, 1080), "as it was");
        let terminal = open(&mut desktop, "terminal");
        assert_eq!(part(&desktop, terminal), (960, 0, 960, 540));
        assert_eq!(desktop.workspaces().count(), 4, "all on the one workspace");
        let places: Vec<Place> = desktop.workspaces().collect();
        assert_eq!(places[1].tiles, [editor, terminal, browser]);

        let lines = desktop.lines(0);
        let whole = Rect::new(Point::new(960, 0), Size::new(1, 1080));
        let right = Rect::new(Point::new(960, 540), Size::new(960, 1));
        assert_eq!(
            (lines[0].which, lines[0].at, lines[0].across),
            (0, whole, false)
        );
        assert_eq!(
            (lines[1].which, lines[1].at, lines[1].across),
            (2, right, true)
        );
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn four_windows_have_a_quarter_each() {
        let mut desktop = desktop();
        let [a, b, c, d] = quarters(&mut desktop);
        assert_eq!(part(&desktop, a), (0, 0, 960, 540));
        assert_eq!(part(&desktop, b), (0, 540, 960, 540));
        assert_eq!(part(&desktop, c), (960, 0, 960, 540));
        assert_eq!(part(&desktop, d), (960, 540, 960, 540));
        let which: Vec<usize> = desktop.lines(0).iter().map(|line| line.which).collect();
        assert_eq!(which, [0, 1, 2]);

        // With every part taken, the next window is for the desktop, which
        // is not shown for it until it is asked for.
        let fifth = open(&mut desktop, "fifth");
        assert_eq!(desktop.workspace_of(fifth), Some(0));
        assert_eq!(stack(&desktop), [a, c, b, d]);
        desktop.activate(fifth);
        assert_eq!(desktop.active_workspace(), 0);
        assert_eq!(stack(&desktop), [fifth]);
    }

    #[test]
    fn nothing_is_cut_more_than_twice_or_twice_the_same_way() {
        let mut desktop = desktop();
        let [a, b, c, d] = quarters(&mut desktop);
        let before = desktop.clone();
        desktop.tile(a, Half::Left);
        desktop.tile(a, Half::Top);
        desktop.tile(d, Half::Bottom);
        assert_eq!(desktop, before, "each has that half of where it is");

        // A quarter is not halved. Asked for the other half, it goes
        // there: down its side, or with its side across.
        desktop.tile(a, Half::Bottom);
        assert_eq!(part(&desktop, a), (0, 540, 960, 540));
        assert_eq!(part(&desktop, b), (0, 0, 960, 540));
        desktop.tile(a, Half::Right);
        assert_eq!(part(&desktop, a), (960, 540, 960, 540));
        assert_eq!(part(&desktop, b), (960, 0, 960, 540));
        assert_eq!(part(&desktop, c), (0, 0, 960, 540));
        assert_eq!(part(&desktop, d), (0, 540, 960, 540));
        assert_eq!(desktop.lines(0).len(), 3, "still four parts");

        // Nor is a half halved the way the workspace already is.
        let mut desktop = self::desktop();
        let left = open(&mut desktop, "left");
        desktop.tile(left, Half::Left);
        let right = open(&mut desktop, "right");
        let before = desktop.clone();
        desktop.tile(left, Half::Left);
        desktop.tile(right, Half::Right);
        assert_eq!(desktop, before);

        // Asked for the half the other has, the two change places.
        desktop.tile(left, Half::Right);
        assert_eq!(part(&desktop, left), (960, 0, 960, 1080));
        assert_eq!(part(&desktop, right), (0, 0, 960, 1080));
        desktop.tile(right, Half::Right);
        assert_eq!(part(&desktop, right), (960, 0, 960, 1080));
    }

    #[test]
    fn a_fullscreen_window_takes_a_half_of_the_workspace_it_has() {
        let mut desktop = desktop();
        let video = open(&mut desktop, "video");
        desktop.set_fullscreen(video, true);
        let dialog = open_of(&mut desktop, video);

        desktop.tile(video, Half::Right);
        assert_eq!(desktop.workspaces().count(), 4, "the same workspace");
        assert_eq!(part(&desktop, video), (960, 0, 960, 1080));
        assert_eq!(frame(&desktop, video).home, Home::Shared);

        // What only opened over it is not of it: tiled, it has a
        // workspace of its own, from the same desktop.
        desktop.tile(dialog, Half::Top);
        assert_eq!(desktop.workspaces().count(), 5);
        assert_eq!(part(&desktop, dialog), (0, 0, 1920, 540));
        desktop.set_fullscreen(dialog, false);
        assert_eq!(desktop.active_workspace(), 0);
        assert_eq!(stack(&desktop), [dialog]);
    }

    #[test]
    fn two_parts_with_nothing_in_them_are_filled_in_the_order_they_are_read() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        desktop.tile(first, Half::Left);
        desktop.tile(first, Half::Top);

        let second = open(&mut desktop, "second");
        assert_eq!(
            part(&desktop, second),
            (0, 540, 960, 540),
            "under the first"
        );
        let third = open(&mut desktop, "third");
        assert_eq!(
            part(&desktop, third),
            (960, 0, 960, 1080),
            "then beside them"
        );
    }

    #[test]
    fn a_window_that_leaves_gives_its_room_to_the_one_beside_it() {
        let mut desktop = desktop();
        let [a, b, c, d] = quarters(&mut desktop);

        // Four become three: the one under it has the whole of the right.
        desktop.remove_window(c);
        assert_eq!(part(&desktop, d), (960, 0, 960, 1080));
        assert_eq!(part(&desktop, a), (0, 0, 960, 540));

        // Three become two, one above the other no longer: side by side.
        desktop.remove_window(b);
        assert_eq!(part(&desktop, a), (0, 0, 960, 1080));
        assert_eq!(part(&desktop, d), (960, 0, 960, 1080));

        // And with a whole side gone, what is left of the other has the
        // screen between them, as they were: one above the other.
        let mut desktop = self::desktop();
        let [a, b, c, d] = quarters(&mut desktop);
        desktop.move_line(0, 1, Some(Point::new(0, 300)));
        desktop.remove_window(c);
        desktop.remove_window(d);
        assert_eq!(part(&desktop, a), (0, 0, 1920, 300));
        assert_eq!(part(&desktop, b), (0, 300, 1920, 780));
        let only = desktop.lines(0);
        assert_eq!((only.len(), only[0].which, only[0].across), (1, 0, true));
    }

    #[test]
    fn each_line_is_moved_by_itself_and_only_if_it_is_there() {
        let mut desktop = desktop();
        let [a, b, c, d] = quarters(&mut desktop);

        desktop.move_line(0, 2, Some(Point::new(1500, 200)));
        assert_eq!(part(&desktop, c), (960, 0, 960, 200));
        assert_eq!(part(&desktop, d), (960, 200, 960, 880));
        assert_eq!(part(&desktop, a), (0, 0, 960, 540), "the left is as it was");

        desktop.move_line(0, 0, Some(Point::new(600, 0)));
        assert_eq!(part(&desktop, b), (0, 540, 600, 540));
        assert_eq!(part(&desktop, c), (600, 0, 1320, 200));
        desktop.move_line(0, 2, None);
        assert_eq!(part(&desktop, c), (600, 0, 1320, 540));

        // One that is not there is not moved, and nothing else is.
        desktop.remove_window(b);
        let before = desktop.clone();
        desktop.move_line(0, 1, Some(Point::new(0, 100)));
        desktop.move_line(0, 9, Some(Point::new(0, 100)));
        assert_eq!(desktop, before);

        // Where the workspace is cut across, its sides are cut down.
        let mut desktop = self::desktop();
        let above = open(&mut desktop, "above");
        desktop.tile(above, Half::Top);
        let below = open(&mut desktop, "below");
        desktop.tile(below, Half::Right);
        desktop.move_line(0, 2, Some(Point::new(500, 900)));
        assert_eq!(part(&desktop, below), (500, 540, 1420, 540));
        desktop.move_line(0, 0, Some(Point::new(500, 900)));
        assert_eq!(part(&desktop, above), (0, 0, 1920, 900));
    }

    #[test]
    fn a_window_that_is_open_already_joins_those_sharing_a_workspace() {
        let mut desktop = desktop();
        let editor = open(&mut desktop, "editor");
        let browser = open(&mut desktop, "browser");
        let chat = open(&mut desktop, "chat");
        desktop.set_fullscreen(editor, true);
        let made = desktop.workspaces().nth(1).unwrap();
        assert!(made.room, "one window can give up half");
        desktop.switch_workspace(0);

        // Beside a fullscreen window, which keeps the left.
        desktop.join(browser, made.id);
        assert_eq!(desktop.active_workspace(), 1, "and is followed there");
        assert_eq!(part(&desktop, editor), (0, 0, 960, 1080));
        assert_eq!(part(&desktop, browser), (960, 0, 960, 1080));
        assert_eq!(desktop.focused(), Some(browser));
        assert!(
            !desktop.workspaces().nth(1).unwrap().room,
            "both halves taken"
        );

        // Into a part with nothing in it.
        desktop.tile(editor, Half::Top);
        assert!(desktop.workspaces().nth(1).unwrap().room);
        desktop.join(chat, made.id);
        assert_eq!(part(&desktop, chat), (0, 540, 960, 540));
        assert_eq!(desktop.workspaces().count(), 4);

        // Not where every part is taken, where it is already, on a
        // desktop, or anywhere that is not.
        let before = desktop.clone();
        let late = open(&mut desktop, "late");
        desktop.join(late, made.id);
        desktop.join(chat, made.id);
        let first = desktop.workspaces().next().unwrap().id;
        desktop.join(late, first);
        desktop.join(late, WorkspaceId::from_raw(999));
        desktop.join(WindowId(999), made.id);
        assert_eq!(desktop.workspace_of(late), Some(0));
        desktop.activate(chat);
        assert_eq!(part(&desktop, chat), part(&before, chat));
        assert_eq!(part(&desktop, browser), part(&before, browser));
    }

    #[test]
    fn a_window_leaves_the_workspace_it_shared_to_join_another() {
        let mut desktop = desktop();
        let editor = open(&mut desktop, "editor");
        let video = open(&mut desktop, "video");
        desktop.set_fullscreen(video, true);
        desktop.switch_workspace(0);
        desktop.tile(editor, Half::Left);
        let other = open(&mut desktop, "other");
        let videos = desktop.workspaces().find(|place| place.tiles == [video]);

        // The one it was beside has the whole of what they shared.
        desktop.join(editor, videos.unwrap().id);
        assert_eq!(part(&desktop, video), (0, 0, 960, 1080));
        assert_eq!(part(&desktop, editor), (960, 0, 960, 1080));
        desktop.activate(other);
        assert_eq!(part(&desktop, other), (0, 0, 1920, 1080));
    }

    #[test]
    fn a_window_is_put_in_any_part_of_any_arrangement() {
        let wide = Rect::new(Point::new(0, 0), Size::new(1920, 1080));
        let quarter = |x: i32, y: i32| Rect::new(Point::new(x, y), Size::new(960, 540));
        let parts = Arrangement::OneBesideTwo.parts(wide);
        let left = Rect::new(Point::new(0, 0), Size::new(960, 1080));
        assert_eq!(parts, [left, quarter(960, 0), quarter(960, 540)]);
        let counts: Vec<usize> = Arrangement::ALL
            .iter()
            .map(|arrangement| arrangement.parts(wide).len())
            .collect();
        assert_eq!(counts, [2, 2, 3, 3, 3, 3, 4]);

        // Every part of every arrangement is where its picture has it.
        for arrangement in Arrangement::ALL {
            for (index, expected) in arrangement.parts(wide).into_iter().enumerate() {
                let mut desktop = desktop();
                let id = open(&mut desktop, "app");
                desktop.arrange(id, arrangement, index);
                assert_eq!(frame(&desktop, id).geometry, expected, "{arrangement:?}");
                assert_eq!(frame(&desktop, id).home, Home::Shared);
                assert_eq!(desktop.active_workspace(), 1);
            }
        }

        // The other parts are filled as windows open, in order; a part
        // past the last is the last; and a window that is fullscreen
        // leaves that to be arranged.
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        desktop.set_fullscreen(first, true);
        desktop.arrange(first, Arrangement::Quarters, 99);
        desktop.arrange(WindowId(999), Arrangement::Quarters, 0);
        assert_eq!(desktop.workspaces().count(), 4, "one made for it, not two");
        assert_eq!(frame(&desktop, first).geometry, quarter(960, 540));
        let second = open(&mut desktop, "second");
        let third = open(&mut desktop, "third");
        assert_eq!(frame(&desktop, second).geometry, quarter(0, 0));
        assert_eq!(frame(&desktop, third).geometry, quarter(0, 540));
    }

    #[test]
    fn a_window_changes_places_with_the_part_it_is_dropped_on() {
        let mut desktop = desktop();
        let [a, b, c, d] = quarters(&mut desktop);
        // The top left, dropped on the bottom right.
        let there = Point::new(1500, 900);
        let place = Rect::new(Point::new(960, 540), Size::new(960, 540));
        assert_eq!(desktop.part_under(a, there), Some(place));
        desktop.swap_at(a, there);
        assert_eq!(part(&desktop, a), (960, 540, 960, 540));
        assert_eq!(part(&desktop, d), (0, 0, 960, 540));
        assert_eq!(part(&desktop, b), (0, 540, 960, 540), "the others stay");
        assert_eq!(part(&desktop, c), (960, 0, 960, 540));

        // On its own part, or off the screen, it stays where it is; and a
        // window that fills no part has none to change with.
        let before = desktop.clone();
        desktop.swap_at(a, there);
        desktop.swap_at(a, Point::new(5000, 5000));
        desktop.swap_at(WindowId(999), there);
        assert_eq!(desktop, before);
        assert_eq!(desktop.part_under(a, there), None);

        // A part with nothing in it is changed with as well.
        let mut desktop = self::desktop();
        let only = open(&mut desktop, "only");
        desktop.tile(only, Half::Left);
        desktop.swap_at(only, Point::new(1500, 500));
        assert_eq!(part(&desktop, only), (960, 0, 960, 1080));
    }

    #[test]
    fn a_window_changes_places_with_what_is_beside_it() {
        let mut desktop = desktop();
        let editor = open(&mut desktop, "editor");
        desktop.tile(editor, Half::Left);

        // With nothing, which is then on the other side.
        desktop.swap_toward(editor, Half::Right);
        assert_eq!(part(&desktop, editor), (960, 0, 960, 1080));
        let browser = open(&mut desktop, "browser");
        assert_eq!(part(&desktop, browser), (0, 0, 960, 1080));

        // With another window.
        desktop.swap_toward(editor, Half::Left);
        assert_eq!(part(&desktop, editor), (0, 0, 960, 1080));
        assert_eq!(part(&desktop, browser), (960, 0, 960, 1080));

        // And with nothing at all where there is no part that side, or
        // the window does not fill a part.
        let before = desktop.clone();
        desktop.swap_toward(editor, Half::Left);
        desktop.swap_toward(editor, Half::Top);
        desktop.swap_toward(WindowId(999), Half::Left);
        let dialog = open_of(&mut desktop, editor);
        desktop.swap_toward(dialog, Half::Right);
        desktop.remove_window(dialog);
        desktop.focus_toward(Half::Left);
        assert_eq!(part(&desktop, editor), part(&before, editor));
        let loose = open(&mut desktop, "loose");
        desktop.activate(loose);
        desktop.swap_toward(loose, Half::Left);
        assert_eq!(stack(&desktop), [loose]);
    }

    #[test]
    fn focus_goes_to_the_window_beside_the_one_that_has_it() {
        let mut desktop = desktop();
        let [a, b, c, d] = quarters(&mut desktop);
        assert_eq!(desktop.focused(), Some(d));

        desktop.focus_toward(Half::Left);
        assert_eq!(desktop.focused(), Some(b));
        desktop.focus_toward(Half::Top);
        assert_eq!(desktop.focused(), Some(a));
        desktop.focus_toward(Half::Right);
        assert_eq!(desktop.focused(), Some(c));
        desktop.focus_toward(Half::Bottom);
        assert_eq!(desktop.focused(), Some(d));
        desktop.focus_toward(Half::Right);
        desktop.focus_toward(Half::Bottom);
        assert_eq!(desktop.focused(), Some(d), "nothing that way");

        // Of two beside it, the one it shares more of its edge with.
        desktop.move_line(0, 1, Some(Point::new(0, 300)));
        desktop.focus_toward(Half::Left);
        assert_eq!(desktop.focused(), Some(b), "below the line at 300");
        desktop.move_line(0, 1, Some(Point::new(0, 900)));
        desktop.focus_toward(Half::Right);
        desktop.focus_toward(Half::Left);
        assert_eq!(desktop.focused(), Some(a), "above the line at 900");
        desktop.focus_toward(Half::Right);
        assert_eq!(desktop.focused(), Some(c), "the first of two alike");

        // A part with nothing in it has nothing to give focus to, and on
        // a desktop there is no beside.
        let mut desktop = self::desktop();
        let alone = open(&mut desktop, "alone");
        desktop.focus_toward(Half::Right);
        desktop.tile(alone, Half::Left);
        desktop.focus_toward(Half::Right);
        assert_eq!(desktop.focused(), Some(alone));
    }

    #[test]
    fn windows_share_a_workspace_with_no_output_to_measure_it_by() {
        let mut desktop = Desktop::new(Settings::default(), 1);
        let first = open(&mut desktop, "first");
        desktop.tile(first, Half::Left);
        let second = open(&mut desktop, "second");
        assert_eq!(desktop.focused(), Some(second));
        desktop.focus_toward(Half::Left);
        assert_eq!(desktop.focused(), Some(first));
        desktop.swap_toward(first, Half::Right);
        desktop.focus_toward(Half::Left);
        assert_eq!(desktop.focused(), Some(second));
        assert_eq!(desktop.lines(0), []);
    }

    #[test]
    fn a_window_that_leaves_a_half_gives_it_to_the_other() {
        let mut desktop = desktop();
        let editor = open(&mut desktop, "editor");
        desktop.tile(editor, Half::Left);
        let browser = open(&mut desktop, "browser");

        desktop.remove_window(editor);
        assert_eq!(stack(&desktop), [browser]);
        assert_eq!(frame(&desktop, browser).geometry, SCREEN);
        assert_eq!(frame(&desktop, browser).home, Home::Solo, "it is alone now");

        // The last to leave ends the workspace, by whatever way it goes.
        desktop.set_fullscreen(browser, false);
        assert_eq!(desktop.workspaces().count(), 3);
        assert_eq!(desktop.active_workspace(), 0);
        assert_eq!(stack(&desktop), [browser]);
    }

    #[test]
    fn a_window_that_leaves_a_half_goes_back_to_the_desktop_and_is_followed() {
        let mut desktop = desktop();
        let editor = open(&mut desktop, "editor");
        desktop.tile(editor, Half::Left);
        let browser = open(&mut desktop, "browser");
        assert_eq!(desktop.focused(), Some(browser));

        desktop.toggle_fullscreen(browser);
        assert_eq!(
            desktop.active_workspace(),
            0,
            "the desktop it would have had"
        );
        assert_eq!(stack(&desktop), [browser], "and not over the other");
        assert_eq!(frame(&desktop, browser).home, Home::Desktop);
        assert_eq!(desktop.focused(), Some(browser));

        // The other has the whole of the workspace they shared.
        desktop.next_workspace();
        assert_eq!(stack(&desktop), [editor]);
        assert_eq!(frame(&desktop, editor).geometry, SCREEN);

        // One that leaves where nobody is looking takes nobody with it.
        desktop.tile(browser, Half::Top);
        let third = open(&mut desktop, "third");
        desktop.switch_workspace(1);
        desktop.set_fullscreen(third, false);
        assert_eq!(desktop.active_workspace(), 3, "the second desktop still");
        assert_eq!(desktop.workspace_of(third), Some(0));
    }

    #[test]
    fn a_half_with_nothing_beside_it_ends_with_its_window() {
        let mut desktop = desktop();
        let editor = open(&mut desktop, "editor");
        desktop.tile(editor, Half::Bottom);

        desktop.toggle_fullscreen(editor);
        assert_eq!(desktop.workspaces().count(), 3);
        assert_eq!(stack(&desktop), [editor]);
        assert!(!frame(&desktop, editor).fullscreen);
    }

    #[test]
    fn a_tile_is_put_away_alone_and_the_desktop_shown_when_all_are() {
        let mut desktop = desktop();
        let editor = open(&mut desktop, "editor");
        desktop.tile(editor, Half::Left);
        let browser = open(&mut desktop, "browser");

        desktop.minimize(editor);
        assert_eq!(stack(&desktop), [browser], "its half shows nothing");
        assert_eq!(desktop.active_workspace(), 1);
        assert_eq!(frame(&desktop, browser).geometry.size, Size::new(960, 1080));

        desktop.minimize(browser);
        assert_eq!(desktop.active_workspace(), 0, "nothing was left to see");
        desktop.activate(editor);
        assert_eq!(stack(&desktop), [editor]);
    }

    #[test]
    fn windows_that_share_a_workspace_keep_their_titlebars() {
        let mut desktop = decorated_desktop();
        let editor = open(&mut desktop, "editor");
        desktop.tile(editor, Half::Top);
        let browser = open(&mut desktop, "browser");
        let bare = desktop.add_window("bare", "bare", SIZE, POINTER, false);
        assert_eq!(
            desktop.workspace_of(bare),
            Some(0),
            "both halves were taken"
        );
        desktop.activate(editor);

        // Each has its titlebar across the top of its own part, with no
        // borders, and its window the room under it.
        let wide = Size::new(1920, METRICS.titlebar);
        let under = Size::new(1920, 540 - METRICS.titlebar);
        let seen = |desktop: &Desktop, id| {
            let frame = frame(desktop, id);
            (
                frame.decoration.map(|decoration| decoration.titlebar()),
                frame.client(),
            )
        };
        let top = Rect::new(Point::new(0, 0), wide);
        let room = Rect::new(Point::new(0, METRICS.titlebar.cast_signed()), under);
        assert_eq!(seen(&desktop, editor), (Some(top), room));
        let lower = Rect::new(Point::new(0, 540), wide);
        let room = Rect::new(Point::new(0, 540 + METRICS.titlebar.cast_signed()), under);
        assert_eq!(seen(&desktop, browser), (Some(lower), room));

        // Asking for titlebars to be brought back changes nothing here:
        // they have not gone.
        let before: Vec<Rect> = desktop.frames().map(|frame| frame.client()).collect();
        desktop.reveal(0, Some(30));
        let after: Vec<Rect> = desktop.frames().map(|frame| frame.client()).collect();
        assert_eq!(after, before);
        desktop.reveal(0, None);

        // A window that draws its own titlebar is given none, and has the
        // whole of its part.
        desktop.remove_window(browser);
        desktop.join(bare, desktop_made(&desktop));
        assert_eq!(frame(&desktop, bare).decoration, None);
        assert_eq!(frame(&desktop, bare).client().size, Size::new(960, 1080));

        // Alone again, a window has the screen to itself, titlebar and
        // all.
        desktop.remove_window(bare);
        assert_eq!(seen(&desktop, editor), (None, SCREEN));
    }

    #[test]
    fn leaving_fullscreen_takes_a_frame_back_where_it_was() {
        let mut desktop = desktop();
        let video = open(&mut desktop, "video");
        let chat = open(&mut desktop, "chat");
        let was = frame(&desktop, video).geometry;

        desktop.toggle_fullscreen(video);
        desktop.toggle_fullscreen(video);

        assert_eq!(desktop.workspaces().count(), 3, "its workspace has gone");
        assert_eq!(desktop.active_workspace(), 0);
        assert_eq!(stack(&desktop), [chat, video]);
        assert_eq!(frame(&desktop, video).geometry, was);
        assert_eq!(frame(&desktop, video).home, Home::Desktop);
        assert_eq!(desktop.focused(), Some(video));
    }

    #[test]
    fn closing_a_fullscreen_window_shows_the_desktop_it_came_from() {
        let mut desktop = desktop();
        desktop.switch_workspace(1);
        let chat = open(&mut desktop, "chat");
        let video = open(&mut desktop, "video");
        desktop.set_fullscreen(video, true);
        assert_eq!(desktop.active_workspace(), 2);

        desktop.remove_window(video);
        assert_eq!(desktop.active_workspace(), 1);
        assert_eq!(desktop.workspaces().count(), 3);
        assert_eq!(desktop.focused(), Some(chat));
    }

    #[test]
    fn a_window_made_fullscreen_where_nobody_is_looking_is_not_shown() {
        let mut desktop = desktop();
        desktop.switch_workspace(2);
        let late = open(&mut desktop, "late");
        desktop.switch_workspace(1);
        let middle = open(&mut desktop, "middle");

        // One on a later workspace than the one shown.
        desktop.set_fullscreen(late, true);
        assert_eq!(desktop.active_workspace(), 1);
        assert_eq!(stack(&desktop), [middle]);
        assert_eq!(desktop.workspace_of(late), Some(3));

        // And one on an earlier: what is shown is still what was.
        desktop.switch_workspace(0);
        let early = open(&mut desktop, "early");
        desktop.switch_workspace(1);
        desktop.set_fullscreen(early, true);
        assert_eq!(stack(&desktop), [middle]);
        assert_eq!(desktop.workspace_of(early), Some(1));
        assert_eq!(desktop.workspace_of(middle), Some(2));

        // Closed where nobody is looking, it takes nothing with it.
        desktop.remove_window(late);
        assert_eq!(stack(&desktop), [middle]);
        assert_eq!(desktop.workspaces().count(), 4);
    }

    #[test]
    fn numbers_count_desktops_and_next_goes_along_the_whole_row() {
        let mut desktop = desktop();
        let video = open(&mut desktop, "video");
        desktop.set_fullscreen(video, true);

        desktop.switch_workspace(1);
        assert_eq!(desktop.active_workspace(), 2, "the second desktop");
        desktop.previous_workspace();
        assert_eq!(stack(&desktop), [video], "the one made for it is between");
        desktop.previous_workspace();
        assert_eq!(desktop.active_workspace(), 0);
        desktop.next_workspace();
        desktop.next_workspace();
        desktop.next_workspace();
        desktop.next_workspace();
        assert_eq!(desktop.active_workspace(), 0, "round the four of them");

        // Sent to a desktop by its number, it leaves fullscreen to go.
        desktop.move_to_workspace(video, 2);
        assert_eq!(desktop.workspaces().count(), 3);
        assert_eq!(desktop.workspace_of(video), Some(2));
        desktop.switch_workspace(2);
        assert!(!frame(&desktop, video).fullscreen);
    }

    #[test]
    fn what_belongs_to_a_fullscreen_window_opens_over_it_and_the_rest_on_the_desktop() {
        let mut desktop = desktop();
        let video = open(&mut desktop, "video");
        desktop.set_fullscreen(video, true);

        let dialog = open_of(&mut desktop, video);
        assert_eq!(stack(&desktop), [video, dialog], "over it");
        assert_eq!(frame(&desktop, dialog).home, Home::Solo);
        assert!(!frame(&desktop, dialog).fullscreen);
        assert_eq!(desktop.focused(), Some(dialog));
        desktop.clicked(video);
        assert_eq!(stack(&desktop), [video, dialog], "and stays over it");

        // A window that is nobody's is not for this workspace, though it
        // be another of the same program's.
        let second = open(&mut desktop, "video");
        assert_eq!(desktop.workspace_of(second), Some(0));
        assert_eq!(stack(&desktop), [video, dialog], "nor is it shown");
        desktop.activate(second);
        assert_eq!(desktop.active_workspace(), 0);
        assert_eq!(stack(&desktop), [second]);
        assert_eq!(desktop.workspace_of(video), Some(1));

        // Nor is one that belongs to a window on a desktop, which opens
        // where things do.
        let child = open_of(&mut desktop, second);
        assert_eq!(stack(&desktop), [second, child]);

        // What belongs to the fullscreen one opens over it even while
        // nobody is looking, and nothing is shown that was not.
        let unseen = open_of(&mut desktop, video);
        assert_eq!(stack(&desktop), [second, child]);
        assert_eq!(desktop.workspace_of(unseen), Some(1));
        desktop.remove_window(unseen);
        desktop.remove_window(child);
        desktop.remove_window(second);

        // One that a rule sends elsewhere goes there, and nothing is shown
        // that was not.
        desktop.switch_workspace(0);
        desktop.next_workspace();
        desktop.set_rules(vec![rule(
            "mail",
            Effects {
                workspace: Some(2),
                ..Effects::default()
            },
        )]);
        let mail = open(&mut desktop, "mail");
        assert_eq!(stack(&desktop), [video, dialog]);
        assert_eq!(desktop.workspace_of(mail), Some(3));
    }

    #[test]
    fn what_opened_over_a_fullscreen_window_goes_back_with_it() {
        let mut desktop = desktop();
        let video = open(&mut desktop, "video");
        desktop.set_fullscreen(video, true);
        let dialog = open_of(&mut desktop, video);

        desktop.set_fullscreen(video, false);
        assert_eq!(stack(&desktop), [video, dialog]);
        assert_eq!(desktop.workspaces().count(), 3);

        // And is left on the desktop if the fullscreen window closes.
        desktop.set_fullscreen(video, true);
        let again = open_of(&mut desktop, video);
        desktop.remove_window(video);
        assert_eq!(desktop.active_workspace(), 0);
        assert_eq!(stack(&desktop), [dialog, again]);
        assert_eq!(desktop.focused(), Some(again));
    }

    #[test]
    fn a_window_made_fullscreen_over_another_goes_back_where_that_one_would() {
        let mut desktop = desktop();
        let video = open(&mut desktop, "video");
        desktop.set_fullscreen(video, true);
        let second = open_of(&mut desktop, video);

        desktop.set_fullscreen(second, true);
        assert_eq!(stack(&desktop), [second]);
        assert_eq!(desktop.active_workspace(), 2);

        desktop.set_fullscreen(second, false);
        assert_eq!(
            desktop.active_workspace(),
            0,
            "the desktop, not the other's"
        );
        assert_eq!(stack(&desktop), [second]);
        assert_eq!(desktop.workspace_of(video), Some(1));
    }

    #[test]
    fn a_frame_on_every_workspace_is_not_on_one_made_for_a_fullscreen_window() {
        let mut desktop = desktop();
        let note = open(&mut desktop, "note");
        desktop.toggle_sticky(note);
        let video = open(&mut desktop, "video");

        desktop.set_fullscreen(video, true);
        assert_eq!(stack(&desktop), [video]);

        // It is on the next desktop to be shown, all the same.
        desktop.switch_workspace(1);
        assert_eq!(stack(&desktop), [note]);
        assert_eq!(desktop.focused(), Some(note));
    }

    #[test]
    fn minimizing_a_fullscreen_window_shows_the_desktop_until_it_is_brought_back() {
        let mut desktop = desktop();
        let chat = open(&mut desktop, "chat");
        let video = open(&mut desktop, "video");
        desktop.set_fullscreen(video, true);
        let dialog = open_of(&mut desktop, video);

        // Putting away what opened over it changes nothing else.
        desktop.minimize(dialog);
        assert_eq!(stack(&desktop), [video]);

        desktop.minimize(video);
        assert_eq!(desktop.active_workspace(), 0);
        assert_eq!(stack(&desktop), [chat]);
        assert_eq!(desktop.workspaces().count(), 4, "its workspace waits");

        desktop.activate(video);
        assert_eq!(stack(&desktop), [video]);
        assert!(frame(&desktop, video).fullscreen);
    }

    #[test]
    fn a_fullscreen_frame_leaves_fullscreen_to_go_to_another_output() {
        let mut desktop = two_outputs();
        let video = open(&mut desktop, "video");
        desktop.set_fullscreen(video, true);

        desktop.move_to_next_output(video);
        assert_eq!(shown(&desktop), [0, 0]);
        assert_eq!(desktop.workspaces().count(), 6);
        assert!(!frame(&desktop, video).fullscreen);
        assert_eq!(desktop.current_output(), 1);
    }

    #[test]
    fn a_frame_sent_to_an_output_showing_a_fullscreen_window_goes_to_its_desktop() {
        let mut desktop = two_outputs();
        let video = open_on_second(&mut desktop, "video");
        desktop.set_fullscreen(video, true);
        let chat = open(&mut desktop, "chat");

        desktop.move_to_next_output(chat);
        assert_eq!(shown(&desktop), [0, 0], "which is shown for it");
        assert_eq!(desktop.focused(), Some(chat));
        assert_eq!(desktop.workspace_of(video), Some(1));
    }

    #[test]
    fn an_output_that_goes_takes_its_fullscreen_windows_to_the_first() {
        let mut desktop = two_outputs();
        let video = open_on_second(&mut desktop, "video");
        desktop.set_fullscreen(video, true);
        // And one that came from a desktop the first output has not got.
        desktop.turn_to(SECOND_SCREEN.origin);
        desktop.new_workspace();
        let film = open_on_second(&mut desktop, "film");
        desktop.set_fullscreen(film, true);

        desktop.remove_output(1);
        assert_eq!(desktop.workspaces().count(), 6);
        let tiles: Vec<Vec<WindowId>> = desktop.workspaces().map(|place| place.tiles).collect();
        assert_eq!(
            tiles,
            [vec![], vec![], vec![], vec![], vec![video], vec![film]]
        );

        // Each goes back to the desktop its own became.
        desktop.set_fullscreen(video, false);
        assert_eq!(desktop.workspace_of(video), Some(0));
        desktop.set_fullscreen(film, false);
        assert_eq!(desktop.workspace_of(film), Some(3));
    }

    #[test]
    fn desktops_are_kept_by_their_number_whatever_is_between_them() {
        let mut desktop = desktop();
        let video = open(&mut desktop, "video");
        desktop.set_fullscreen(video, true);

        desktop.set_workspace_count(2);
        let kept: Vec<bool> = desktop.workspaces().map(|place| place.kept).collect();
        assert_eq!(kept, [true, false, true]);
        desktop.set_workspace_count(4);
        let kept: Vec<bool> = desktop.workspaces().map(|place| place.kept).collect();
        assert_eq!(kept, [true, false, true, true, true]);
    }

    // --- Frame state -------------------------------------------------------

    #[test]
    fn a_maximized_frame_fills_its_output_and_restores_afterwards() {
        let mut desktop = desktop();
        let id = open(&mut desktop, "app");
        let normal = frame(&desktop, id).geometry;

        desktop.set_maximized(id, true);
        assert!(frame(&desktop, id).maximized);
        assert_eq!(frame(&desktop, id).geometry, SCREEN);

        desktop.set_maximized(id, false);
        assert_eq!(frame(&desktop, id).geometry, normal);

        desktop.toggle_maximized(id);
        assert!(frame(&desktop, id).maximized);
        desktop.toggle_maximized(id);
        assert!(!frame(&desktop, id).maximized);
    }

    #[test]
    fn a_frame_maximizes_onto_the_output_it_mostly_covers() {
        let mut desktop = desktop();
        desktop.set_outputs(vec![SCREEN, SECOND_SCREEN]);
        let id = open(&mut desktop, "app");

        desktop.set_geometry(id, Rect::new(Point::new(1800, 100), SIZE));
        desktop.set_maximized(id, true);
        assert_eq!(frame(&desktop, id).geometry, SECOND_SCREEN);

        desktop.set_geometry(id, Rect::new(Point::new(1000, 100), SIZE));
        desktop.set_maximized(id, true);
        assert_eq!(frame(&desktop, id).geometry, SCREEN);
    }

    #[test]
    fn a_frame_touching_no_output_maximizes_onto_the_first() {
        let mut desktop = desktop();
        desktop.set_outputs(vec![SCREEN, SECOND_SCREEN]);
        let id = open(&mut desktop, "app");
        desktop.set_geometry(id, Rect::new(Point::new(-5000, -5000), SIZE));
        desktop.set_maximized(id, true);
        assert_eq!(frame(&desktop, id).geometry, SCREEN);
    }

    #[test]
    fn without_outputs_a_maximized_frame_keeps_its_own_geometry() {
        let mut desktop = Desktop::new(Settings::default(), 1);
        let id = open(&mut desktop, "app");
        desktop.set_maximized(id, true);
        assert_eq!(
            frame(&desktop, id).geometry,
            Rect::new(Point::new(0, 0), SIZE)
        );
    }

    #[test]
    fn fullscreen_covers_the_output_and_returns_to_the_previous_state() {
        let mut desktop = desktop();
        let id = open(&mut desktop, "app");
        desktop.set_maximized(id, true);

        desktop.set_fullscreen(id, true);
        assert!(frame(&desktop, id).fullscreen);
        assert_eq!(frame(&desktop, id).geometry, SCREEN);

        desktop.set_fullscreen(id, false);
        assert!(frame(&desktop, id).maximized, "it was maximized before");

        desktop.set_maximized(id, false);
        desktop.toggle_fullscreen(id);
        assert_eq!(frame(&desktop, id).geometry, SCREEN);
        desktop.toggle_fullscreen(id);
        assert_eq!(frame(&desktop, id).geometry.size, SIZE);
    }

    #[test]
    fn moving_or_resizing_a_frame_leaves_maximized_and_fullscreen() {
        let mut desktop = desktop();
        let id = open(&mut desktop, "app");
        desktop.set_maximized(id, true);
        desktop.set_fullscreen(id, true);

        let geometry = Rect::new(Point::new(50, 60), Size::new(300, 200));
        desktop.set_geometry(id, geometry);

        let frame = frame(&desktop, id);
        assert_eq!(
            (frame.geometry, frame.maximized, frame.fullscreen),
            (geometry, false, false)
        );
    }

    #[test]
    fn minimizing_hides_a_frame_and_passes_focus_on() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");

        desktop.minimize(second);
        assert_eq!(stack(&desktop), [first]);
        assert_eq!(desktop.focused(), Some(first));
        assert!(desktop.window(second).is_some(), "the window still exists");

        desktop.minimize(first);
        assert_eq!(desktop.focused(), None);
    }

    #[test]
    fn minimizing_an_unfocused_frame_leaves_focus_alone() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");
        desktop.minimize(first);
        assert_eq!(desktop.focused(), Some(second));
    }

    #[test]
    fn activating_a_minimized_window_restores_focuses_and_raises_it() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");
        desktop.minimize(first);

        desktop.activate(first);
        assert_eq!(stack(&desktop), [second, first]);
        assert_eq!(desktop.focused(), Some(first));
    }

    // --- Focus -------------------------------------------------------------

    #[test]
    fn clicking_a_window_focuses_and_raises_it() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");
        desktop.clicked(first);
        assert_eq!(desktop.focused(), Some(first));
        assert_eq!(stack(&desktop), [second, first]);
    }

    #[test]
    fn clicking_focuses_without_raising_when_raise_on_focus_is_off() {
        let mut desktop = desktop();
        desktop.set_settings(Settings {
            raise_on_focus: false,
            ..Settings::default()
        });
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");
        desktop.clicked(first);
        assert_eq!(desktop.focused(), Some(first));
        assert_eq!(stack(&desktop), [first, second]);
    }

    #[test]
    fn the_pointer_does_not_move_focus_under_click_to_focus() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");
        desktop.pointer_entered(first);
        assert_eq!(desktop.focused(), Some(second));
    }

    #[test]
    fn the_pointer_moves_focus_when_focus_follows_it() {
        let mut desktop = desktop();
        let sloppy = Settings {
            focus_follows_pointer: true,
            ..Settings::default()
        };
        desktop.set_settings(sloppy);
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");

        desktop.pointer_entered(first);
        assert_eq!(desktop.focused(), Some(first));
        assert_eq!(stack(&desktop), [second, first]);

        desktop.set_settings(Settings {
            raise_on_focus: false,
            ..sloppy
        });
        desktop.pointer_entered(second);
        assert_eq!(desktop.focused(), Some(second));
        assert_eq!(
            stack(&desktop),
            [second, first],
            "focused without being raised"
        );
    }

    #[test]
    fn focus_cycles_through_frames_in_the_order_they_were_created() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");
        let third = open(&mut desktop, "third");

        let mut visited = Vec::new();
        for _ in 0..4 {
            desktop.focus_next();
            visited.push(desktop.focused().unwrap());
        }
        assert_eq!(
            visited,
            [first, second, third, first],
            "raising does not disturb the order"
        );
        assert_eq!(
            stack(&desktop).last(),
            Some(&first),
            "the focused frame is raised"
        );

        visited.clear();
        for _ in 0..3 {
            desktop.focus_previous();
            visited.push(desktop.focused().unwrap());
        }
        assert_eq!(visited, [third, second, first]);
    }

    #[test]
    fn focus_cycling_skips_minimized_frames() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");
        let third = open(&mut desktop, "third");
        desktop.minimize(second);
        desktop.clicked(first);

        desktop.focus_next();
        assert_eq!(desktop.focused(), Some(third));
        desktop.focus_next();
        assert_eq!(desktop.focused(), Some(first));
    }

    #[test]
    fn focus_cycling_with_one_or_no_frames_changes_nothing() {
        let mut desktop = desktop();
        desktop.focus_next();
        desktop.focus_previous();
        assert_eq!(desktop.focused(), None);

        let only = open(&mut desktop, "only");
        desktop.focus_next();
        assert_eq!(desktop.focused(), Some(only));
    }

    #[test]
    fn activating_a_window_switches_to_its_workspace() {
        let mut desktop = desktop();
        let id = open(&mut desktop, "app");
        desktop.switch_workspace(2);
        desktop.activate(id);
        assert_eq!(desktop.active_workspace(), 0);
        assert_eq!(desktop.focused(), Some(id));
    }

    // --- Tabs --------------------------------------------------------------

    #[test]
    fn attaching_a_window_makes_it_the_shown_tab_of_the_target_frame() {
        let mut desktop = desktop();
        let editor = open(&mut desktop, "editor");
        let terminal = open(&mut desktop, "terminal");
        let other = open(&mut desktop, "other");
        let editor_geometry = frame(&desktop, editor).geometry;

        desktop.attach(terminal, editor);

        assert_eq!(desktop.frames().count(), 2);
        let frame = frame(&desktop, editor);
        let tabs: Vec<WindowId> = frame.tabs.iter().map(|tab| tab.id).collect();
        assert_eq!(tabs, [editor, terminal]);
        assert_eq!(frame.window().id, terminal);
        assert_eq!(
            frame.geometry, editor_geometry,
            "the window takes the frame's position"
        );
        assert_eq!(desktop.focused(), Some(terminal));
        assert_eq!(stack(&desktop), [other, terminal], "the frame is raised");
    }

    #[test]
    fn attaching_a_window_to_its_own_frame_does_nothing() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");
        desktop.attach(second, first);
        let before = desktop.clone();

        desktop.attach(first, second);
        desktop.attach(first, first);
        assert_eq!(desktop, before);
    }

    #[test]
    fn attaching_across_workspaces_moves_the_window_and_refocuses_the_source() {
        let mut desktop = desktop();
        let target = open(&mut desktop, "target");
        desktop.switch_workspace(1);
        let stays = open(&mut desktop, "stays");
        let moves = open(&mut desktop, "moves");

        desktop.attach(moves, target);

        assert_eq!(desktop.focused(), Some(stays));
        assert_eq!(desktop.workspace_of(moves), Some(0));
        desktop.switch_workspace(0);
        assert_eq!(desktop.focused(), Some(moves));
    }

    #[test]
    fn tabs_cycle_in_both_directions_and_focus_follows_the_shown_tab() {
        let mut desktop = desktop();
        let a = open(&mut desktop, "a");
        let b = open(&mut desktop, "b");
        let c = open(&mut desktop, "c");
        desktop.attach(b, a);
        desktop.attach(c, a);
        assert_eq!(desktop.focused(), Some(c));

        desktop.next_tab(c);
        assert_eq!(desktop.focused(), Some(a), "wrapped to the first tab");
        desktop.next_tab(a);
        assert_eq!(desktop.focused(), Some(b));
        desktop.previous_tab(b);
        assert_eq!(desktop.focused(), Some(a));
        desktop.previous_tab(a);
        assert_eq!(desktop.focused(), Some(c), "wrapped to the last tab");
    }

    #[test]
    fn clicking_or_activating_a_tab_shows_it() {
        let mut desktop = desktop();
        let a = open(&mut desktop, "a");
        let b = open(&mut desktop, "b");
        desktop.attach(b, a);

        desktop.clicked(a);
        assert_eq!(desktop.focused(), Some(a));
        desktop.activate(b);
        assert_eq!(desktop.focused(), Some(b));
    }

    #[test]
    fn closing_a_tab_keeps_the_frame_and_picks_a_sensible_tab_to_show() {
        let mut desktop = desktop();
        let a = open(&mut desktop, "a");
        let b = open(&mut desktop, "b");
        let c = open(&mut desktop, "c");
        let d = open(&mut desktop, "d");
        desktop.attach(b, a);
        desktop.attach(c, a);
        desktop.attach(d, a);
        desktop.activate(c);

        desktop.remove_window(a);
        assert_eq!(
            desktop.focused(),
            Some(c),
            "closing an earlier tab keeps the shown one"
        );

        desktop.remove_window(c);
        assert_eq!(
            desktop.focused(),
            Some(d),
            "closing the shown tab shows its right neighbour"
        );

        desktop.remove_window(d);
        assert_eq!(
            desktop.focused(),
            Some(b),
            "closing the last tab shows the new last one"
        );
        assert_eq!(desktop.frames().count(), 1);
    }

    #[test]
    fn detaching_a_tab_gives_it_a_focused_frame_just_off_the_old_one() {
        let mut desktop = desktop();
        let a = open(&mut desktop, "a");
        let b = open(&mut desktop, "b");
        desktop.attach(b, a);
        desktop.set_maximized(a, true);
        let origin = Point::new(0, 0);

        desktop.detach(a);

        assert_eq!(stack(&desktop), [b, a]);
        assert_eq!(desktop.focused(), Some(a));
        let detached = frame(&desktop, a);
        assert_eq!(detached.geometry, Rect::new(origin.offset(32, 32), SIZE));
        assert!(!detached.maximized, "the new frame starts out normal");
        assert_eq!(frame(&desktop, b).tabs.len(), 1);
    }

    #[test]
    fn detaching_a_window_that_is_alone_in_its_frame_does_nothing() {
        let mut desktop = desktop();
        let only = open(&mut desktop, "only");
        let geometry = frame(&desktop, only).geometry;
        desktop.detach(only);
        assert_eq!(desktop.frames().count(), 1);
        assert_eq!(frame(&desktop, only).geometry, geometry);
    }

    // --- Workspaces --------------------------------------------------------

    #[test]
    fn each_workspace_has_its_own_frames_and_focus() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        desktop.switch_workspace(1);
        assert_eq!(desktop.active_workspace(), 1);
        assert_eq!(desktop.focused(), None);

        let second = open(&mut desktop, "second");
        assert_eq!(stack(&desktop), [second]);

        desktop.switch_workspace(0);
        assert_eq!(stack(&desktop), [first]);
        assert_eq!(desktop.focused(), Some(first));
    }

    #[test]
    fn switching_further_than_one_past_the_last_workspace_is_ignored() {
        let mut desktop = desktop();
        desktop.switch_workspace(4);
        assert_eq!(desktop.active_workspace(), 0);
        assert_eq!(desktop.workspace_count(), 3);
    }

    #[test]
    fn switching_one_past_the_last_workspace_makes_it() {
        let mut desktop = desktop();
        desktop.switch_workspace(3);
        assert_eq!(desktop.active_workspace(), 3);
        assert_eq!(desktop.workspace_count(), 4);

        // Left empty, it goes.
        desktop.switch_workspace(0);
        assert_eq!(desktop.workspace_count(), 3);
    }

    #[test]
    fn a_new_workspace_is_shown_and_goes_when_left_empty() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        desktop.new_workspace();
        assert_eq!(desktop.active_workspace(), 3);
        assert_eq!(desktop.focused(), None);

        // Asked for again from an empty one, there is still only the one.
        desktop.new_workspace();
        assert_eq!(desktop.workspace_count(), 4);
        assert_eq!(desktop.active_workspace(), 3);

        desktop.previous_workspace();
        assert_eq!(desktop.workspace_count(), 3);
        desktop.switch_workspace(0);
        assert_eq!(desktop.focused(), Some(first));
    }

    #[test]
    fn a_new_workspace_with_a_window_on_it_stays_when_left() {
        let mut desktop = desktop();
        desktop.new_workspace();
        let id = open(&mut desktop, "app");
        desktop.switch_workspace(0);
        assert_eq!(desktop.workspace_count(), 4);
        assert_eq!(desktop.workspace_of(id), Some(3));

        desktop.remove_window(id);
        assert_eq!(desktop.workspace_count(), 3);
    }

    #[test]
    fn a_workspace_keeps_its_identity_as_others_come_and_go() {
        let mut desktop = desktop();
        desktop.new_workspace();
        let first = open(&mut desktop, "first");
        desktop.new_workspace();
        let second = open(&mut desktop, "second");
        let places: Vec<Place> = desktop.workspaces().collect();
        assert_eq!(places.len(), 5);
        assert!(
            places[..3]
                .iter()
                .all(|place| place.kept && place.output == 0)
        );
        assert!(!places[3].kept && !places[4].kept);
        assert!(places[4].shown && !places[3].shown);
        assert_eq!(places[4].index, 4);

        desktop.remove_window(first);
        let now: Vec<Place> = desktop.workspaces().collect();
        assert_eq!(now.len(), 4);
        assert_eq!(now[3].id, places[4].id, "the same one, a place earlier");
        assert_ne!(now[3].id.get(), places[3].id.get());
        assert_eq!(desktop.workspace_of(second), Some(3));
    }

    #[test]
    fn there_are_never_more_workspaces_than_the_most() {
        let mut desktop = desktop();
        desktop.set_workspace_count(99);
        assert_eq!(desktop.workspace_count(), Desktop::MOST_WORKSPACES);

        // Asking for one more goes to the last there is.
        desktop.new_workspace();
        assert_eq!(desktop.workspace_count(), Desktop::MOST_WORKSPACES);
        assert_eq!(desktop.active_workspace(), Desktop::MOST_WORKSPACES - 1);
        assert_eq!(
            Desktop::new(Settings::default(), 99).workspace_count(),
            Desktop::MOST_WORKSPACES
        );
    }

    #[test]
    fn an_output_that_goes_takes_its_made_workspaces_to_the_first() {
        let mut desktop = two_outputs();
        let id = open_on_second(&mut desktop, "app");
        desktop.move_to_workspace(id, 3);
        assert_eq!(desktop.workspace_of(id), Some(3));

        desktop.remove_output(1);
        assert_eq!(desktop.workspace_count(), 4);
        assert_eq!(desktop.workspace_of(id), Some(3));
        let last = desktop.workspaces().last().unwrap();
        assert!(!last.kept);
    }

    #[test]
    fn next_and_previous_workspace_wrap_around() {
        let mut desktop = desktop();
        desktop.previous_workspace();
        assert_eq!(desktop.active_workspace(), 2);
        desktop.next_workspace();
        assert_eq!(desktop.active_workspace(), 0);
        desktop.next_workspace();
        assert_eq!(desktop.active_workspace(), 1);
    }

    #[test]
    fn moving_a_frame_to_another_workspace_takes_every_tab_and_stays_put() {
        let mut desktop = desktop();
        let stays = open(&mut desktop, "stays");
        let a = open(&mut desktop, "a");
        let b = open(&mut desktop, "b");
        desktop.attach(b, a);

        desktop.move_to_workspace(a, 2);

        assert_eq!(desktop.active_workspace(), 0);
        assert_eq!(stack(&desktop), [stays]);
        assert_eq!(desktop.focused(), Some(stays));
        assert_eq!(
            (desktop.workspace_of(a), desktop.workspace_of(b)),
            (Some(2), Some(2))
        );

        desktop.switch_workspace(2);
        assert_eq!(desktop.focused(), Some(b));
    }

    #[test]
    fn a_minimized_frame_moved_to_another_workspace_stays_hidden_and_unfocused() {
        let mut desktop = desktop();
        let hidden = open(&mut desktop, "hidden");
        desktop.switch_workspace(1);
        let shown = open(&mut desktop, "shown");
        desktop.switch_workspace(0);
        desktop.minimize(hidden);

        desktop.move_to_workspace(hidden, 1);

        desktop.switch_workspace(1);
        assert_eq!(stack(&desktop), [shown]);
        assert_eq!(desktop.focused(), Some(shown));
        desktop.activate(hidden);
        assert_eq!(desktop.focused(), Some(hidden));
    }

    #[test]
    fn moving_a_frame_to_its_own_workspace_or_too_far_does_nothing() {
        let mut desktop = desktop();
        let id = open(&mut desktop, "app");
        let before = desktop.clone();
        desktop.move_to_workspace(id, 0);
        desktop.move_to_workspace(id, 4);
        assert_eq!(desktop, before);
    }

    #[test]
    fn moving_a_frame_one_past_the_last_workspace_makes_one_for_it() {
        let mut desktop = desktop();
        let id = open(&mut desktop, "app");

        desktop.move_to_workspace(id, 3);
        assert_eq!(desktop.workspace_count(), 4);
        assert_eq!(desktop.workspace_of(id), Some(3));
        assert_eq!(desktop.active_workspace(), 0, "what is shown stays");

        // Moved away again, the workspace that was made has nothing on it
        // and nobody looking at it.
        desktop.move_to_workspace(id, 1);
        assert_eq!(desktop.workspace_count(), 3);
        assert_eq!(desktop.workspace_of(id), Some(1));
    }

    #[test]
    fn a_frame_is_sent_to_a_workspace_made_for_it_however_many_there_are() {
        let mut desktop = desktop();
        let stays = open(&mut desktop, "stays");
        let goes = open(&mut desktop, "goes");

        desktop.move_to_new_workspace(goes);
        assert_eq!(desktop.workspace_count(), 4);
        assert_eq!(desktop.workspace_of(goes), Some(3));
        assert_eq!(desktop.active_workspace(), 0);
        assert_eq!(desktop.focused(), Some(stays));
    }

    #[test]
    fn a_window_alone_on_a_made_workspace_stays_there_when_sent_to_a_new_one() {
        let mut desktop = desktop();
        desktop.new_workspace();
        let id = open(&mut desktop, "app");
        let before = desktop.clone();

        // The one made for it would take the place of the one it left.
        desktop.move_to_workspace(id, 4);
        assert_eq!(desktop.workspace_count(), before.workspace_count() + 1);
        assert_eq!(desktop.workspace_of(id), Some(4));
        assert_eq!(desktop.active_workspace(), 3, "still shown, and empty");
    }

    #[test]
    fn a_workspace_no_longer_kept_stays_while_it_is_shown_or_has_frames() {
        let mut desktop = desktop();
        let kept = open(&mut desktop, "kept");
        desktop.switch_workspace(2);
        let orphan = open(&mut desktop, "orphan");

        desktop.set_workspace_count(2);
        assert_eq!(desktop.workspace_count(), 3, "nothing moves");
        assert_eq!(desktop.active_workspace(), 2);
        assert_eq!(desktop.workspace_of(orphan), Some(2));
        assert_eq!(desktop.focused(), Some(orphan));

        desktop.switch_workspace(0);
        assert_eq!(desktop.workspace_count(), 3, "it has a frame on it");
        assert_eq!(desktop.focused(), Some(kept));

        desktop.remove_window(orphan);
        assert_eq!(desktop.workspace_count(), 2, "now it has gone");
    }

    #[test]
    fn workspaces_can_be_added_and_never_drop_below_one() {
        let mut desktop = desktop();
        let id = open(&mut desktop, "app");

        desktop.set_workspace_count(5);
        assert_eq!(desktop.workspace_count(), 5);
        assert_eq!(desktop.workspace_of(id), Some(0));

        desktop.set_workspace_count(0);
        assert_eq!(desktop.workspace_count(), 1);
        assert_eq!(desktop.focused(), Some(id));
    }

    /// Where each frame on the screen is across, by the window it shows,
    /// and whether it has the keyboard.
    fn across(desktop: &Desktop) -> Vec<(WindowId, i32, bool)> {
        let frames = desktop.frames();
        frames
            .map(|frame| (frame.window().id, frame.geometry.origin.x, frame.focused))
            .collect()
    }

    #[test]
    fn slid_left_the_workspace_shown_goes_with_the_next_beside_it_on_the_right() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        desktop.next_workspace();
        let second = open(&mut desktop, "second");
        desktop.previous_workspace();
        let at = frame(&desktop, first).geometry.origin.x;
        let there = |desktop: &mut Desktop| {
            desktop.next_workspace();
            let x = frame(desktop, second).geometry.origin.x;
            desktop.previous_workspace();
            x
        };
        let there = there(&mut desktop);

        desktop.slide(-300);
        assert_eq!(desktop.slid(), -300);
        let width = SCREEN.size.width.cast_signed();
        assert_eq!(
            across(&desktop),
            [
                (first, at - 300, true),
                (second, there - 300 + width, false)
            ],
            "only the one shown has the keyboard"
        );
        assert_eq!(
            desktop.active_workspace(),
            0,
            "and it is still the one shown"
        );

        desktop.slide(0);
        assert_eq!(across(&desktop), [(first, at, true)], "and back");
    }

    #[test]
    fn slid_right_the_one_before_comes_in_on_the_left_round_from_the_last() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        desktop.previous_workspace();
        assert_eq!(desktop.active_workspace(), 2, "round to the last");
        let last = open(&mut desktop, "last");
        desktop.next_workspace();
        let at = frame(&desktop, first).geometry.origin.x;

        desktop.slide(200);
        let width = SCREEN.size.width.cast_signed();
        let shown = across(&desktop);
        assert_eq!(shown[0], (first, at + 200, true));
        assert_eq!(shown[1].0, last);
        assert!(shown[1].1 < 200 && shown[1].1 >= 200 - width, "{shown:?}");
    }

    #[test]
    fn with_one_workspace_nothing_comes_in_beside_it() {
        let mut desktop = Desktop::new(Settings::default(), 1);
        desktop.set_outputs(vec![SCREEN]);
        let only = open(&mut desktop, "only");
        let at = frame(&desktop, only).geometry.origin.x;

        desktop.slide(-100);
        assert_eq!(across(&desktop), [(only, at - 100, true)]);
    }

    #[test]
    fn a_frame_on_every_workspace_stays_where_it_is_while_they_slide() {
        let mut desktop = desktop();
        let note = open(&mut desktop, "note");
        desktop.toggle_sticky(note);
        let at = frame(&desktop, note).geometry.origin.x;

        desktop.slide(-400);
        assert_eq!(across(&desktop), [(note, at, true)]);
    }

    // --- Outputs -----------------------------------------------------------

    /// A window opened with the pointer on the second output.
    fn open_on_second(desktop: &mut Desktop, app_id: &str) -> WindowId {
        let pointer = SECOND_SCREEN.origin.offset(10, 10);
        desktop.add_window(app_id, format!("{app_id} window"), SIZE, pointer, true)
    }

    /// A desktop with two outputs side by side.
    fn two_outputs() -> Desktop {
        let mut desktop = desktop();
        desktop.set_outputs(vec![SCREEN, SECOND_SCREEN]);
        desktop
    }

    fn shown(desktop: &Desktop) -> Vec<usize> {
        desktop.shown_workspaces().collect()
    }

    fn visible(desktop: &Desktop, id: WindowId) -> bool {
        desktop.frames().any(|frame| frame.window().id == id)
    }

    #[test]
    fn each_output_shows_a_workspace_of_its_own() {
        let mut desktop = two_outputs();
        let left = open(&mut desktop, "left");
        let right = open_on_second(&mut desktop, "right");
        assert_eq!(shown(&desktop), [0, 0]);
        assert_eq!(desktop.current_output(), 1, "where the last window opened");

        desktop.switch_workspace(1);
        assert_eq!(shown(&desktop), [0, 1], "only the second output changed");
        assert!(visible(&desktop, left));
        assert!(!visible(&desktop, right));
        assert_eq!(desktop.active_workspace(), 1);

        // Turning to the first output, switching applies there.
        desktop.turn_to(POINTER);
        assert_eq!(desktop.current_output(), 0);
        assert_eq!(desktop.active_workspace(), 0);
        desktop.next_workspace();
        desktop.next_workspace();
        desktop.previous_workspace();
        assert_eq!(shown(&desktop), [1, 1]);
        assert!(!visible(&desktop, left));
        assert!(!visible(&desktop, right), "each on the first of its own");

        // A point on no output turns nowhere.
        desktop.turn_to(Point::new(-50, -50));
        assert_eq!(desktop.current_output(), 0);
    }

    #[test]
    fn a_window_is_listed_with_its_output_and_its_workspace_there() {
        let mut desktop = two_outputs();
        let left = open(&mut desktop, "left");
        let right = open_on_second(&mut desktop, "right");
        desktop.move_to_workspace(right, 2);

        let listed: Vec<(WindowId, usize, usize)> = desktop
            .windows()
            .map(|listed| (listed.window.id, listed.output, listed.workspace))
            .collect();
        assert_eq!(listed, [(left, 0, 0), (right, 1, 2)]);
        assert_eq!(desktop.workspace_of(right), Some(2));
        assert!(
            visible(&desktop, left),
            "the first output still shows its own"
        );
    }

    #[test]
    fn only_the_current_output_has_a_focused_frame() {
        let mut desktop = two_outputs();
        let left = open(&mut desktop, "left");
        let right = open_on_second(&mut desktop, "right");
        assert_eq!(desktop.focused(), Some(right));
        assert_eq!(
            stack(&desktop),
            [left, right],
            "the current output's on top"
        );

        desktop.clicked(left);
        assert_eq!(desktop.current_output(), 0);
        assert_eq!(desktop.focused(), Some(left));
        assert_eq!(stack(&desktop), [right, left]);
        assert_eq!(desktop.frames().filter(|frame| frame.focused).count(), 1);

        desktop.activate(right);
        assert_eq!(desktop.current_output(), 1);
        assert_eq!(desktop.focused(), Some(right));

        // Cycling stays on the output that has the keyboard.
        desktop.focus_next();
        assert_eq!(desktop.focused(), Some(right));
    }

    #[test]
    fn the_pointer_takes_focus_to_another_output_when_focus_follows_it() {
        let mut desktop = two_outputs();
        desktop.set_settings(Settings {
            focus_follows_pointer: true,
            ..Settings::default()
        });
        let left = open(&mut desktop, "left");
        open_on_second(&mut desktop, "right");
        desktop.pointer_entered(left);
        assert_eq!(desktop.current_output(), 0);
        assert_eq!(desktop.focused(), Some(left));
    }

    #[test]
    fn a_frame_dragged_onto_another_output_joins_the_workspace_shown_there() {
        let mut desktop = two_outputs();
        let dragged = open(&mut desktop, "dragged");
        let other = open_on_second(&mut desktop, "other");
        desktop.switch_workspace(1);
        assert_eq!(shown(&desktop), [0, 1]);
        desktop.turn_to(POINTER);

        // Half across is not most of it.
        let straddling = Rect::new(Point::new(1920 - 500, 100), SIZE);
        desktop.set_geometry(dragged, straddling);
        assert_eq!(desktop.windows().next().unwrap().output, 0);
        assert_eq!(desktop.current_output(), 0);

        let across = Rect::new(Point::new(1920 - 100, 100), SIZE);
        desktop.set_geometry(dragged, across);
        let listed = desktop.windows().next().unwrap();
        assert_eq!((listed.output, listed.workspace), (1, 1));
        assert!(visible(&desktop, dragged), "it did not vanish on the way");
        assert!(
            !visible(&desktop, other),
            "which is on the workspace not shown"
        );
        assert_eq!(desktop.current_output(), 1);
        assert_eq!(desktop.focused(), Some(dragged));
        assert_eq!(frame(&desktop, dragged).geometry, across);
    }

    #[test]
    fn a_frame_can_be_sent_to_the_next_output_keeping_its_place() {
        let mut desktop = two_outputs();
        let id = open(&mut desktop, "app");
        desktop.set_geometry(id, Rect::new(Point::new(100, 200), SIZE));

        desktop.move_to_next_output(id);
        assert_eq!(desktop.windows().next().unwrap().output, 1);
        assert_eq!(
            frame(&desktop, id).geometry.origin,
            SECOND_SCREEN.origin.offset(100, 200)
        );
        assert_eq!(desktop.current_output(), 1);

        // And round again to the first.
        desktop.move_to_next_output(id);
        assert_eq!(desktop.windows().next().unwrap().output, 0);
        assert_eq!(frame(&desktop, id).geometry.origin, Point::new(100, 200));

        // A place that the next output does not reach becomes its corner.
        desktop.set_geometry(id, Rect::new(Point::new(1500, 1050), SIZE));
        desktop.move_to_next_output(id);
        assert_eq!(frame(&desktop, id).geometry.origin, SECOND_SCREEN.origin);
    }

    #[test]
    fn with_one_output_or_none_there_is_no_next_one() {
        let mut desktop = desktop();
        let id = open(&mut desktop, "app");
        let before = desktop.clone();
        desktop.move_to_next_output(id);
        assert_eq!(desktop, before);

        desktop.set_outputs(Vec::<Rect>::new());
        let before = desktop.clone();
        desktop.move_to_next_output(id);
        assert_eq!(desktop, before);
    }

    #[test]
    fn frames_of_an_output_that_goes_move_to_the_one_that_comes_first() {
        let third = Rect::new(Point::new(3200, 0), Size::new(800, 600));
        let mut desktop = desktop();
        desktop.set_outputs(vec![SCREEN, SECOND_SCREEN, third]);
        desktop.set_primary(2);
        let on_first = open(&mut desktop, "first");
        assert_eq!(desktop.current_output(), 0);
        // The first goes: its frame, and the keyboard, go to the third,
        // which is counted second from then on.
        desktop.remove_output(0);
        desktop.set_outputs(vec![SECOND_SCREEN, third]);
        let output = |desktop: &Desktop, id: WindowId| {
            let mut windows = desktop.windows();
            windows
                .find(|listed| listed.window.id == id)
                .map(|listed| listed.output)
        };
        assert_eq!(output(&desktop, on_first), Some(1));
        assert_eq!(desktop.current_output(), 1);
        // The one that comes first goes: its frames go to the first left.
        desktop.remove_output(1);
        desktop.set_outputs(vec![SECOND_SCREEN]);
        assert_eq!(output(&desktop, on_first), Some(0));
        assert_eq!(desktop.current_output(), 0);
    }

    #[test]
    fn frames_of_an_output_that_goes_move_to_the_same_workspace_of_the_first() {
        let mut desktop = two_outputs();
        let stays = open(&mut desktop, "stays");
        let first = open_on_second(&mut desktop, "first");
        desktop.switch_workspace(1);
        let second = open_on_second(&mut desktop, "second");
        assert_eq!(desktop.current_output(), 1);

        desktop.remove_output(1);
        desktop.set_outputs(vec![SCREEN]);

        assert_eq!(desktop.current_output(), 0, "the output that is left");
        let listed: Vec<(WindowId, usize, usize)> = desktop
            .windows()
            .map(|listed| (listed.window.id, listed.output, listed.workspace))
            .collect();
        assert_eq!(listed, [(stays, 0, 0), (first, 0, 0), (second, 0, 1)]);
        assert_eq!(shown(&desktop), [0], "the first output shows what it did");
        assert_eq!(frame(&desktop, stays).geometry.origin, SCREEN.origin);
        assert_eq!(
            frame(&desktop, first).geometry.origin,
            SCREEN.origin,
            "brought onto the output it now belongs to"
        );
        desktop.switch_workspace(1);
        assert_eq!(frame(&desktop, second).geometry.origin, SCREEN.origin);

        // An output that is not there cannot be taken away.
        let before = desktop.clone();
        desktop.remove_output(1);
        assert_eq!(desktop, before);
    }

    #[test]
    fn taking_away_the_first_output_makes_the_next_the_first() {
        let mut desktop = two_outputs();
        let left = open(&mut desktop, "left");
        let right = open_on_second(&mut desktop, "right");
        desktop.turn_to(SECOND_SCREEN.origin);
        desktop.switch_workspace(2);

        desktop.remove_output(0);
        desktop.set_outputs(vec![SECOND_SCREEN]);

        assert_eq!(shown(&desktop), [2], "what the second output was showing");
        assert_eq!(desktop.current_output(), 0);
        desktop.switch_workspace(0);
        assert!(visible(&desktop, left));
        assert!(visible(&desktop, right));
        assert!(
            SECOND_SCREEN.contains(frame(&desktop, left).geometry.origin),
            "on the output that is left"
        );
    }

    #[test]
    fn fewer_outputs_than_before_means_the_last_ones_went() {
        let mut desktop = two_outputs();
        let left = open(&mut desktop, "left");
        let right = open_on_second(&mut desktop, "right");

        desktop.set_outputs(vec![SCREEN]);
        assert!(visible(&desktop, left));
        assert_eq!(frame(&desktop, right).geometry.origin, SCREEN.origin);
        assert_eq!(desktop.windows().last().unwrap().output, 0);
    }

    #[test]
    fn a_frame_keeps_its_place_on_an_output_that_moves() {
        let mut desktop = two_outputs();
        let id = open_on_second(&mut desktop, "app");
        desktop.set_geometry(id, Rect::new(SECOND_SCREEN.origin.offset(40, 60), SIZE));

        // The first output becomes narrower, so the second starts further
        // to the left.
        let narrower = Rect::new(Point::new(0, 0), Size::new(1280, 1080));
        let moved = Rect::new(Point::new(1280, 0), SECOND_SCREEN.size);
        desktop.set_outputs(vec![narrower, moved]);

        assert_eq!(
            frame(&desktop, id).geometry.origin,
            moved.origin.offset(40, 60)
        );
        assert_eq!(desktop.windows().next().unwrap().output, 1);
    }

    #[test]
    fn a_new_output_has_the_workspaces_that_are_always_there() {
        let mut desktop = desktop();
        desktop.set_workspace_count(5);
        desktop.set_outputs(vec![SCREEN, SECOND_SCREEN]);
        let id = open_on_second(&mut desktop, "app");
        desktop.move_to_workspace(id, 4);
        assert_eq!(desktop.workspace_of(id), Some(4));

        // Fewer kept means fewer on every output, but not from under a
        // frame or the user: the one being shown stays, after the two.
        desktop.turn_to(SECOND_SCREEN.origin);
        desktop.switch_workspace(4);
        desktop.set_workspace_count(2);
        assert_eq!(desktop.workspace_count(), 3);
        assert_eq!(shown(&desktop), [0, 2]);
        assert_eq!(desktop.workspace_of(id), Some(2));
        desktop.turn_to(SCREEN.origin);
        assert_eq!(desktop.workspace_count(), 2);
    }

    #[test]
    fn a_rule_names_a_workspace_of_the_output_the_window_opens_on() {
        let mut desktop = two_outputs();
        desktop.set_rules(vec![Rule {
            app_id: Some(Pattern::new("chat")),
            title: None,
            effects: Effects {
                workspace: Some(2),
                ..Effects::default()
            },
        }]);
        let id = open_on_second(&mut desktop, "chat");
        let listed = desktop.windows().next().unwrap();
        assert_eq!(
            (listed.window.id, listed.output, listed.workspace),
            (id, 1, 2)
        );
        assert!(!visible(&desktop, id), "that output is showing another");
    }

    #[test]
    fn removing_every_output_leaves_frames_where_they_are() {
        let mut desktop = desktop();
        let id = open(&mut desktop, "app");
        let geometry = Rect::new(Point::new(300, 200), SIZE);
        desktop.set_geometry(id, geometry);
        desktop.set_outputs(Vec::<Rect>::new());
        assert_eq!(frame(&desktop, id).geometry, geometry);
    }

    #[test]
    fn windows_keep_clear_of_the_part_of_an_output_reserved_for_panels() {
        let mut desktop = Desktop::new(Settings::default(), 1);
        // A 30 pixel panel along the top and a 50 pixel dock on the left.
        let usable = Rect::new(Point::new(50, 30), Size::new(1870, 1050));
        desktop.set_outputs([Screen {
            area: SCREEN,
            usable,
        }]);

        let id = open(&mut desktop, "app");
        assert_eq!(
            frame(&desktop, id).geometry.origin,
            usable.origin,
            "placed in the usable part"
        );

        desktop.set_maximized(id, true);
        assert_eq!(
            frame(&desktop, id).geometry,
            usable,
            "maximized up to the panels"
        );

        desktop.set_fullscreen(id, true);
        assert_eq!(
            frame(&desktop, id).geometry,
            SCREEN,
            "fullscreen covers them"
        );
    }

    #[test]
    fn a_stranded_frame_moves_to_the_usable_part_of_the_first_output() {
        let mut desktop = desktop();
        let id = open(&mut desktop, "app");
        desktop.set_geometry(id, Rect::new(Point::new(5000, 5000), SIZE));
        let usable = Rect::new(Point::new(0, 40), Size::new(1920, 1040));

        desktop.set_outputs([Screen {
            area: SCREEN,
            usable,
        }]);

        assert_eq!(frame(&desktop, id).geometry.origin, usable.origin);
    }

    // --- Robustness --------------------------------------------------------

    #[test]
    fn events_for_a_window_that_has_closed_are_ignored() {
        let mut desktop = desktop();
        let open_window = open(&mut desktop, "open");
        let closed = open(&mut desktop, "closed");
        desktop.remove_window(closed);
        let before = desktop.clone();

        desktop.remove_window(closed);
        desktop.set_title(closed, "ghost");
        desktop.set_decorated(closed, false);
        desktop.set_client_size(closed, SIZE);
        desktop.set_geometry(closed, SCREEN);
        desktop.set_maximized(closed, true);
        desktop.toggle_maximized(closed);
        desktop.set_fullscreen(closed, true);
        desktop.toggle_fullscreen(closed);
        desktop.minimize(closed);
        desktop.activate(closed);
        desktop.clicked(closed);
        desktop.pointer_entered(closed);
        desktop.next_tab(closed);
        desktop.previous_tab(closed);
        desktop.attach(closed, open_window);
        desktop.attach(open_window, closed);
        desktop.detach(closed);
        desktop.move_to_workspace(closed, 1);
        desktop.move_to_new_workspace(closed);

        assert_eq!(desktop, before);
        assert_eq!(desktop.window(closed), None);
        assert_eq!(desktop.workspace_of(closed), None);
    }

    /// Checks what must hold after any sequence of operations.
    fn assert_consistent(desktop: &Desktop) {
        assert_eq!(desktop.sets.len(), desktop.outputs.len().max(1));
        assert!(
            desktop.current < desktop.sets.len(),
            "the current set exists"
        );
        for set in &desktop.sets {
            let count = set.workspaces.len();
            assert!(set.active < count, "the workspace shown exists");
            let desktops = set.desktops();
            assert!(!desktops.is_empty(), "there is a desktop");
            assert!(
                desktops.len() <= Desktop::MOST_WORKSPACES,
                "no more than the most"
            );
            for (count, index) in desktops.iter().enumerate() {
                let kept = set.workspaces[*index].kept;
                assert_eq!(kept, count < desktop.kept, "the first are kept");
            }
            let solos = set.workspaces.iter().filter_map(|workspace| workspace.solo);
            let origins: Vec<WorkspaceId> = solos.map(|solo| solo.origin).collect();
            for origin in &origins {
                let known = |index: &&usize| set.workspaces[**index].id == *origin;
                assert!(
                    desktops.iter().any(|index| known(&index)),
                    "a desktop to go back to"
                );
            }
            for (index, workspace) in set.workspaces.iter().enumerate() {
                let needed = !workspace.groups.is_empty() || index == set.active;
                let needed = needed || origins.contains(&workspace.id);
                assert!(workspace.kept || needed, "none is left for nothing");
                let fullscreen = workspace.groups.iter().filter(|group| group.fullscreen);
                let tiles = workspace.tiles().len();
                assert_eq!(fullscreen.count(), tiles, "what fills a part is a tile");
                if let Some(tiling) = workspace.solo {
                    assert!(!workspace.kept, "one made for windows is not kept");
                    assert!(tiles > 0, "and a window of them is there");
                    assert_eq!(tiling.tiles().count(), tiles, "none has gone unnoticed");
                }
            }
        }
        let mut places: Vec<WorkspaceId> = desktop.workspaces().map(|place| place.id).collect();
        let every = places.len();
        places.sort_unstable();
        places.dedup();
        assert_eq!(places.len(), every, "no two workspaces are one");
        let workspaces = desktop.sets.iter().flat_map(|set| &set.workspaces);
        for workspace in workspaces.clone() {
            let focusable: Vec<u64> = workspace
                .groups
                .iter()
                .filter(|group| !group.minimized)
                .map(|group| group.serial)
                .collect();
            match workspace.focused {
                Some(serial) => assert!(focusable.contains(&serial), "focus is on a shown frame"),
                None => assert_eq!(focusable, [], "a shown frame exists but nothing is focused"),
            }
            for group in &workspace.groups {
                assert!(group.active < group.tabs.len(), "the shown tab exists");
            }
        }
        let mut ids: Vec<WindowId> = workspaces
            .flat_map(|workspace| &workspace.groups)
            .flat_map(|group| &group.tabs)
            .map(|window| window.id)
            .collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total, "every window is in exactly one frame");
    }

    #[test]
    fn the_desktop_stays_consistent_under_arbitrary_sequences_of_operations() {
        // A small linear congruential generator keeps the run reproducible
        // without a dependency.
        let mut state: u64 = 0x5eed;
        let mut random = |bound: usize| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            usize::try_from(state >> 33).unwrap() % bound
        };
        let mut desktop = desktop();
        desktop.set_outputs(vec![SCREEN, SECOND_SCREEN]);
        let mut ids = vec![open(&mut desktop, "seed")];

        for step in 0..5000 {
            let id = ids[random(ids.len())];
            let other = ids[random(ids.len())];
            let screens = [SCREEN, SECOND_SCREEN];
            match random(40) {
                0 | 1 => ids.push(open(&mut desktop, "app")),
                27 => desktop.toggle_shaded(id),
                28 => desktop.toggle_above(id),
                29 => desktop.toggle_sticky(id),
                2 => desktop.remove_window(id),
                3 => desktop.minimize(id),
                4 => desktop.activate(id),
                5 => desktop.clicked(id),
                6 => desktop.attach(id, other),
                7 => desktop.detach(id),
                8 => desktop.next_tab(id),
                9 => desktop.previous_tab(id),
                10 => desktop.move_to_workspace(id, random(6)),
                11 => desktop.switch_workspace(random(6)),
                30 => desktop.new_workspace(),
                32 => desktop.tile(id, HALVES[random(4)]),
                33 => desktop.swap_toward(id, HALVES[random(4)]),
                34 => desktop.focus_toward(HALVES[random(4)]),
                35 => desktop.move_line(random(2), random(3), Some(Point::new(700, 400))),
                39 => {
                    let least = Size::new(
                        u32::try_from(random(2400)).unwrap(),
                        u32::try_from(random(1400)).unwrap(),
                    );
                    desktop.set_least(id, least);
                }
                37 => desktop.arrange(id, Arrangement::ALL[random(7)], random(4)),
                38 => desktop.swap_at(id, Point::new(1000, 600)),
                36 => {
                    let places: Vec<Place> = desktop.workspaces().collect();
                    desktop.join(id, places[random(places.len())].id);
                }
                31 => desktop.move_to_new_workspace(id),
                12 => desktop.focus_next(),
                13 => desktop.focus_previous(),
                14 => desktop.set_workspace_count(1 + random(4)),
                15 => desktop.toggle_maximized(id),
                16 => desktop.toggle_fullscreen(id),
                17 => desktop.next_workspace(),
                19 => desktop.set_decorated(id, random(2) == 0),
                20 => desktop.set_client_size(id, Size::new(100, 100)),
                18 => desktop.previous_workspace(),
                21 => desktop.set_outputs(vec![SCREEN; random(3)]),
                22 => desktop.set_outputs(screens[..random(3)].to_vec()),
                23 => desktop.remove_output(random(3)),
                24 => desktop.move_to_next_output(id),
                25 => desktop.turn_to(Point::new(
                    i32::try_from(random(3200)).unwrap(),
                    i32::try_from(random(1000)).unwrap(),
                )),
                _ => {
                    let origin = Point::new(
                        i32::try_from(random(3200)).unwrap() - 200,
                        i32::try_from(random(1000)).unwrap(),
                    );
                    desktop.set_geometry(id, Rect::new(origin, Size::new(400, 300)));
                }
            }
            assert_consistent(&desktop);
            assert!(
                desktop.frames().filter(|frame| frame.focused).count() <= 1,
                "step {step}"
            );
        }
    }

    // --- Standing apart ------------------------------------------------------

    #[test]
    fn a_shaded_frame_is_rolled_up_into_its_titlebar() {
        let mut desktop = decorated_desktop();
        let id = open(&mut desktop, "app");
        let whole = frame(&desktop, id).geometry;
        assert!(!frame(&desktop, id).standing.shaded);

        desktop.toggle_shaded(id);
        let rolled = frame(&desktop, id);
        assert!(rolled.standing.shaded);
        assert_eq!(rolled.geometry.origin, whole.origin);
        assert_eq!(
            rolled.geometry.size,
            Size::new(whole.size.width, METRICS.titlebar)
        );
        assert_eq!(rolled.client().size.height, 0, "none of the window shows");

        // Maximized, it is as wide as the screen and still only a titlebar.
        desktop.toggle_maximized(id);
        let size = frame(&desktop, id).geometry.size;
        assert_eq!(size, Size::new(SCREEN.size.width, METRICS.titlebar));
        desktop.toggle_maximized(id);

        desktop.toggle_shaded(id);
        assert_eq!(frame(&desktop, id).geometry, whole, "let down as it was");
    }

    #[test]
    fn a_frame_without_a_titlebar_cannot_be_rolled_up_into_one() {
        let mut desktop = decorated_desktop();
        let bare = desktop.add_window("bare", "bare", SIZE, POINTER, false);
        let whole = frame(&desktop, bare).geometry;
        desktop.toggle_shaded(bare);
        assert_eq!(frame(&desktop, bare).geometry, whole);
        assert!(!frame(&desktop, bare).standing.shaded);

        // Nor one that is fullscreen, which has none while it is.
        let id = open(&mut desktop, "app");
        desktop.toggle_shaded(id);
        desktop.toggle_fullscreen(id);
        assert_eq!(frame(&desktop, id).geometry, SCREEN);
        assert!(!frame(&desktop, id).standing.shaded);
        desktop.toggle_fullscreen(id);
        assert!(
            frame(&desktop, id).standing.shaded,
            "it is again afterwards"
        );
    }

    #[test]
    fn a_frame_kept_above_is_over_the_others_whichever_has_focus() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");
        let third = open(&mut desktop, "third");
        assert_eq!(stack(&desktop), [first, second, third]);

        desktop.toggle_above(first);
        assert!(frame(&desktop, first).standing.above);
        assert_eq!(stack(&desktop), [second, third, first]);
        desktop.clicked(second);
        assert_eq!(stack(&desktop), [third, second, first], "raised under it");
        assert_eq!(desktop.focused(), Some(second));

        desktop.toggle_above(first);
        assert_eq!(
            stack(&desktop),
            [first, third, second],
            "where it was raised to"
        );
    }

    #[test]
    fn a_sticky_frame_is_on_whichever_workspace_its_output_shows() {
        let mut desktop = desktop();
        let staying = open(&mut desktop, "staying");
        let sticky = open(&mut desktop, "sticky");
        desktop.toggle_sticky(sticky);
        assert!(frame(&desktop, sticky).standing.sticky);

        desktop.switch_workspace(1);
        assert_eq!(stack(&desktop), [sticky]);
        assert_eq!(desktop.focused(), Some(sticky), "it had focus and keeps it");
        desktop.next_workspace();
        assert_eq!(stack(&desktop), [sticky]);
        desktop.previous_workspace();
        desktop.previous_workspace();
        assert_eq!(stack(&desktop), [staying, sticky], "over what was there");

        // One that did not have focus does not take it.
        desktop.clicked(staying);
        desktop.switch_workspace(1);
        let other = open(&mut desktop, "other");
        desktop.switch_workspace(0);
        assert_eq!(desktop.focused(), Some(staying));
        desktop.switch_workspace(1);
        assert_eq!(desktop.focused(), Some(other));
        assert_eq!(stack(&desktop), [other, sticky]);

        // Showing the workspace it is on already changes nothing.
        desktop.switch_workspace(1);
        assert_eq!(stack(&desktop), [other, sticky]);

        desktop.toggle_sticky(sticky);
        desktop.switch_workspace(2);
        assert_eq!(stack(&desktop), [], "it stays where it was left");
    }

    #[test]
    fn a_sticky_frame_comes_along_when_another_window_is_brought_up() {
        let mut desktop = desktop();
        let elsewhere = open(&mut desktop, "elsewhere");
        desktop.switch_workspace(1);
        let sticky = open(&mut desktop, "sticky");
        desktop.toggle_sticky(sticky);

        desktop.activate(elsewhere);
        assert_eq!(desktop.active_workspace(), 0);
        assert_eq!(stack(&desktop), [sticky, elsewhere]);
        assert_eq!(desktop.focused(), Some(elsewhere));
        // Brought up itself, it is where it already was.
        desktop.activate(sticky);
        assert_eq!(desktop.active_workspace(), 0);
        assert_eq!(desktop.focused(), Some(sticky));
    }

    #[test]
    fn a_frame_sent_to_one_workspace_is_no_longer_on_all_of_them() {
        let mut desktop = desktop();
        let sticky = open(&mut desktop, "sticky");
        desktop.toggle_sticky(sticky);
        desktop.move_to_workspace(sticky, 2);
        assert_eq!(stack(&desktop), []);
        desktop.switch_workspace(1);
        assert_eq!(stack(&desktop), []);
        desktop.switch_workspace(2);
        assert_eq!(stack(&desktop), [sticky]);
        assert!(!frame(&desktop, sticky).standing.sticky);
    }

    #[test]
    fn standing_apart_is_not_asked_of_a_window_that_is_not_there() {
        let mut desktop = desktop();
        let id = open(&mut desktop, "app");
        let gone = open(&mut desktop, "gone");
        desktop.remove_window(gone);
        desktop.toggle_shaded(gone);
        desktop.toggle_above(gone);
        desktop.toggle_sticky(gone);
        assert_eq!(frame(&desktop, id).standing, Standing::default());
    }
}
