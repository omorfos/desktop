//! How each part of the desktop looks, in each state it can be in.
//!
//! Whatever draws something asks the theme for the [`Style`] of a [`Part`]
//! in its [`States`], and draws from that alone. It does not know where the
//! answer comes from: a stylesheet, in which rules written for parts and
//! states give them these properties.

use crate::{Color, Gradient, Transition};

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
    /// What holds a titlebar's three buttons, as GTK calls the same box.
    /// Nothing of it is drawn: it is there to be named before a button,
    /// as in `windowcontrols:hover button-close`, for the button to look
    /// as the three do together. It is [`State::Focused`] with its frame,
    /// and [`State::Hovered`] while the pointer is on any of its buttons.
    WindowControls,
    /// The button at the left of a titlebar that closes the window. Like
    /// the two after it, it is a box in its share of the titlebar: its
    /// width is its `min-width` and its margin is the room around it, and
    /// its text colour is that of the sign drawn on it. It is in
    /// [`Part::WindowControls`], is [`State::Focused`] with its frame, and
    /// is [`State::Hovered`] while the pointer is on it.
    /// One with no width is not there.
    ButtonClose,
    /// The button that makes the frame fill its screen, or puts it back.
    ButtonMaximize,
    /// The button that takes the frame off the screen until it is asked
    /// for again.
    ButtonMinimize,
    /// A panel of the menu.
    Menu,
    /// A row of the menu. It is [`State::Hovered`] under the pointer, and
    /// [`State::Focused`] where the keyboard has put it.
    MenuItem,
    /// The line between groups of rows in the menu, drawn in its text
    /// colour.
    MenuSeparator,
    /// What stands before the label of a row of the menu: a sign, drawn in
    /// its text colour, or a program's icon, inside its padding. It is
    /// [`State::Hovered`] or [`State::Focused`] with its row, and
    /// [`State::Active`] while what its row turns on and off is on.
    MenuSign,
    /// The keys that do what a row of the menu does, at the row's end, in
    /// its text colour. It is [`State::Hovered`] or [`State::Focused`] with
    /// its row.
    MenuKeys,
    /// The notice that says something is wrong.
    Notice,
    /// The first line of the notice.
    NoticeHeading,
    /// The mark shown while a program takes pictures of the screen: a
    /// dot in its text colour on its background. It cannot be hidden.
    Capture,
    /// The line round a screen while it is shared or recorded: its border.
    /// It is in no picture of the screen.
    CaptureBorder,
    /// What the compositor says of something the pointer rests on, such
    /// as the mark: what is being done, and by which program.
    Tooltip,
    /// The panel's bar along the edge of a screen. Its margin is room
    /// left between it and the edges of the screen.
    Panel,
    /// A workspace on the panel. It is [`State::Active`] when it is the
    /// one being shown, and [`State::Focused`] while the keyboard rings it.
    Workspace,
    /// A window on the panel. It is [`State::Active`] when it is the one
    /// with focus, and [`State::Focused`] while the keyboard rings it.
    Task,
    /// The time on the panel, and what says the session is kept awake. It
    /// is [`State::Focused`] while the keyboard rings it.
    Clock,
    /// What hangs from a bar, as the month from the clock: a box drawn as
    /// its program draws it, placed by the compositor, which draws its glass
    /// and shadow under it, and has it come and go over its transition.
    /// It is any pop-up of a component's panel.
    Popover,
    /// A row, a button or a heading in a popover, [`State::Hovered`]
    /// under the pointer.
    PopoverItem,
    /// A day of the month a popover shows: [`State::Hovered`] under the
    /// pointer, [`State::Focused`] in the week or the weekday of the day
    /// that is, and [`State::Active`] today. Its height, or its
    /// `min-width` if that is more, is how large a day is, and its margin
    /// the room between days.
    PopoverDay,
    /// A day of the month before or after, filling the weeks the month
    /// shown begins and ends in: in the same states as a
    /// [`Part::PopoverDay`], and as large.
    PopoverDayOutside,
    /// The launcher: the box that programs are picked from.
    Launcher,
    /// Where what is being looked for is typed in the launcher;
    /// [`State::Active`] what is typed when all of it is chosen, its
    /// background behind it.
    LauncherInput,
    /// A program in the launcher's list: its name, and under it what it
    /// says of itself, if that is shown, each in half of the room inside
    /// its padding, and its icon as tall as both. It is
    /// [`State::Active`] when it is the one that would be started.
    LauncherItem,
    /// What a program in the launcher's list says of itself, under its
    /// name. It is [`State::Active`] with its program.
    LauncherDescription,
    /// A program in the launcher's grid of every program: its icon, and
    /// its name under it, in one line. It is as wide as its `min-width`,
    /// the icon half as wide as the room inside its padding, and as tall
    /// as those and its name. It is [`State::Active`] when it is the one
    /// that would be started.
    LauncherTile,
    /// The best of all the launcher found, alone at the top in the mode
    /// that finds everything: a row as `launcher-item` is, its border
    /// saying it is what Enter takes while nothing is lit. It is
    /// [`State::Active`] when it is lit.
    LauncherTop,
    /// The heading over each section of what the launcher found: its
    /// colour, font and padding.
    LauncherHeading,
    /// The row of the launcher's modes at its bottom, shown while the
    /// pointer is over an empty launcher, and while the keyboard is on
    /// it: its border, padding and height.
    LauncherModes,
    /// A mode in that row, its sign beside its name and its digit:
    /// [`State::Active`] for the one shown, [`State::Hovered`] under the
    /// pointer, and [`State::Focused`] for the one the keyboard is on.
    LauncherMode,
    /// The key that goes to a mode, after its name in that row: the sign of
    /// the key held and the digit, in its colour, and its font for the
    /// digit; [`State::Active`] on the mode shown. `display: none` leaves
    /// it out, and the mode's button is narrower by it.
    LauncherModeKey,
    /// The whole of the screen behind the launcher while it is open. Only
    /// its background is used.
    LauncherBackdrop,
    /// A notification: what a program has to tell the user. It is
    /// [`State::Urgent`] when the program marked it critical,
    /// [`State::Hovered`] under the pointer, and [`State::Focused`] while
    /// it has the keyboard, and not one of its buttons, which are drawn as
    /// `question-button`. Its margin is room kept between it, the next one
    /// and the edges of the screen.
    Notification,
    /// The first line of a notification, which says what it is about.
    NotificationTitle,
    /// The rest of what a notification says.
    NotificationBody,
    /// How full something is that a notification tells of, such as the
    /// volume: a thin bar under what it says, whose background is the
    /// whole and whose colour is how much of it is full.
    NotificationLevel,
    /// The question the portals put to the user, over everything: whether
    /// an application may have what it asks for. Its border is also the
    /// ring of the loupe a colour is picked with.
    Question,
    /// What giving an application what it asks for would let it see.
    QuestionDetail,
    /// A button of the question. The one that gives is [`State::Active`].
    QuestionButton,
    /// Where a question asks for a secret to be typed, such as a password:
    /// [`State::Focused`] while it is typed into, and [`State::Active`]
    /// what is typed when all of it is chosen, its background behind it.
    QuestionInput,
    /// The switcher on `Super+Tab`: the box the windows are shown in, in the
    /// middle of the screen, or under them where they stand on a diagonal,
    /// with the title of the one picked.
    Switcher,
    /// A window's place in the switcher in rows, its picture over its
    /// title. The one picked is [`State::Active`]. Its `min-width` is how
    /// wide each is.
    SwitcherItem,
    /// The title of a window in the switcher.
    SwitcherTitle,
    /// A window in the switcher on a diagonal: its border round its
    /// picture, its padding away. Only its border, padding and corners are
    /// used. The one picked is [`State::Active`].
    SwitcherWindow,
    /// The screen behind the switcher while it is open: its background dims
    /// it, and its `backdrop-filter` blurs what is behind.
    SwitcherBackdrop,
    /// A window in the overview: its border round its picture, its padding
    /// away. The one picked, by the pointer or the arrows, is
    /// [`State::Active`]. Only its border, padding and corners are used.
    OverviewWindow,
    /// The title of the window picked in the overview, under it.
    OverviewTitle,
    /// A workspace in the overview's strip: its border round it. The one
    /// shown is [`State::Active`], and one under the pointer
    /// [`State::Hovered`]. Only its border and corners are used.
    OverviewWorkspace,
    /// The button after the overview's strip that makes a new workspace:
    /// its border, and the plus in its middle in its colour. It is
    /// [`State::Hovered`] under the pointer.
    OverviewAdd,
    /// The button on a window, or a workspace, in the overview that closes
    /// it: its border the ring round it, its background the disc inside,
    /// and its colour the cross on it, as large as the button is.
    OverviewClose,
    /// The screen behind the overview while it is open: its background dims
    /// it, and its `backdrop-filter` blurs what is behind.
    OverviewBackdrop,
    /// The screens shown still to pick what a picture is to be of: its
    /// background dims what is not picked, and its border is round what
    /// is drawn.
    Picker,
    /// What the compositor dims the session with while a prompt asks for a
    /// secret, such as the administrator's password: its background.
    Sealed,
    /// "About omorfos", over everything in the middle of the screen: the
    /// mark, the version, and what omorfos is made with that is someone
    /// else's, in its text colour.
    About,
    /// The name and version at the top of "About", and the heading of each
    /// list of what is someone else's.
    AboutHeading,
    /// Whose each thing is, under it, and the kind of each thing the
    /// computer has.
    AboutDetail,
    /// The button of "About" that copies what the computer is, `:hover`
    /// under the pointer.
    AboutButton,
}

