//! How each part of the desktop looks, in each state it can be in.
//!
//! Whatever draws something asks the theme for the [`Style`] of a [`Part`]
//! in its [`States`], and draws from that alone. It does not know where the
//! answer comes from: a stylesheet, in which rules written for parts and
//! states give them these properties.

use crate::{Color, Transition};

/// A part of the desktop that a theme gives a look to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Part {
    /// What is behind everything, where there is no wallpaper. Only its
    /// background is used.
    Desktop,
    /// A window's frame: the border around it and how far its corners are
    /// rounded. It can be [`State::Focused`].
    Window,
    /// The line between windows that share a workspace, drawn a hair wide
    /// in its text colour.
    TileLine,
    /// A window offered in a part of a workspace with nothing in it: its
    /// border is the box round its picture and its title, and its text
    /// colour and font are the title's. It is [`State::Hovered`] while the
    /// pointer is over the box.
    Offer,
    /// The bar across the top of a frame. It can be [`State::Focused`],
    /// with its window.
    Titlebar,
    /// One window's title in a titlebar. It is [`State::Active`] when it
    /// is the window being shown, and [`State::Focused`] with its frame.
    /// Its border is the line between it and the tab before it.
    Tab,
    /// The button at the left of a titlebar that closes the window. Like
    /// the two after it, it is a box in its share of the titlebar: its
    /// width is its `min-width` and its margin is the room around it, and
    /// its text colour is that of the sign drawn on it. It is
    /// [`State::Focused`] with its frame and [`State::Hovered`] under the
    /// pointer. One with no width is not there.
    ButtonClose,
    /// The button that makes the frame fill its screen, or puts it back.
    ButtonMaximize,
    /// The button that takes the frame off the screen until it is asked
    /// for again.
    ButtonMinimize,
    /// A panel of the menu.
    Menu,
    /// A row of the menu. It is [`State::Hovered`] under the pointer.
    MenuItem,
    /// The line between groups of rows in the menu, drawn in its text
    /// colour.
    MenuSeparator,
    /// The notice that says something is wrong.
    Notice,
    /// The first line of the notice.
    NoticeHeading,
    /// The mark shown while a program takes pictures of the screen: a
    /// dot in its text colour on its background. It cannot be hidden.
    Capture,
    /// What the compositor says of something the pointer rests on, such
    /// as the mark: what is being done, and by which program.
    Tooltip,
    /// The panel's bar along the edge of a screen. Its margin is room
    /// left between it and the edges of the screen.
    Panel,
    /// A workspace on the panel. It is [`State::Active`] when it is the
    /// one being shown.
    Workspace,
    /// A window on the panel. It is [`State::Active`] when it is the one
    /// with focus.
    Task,
    /// The time on the panel.
    Clock,
    /// The launcher: the box that programs are picked from.
    Launcher,
    /// Where what is being looked for is typed in the launcher.
    LauncherInput,
    /// A program in the launcher's list. It is [`State::Active`] when it
    /// is the one that would be started.
    LauncherItem,
    /// What a program in the launcher's list says of itself, after its
    /// name. It is [`State::Active`] with its program.
    LauncherDescription,
    /// The whole of the screen behind the launcher while it is open. Only
    /// its background is used.
    LauncherBackdrop,
    /// A notification: what a program has to tell the user. It is
    /// [`State::Urgent`] when the program marked it critical, and
    /// [`State::Hovered`] under the pointer. Its margin is room kept
    /// between it, the next one and the edges of the screen.
    Notification,
    /// The first line of a notification, which says what it is about.
    NotificationTitle,
    /// The rest of what a notification says.
    NotificationBody,
    /// The question the portals put to the user, over everything: whether
    /// an application may have what it asks for. Its border is also the
    /// ring of the loupe a colour is picked with.
    Question,
    /// What giving an application what it asks for would let it see.
    QuestionDetail,
    /// A button of the question. The one that gives is [`State::Active`].
    QuestionButton,
}

