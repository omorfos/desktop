//! Things a key binding or menu entry can do.

use std::fmt;

use serde::de::value::{MapAccessDeserializer, SeqAccessDeserializer};
use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

use crate::Bounded;

/// The largest number of workspaces a session can have.
pub const MAX_WORKSPACES: u32 = 32;

/// A one-based workspace number, as users count them.
pub type WorkspaceNumber = Bounded<1, MAX_WORKSPACES>;

/// A half of the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Half {
    /// The left half.
    Left,
    /// The right half.
    Right,
    /// The top half, or upwards.
    #[serde(alias = "up")]
    Top,
    /// The bottom half, or downwards.
    #[serde(alias = "down")]
    Bottom,
}

/// An action triggered by a key binding or a menu entry.
///
/// In YAML, actions without a parameter are plain strings (`close`) and
/// actions with one are single-key mappings (`workspace: 2`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Start a program, given as a list: the program followed by its
    /// arguments. No shell is involved, so nothing is expanded or split.
    Spawn(Started),
    /// Ask the focused window to close.
    Close,
    /// Toggle the focused window between maximized and its normal size.
    Maximize,
    /// Toggle fullscreen for the focused window.
    Fullscreen,
    /// Hide the focused window.
    Minimize,
    /// Roll the focused window up into its titlebar, or let it down again.
    Shade,
    /// Keep the focused window above the others, or stop keeping it there.
    KeepAbove,
    /// Have the focused window on whichever workspace its screen is
    /// showing, or leave it on the one it is on.
    Stick,
    /// Focus the next window on the workspace.
    FocusNext,
    /// Focus the previous window on the workspace.
    FocusPrevious,
    /// Open the switcher at the window used before the one in use, or,
    /// while it is open, go on to the next. Letting go of the keys held
    /// with it chooses the window it is at.
    SwitchWindows,
    /// The same, going back: the switcher opens at the window used longest
    /// ago.
    SwitchWindowsBack,
    /// Switch to the tab after the active one in the focused window's group.
    NextTab,
    /// Switch to the tab before the active one in the focused window's group.
    PreviousTab,
    /// Have the focused window take a half of where it is: `left`,
    /// `right`, `top` or `bottom`. The next window to open there has the
    /// other half. From a desktop, where it is becomes a workspace made
    /// for it.
    Tile(Half),
    /// Give focus to the window beside the focused one, among windows
    /// that share a workspace: `left`, `right`, `up` or `down`.
    Focus(Half),
    /// Change the focused window over with what is beside it, among
    /// windows that share a workspace: `left`, `right`, `up` or `down`.
    Swap(Half),
    /// Switch to the workspace with this number. One past the last makes
    /// that workspace first.
    Workspace(WorkspaceNumber),
    /// Make a workspace after the last and switch to it. It goes again
    /// once it is empty and another is shown.
    NewWorkspace,
    /// Switch to the following workspace.
    NextWorkspace,
    /// Switch to the preceding workspace.
    PreviousWorkspace,
    /// Send the focused window to the workspace with this number, on the
    /// screen it is on. One past the last makes that workspace first.
    MoveToWorkspace(WorkspaceNumber),
    /// Send the focused window to a workspace made for it, after the last.
    MoveToNewWorkspace,
    /// Put the focused window beside those that share a workspace made
    /// for windows, known by the `id` the state gives it, and show that
    /// workspace.
    JoinWorkspace(u64),
    /// Send the focused window to the next screen.
    MoveToNextOutput,
    /// Open the root menu at the pointer.
    ShowMenu,
    /// Open the launcher, to start a program by typing its name.
    ShowLauncher,
    /// Lock the session, with the program `locker` in config.yaml names.
    Lock,
    /// Type with the next of the keyboard's layouts, and after the last
    /// with the first again.
    NextLayout,
    /// Re-read the configuration now.
    ReloadConfig,
    /// End the session.
    Quit,
}

/// A program to start, and what it may do that an ordinary program may
/// not.
///
/// Written as the program and its arguments alone, `[waybar]`, it is an
/// ordinary program. Written `{ run: [waybar], may: [panel] }` it is given
/// the roles named, by being started with a connection that has them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Started {
    /// The program and its arguments.
    pub run: Command,
    /// What it may do that an ordinary program may not. Nothing, unless
    /// said.
    pub may: Vec<Role>,
}

/// A program to start, written with what it may do.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Trusted {
    run: Command,
    #[serde(default)]
    may: Vec<Role>,
}

/// Reads a program to start in either of the ways it is written, saying
/// what is wrong with it as it would be said of either alone.
struct Either;