impl Part {
    /// Every part there is.
    pub const ALL: [Self; 64] = [
        Self::Desktop,
        Self::Window,
        Self::TileLine,
        Self::Offer,
        Self::Titlebar,
        Self::Tab,
        Self::WindowControls,
        Self::ButtonClose,
        Self::ButtonMaximize,
        Self::ButtonMinimize,
        Self::Menu,
        Self::MenuItem,
        Self::MenuSeparator,
        Self::MenuSign,
        Self::MenuKeys,
        Self::Notice,
        Self::NoticeHeading,
        Self::Capture,
        Self::CaptureBorder,
        Self::Tooltip,
        Self::Panel,
        Self::Workspace,
        Self::Task,
        Self::Clock,
        Self::Popover,
        Self::PopoverItem,
        Self::PopoverDay,
        Self::PopoverDayOutside,
        Self::Launcher,
        Self::LauncherInput,
        Self::LauncherItem,
        Self::LauncherDescription,
        Self::LauncherTile,
        Self::LauncherTop,
        Self::LauncherHeading,
        Self::LauncherModes,
        Self::LauncherMode,
        Self::LauncherModeKey,
        Self::LauncherBackdrop,
        Self::Notification,
        Self::NotificationTitle,
        Self::NotificationBody,
        Self::NotificationLevel,
        Self::Question,
        Self::QuestionDetail,
        Self::QuestionButton,
        Self::QuestionInput,
        Self::Switcher,
        Self::SwitcherItem,
        Self::SwitcherTitle,
        Self::SwitcherWindow,
        Self::SwitcherBackdrop,
        Self::OverviewWindow,
        Self::OverviewTitle,
        Self::OverviewWorkspace,
        Self::OverviewAdd,
        Self::OverviewClose,
        Self::OverviewBackdrop,
        Self::Picker,
        Self::Sealed,
        Self::About,
        Self::AboutHeading,
        Self::AboutDetail,
        Self::AboutButton,
    ];

