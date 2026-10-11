//! How each part of the desktop looks, in each state it can be in.
//!
//! Whatever draws something asks the theme for the [`Style`] of a [`Part`]
//! in its [`States`], and draws from that alone. It does not know where the
//! answer comes from. Today it is worked out from the few colours and sizes
//! in `theme.yaml`; the same question can as well be answered from rules
//! written for each part and state.

use crate::{Color, Theme};

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

    const fn new(color: Color, width: u32) -> Self {
        Self { color, width }
    }
}

/// Nothing: fully transparent.
const CLEAR: Color = Color::rgba(0, 0, 0, 0);

/// The thickness of the line under what is active, in units of layout.
const UNDERLINE: u32 = 2;

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
    const NONE: Self = Self {
        background: CLEAR,
        color: CLEAR,
        border: Line::NONE,
        underline: Line::NONE,
        radius: 0,
        height: 0,
    };

    /// A filled part with text on it.
    const fn filled(background: Color, color: Color) -> Self {
        Self {
            background,
            color,
            ..Self::NONE
        }
    }

    /// A part of which only text, or a line in the text's colour, is drawn.
    const fn inked(color: Color) -> Self {
        Self {
            color,
            ..Self::NONE
        }
    }
}

impl Theme {
    /// How `part` looks while it is in `states`. States a part cannot be
    /// in make no difference to it.
    #[must_use]
    pub fn style(&self, part: Part, states: States) -> Style {
        let colors = &self.colors;
        let (focused, plain) = (colors.focused, colors.unfocused);
        let own = if states.has(State::Focused) {
            focused
        } else {
            plain
        };
        let active = states.has(State::Active);
        let row = self.titlebar.height.get();
        match part {
            Part::Desktop => Style::filled(colors.background, CLEAR),
            Part::Window => Style {
                border: Line::new(own.border, self.border.width.get()),
                radius: self.border.radius.get(),
                ..Style::NONE
            },
            Part::Titlebar => Style {
                height: row,
                ..Style::filled(own.titlebar, own.text)
            },
            Part::Tab if active => {
                let marked = states.has(State::Focused);
                let mark = if marked { colors.accent } else { own.border };
                Style {
                    border: Line::new(own.border, 1),
                    underline: Line::new(mark, UNDERLINE),
                    ..Style::filled(own.titlebar, own.text)
                }
            }
            Part::Tab => Style {
                border: Line::new(own.border, 1),
                ..Style::filled(plain.titlebar, plain.text)
            },
            Part::Menu => Style {
                border: Line::new(plain.border, 1),
                ..Style::filled(focused.titlebar, focused.text)
            },
            Part::MenuItem if states.has(State::Hovered) => Style {
                height: row,
                ..Style::filled(colors.accent, colors.background)
            },
            Part::MenuItem => Style {
                height: row,
                ..Style::inked(focused.text)
            },
            Part::MenuSeparator => Style::inked(plain.border),
            Part::Notice => Style {
                border: Line::new(colors.accent, 1),
                ..Style::filled(focused.titlebar, focused.text)
            },
            Part::NoticeHeading => Style::inked(colors.accent),
            Part::Panel => Style {
                height: row,
                ..Style::filled(plain.titlebar, plain.text)
            },
            Part::PanelButton if active => Style {
                underline: Line::new(colors.accent, UNDERLINE),
                ..Style::filled(focused.titlebar, focused.text)
            },
            Part::PanelButton => Style::filled(plain.titlebar, plain.text),
            Part::Clock => Style::inked(focused.text),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FOCUSED: States = States::NONE.with(State::Focused);
    const ACTIVE: States = States::NONE.with(State::Active);
    const HOVERED: States = States::NONE.with(State::Hovered);

    #[test]
    fn a_part_is_in_the_states_it_was_given_and_no_others() {
        assert_eq!(States::NONE, States::default());
        for state in [State::Focused, State::Active, State::Hovered] {
            assert!(!States::NONE.has(state));
            assert!(States::NONE.with(state).has(state));
            assert_eq!(
                States::NONE.with(state).with(state),
                States::NONE.with(state)
            );
        }
        let both = FOCUSED.with(State::Active);
        assert!(both.has(State::Focused) && both.has(State::Active));
        assert!(!both.has(State::Hovered));
        assert_eq!(States::NONE.when(State::Hovered, true), HOVERED);
        assert_eq!(FOCUSED.when(State::Hovered, false), FOCUSED);
    }

    #[test]
    fn a_frame_has_the_themes_border_in_the_colour_of_its_focus() {
        let theme = Theme::default();
        let colors = theme.colors;
        let focused = theme.style(Part::Window, FOCUSED);
        assert_eq!(focused.border, Line::new(colors.focused.border, 1));
        assert_eq!(focused.radius, 8);
        assert_eq!(focused.background, CLEAR, "the window fills it itself");
        let other = theme.style(Part::Window, States::NONE);
        assert_eq!(other.border.color, colors.unfocused.border);
        assert_eq!((other.border.width, other.radius), (1, 8));
    }

    #[test]
    fn a_titlebar_takes_its_colours_from_its_focus_and_its_height_from_the_theme() {
        let theme = Theme::default();
        let colors = theme.colors;
        let focused = theme.style(Part::Titlebar, FOCUSED);
        assert_eq!(focused.background, colors.focused.titlebar);
        assert_eq!(focused.color, colors.focused.text);
        assert_eq!(focused.height, 28);
        let other = theme.style(Part::Titlebar, States::NONE);
        assert_eq!(other.background, colors.unfocused.titlebar);
        assert_eq!(other.color, colors.unfocused.text);
        assert_eq!(focused.border, Line::NONE, "the frame has the border");
    }

    #[test]
    fn the_tab_being_shown_is_underlined_and_the_others_are_dimmed() {
        let theme = Theme::default();
        let colors = theme.colors;
        let shown = theme.style(Part::Tab, FOCUSED.with(State::Active));
        assert_eq!(shown.background, colors.focused.titlebar);
        assert_eq!(shown.color, colors.focused.text);
        assert_eq!(shown.underline, Line::new(colors.accent, 2));
        assert_eq!(shown.border, Line::new(colors.focused.border, 1));

        // In a frame without focus the line is there but does not stand out.
        let shown = theme.style(Part::Tab, ACTIVE);
        assert_eq!(shown.background, colors.unfocused.titlebar);
        assert_eq!(shown.underline, Line::new(colors.unfocused.border, 2));

        let behind = theme.style(Part::Tab, FOCUSED);
        assert_eq!(behind.background, colors.unfocused.titlebar);
        assert_eq!(behind.color, colors.unfocused.text);
        assert_eq!(behind.underline, Line::NONE);
        assert_eq!(behind.border, Line::new(colors.focused.border, 1));
    }

    #[test]
    fn the_menu_row_under_the_pointer_is_filled_with_the_accent() {
        let theme = Theme::default();
        let colors = theme.colors;
        let menu = theme.style(Part::Menu, States::NONE);
        assert_eq!(menu.background, colors.focused.titlebar);
        assert_eq!(menu.border, Line::new(colors.unfocused.border, 1));
        let row = theme.style(Part::MenuItem, States::NONE);
        assert_eq!((row.background, row.color), (CLEAR, colors.focused.text));
        let under = theme.style(Part::MenuItem, HOVERED);
        assert_eq!(
            (under.background, under.color),
            (colors.accent, colors.background)
        );
        assert_eq!((row.height, under.height), (28, 28));
        let line = theme.style(Part::MenuSeparator, States::NONE);
        assert_eq!(line.color, colors.unfocused.border);
    }

    #[test]
    fn the_notice_is_outlined_and_headed_in_the_accent() {
        let theme = Theme::default();
        let colors = theme.colors;
        let notice = theme.style(Part::Notice, States::NONE);
        assert_eq!(notice.background, colors.focused.titlebar);
        assert_eq!(notice.color, colors.focused.text);
        assert_eq!(notice.border, Line::new(colors.accent, 1));
        let heading = theme.style(Part::NoticeHeading, States::NONE);
        assert_eq!(heading.color, colors.accent);
    }

    #[test]
    fn what_is_active_on_the_panel_is_lit_and_underlined() {
        let theme = Theme::default();
        let colors = theme.colors;
        let panel = theme.style(Part::Panel, States::NONE);
        assert_eq!(panel.background, colors.unfocused.titlebar);
        assert_eq!(panel.height, 28);
        let button = theme.style(Part::PanelButton, States::NONE);
        assert_eq!(button.background, panel.background);
        assert_eq!(button.color, colors.unfocused.text);
        assert_eq!(button.underline, Line::NONE);
        let lit = theme.style(Part::PanelButton, ACTIVE);
        assert_eq!(lit.background, colors.focused.titlebar);
        assert_eq!(lit.color, colors.focused.text);
        assert_eq!(lit.underline, Line::new(colors.accent, 2));
        let clock = theme.style(Part::Clock, States::NONE);
        assert_eq!(clock.color, colors.focused.text);
    }

    #[test]
    fn the_desktop_is_the_themes_background() {
        let theme = Theme::default();
        let desktop = theme.style(Part::Desktop, FOCUSED);
        assert_eq!(desktop.background, theme.colors.background);
    }

    #[test]
    fn styles_follow_what_the_theme_file_says() {
        let yaml = "border: { width: 3, radius: 0 }\ntitlebar: { height: 40 }\n";
        let theme: Theme = serde_saphyr::from_str(yaml).unwrap();
        let frame = theme.style(Part::Window, States::NONE);
        assert_eq!((frame.border.width, frame.radius), (3, 0));
        assert_eq!(theme.style(Part::Titlebar, States::NONE).height, 40);
        assert_eq!(theme.style(Part::Panel, States::NONE).height, 40);
        assert_eq!(theme.style(Part::MenuItem, States::NONE).height, 40);
    }
}
