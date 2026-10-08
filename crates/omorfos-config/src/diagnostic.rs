//! Problems found while loading configuration.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// A one-based line and column in a configuration file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    /// Line number, starting at 1.
    pub line: u64,
    /// Column number, starting at 1.
    pub column: u64,
}

/// A problem that prevented a configuration file from being applied.
///
/// Displays as `path:line:column: message`, the format editors and terminals
/// recognise as a clickable location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// The file the problem is in.
    pub file: PathBuf,
    /// Where in the file the problem is, when known.
    pub position: Option<Position>,
    /// What is wrong, written for the user.
    pub message: String,
}

impl Diagnostic {
    pub(crate) fn new(file: &Path, message: impl Into<String>) -> Self {
        Self {
            file: file.to_owned(),
            position: None,
            message: message.into(),
        }
    }

    pub(crate) fn io(file: &Path, error: &io::Error) -> Self {
        Self::new(file, format!("cannot read file: {error}"))
    }

    pub(crate) fn yaml(file: &Path, error: &serde_saphyr::Error) -> Self {
        let position = error.location().map(|location| Position {
            line: location.line(),
            column: location.column(),
        });
        // The parser appends the location to its message; we report it
        // separately, so drop the duplicate.
        let suffix = position
            .map(|at| format!(" at line {}, column {}", at.line, at.column))
            .unwrap_or_default();
        let rendered = error
            .without_snippet()
            .render_with_formatter(&serde_saphyr::UserMessageFormatter);
        let message = rendered
            .strip_suffix(&suffix)
            .unwrap_or(&rendered)
            .to_owned();
        Self {
            file: file.to_owned(),
            position,
            message,
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let location = self
            .position
            .map(|at| format!(":{}:{}", at.line, at.column))
            .unwrap_or_default();
        write!(f, "{}{location}: {}", self.file.display(), self.message)
    }
}

impl std::error::Error for Diagnostic {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_as_a_clickable_location_when_the_position_is_known() {
        let diagnostic = Diagnostic {
            file: PathBuf::from("/etc/omorfos/keys.yaml"),
            position: Some(Position {
                line: 12,
                column: 5,
            }),
            message: "unknown action".to_owned(),
        };
        assert_eq!(
            diagnostic.to_string(),
            "/etc/omorfos/keys.yaml:12:5: unknown action"
        );
    }

    #[test]
    fn displays_only_the_file_when_the_position_is_unknown() {
        let diagnostic = Diagnostic::new(Path::new("theme.yaml"), "file is too large");
        assert_eq!(diagnostic.to_string(), "theme.yaml: file is too large");
    }

    #[test]
    fn io_errors_explain_that_the_file_could_not_be_read() {
        let error = io::Error::new(io::ErrorKind::PermissionDenied, "permission denied");
        let diagnostic = Diagnostic::io(Path::new("menu.yaml"), &error);
        assert_eq!(
            diagnostic.to_string(),
            "menu.yaml: cannot read file: permission denied"
        );
    }

    #[test]
    fn yaml_errors_carry_the_position_without_repeating_it_in_the_message() {
        let error = serde_saphyr::from_str::<u8>("\n  300\n").unwrap_err();
        let diagnostic = Diagnostic::yaml(Path::new("config.yaml"), &error);
        assert_eq!(diagnostic.to_string(), "config.yaml:2:3: invalid u8");
    }
}