    /// The name a stylesheet called the part by before ADR 110, kept only
    /// to say what it is called now.
    #[must_use]
    pub const fn former(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Window => "window",
            Self::TileLine => "tile-line",
            Self::Offer => "offer",
            Self::Titlebar => "titlebar",
            Self::Tab => "tab",
            Self::WindowControls => "windowcontrols",
            Self::ButtonClose => "button-close",
            Self::ButtonMaximize => "button-maximize",
            Self::ButtonMinimize => "button-minimize",
            Self::Menu => "menu",
            Self::MenuItem => "menu-item",
            Self::MenuSeparator => "menu-separator",
            Self::MenuSign => "menu-sign",
            Self::MenuKeys => "menu-keys",
            Self::Notice => "notice",
            Self::NoticeHeading => "notice-heading",
            Self::Capture => "capture",
            Self::CaptureBorder => "capture-border",
            Self::Tooltip => "tooltip",
            Self::Panel => "panel",
            Self::Workspace => "workspace",
            Self::Task => "task",
            Self::Clock => "clock",
            Self::Popover => "popover",
            Self::PopoverItem => "popover-item",
            Self::PopoverDay => "popover-day",
            Self::PopoverDayOutside => "popover-day-outside",
            Self::Launcher => "launcher",
            Self::LauncherInput => "launcher-input",
            Self::LauncherItem => "launcher-item",
            Self::LauncherDescription => "launcher-description",
            Self::LauncherTile => "launcher-tile",
            Self::LauncherTop => "launcher-top",
            Self::LauncherHeading => "launcher-heading",
            Self::LauncherModes => "launcher-modes",
            Self::LauncherMode => "launcher-mode",
            Self::LauncherModeKey => "launcher-mode-key",
            Self::LauncherBackdrop => "launcher-backdrop",
            Self::Notification => "notification",
            Self::NotificationTitle => "notification-title",
            Self::NotificationBody => "notification-body",
            Self::NotificationLevel => "notification-level",
            Self::Question => "question",
            Self::QuestionDetail => "question-detail",
            Self::QuestionButton => "question-button",
            Self::QuestionInput => "question-input",
            Self::Switcher => "switcher",
            Self::SwitcherItem => "switcher-item",
            Self::SwitcherTitle => "switcher-title",
            Self::SwitcherWindow => "switcher-window",
            Self::SwitcherBackdrop => "switcher-backdrop",
            Self::OverviewWindow => "overview-window",
            Self::OverviewTitle => "overview-title",
            Self::OverviewWorkspace => "overview-workspace",
            Self::OverviewAdd => "overview-add",
            Self::OverviewClose => "overview-close",
            Self::OverviewBackdrop => "overview-backdrop",
            Self::Picker => "picker",
            Self::Sealed => "sealed",
            Self::About => "about",
            Self::AboutHeading => "about-heading",
            Self::AboutDetail => "about-detail",
            Self::AboutButton => "about-button",
        }
    }

    /// The part this one is in, which a rule may name before it, as in
    /// `windowcontrols:hover button-close`, to give it a look for while
    /// that part is in some states: the three buttons of a titlebar are in
    /// [`Part::WindowControls`]. Nothing, for a part in no other.
    #[must_use]
    pub const fn holder(self) -> Option<Self> {
        match self {
            Self::ButtonClose | Self::ButtonMaximize | Self::ButtonMinimize => {
                Some(Self::WindowControls)
            }
            _ => None,
        }
    }

    /// The parts that are in another, as [`Part::holder`] says, in order.
    pub const HELD: [Self; 3] = [
        Self::ButtonClose,
        Self::ButtonMaximize,
        Self::ButtonMinimize,
    ];

    /// The states a part that holds others, as [`Part::holder`] says, can
    /// be in: [`Part::WindowControls`] is focused with its frame, and
    /// hovered with the pointer on any of its buttons, and nothing else.
    pub const HOLDER_STATES: [State; 2] = [State::Focused, State::Hovered];
}

/// One element of a part's path in the tree of elements (ADR 110): its
/// type, its classes, and, for what is behind it over the whole screen,
/// the pseudo-element it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Node {
    /// Its type, as a selector names it: `button`.
    pub kind: &'static str,
    /// Its classes: `close` of `button.close`.
    pub classes: &'static [&'static str],
    /// The pseudo-element it is of its type, as `backdrop` of
    /// `launcher::backdrop`.
    pub pseudo: Option<&'static str>,
}

/// Where a state of a part is kept: in its own states, or in those of the
/// part it is in, as [`Part::holder`] says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// One of the part's own states.
    Own(State),
    /// One of the states of the part it is in.
    Holder(State),
}

/// How a state of a part is written in a stylesheet, and on which element
/// of its path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spelling {
    /// The state it is.
    pub slot: Slot,
    /// Which element of the part's path it is written on, from the first.
    pub node: usize,
    /// As it is written after that element: `:hover`, `:state(urgent)`,
    /// `::selection`.
    pub name: &'static str,
}

/// An element of a type alone.
const fn node(kind: &'static str) -> Node {
    Node {
        kind,
        classes: &[],
        pseudo: None,
    }
}

