//! Things a key binding or menu entry can do.

use serde::Deserialize;

use crate::Bounded;

/// The largest number of workspaces a session can have.
pub const MAX_WORKSPACES: u32 = 32;

/// A one-based workspace number, as users count them.
pub type WorkspaceNumber = Bounded<1, MAX_WORKSPACES>;

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
    Spawn(Command),
    /// Ask the focused window to close.
    Close,
    /// Toggle the focused window between maximized and its normal size.
    Maximize,
    /// Toggle fullscreen for the focused window.
    Fullscreen,
    /// Hide the focused window.
    Minimize,
    /// Focus the next window on the workspace.
    FocusNext,
    /// Focus the previous window on the workspace.
    FocusPrevious,
    /// Switch to the tab after the active one in the focused window's group.
    NextTab,
    /// Switch to the tab before the active one in the focused window's group.
    PreviousTab,
    /// Switch to the workspace with this number.
    Workspace(WorkspaceNumber),
    /// Switch to the following workspace.
    NextWorkspace,
    /// Switch to the preceding workspace.
    PreviousWorkspace,
    /// Send the focused window to the workspace with this number, on the
    /// screen it is on.
    MoveToWorkspace(WorkspaceNumber),
    /// Send the focused window to the next screen.
    MoveToNextOutput,
    /// Open the root menu at the pointer.
    ShowMenu,
    /// Open the launcher, to start a program by typing its name.
    ShowLauncher,
    /// Re-read the configuration now.
    ReloadConfig,
    /// End the session.
    Quit,
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
        assert_eq!(parse("reload_config"), Ok(Action::ReloadConfig));
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
    }

    #[test]
    fn spawn_takes_a_program_and_its_arguments() {
        let command = Command::new("foot", ["-e", "man 1 ls"]).unwrap();
        assert_eq!(command.program(), "foot");
        assert_eq!(command.arguments(), ["-e", "man 1 ls"]);
        assert_eq!(
            parse("spawn: [foot, -e, 'man 1 ls']"),
            Ok(Action::Spawn(command))
        );
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