impl<'de> Visitor<'de> for Either {
    type Value = Started;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a program to run followed by its arguments, or `run` and `may`")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, words: A) -> Result<Started, A::Error> {
        Command::deserialize(SeqAccessDeserializer::new(words)).map(Started::from)
    }

    fn visit_map<A: MapAccess<'de>>(self, fields: A) -> Result<Started, A::Error> {
        let trusted = Trusted::deserialize(MapAccessDeserializer::new(fields));
        trusted.map(|Trusted { run, may }| Started { run, may })
    }
}

impl<'de> Deserialize<'de> for Started {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(Either)
    }
}

impl Started {
    /// The program the built-in bindings take a picture of the screen
    /// with, `omorfos-screenshot`, which may, and may put it on the
    /// clipboard if the configuration says that is where it goes.
    #[must_use]
    pub fn screenshot() -> Self {
        Self {
            run: Command {
                program: "omorfos-screenshot".to_owned(),
                arguments: Vec::new(),
            },
            may: vec![Role::Capture, Role::Clipboard],
        }
    }

    /// The same program, told to take the window with focus in place of
    /// the screens. It is told what windows there are, which is how it
    /// says which it wants.
    #[must_use]
    pub fn screenshot_window() -> Self {
        let mut started = Self::screenshot();
        started.run.arguments = vec!["--window".to_owned()];
        started.may.push(Role::Panel);
        started
    }

    /// The same program, told to let the user pick a screen or a region of
    /// one, over everything and with the keyboard, and to put the picture
    /// on the clipboard and nowhere else.
    #[must_use]
    pub fn screenshot_copied() -> Self {
        let mut started = Self::screenshot();
        started.run.arguments = ["--pick", "--to", "clipboard"].map(str::to_owned).to_vec();
        started.may.extend([Role::Panel, Role::Overlay]);
        started
    }
}

impl From<Command> for Started {
    fn from(run: Command) -> Self {
        Self {
            run,
            may: Vec::new(),
        }
    }
}

/// Something a program may do only if the compositor started it and was
/// told it may.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Put surfaces on the screen outside windows and keep room for them,
    /// and list the windows.
    Panel,
    /// Cover the screen and take the keyboard while it does.
    Overlay,
    /// Lock the session.
    Lock,
    /// Show the prompt that asks for a secret.
    Ask,
    /// Take pictures of the screen and of windows.
    Capture,
    /// Read and set the clipboard at any time.
    Clipboard,
    /// Type and click as the user, and see what is typed into fields.
    Input,
    /// Arrange the screens and change their colour.
    Screens,
}

/// A program and its arguments, run directly without a shell.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "Vec<String>")]
pub struct Command {
    program: String,
    arguments: Vec<String>,
}

impl Command {
    /// Creates a command from a program and its arguments.
    ///
    /// Returns `None` if `program` is empty.
    pub fn new<A>(program: impl Into<String>, arguments: A) -> Option<Self>
    where
        A: IntoIterator,
        A::Item: Into<String>,
    {
        let program = program.into();
        let arguments = arguments.into_iter().map(Into::into).collect();
        (!program.is_empty()).then_some(Self { program, arguments })
    }

    /// The terminal emulator started by the built-in bindings and menu.
    #[must_use]
    pub fn terminal() -> Self {
        Self {
            program: "foot".to_owned(),
            arguments: Vec::new(),
        }
    }

    /// `omorfos-session`, asked for what the words of `asked` say, such as
    /// `volume up`.
    #[must_use]
    pub fn session(asked: &str) -> Self {
        Self {
            program: "omorfos-session".to_owned(),
            arguments: asked.split(' ').map(str::to_owned).collect(),
        }
    }

    /// The program to run: a name looked up in `PATH`, or a path.
    #[must_use]
    pub fn program(&self) -> &str {
        &self.program
    }

    /// The arguments passed to the program.
    #[must_use]
    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }
}

impl TryFrom<Vec<String>> for Command {
    type Error = &'static str;