/// An element of a type, of some classes.
const fn classed(kind: &'static str, classes: &'static [&'static str]) -> Node {
    Node {
        kind,
        classes,
        pseudo: None,
    }
}

/// What is behind an element of a type over the whole screen.
const fn backdrop(kind: &'static str) -> Node {
    Node {
        kind,
        classes: &[],
        pseudo: Some("backdrop"),
    }
}

/// A state of the part itself, written on element `node` of its path.
const fn own(state: State, node: usize, name: &'static str) -> Spelling {
    Spelling {
        slot: Slot::Own(state),
        node,
        name,
    }
}

const DESKTOP: Node = node("desktop");
const WINDOW: Node = node("window");
const TITLEBAR: Node = node("titlebar");
const CONTROLS: Node = node("controls");
const MENU: Node = node("menu");
const ITEM: Node = node("item");
const PANEL: Node = node("panel");
const POPOVER: Node = node("popover");
const CALENDAR: Node = node("calendar");
const LAUNCHER: Node = node("launcher");
const LIST: Node = node("list");
const MODES: Node = node("modes");
const BUTTON: Node = node("button");
const NOTIFICATION: Node = node("notification");
const QUESTION: Node = classed("dialog", &["question"]);
const SWITCHER: Node = node("switcher");
const OVERVIEW: Node = node("overview");
const ABOUT: Node = classed("dialog", &["about"]);

/// A window's focus, said of what is in it.
const WINDOW_FOCUS: Spelling = own(State::Focused, 0, ":focus-within");
/// Coming and going, said of the first element of a path.
const CLOSING: Spelling = own(State::Closing, 0, ":state(closing)");
/// The three buttons of a titlebar's states.
const TITLEBAR_BUTTON: [Spelling; 3] = [
    WINDOW_FOCUS,
    own(State::Hovered, 3, ":hover"),
    Spelling {
        slot: Slot::Holder(State::Hovered),
        node: 2,
        name: ":hover",
    },
];
/// A day's states in the calendar.
const DAY: [Spelling; 3] = [
    own(State::Hovered, 2, ":hover"),
    own(State::Active, 2, ":state(today)"),
    own(State::Focused, 2, ":state(in-line)"),
];

impl Part {
    /// Where the part is in the tree of elements (ADR 110), from the
    /// element it is in at the root to itself.
    #[must_use]
    pub const fn path(self) -> &'static [Node] {
        match self {
            Self::Desktop => &const { [DESKTOP] },
            Self::Window => &const { [WINDOW] },
            Self::TileLine => &const { [DESKTOP, node("separator")] },
            Self::Offer => &const { [DESKTOP, classed("button", &["offer"])] },
            Self::Titlebar => &const { [WINDOW, TITLEBAR] },
            Self::Tab => &const { [WINDOW, TITLEBAR, node("tab")] },
            Self::WindowControls => &const { [WINDOW, TITLEBAR, CONTROLS] },
            Self::ButtonClose => {
                &const { [WINDOW, TITLEBAR, CONTROLS, classed("button", &["close"])] }
            }
            Self::ButtonMaximize => {
                &const { [WINDOW, TITLEBAR, CONTROLS, classed("button", &["maximize"])] }
            }
            Self::ButtonMinimize => {
                &const { [WINDOW, TITLEBAR, CONTROLS, classed("button", &["minimize"])] }
            }
            Self::Menu => &const { [MENU] },
            Self::MenuItem => &const { [MENU, ITEM] },
            Self::MenuSeparator => &const { [MENU, node("separator")] },
            Self::MenuSign => &const { [MENU, ITEM, node("icon")] },
            Self::MenuKeys => &const { [MENU, ITEM, node("kbd")] },
            Self::Notice => &const { [node("notice")] },
            Self::NoticeHeading => &const { [node("notice"), node("heading")] },
            Self::Capture => &const { [node("capture")] },
            Self::CaptureBorder => &const { [classed("frame", &["capture"])] },
            Self::Tooltip => &const { [node("tooltip")] },
            Self::Panel => &const { [PANEL] },
            Self::Workspace => &const { [PANEL, node("workspace")] },
            Self::Task => &const { [PANEL, node("task")] },
            Self::Clock => &const { [PANEL, node("clock")] },
            Self::Popover => &const { [POPOVER] },
            Self::PopoverItem => &const { [POPOVER, ITEM] },
            Self::PopoverDay => &const { [POPOVER, CALENDAR, node("day")] },
            Self::PopoverDayOutside => &const { [POPOVER, CALENDAR, classed("day", &["outside"])] },
            Self::Launcher => &const { [LAUNCHER] },
            Self::LauncherInput => &const { [LAUNCHER, node("input")] },
            Self::LauncherItem => &const { [LAUNCHER, LIST, ITEM] },
            Self::LauncherDescription => &const { [LAUNCHER, LIST, ITEM, node("description")] },
            Self::LauncherTile => &const { [LAUNCHER, node("grid"), ITEM] },
            Self::LauncherTop => &const { [LAUNCHER, LIST, classed("item", &["top"])] },
            Self::LauncherHeading => &const { [LAUNCHER, LIST, node("heading")] },
            Self::LauncherModes => &const { [LAUNCHER, MODES] },
            Self::LauncherMode => &const { [LAUNCHER, MODES, BUTTON] },
            Self::LauncherModeKey => &const { [LAUNCHER, MODES, BUTTON, node("kbd")] },
            Self::LauncherBackdrop => &const { [backdrop("launcher")] },
            Self::Notification => &const { [NOTIFICATION] },
            Self::NotificationTitle => &const { [NOTIFICATION, node("title")] },
            Self::NotificationBody => &const { [NOTIFICATION, node("body")] },
            Self::NotificationLevel => &const { [NOTIFICATION, node("meter")] },
            Self::Question => &const { [QUESTION] },
            Self::QuestionDetail => &const { [QUESTION, node("detail")] },
            Self::QuestionButton => &const { [QUESTION, BUTTON] },
            Self::QuestionInput => &const { [QUESTION, node("input")] },
            Self::Switcher => &const { [SWITCHER] },
            Self::SwitcherItem => &const { [SWITCHER, ITEM] },
            Self::SwitcherTitle => &const { [SWITCHER, ITEM, node("title")] },
            Self::SwitcherWindow => &const { [SWITCHER, node("thumbnail")] },
            Self::SwitcherBackdrop => &const { [backdrop("switcher")] },
            Self::OverviewWindow => &const { [OVERVIEW, node("thumbnail")] },
            Self::OverviewTitle => &const { [OVERVIEW, node("title")] },
            Self::OverviewWorkspace => &const { [OVERVIEW, node("workspace")] },
            Self::OverviewAdd => &const { [OVERVIEW, classed("button", &["add"])] },
            Self::OverviewClose => {
                &const { [OVERVIEW, node("thumbnail"), classed("button", &["close"])] }
            }
            Self::OverviewBackdrop => &const { [backdrop("overview")] },
            Self::Picker => &const { [node("picker")] },
            Self::Sealed => &const { [node("sealed")] },
            Self::About => &const { [ABOUT] },
            Self::AboutHeading => &const { [ABOUT, node("heading")] },
            Self::AboutDetail => &const { [ABOUT, node("detail")] },
            Self::AboutButton => &const { [ABOUT, BUTTON] },
        }
    }

    /// How each state the part can be in is written, and on which element
    /// of its path (ADR 110, addendum). A state not here is one a
    /// stylesheet cannot name of it.
    #[must_use]
    #[expect(
        clippy::too_many_lines,
        reason = "a table, an arm for each part that can be in a state"
    )]
    pub const fn spellings(self) -> &'static [Spelling] {
        match self {
            Self::Window => &const { [WINDOW_FOCUS, CLOSING] },
            Self::Titlebar => &const { [WINDOW_FOCUS] },
            Self::Tab => &const { [own(State::Active, 2, ":checked"), WINDOW_FOCUS] },
            Self::WindowControls => &const { [WINDOW_FOCUS, own(State::Hovered, 2, ":hover")] },
            Self::ButtonClose | Self::ButtonMaximize | Self::ButtonMinimize => &TITLEBAR_BUTTON,
            Self::Menu
            | Self::Notice
            | Self::Tooltip
            | Self::Panel
            | Self::Popover
            | Self::Launcher
            | Self::LauncherBackdrop
            | Self::Question
            | Self::Switcher
            | Self::About => &const { [CLOSING] },
            Self::MenuItem | Self::MenuKeys => {
                &const {
                    [
                        own(State::Hovered, 1, ":hover"),
                        own(State::Focused, 1, ":focus"),
                        own(State::Pressed, 1, ":active"),
                    ]
                }
            }
            Self::MenuSign => {
                &const {
                    [
                        own(State::Hovered, 1, ":hover"),
                        own(State::Focused, 1, ":focus"),
                        own(State::Active, 1, ":checked"),
                        own(State::Pressed, 1, ":active"),
                    ]
                }
            }
            Self::Workspace => {
                &const {
                    [
                        own(State::Active, 1, ":checked"),
                        own(State::Focused, 1, ":focus"),
                    ]
                }
            }
            Self::Task => {
                &const {
                    [
                        own(State::Active, 1, ":checked"),
                        own(State::Focused, 1, ":focus"),
                        own(State::Closing, 1, ":state(closing)"),
                    ]
                }
            }
            Self::Clock => {
                &const {
                    [
                        own(State::Focused, 1, ":focus"),
                        own(State::Closing, 1, ":state(closing)"),
                    ]
                }
            }
            Self::Offer | Self::PopoverItem | Self::OverviewAdd => {
                &const { [own(State::Hovered, 1, ":hover")] }
            }
            Self::PopoverDay | Self::PopoverDayOutside => &DAY,
            Self::LauncherInput => &const { [own(State::Active, 1, "::selection")] },
            Self::LauncherItem
            | Self::LauncherDescription
            | Self::LauncherTile
            | Self::LauncherTop
            | Self::LauncherModeKey => &const { [own(State::Active, 2, ":checked")] },
            Self::LauncherMode => {
                &const {
                    [
                        own(State::Active, 2, ":checked"),
                        own(State::Hovered, 2, ":hover"),
                        own(State::Focused, 2, ":focus"),
                    ]
                }
            }
            Self::Notification => {
                &const {
                    [
                        own(State::Urgent, 0, ":state(urgent)"),
                        own(State::Hovered, 0, ":hover"),
                        own(State::Focused, 0, ":focus"),
                        CLOSING,
                    ]
                }
            }
            Self::QuestionButton => {
                &const {
                    [
                        own(State::Active, 1, ":default"),
                        own(State::Hovered, 1, ":hover"),
                        own(State::Focused, 1, ":focus"),
                        own(State::Pressed, 1, ":active"),
                    ]
                }
            }
            Self::QuestionInput => {
                &const {
                    [
                        own(State::Focused, 1, ":focus"),
                        own(State::Active, 1, "::selection"),
                    ]
                }
            }
            Self::SwitcherItem | Self::SwitcherWindow | Self::OverviewWindow => {
                &const { [own(State::Active, 1, ":checked")] }
            }
            Self::OverviewWorkspace => {
                &const {
                    [
                        own(State::Active, 1, ":checked"),
                        own(State::Hovered, 1, ":hover"),
                    ]
                }
            }
            Self::AboutButton => {
                &const {
                    [
                        own(State::Hovered, 1, ":hover"),
                        own(State::Active, 1, ":state(copied)"),
                    ]
                }
            }
            Self::Desktop
            | Self::TileLine
            | Self::MenuSeparator
            | Self::NoticeHeading
            | Self::Capture
            | Self::CaptureBorder
            | Self::LauncherHeading
            | Self::LauncherModes
            | Self::NotificationTitle
            | Self::NotificationBody
            | Self::NotificationLevel
            | Self::QuestionDetail
            | Self::SwitcherTitle
            | Self::SwitcherBackdrop
            | Self::OverviewTitle
            | Self::OverviewBackdrop
            | Self::OverviewClose
            | Self::Picker
            | Self::Sealed
            | Self::AboutHeading
            | Self::AboutDetail => &const { [] },
        }
    }

    /// The part's path written as a selector that names it alone:
    /// `launcher > modes > button`.
    #[must_use]
    pub fn selector(self) -> String {
        let written: Vec<String> = self
            .path()
            .iter()
            .map(|node| {
                let mut one = node.kind.to_owned();
                for class in node.classes {
                    one.push('.');
                    one.push_str(class);
                }
                if let Some(pseudo) = node.pseudo {
                    one.push_str("::");
                    one.push_str(pseudo);
                }
                one
            })
            .collect();
        written.join(" > ")
    }

    /// The namespace of the layer surface a program puts the part on the
    /// screen as, for those that are one: what the compositor finds a
    /// part by. These are said between programs, and stay as they are
    /// whatever the theme calls the part.
    #[must_use]
    pub const fn namespace(self) -> Option<&'static str> {
        match self {
            Self::Launcher => Some("launcher"),
            Self::LauncherBackdrop => Some("launcher-backdrop"),
            Self::Panel => Some("panel"),
            Self::Notification => Some("notification"),
            Self::Question => Some("question"),
            _ => None,
        }
    }
}