impl Part {
    /// Every part there is.
    pub const ALL: [Self; 31] = [
        Self::Desktop,
        Self::Window,
        Self::TileLine,
        Self::Offer,
        Self::Titlebar,
        Self::Tab,
        Self::ButtonClose,
        Self::ButtonMaximize,
        Self::ButtonMinimize,
        Self::Menu,
        Self::MenuItem,
        Self::MenuSeparator,
        Self::Notice,
        Self::NoticeHeading,
        Self::Capture,
        Self::Tooltip,
        Self::Panel,
        Self::Workspace,
        Self::Task,
        Self::Clock,
        Self::Launcher,
        Self::LauncherInput,
        Self::LauncherItem,
        Self::LauncherDescription,
        Self::LauncherBackdrop,
        Self::Notification,
        Self::NotificationTitle,
        Self::NotificationBody,
        Self::Question,
        Self::QuestionDetail,
        Self::QuestionButton,
    ];

    /// What the part is called in a stylesheet.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Window => "window",
            Self::TileLine => "tile-line",
            Self::Offer => "offer",
            Self::Titlebar => "titlebar",
            Self::Tab => "tab",
            Self::ButtonClose => "button-close",
            Self::ButtonMaximize => "button-maximize",
            Self::ButtonMinimize => "button-minimize",
            Self::Menu => "menu",
            Self::MenuItem => "menu-item",
            Self::MenuSeparator => "menu-separator",
            Self::Notice => "notice",
            Self::NoticeHeading => "notice-heading",
            Self::Capture => "capture",
            Self::Tooltip => "tooltip",
            Self::Panel => "panel",
            Self::Workspace => "workspace",
            Self::Task => "task",
            Self::Clock => "clock",
            Self::Launcher => "launcher",
            Self::LauncherInput => "launcher-input",
            Self::LauncherItem => "launcher-item",
            Self::LauncherDescription => "launcher-description",
            Self::LauncherBackdrop => "launcher-backdrop",
            Self::Notification => "notification",
            Self::NotificationTitle => "notification-title",
            Self::NotificationBody => "notification-body",
            Self::Question => "question",
            Self::QuestionDetail => "question-detail",
            Self::QuestionButton => "question-button",
        }
    }
}

/// A state a part can be in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum State {
    /// It has keyboard focus, or belongs to the window that has.
    Focused,
    /// It is the one of its kind that is in use: the tab or the workspace
    /// being shown.
    Active,
    /// The pointer is over it.
    Hovered,
    /// It wants attention now, as a notification marked critical does.
    Urgent,
}

impl State {
    /// Every state there is.
    pub const ALL: [Self; 4] = [Self::Focused, Self::Active, Self::Hovered, Self::Urgent];

    /// What the state is called in a stylesheet, after a `:`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Focused => "focus",
            Self::Active => "active",
            Self::Hovered => "hover",
            Self::Urgent => "urgent",
        }
    }

    const fn bit(self) -> u8 {
        1 << self as u8
    }
}

/// The states a part is in at once. A part may be in none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct States(u8);

impl States {
    /// No state at all.
    pub const NONE: Self = Self(0);

    /// These states and `state` as well.
    #[must_use]
    pub const fn with(self, state: State) -> Self {
        Self(self.0 | state.bit())
    }

    /// These states, and `state` as well if it `holds`.
    #[must_use]
    pub const fn when(self, state: State, holds: bool) -> Self {
        if holds { self.with(state) } else { self }
    }

    /// Every combination of states there is.
    pub(crate) const COMBINATIONS: usize = 1 << State::ALL.len();

    /// Every combination of states, in the order of [`States::index`].
    pub(crate) fn every() -> impl Iterator<Item = Self> {
        (0..=u8::MAX).take(Self::COMBINATIONS).map(Self)
    }

    /// Which of the [`States::COMBINATIONS`] this is.
    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }

    /// How many states these are.
    pub(crate) const fn count(self) -> u32 {
        self.0.count_ones()
    }

    /// Whether every one of these states is among `others`.
    pub(crate) const fn within(self, others: Self) -> bool {
        self.0 & others.0 == self.0
    }

    /// Whether `state` is among them.
    #[must_use]
    pub const fn has(self, state: State) -> bool {
        self.0 & state.bit() != 0
    }
}

