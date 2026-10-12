//! How each part of the desktop looks, in each state it can be in.
//!
//! Whatever draws something asks the theme for the [`Style`] of a [`Part`]
//! in its [`States`], and draws from that alone. It does not know where the
//! answer comes from: a stylesheet, in which rules written for parts and
//! states give them these properties.

use crate::Color;

/// A part of the desktop that a theme gives a look to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Part {
    /// What is behind everything, where there is no wallpaper. Only its
    /// background is used.
    Desktop,
    /// A window's frame: the border around it and how far its corners are
    /// rounded. It can be [`State::Focused`].
    Window,
    /// The bar across the top of a frame. It can be [`State::Focused`],
    /// with its window.
    Titlebar,
    /// One window's title in a titlebar. It is [`State::Active`] when it
    /// is the window being shown, and [`State::Focused`] with its frame.
    /// Its border is the line between it and the tab before it.
    Tab,
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
    /// The panel's bar along the edge of a screen.
    Panel,
    /// A workspace or a window on the panel. It is [`State::Active`] when
    /// it is the workspace being shown or the window with focus.
    PanelButton,
    /// The time on the panel.
    Clock,
}

impl Part {
    /// Every part there is.
    pub const ALL: [Self; 12] = [
        Self::Desktop,
        Self::Window,
        Self::Titlebar,
        Self::Tab,
        Self::Menu,
        Self::MenuItem,
        Self::MenuSeparator,
        Self::Notice,
        Self::NoticeHeading,
        Self::Panel,
        Self::PanelButton,
        Self::Clock,
    ];

    /// What the part is called in a stylesheet.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Window => "window",
            Self::Titlebar => "titlebar",
            Self::Tab => "tab",
            Self::Menu => "menu",
            Self::MenuItem => "menu-item",
            Self::MenuSeparator => "menu-separator",
            Self::Notice => "notice",
            Self::NoticeHeading => "notice-heading",
            Self::Panel => "panel",
            Self::PanelButton => "panel-button",
            Self::Clock => "clock",
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
}

impl State {
    /// Every state there is.
    pub const ALL: [Self; 3] = [Self::Focused, Self::Active, Self::Hovered];

    /// What the state is called in a stylesheet, after a `:`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Focused => "focus",
            Self::Active => "active",
            Self::Hovered => "hover",
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

/// How a part looks. What does not apply to a part is left as nothing: a
/// transparent colour, no line, no size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    /// What it is filled with.
    pub background: Color,
    /// What its text is written in.
    pub color: Color,
    /// The line around it.
    pub border: Line,
    /// The line along its bottom edge, inside it.
    pub underline: Line,
    /// How far its corners are rounded, in units of layout.
    pub radius: u32,
    /// How tall it is, in units of layout, if that is fixed.
    pub height: u32,
}

impl Style {
    /// A part of which nothing is drawn.
    pub const NONE: Self = Self {
        background: CLEAR,
        color: CLEAR,
        border: Line::NONE,
        underline: Line::NONE,
        radius: 0,
        height: 0,
    };
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
    fn parts_and_states_have_the_names_a_stylesheet_calls_them_by() {
        let parts: Vec<&str> = Part::ALL.iter().map(|part| part.name()).collect();
        assert_eq!(
            parts,
            [
                "desktop",
                "window",
                "titlebar",
                "tab",
                "menu",
                "menu-item",
                "menu-separator",
                "notice",
                "notice-heading",
                "panel",
                "panel-button",
                "clock"
            ]
        );
        let states: Vec<&str> = State::ALL.iter().map(|state| state.name()).collect();
        assert_eq!(states, ["focus", "active", "hover"]);
        // The table of looks is laid out by a part's place in the list.
        for (index, part) in Part::ALL.into_iter().enumerate() {
            assert_eq!(part as usize, index);
        }
    }
}