/// Whether two names are the same, as the tables are checked while the
/// program is built.
const fn same(one: &str, other: &str) -> bool {
    let (one, other) = (one.as_bytes(), other.as_bytes());
    if one.len() != other.len() {
        return false;
    }
    let mut at = 0;
    while at < one.len() {
        if one[at] != other[at] {
            return false;
        }
        at += 1;
    }
    true
}

/// Whether `path` has a type twice.
const fn repeats(path: &[Node]) -> bool {
    let mut one = 0;
    while one < path.len() {
        let mut other = one + 1;
        while other < path.len() {
            if same(path[one].kind, path[other].kind) {
                return true;
            }
            other += 1;
        }
        one += 1;
    }
    false
}

/// Whether some path has a type twice, which would make `button button`
/// mean one button or two, and the suggestions said of a selector that
/// reaches nothing, by the types they share with it, unclear.
const fn a_kind_twice() -> bool {
    let mut any = false;
    let mut part = 0;
    while part < Part::ALL.len() {
        any |= repeats(Part::ALL[part].path());
        part += 1;
    }
    any
}

const _: () = assert!(!a_kind_twice(), "a path in the tree has a type twice");

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
    /// It is going from the screen. A part comes onto the screen over its
    /// own transition and leaves over the one it has in this state, so
    /// that what leaves can go quicker than it came.
    Closing,
    /// The pointer's button went down on it and is held, the pointer on
    /// it still: it acts as the button is let go (ADR 113). `:active`.
    Pressed,
}