/// A line of some thickness. One of no thickness is not drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Line {
    /// Its colour.
    pub color: Color,
    /// Its thickness, in units of layout.
    pub width: u32,
}

impl Line {
    /// No line.
    pub const NONE: Self = Self {
        color: CLEAR,
        width: 0,
    };
}

/// Nothing: fully transparent.
pub(crate) const CLEAR: Color = Color::rgba(0, 0, 0, 0);

/// Something a part has on each of its four sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Edges<T> {
    /// Along the top.
    pub top: T,
    /// Down the right-hand side.
    pub right: T,
    /// Along the bottom.
    pub bottom: T,
    /// Down the left-hand side.
    pub left: T,
}

impl<T: Copy> Edges<T> {
    /// The same on every side.
    #[must_use]
    pub const fn all(each: T) -> Self {
        Self {
            top: each,
            right: each,
            bottom: each,
            left: each,
        }
    }
}

impl Edges<u32> {
    /// Nothing on any side.
    pub const NONE: Self = Self::all(0);

    /// What the left and right sides come to together.
    #[must_use]
    pub const fn across(&self) -> u32 {
        self.left.saturating_add(self.right)
    }

    /// What the top and bottom come to together.
    #[must_use]
    pub const fn down(&self) -> u32 {
        self.top.saturating_add(self.bottom)
    }
}

impl Edges<Line> {
    /// How thick the line on each side is.
    #[must_use]
    pub const fn widths(&self) -> Edges<u32> {
        Edges {
            top: self.top.width,
            right: self.right.width,
            bottom: self.bottom.width,
            left: self.left.width,
        }
    }
}

/// How a part looks. What does not apply to a part is left as nothing: a
/// transparent colour, no line, no size.
///
/// A part is a box. From the outside in it has a margin, which is room
/// kept clear around it; a border; padding, which is room between the
/// border and what the part holds; and its content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    /// What it is filled with, inside its border.
    pub background: Color,
    /// What its text is written in.
    pub color: Color,
    /// The line along each side.
    pub border: Edges<Line>,
    /// The room between its border and what it holds.
    pub padding: Edges<u32>,
    /// The room kept clear around it.
    pub margin: Edges<u32>,
    /// How far its corners are rounded, in units of layout.
    pub radius: u32,
    /// How tall it is, in units of layout, if that is fixed.
    pub height: u32,
    /// The narrowest it is made, in units of layout.
    pub min_width: u32,
    /// The widest it is made, in units of layout, or 0 for no limit.
    pub max_width: u32,
    /// How its colours get here from those of the state it was in before.
    pub transition: Transition,
}

impl Style {
    /// A part of which nothing is drawn.
    pub const NONE: Self = Self {
        background: CLEAR,
        color: CLEAR,
        border: Edges::all(Line::NONE),
        padding: Edges::NONE,
        margin: Edges::NONE,
        radius: 0,
        height: 0,
        min_width: 0,
        max_width: 0,
        transition: Transition::NONE,
    };

    /// The room its border and padding take on each side: how far in from
    /// its edges what it holds begins.
    #[must_use]
    pub const fn inset(&self) -> Edges<u32> {
        let (border, padding) = (self.border.widths(), self.padding);
        Edges {
            top: border.top.saturating_add(padding.top),
            right: border.right.saturating_add(padding.right),
            bottom: border.bottom.saturating_add(padding.bottom),
            left: border.left.saturating_add(padding.left),
        }
    }