    fn try_from(words: Vec<String>) -> Result<Self, Self::Error> {
        let mut words = words.into_iter();
        words
            .next()
            .and_then(|program| Self::new(program, words))
            .ok_or(
                "expected a program to run followed by its arguments, such as `[foot, --server]`",
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(yaml: &str) -> Result<Action, String> {
        serde_saphyr::from_str(yaml).map_err(|error| error.without_snippet().to_string())
    }

    #[test]
    fn actions_without_a_parameter_are_plain_strings() {
        assert_eq!(parse("close"), Ok(Action::Close));
        assert_eq!(parse("focus_next"), Ok(Action::FocusNext));
        assert_eq!(parse("switch_windows"), Ok(Action::SwitchWindows));
        assert_eq!(parse("switch_windows_back"), Ok(Action::SwitchWindowsBack));
        assert_eq!(parse("reload_config"), Ok(Action::ReloadConfig));
        assert_eq!(parse("shade"), Ok(Action::Shade));
        assert_eq!(parse("keep_above"), Ok(Action::KeepAbove));
        assert_eq!(parse("stick"), Ok(Action::Stick));
        assert_eq!(parse("new_workspace"), Ok(Action::NewWorkspace));
        assert_eq!(parse("lock"), Ok(Action::Lock));
        assert_eq!(parse("next_layout"), Ok(Action::NextLayout));
        assert_eq!(
            parse("move_to_new_workspace"),
            Ok(Action::MoveToNewWorkspace)
        );
    }

    #[test]
    fn actions_with_a_parameter_are_single_key_mappings() {
        let two = WorkspaceNumber::clamped(2);
        assert_eq!(parse("workspace: 2"), Ok(Action::Workspace(two)));
        assert_eq!(
            parse("move_to_workspace: 2"),
            Ok(Action::MoveToWorkspace(two))
        );
        assert_eq!(parse("move_to_next_output"), Ok(Action::MoveToNextOutput));
        assert_eq!(parse("show_launcher"), Ok(Action::ShowLauncher));
        assert_eq!(parse("tile: left"), Ok(Action::Tile(Half::Left)));
        assert_eq!(parse("tile: bottom"), Ok(Action::Tile(Half::Bottom)));
        assert!(parse("tile: middle").is_err());
        assert_eq!(parse("join_workspace: 7"), Ok(Action::JoinWorkspace(7)));
        assert_eq!(parse("focus: up"), Ok(Action::Focus(Half::Top)));
        assert_eq!(parse("focus: left"), Ok(Action::Focus(Half::Left)));
        assert_eq!(parse("swap: down"), Ok(Action::Swap(Half::Bottom)));
        assert_eq!(parse("swap: right"), Ok(Action::Swap(Half::Right)));
    }

    #[test]
    fn spawn_takes_a_program_and_its_arguments() {
        let command = Command::new("foot", ["-e", "man 1 ls"]).unwrap();
        assert_eq!(command.program(), "foot");
        assert_eq!(command.arguments(), ["-e", "man 1 ls"]);
        assert_eq!(
            parse("spawn: [foot, -e, 'man 1 ls']"),
            Ok(Action::Spawn(command.into()))
        );
        // Told what it may do, as a screen locker bound to a key is.
        let locker = Command::new("swaylock", ["-f"]).unwrap();
        assert_eq!(
            parse("spawn: { run: [swaylock, -f], may: [lock] }"),
            Ok(Action::Spawn(Started {
                run: locker,
                may: vec![Role::Lock]
            }))
        );
    }

    #[test]
    fn a_program_to_start_is_a_list_or_says_what_it_may_do() {
        let wrong = parse("spawn: 5").unwrap_err();
        assert!(wrong.contains("or `run` and `may`"), "{wrong}");
        let wrong = parse("spawn: { run: [], may: [lock] }").unwrap_err();
        assert!(wrong.contains("expected a program to run"), "{wrong}");
        assert!(parse("spawn: { run: [foot], might: [lock] }").is_err());
    }

    #[test]
    fn spawn_rejects_a_missing_or_empty_program() {
        let message = "expected a program to run followed by its arguments";
        assert!(parse("spawn: []").unwrap_err().contains(message));
        assert!(parse("spawn: ['', -e]").unwrap_err().contains(message));
        assert_eq!(Command::new("", ["-e"]), None);
    }

    #[test]
    fn the_built_in_terminal_is_foot() {
        assert_eq!(Some(Command::terminal()), Command::new("foot", [""; 0]));
    }

    #[test]
    fn workspace_numbers_start_at_one_and_stop_at_the_maximum() {
        assert!(
            parse("workspace: 0")
                .unwrap_err()
                .contains("between 1 and 32, got 0")
        );
        assert!(
            parse("workspace: 33")
                .unwrap_err()
                .contains("between 1 and 32, got 33")
        );
        assert!(parse("workspace: 32").is_ok());
    }

    #[test]
    fn unknown_actions_are_rejected() {
        assert!(
            parse("explode")
                .unwrap_err()
                .contains("unknown variant `explode`")
        );
    }
}