impl State {
    /// Every state there is.
    pub const ALL: [Self; 6] = [
        Self::Focused,
        Self::Active,
        Self::Hovered,
        Self::Urgent,
        Self::Closing,
        Self::Pressed,
    ];

    /// What the state is called in a stylesheet, after a `:`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Focused => "focus",
            Self::Active => "active",
            Self::Hovered => "hover",
            Self::Urgent => "urgent",
            Self::Closing => "closing",
            Self::Pressed => "pressed",
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

/// The shadow a part casts on what is under it (`box-shadow`), in units of
/// layout: its box, made larger by `spread` on each side, moved `x` across
/// and `y` down, its edge softened over `blur`, in `color`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shadow {
    /// How far it falls to the right, or to the left if less than none.
    pub x: i32,
    /// How far it falls down, or up if less than none.
    pub y: i32,
    /// How wide its soft edge is.
    pub blur: u32,
    /// How much larger than the part it is on each side, or smaller.
    pub spread: i32,
    /// Its colour.
    pub color: Color,
}

impl Shadow {
    /// No shadow.
    pub const NONE: Self = Self {
        x: 0,
        y: 0,
        blur: 0,
        spread: 0,
        color: CLEAR,
    };

    /// Whether nothing of it is to be seen.
    #[must_use]
    pub const fn is_none(&self) -> bool {
        self.color.alpha == 0
    }
}

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
    /// What is drawn over that, if anything (`background-image`).
    pub gradient: Option<Gradient>,
    /// What its text is written in.
    pub color: Color,
    /// The line along each side.
    pub border: Edges<Line>,
    /// The line round the outside, as far out as `outline_offset`, its
    /// corners as round as the part's are and more by as far as it is
    /// out. A ring round what the keyboard is on.
    pub outline: Line,
    /// How far out from the part its outline is, in units of layout.
    pub outline_offset: u32,
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
    /// The shadow it casts.
    pub shadow: Shadow,
    /// The shadow inside it (`inset`): a sharp line inside its border
    /// along the edges it falls from, as wide as it falls, following the
    /// curve of its corners. Light along the top, as on the rim of a sheet
    /// of glass, is one that falls a pixel down.
    pub inset: Shadow,
    /// How far what is behind it is blurred, in units of layout, where its
    /// background lets it be seen (`backdrop-filter`); 0 for not at all.
    pub blur: u32,
    /// How see-through and how large it is drawn as a whole: what it goes
    /// to as it leaves, while it is `:state(closing)`.
    pub presence: Presence,
    /// Whether it is drawn at all, and given room: false where the theme
    /// says `display: none`. Only the parts whose description says so read
    /// it.
    pub displayed: bool,
}

/// How see-through and how large a part is drawn as a whole: once it is
/// there, as it arrives (`@starting-style`), and as it leaves (`:state(closing)`),
/// from one to the other over its transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Presence {
    /// How much of it is seen, from 0 for nothing to 255 for all of it.
    /// Where it is there, this is already in its colours.
    pub opacity: u8,
    /// How large it is, in thousandths of its size once there.
    pub scale: u16,
}

impl Presence {
    /// All of it, as large as it is.
    pub const WHOLE: Self = Self {
        opacity: u8::MAX,
        scale: 1000,
    };
}

impl Style {
    /// A part of which nothing is drawn.
    pub const NONE: Self = Self {
        background: CLEAR,
        gradient: None,
        color: CLEAR,
        border: Edges::all(Line::NONE),
        outline: Line::NONE,
        outline_offset: 0,
        padding: Edges::NONE,
        margin: Edges::NONE,
        radius: 0,
        height: 0,
        min_width: 0,
        max_width: 0,
        transition: Transition::NONE,
        shadow: Shadow::NONE,
        inset: Shadow::NONE,
        blur: 0,
        presence: Presence::WHOLE,
        displayed: true,
    };

    /// Whether what is behind it can be seen through its background: its
    /// colour and its gradient, if it has one.
    #[must_use]
    pub fn see_through(&self) -> bool {
        let gradient = self.gradient.is_none_or(|gradient| gradient.see_through());
        self.background.alpha < u8::MAX && gradient
    }