    /// `width` kept within the narrowest and widest the part is made.
    #[must_use]
    pub fn fit(&self, width: u32) -> u32 {
        let widest = if self.max_width == 0 {
            u32::MAX
        } else {
            self.max_width
        };
        // Where the two disagree, the narrowest has the last word, as in
        // CSS.
        width.min(widest).max(self.min_width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_part_is_in_the_states_it_was_given_and_no_others() {
        assert_eq!(States::NONE, States::default());
        for state in State::ALL {
            assert!(!States::NONE.has(state));
            let alone = States::NONE.with(state);
            assert!(alone.has(state));
            assert_eq!(alone.with(state), alone);
            assert_eq!(alone.count(), 1);
        }
        let both = States::NONE.with(State::Focused).with(State::Active);
        assert!(both.has(State::Focused) && both.has(State::Active));
        assert!(!both.has(State::Hovered));
        assert_eq!(both.count(), 2);
        let hovered = States::NONE.with(State::Hovered);
        assert_eq!(States::NONE.when(State::Hovered, true), hovered);
        assert_eq!(both.when(State::Hovered, false), both);
    }

    #[test]
    fn states_are_within_any_that_include_them_all() {
        let focused = States::NONE.with(State::Focused);
        let both = focused.with(State::Active);
        assert!(States::NONE.within(focused) && States::NONE.within(States::NONE));
        assert!(focused.within(both) && both.within(both));
        assert!(!both.within(focused));
        assert!(!focused.within(States::NONE.with(State::Hovered)));
    }

    #[test]
    fn every_combination_of_states_is_a_different_one() {
        let all: Vec<States> = States::every().collect();
        assert_eq!(all.len(), States::COMBINATIONS);
        assert_eq!(all[0], States::NONE);
        for (index, states) in all.iter().enumerate() {
            assert_eq!(states.index(), index);
        }
    }

    #[test]
    fn a_box_is_inset_by_its_border_and_its_padding() {
        let line = |width| Line {
            color: CLEAR,
            width,
        };
        let style = Style {
            border: Edges {
                top: line(1),
                right: line(2),
                bottom: line(3),
                left: line(4),
            },
            padding: Edges {
                top: 10,
                right: 20,
                bottom: 30,
                left: 40,
            },
            ..Style::NONE
        };
        let inset = style.inset();
        assert_eq!(
            (inset.top, inset.right, inset.bottom, inset.left),
            (11, 22, 33, 44)
        );
        assert_eq!((inset.across(), inset.down()), (66, 44));
        assert_eq!(Style::NONE.inset(), Edges::NONE);
        assert_eq!(Edges::all(7).across(), 14);
    }

    #[test]
    fn a_width_is_kept_within_the_narrowest_and_widest_a_part_is_made() {
        assert_eq!(Style::NONE.fit(500), 500, "no limits");
        let style = Style {
            min_width: 28,
            max_width: 240,
            ..Style::NONE
        };
        assert_eq!(
            (style.fit(10), style.fit(100), style.fit(500)),
            (28, 100, 240)
        );
        let contrary = Style {
            min_width: 300,
            max_width: 240,
            ..Style::NONE
        };
        assert_eq!(contrary.fit(100), 300);
    }

    #[test]
    fn parts_and_states_have_the_names_a_stylesheet_calls_them_by() {
        let parts: Vec<&str> = Part::ALL.iter().map(|part| part.name()).collect();
        assert_eq!(
            parts,
            [
                "desktop",
                "window",
                "tile-line",
                "offer",
                "titlebar",
                "tab",
                "button-close",
                "button-maximize",
                "button-minimize",
                "menu",
                "menu-item",
                "menu-separator",
                "notice",
                "notice-heading",
                "capture",
                "tooltip",
                "panel",
                "workspace",
                "task",
                "clock",
                "launcher",
                "launcher-input",
                "launcher-item",
                "launcher-description",
                "launcher-backdrop",
                "notification",
                "notification-title",
                "notification-body",
                "question",
                "question-detail",
                "question-button"
            ]
        );
        let states: Vec<&str> = State::ALL.iter().map(|state| state.name()).collect();
        assert_eq!(states, ["focus", "active", "hover", "urgent"]);
        // The table of looks is laid out by a part's place in the list.
        for (index, part) in Part::ALL.into_iter().enumerate() {
            assert_eq!(part as usize, index);
        }
    }
}
