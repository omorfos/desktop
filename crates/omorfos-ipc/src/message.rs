//! The messages exchanged over the socket.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// The version of the protocol described by this crate.
///
/// It increases when a change would break existing clients. Additions that
/// old clients can ignore do not change it.
pub const PROTOCOL: u32 = 1;

/// The longest message either side accepts, in bytes, newline included.
pub const MAX_MESSAGE: usize = 64 * 1024;

/// What a client asks of the compositor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Request {
    /// Ask which protocol and compositor version is running.
    Version,
    /// Ask for the current [`State`].
    State,
    /// Ask to be sent the [`State`] now and again whenever it changes. The
    /// connection is then used for nothing else.
    Subscribe,
    /// Re-read the configuration now.
    Reload,
    /// Perform an action, written exactly as in `keys.yaml`: a string such
    /// as `"close"`, or an object such as `{"workspace": 2}`. Actions on a
    /// window apply to the focused one.
    Perform {
        /// The action.
        action: serde_json::Value,
    },
    /// Bring a window to the user's attention: switch to its workspace,
    /// restore it if minimized, focus and raise it.
    Activate {
        /// The window's [`Window::id`].
        window: u64,
    },
}

/// What the compositor answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reply {
    /// The answer to [`Request::Version`].
    Version {
        /// The [`PROTOCOL`] the compositor speaks.
        protocol: u32,
        /// The compositor's own version, such as `0.3.1`.
        compositor: String,
    },
    /// The answer to [`Request::State`], and each update after
    /// [`Request::Subscribe`].
    State(State),
    /// The request was carried out.
    Done,
    /// The request could not be understood or carried out.
    Error {
        /// Why, for showing to the user.
        message: String,
    },
}

/// Everything a panel or script might show about the session.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct State {
    /// The workspaces, in order.
    pub workspaces: Vec<Workspace>,
    /// Every window on every workspace, in a stable order.
    pub windows: Vec<Window>,
    /// What is currently wrong with the configuration or the environment.
    /// Empty when all is well.
    pub problems: Vec<String>,
}

/// A workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    /// Its number, counting from 1 as key bindings and rules do.
    pub number: u32,
    /// Its configured name.
    pub name: String,
    /// Whether it is the one being shown.
    pub active: bool,
}

/// A window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Window {
    /// An identifier that is unique for the life of the session.
    pub id: u64,
    /// The application the window belongs to.
    pub app_id: String,
    /// The window's title.
    pub title: String,
    /// The number of the workspace it is on, counting from 1.
    pub workspace: u32,
    /// Whether it has keyboard focus.
    pub focused: bool,
    /// Whether it is the tab its frame is showing.
    pub shown: bool,
    /// Whether its frame is minimized.
    pub minimized: bool,
}

/// Writes a message as one line, newline included.
#[must_use]
pub fn encode<T: Serialize>(message: &T) -> String {
    // The message types here cannot fail to serialise: they hold no maps
    // with non-string keys and no values JSON lacks.
    let mut line = serde_json::to_string(message).unwrap_or_default();
    line.push('\n');
    line
}

/// Reads a message from one line.
///
/// # Errors
///
/// Returns a description of the problem if the line is not valid JSON for
/// the expected message.
pub fn decode<T: DeserializeOwned>(line: &str) -> Result<T, String> {
    serde_json::from_str(line.trim()).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(message: &T) {
        let line = encode(message);
        assert!(
            line.ends_with('\n') && !line.trim_end().contains('\n'),
            "one line: {line:?}"
        );
        assert_eq!(decode::<T>(&line).as_ref(), Ok(message));
    }

    #[test]
    fn requests_without_arguments_are_plain_strings() {
        assert_eq!(encode(&Request::Version), "\"version\"\n");
        assert_eq!(encode(&Request::Subscribe), "\"subscribe\"\n");
        assert_eq!(decode("\"state\""), Ok(Request::State));
        assert_eq!(decode("  \"reload\"\n"), Ok(Request::Reload));
    }

    #[test]
    fn requests_with_arguments_are_single_key_objects() {
        let perform = Request::Perform {
            action: json!({ "workspace": 2 }),
        };
        assert_eq!(
            encode(&perform),
            "{\"perform\":{\"action\":{\"workspace\":2}}}\n"
        );
        assert_eq!(
            decode("{\"activate\":{\"window\":7}}"),
            Ok(Request::Activate { window: 7 })
        );
    }

    #[test]
    fn every_message_survives_a_round_trip() {
        for request in [
            Request::Version,
            Request::State,
            Request::Subscribe,
            Request::Reload,
            Request::Perform {
                action: json!("close"),
            },
            Request::Activate { window: u64::MAX },
        ] {
            round_trip(&request);
        }
        let state = State {
            workspaces: vec![Workspace {
                number: 1,
                name: "web".to_owned(),
                active: true,
            }],
            windows: vec![Window {
                id: 3,
                app_id: "foot".to_owned(),
                title: "a \"quoted\"\ntitle".to_owned(),
                workspace: 1,
                focused: true,
                shown: true,
                minimized: false,
            }],
            problems: vec!["keys.yaml:2:3: nope".to_owned()],
        };
        for reply in [
            Reply::Version {
                protocol: PROTOCOL,
                compositor: "0.1.0".to_owned(),
            },
            Reply::State(state),
            Reply::State(State::default()),
            Reply::Done,
            Reply::Error {
                message: "unknown action".to_owned(),
            },
        ] {
            round_trip(&reply);
        }
    }

    #[test]
    fn a_line_that_is_not_a_message_is_reported() {
        assert!(
            decode::<Request>("not json")
                .unwrap_err()
                .contains("expected")
        );
        assert!(
            decode::<Request>("\"explode\"")
                .unwrap_err()
                .contains("unknown variant")
        );
        assert!(decode::<Request>("").is_err());
    }
}