    /// The one colour its background comes to, where one has to stand for
    /// all of it, as for glass drawn solid: its colour, or, where it has
    /// none, its gradient's halfway along.
    #[must_use]
    pub fn ground(&self) -> Color {
        match self.gradient {
            Some(gradient) if self.background.alpha == 0 => gradient.at(0.5),
            _ => self.background,
        }
    }

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
    use crate::{Direction, Stop};

    #[test]
    fn a_background_can_be_seen_through_where_both_its_colour_and_gradient_can() {
        let red = Color::rgb(255, 0, 0);
        let half = Color::rgba(0, 0, 255, 128);
        let gradient = |from, to| {
            let stops = [
                Stop { color: from, at: 0 },
                Stop {
                    color: to,
                    at: 1000,
                },
            ];
            Gradient::new(Direction::Angle(90), &stops)
        };
        let style = |background, gradient| Style {
            background,
            gradient,
            ..Style::NONE
        };
        assert!(style(half, None).see_through());
        assert!(!style(red, None).see_through());
        assert!(style(CLEAR, gradient(red, half)).see_through());
        assert!(!style(CLEAR, gradient(red, red)).see_through(), "covered");
        assert!(
            !style(red, gradient(half, half)).see_through(),
            "over a colour"
        );
        // One colour standing for it all.
        assert_eq!(style(red, gradient(half, half)).ground(), red);
        assert_eq!(
            style(CLEAR, gradient(red, half)).ground(),
            red.toward(half, 0.5)
        );
        assert_eq!(style(half, None).ground(), half);
    }

    #[test]
    fn a_part_is_in_the_states_it_was_given_and_no_others() {
        assert_eq!(States::NONE, States::default());
        for state in State::ALL {
            assert!(!States::NONE.has(state));
            let alone = States::NONE.with(state);
            assert!(alone.has(state));
            assert_eq!(alone.with(state), alone);
            assert_eq!(alone.0.count_ones(), 1);
        }
        let both = States::NONE.with(State::Focused).with(State::Active);
        assert!(both.has(State::Focused) && both.has(State::Active));
        assert!(!both.has(State::Hovered));
        assert_eq!(both.0.count_ones(), 2);
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
    fn parts_had_the_names_a_stylesheet_called_them_by_before_the_tree() {
        let parts: Vec<&str> = Part::ALL.iter().map(|part| part.former()).collect();
        assert_eq!(
            parts,
            [
                "desktop",
                "window",
                "tile-line",
                "offer",
                "titlebar",
                "tab",
                "windowcontrols",
                "button-close",
                "button-maximize",
                "button-minimize",
                "menu",
                "menu-item",
                "menu-separator",
                "menu-sign",
                "menu-keys",
                "notice",
                "notice-heading",
                "capture",
                "capture-border",
                "tooltip",
                "panel",
                "workspace",
                "task",
                "clock",
                "popover",
                "popover-item",
                "popover-day",
                "popover-day-outside",
                "launcher",
                "launcher-input",
                "launcher-item",
                "launcher-description",
                "launcher-tile",
                "launcher-top",
                "launcher-heading",
                "launcher-modes",
                "launcher-mode",
                "launcher-mode-key",
                "launcher-backdrop",
                "notification",
                "notification-title",
                "notification-body",
                "notification-level",
                "question",
                "question-detail",
                "question-button",
                "question-input",
                "switcher",
                "switcher-item",
                "switcher-title",
                "switcher-window",
                "switcher-backdrop",
                "overview-window",
                "overview-title",
                "overview-workspace",
                "overview-add",
                "overview-close",
                "overview-backdrop",
                "picker",
                "sealed",
                "about",
                "about-heading",
                "about-detail",
                "about-button"
            ]
        );
        let states: Vec<&str> = State::ALL.iter().map(|state| state.name()).collect();
        assert_eq!(
            states,
            ["focus", "active", "hover", "urgent", "closing", "pressed"]
        );
        // The table of looks is laid out by a part's place in the list.
        for (index, part) in Part::ALL.into_iter().enumerate() {
            assert_eq!(part as usize, index);
        }
    }

    /// Where each part is in the tree of elements, as ADR 110's addendum
    /// gives it.
    const PATHS: [(Part, &str); 64] = [
        (Part::Desktop, "desktop"),
        (Part::Window, "window"),
        (Part::TileLine, "desktop > separator"),
        (Part::Offer, "desktop > button.offer"),
        (Part::Titlebar, "window > titlebar"),
        (Part::Tab, "window > titlebar > tab"),
        (Part::WindowControls, "window > titlebar > controls"),
        (
            Part::ButtonClose,
            "window > titlebar > controls > button.close",
        ),
        (
            Part::ButtonMaximize,
            "window > titlebar > controls > button.maximize",
        ),
        (
            Part::ButtonMinimize,
            "window > titlebar > controls > button.minimize",
        ),
        (Part::Menu, "menu"),
        (Part::MenuItem, "menu > item"),
        (Part::MenuSeparator, "menu > separator"),
        (Part::MenuSign, "menu > item > icon"),
        (Part::MenuKeys, "menu > item > kbd"),
        (Part::Notice, "notice"),
        (Part::NoticeHeading, "notice > heading"),
        (Part::Capture, "capture"),
        (Part::CaptureBorder, "frame.capture"),
        (Part::Tooltip, "tooltip"),
        (Part::Panel, "panel"),
        (Part::Workspace, "panel > workspace"),
        (Part::Task, "panel > task"),
        (Part::Clock, "panel > clock"),
        (Part::Popover, "popover"),
        (Part::PopoverItem, "popover > item"),
        (Part::PopoverDay, "popover > calendar > day"),
        (Part::PopoverDayOutside, "popover > calendar > day.outside"),
        (Part::Launcher, "launcher"),
        (Part::LauncherInput, "launcher > input"),
        (Part::LauncherItem, "launcher > list > item"),
        (
            Part::LauncherDescription,
            "launcher > list > item > description",
        ),
        (Part::LauncherTile, "launcher > grid > item"),
        (Part::LauncherTop, "launcher > list > item.top"),
        (Part::LauncherHeading, "launcher > list > heading"),
        (Part::LauncherModes, "launcher > modes"),
        (Part::LauncherMode, "launcher > modes > button"),
        (Part::LauncherModeKey, "launcher > modes > button > kbd"),
        (Part::LauncherBackdrop, "launcher::backdrop"),
        (Part::Notification, "notification"),
        (Part::NotificationTitle, "notification > title"),
        (Part::NotificationBody, "notification > body"),
        (Part::NotificationLevel, "notification > meter"),
        (Part::Question, "dialog.question"),
        (Part::QuestionDetail, "dialog.question > detail"),
        (Part::QuestionButton, "dialog.question > button"),
        (Part::QuestionInput, "dialog.question > input"),
        (Part::Switcher, "switcher"),
        (Part::SwitcherItem, "switcher > item"),
        (Part::SwitcherTitle, "switcher > item > title"),
        (Part::SwitcherWindow, "switcher > thumbnail"),
        (Part::SwitcherBackdrop, "switcher::backdrop"),
        (Part::OverviewWindow, "overview > thumbnail"),
        (Part::OverviewTitle, "overview > title"),
        (Part::OverviewWorkspace, "overview > workspace"),
        (Part::OverviewAdd, "overview > button.add"),
        (Part::OverviewClose, "overview > thumbnail > button.close"),
        (Part::OverviewBackdrop, "overview::backdrop"),
        (Part::Picker, "picker"),
        (Part::Sealed, "sealed"),
        (Part::About, "dialog.about"),
        (Part::AboutHeading, "dialog.about > heading"),
        (Part::AboutDetail, "dialog.about > detail"),
        (Part::AboutButton, "dialog.about > button"),
    ];

    #[test]
    fn every_part_is_at_the_path_the_adr_gives() {
        for (part, path) in PATHS {
            assert_eq!(part.selector(), path, "{part:?}");
        }
        let named: Vec<Part> = PATHS.iter().map(|(part, _)| *part).collect();
        assert_eq!(named, Part::ALL, "every part, in order");
    }

    #[test]
    fn no_two_parts_are_at_one_path() {
        let mut paths: Vec<String> = Part::ALL.iter().map(|part| part.selector()).collect();
        paths.sort();
        let before = paths.len();
        paths.dedup();
        assert_eq!(paths.len(), before);
    }

    #[test]
    fn the_tree_is_written_from_elements_and_states_spelled_on_them() {
        let close = classed("button", &["close"]);
        assert_eq!(
            (close.kind, close.classes, close.pseudo),
            ("button", &["close"][..], None)
        );
        assert_eq!(node("menu").classes, [] as [&str; 0]);
        assert_eq!(backdrop("launcher").pseudo, Some("backdrop"));
        let spelled = own(State::Hovered, 2, ":hover");
        assert_eq!(
            (spelled.slot, spelled.node, spelled.name),
            (Slot::Own(State::Hovered), 2, ":hover")
        );
    }

    #[test]
    fn a_path_with_a_type_twice_is_told_from_one_without() {
        assert!(same("kbd", "kbd"));
        assert!(!same("kbd", "kbds"));
        assert!(!same("kbd", "kbe"));
        assert!(repeats(&[node("a"), node("b"), node("a")]));
        assert!(!repeats(&[node("a"), node("b")]));
        assert!(!a_kind_twice());
    }

    #[test]
    fn no_path_has_a_kind_twice() {
        for part in Part::ALL {
            let kinds: Vec<&str> = part.path().iter().map(|node| node.kind).collect();
            for (at, kind) in kinds.iter().enumerate() {
                assert!(!kinds[at + 1..].contains(kind), "{part:?}: {kind}");
            }
        }
    }

    #[test]
    fn every_spelling_is_on_a_node_of_its_path_and_a_state_its_part_is_in() {
        for part in Part::ALL {
            for spelling in part.spellings() {
                assert!(spelling.node < part.path().len(), "{part:?}");
                let held = matches!(spelling.slot, Slot::Holder(_));
                assert!(!held || part.holder().is_some(), "{part:?}");
                assert!(
                    spelling.name.starts_with(':'),
                    "{part:?}: {}",
                    spelling.name
                );
            }
        }
        // As the addendum has them, for a few.
        let spelled = |part: Part| -> Vec<(Slot, usize, &str)> {
            let spellings = part.spellings().iter();
            spellings
                .map(|one| (one.slot, one.node, one.name))
                .collect()
        };
        assert_eq!(
            spelled(Part::Tab),
            [
                (Slot::Own(State::Active), 2, ":checked"),
                (Slot::Own(State::Focused), 0, ":focus-within"),
            ]
        );
        assert_eq!(
            spelled(Part::LauncherInput),
            [(Slot::Own(State::Active), 1, "::selection")]
        );
        assert!(spelled(Part::ButtonClose).contains(&(Slot::Holder(State::Hovered), 2, ":hover")));
        assert_eq!(
            spelled(Part::LauncherBackdrop),
            [(Slot::Own(State::Closing), 0, ":state(closing)")]
        );
        assert_eq!(spelled(Part::Desktop), []);
    }

    #[test]
    fn the_parts_surfaces_send_as_namespaces_are_found_by_them() {
        let found = |namespace: &str| {
            Part::ALL
                .into_iter()
                .find(|part| part.namespace() == Some(namespace))
        };
        assert_eq!(found("launcher"), Some(Part::Launcher));
        assert_eq!(found("launcher-backdrop"), Some(Part::LauncherBackdrop));
        assert_eq!(found("panel"), Some(Part::Panel));
        assert_eq!(found("notification"), Some(Part::Notification));
        assert_eq!(found("question"), Some(Part::Question));
        assert_eq!(found("wallpaper"), None);
        assert_eq!(found("menu"), None, "drawn by the compositor, sent by none");
    }
}
