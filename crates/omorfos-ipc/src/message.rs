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
    /// As [`Request::Subscribe`], from a program that shows the user what
    /// is in [`State::problems`], as the notifications that come with
    /// omorfos do. For as long as such a program is connected, the
    /// compositor does not show them itself.
    ShowProblems,
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
    /// Ask where the windows that can be seen are, as [`Placed`]. It is
    /// answered once and is not part of the [`State`], which would
    /// otherwise be sent again each time a window moved.
    Placed,
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
    /// The answer to [`Request::Placed`]: the part of each window that
    /// can be seen on each screen it is on, the window on top first.
    Placed(Vec<Placed>),
    /// The request was carried out.
    Done,
    /// The request could not be understood or carried out.
    Error {
        /// Why, for showing to the user.
        message: String,
    },
}

impl Reply {
    /// The state of the session, if that is what was answered.
    #[must_use]
    pub fn into_state(self) -> Option<State> {
        match self {
            Self::State(state) => Some(state),
            Self::Version { .. } | Self::Placed(_) | Self::Done | Self::Error { .. } => None,
        }
    }

    /// Where the windows are, if that is what was answered.
    #[must_use]
    pub fn into_placed(self) -> Option<Vec<Placed>> {
        match self {
            Self::Placed(placed) => Some(placed),
            Self::Version { .. } | Self::State(_) | Self::Done | Self::Error { .. } => None,
        }
    }
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
    /// What has happened in the last few moments that the user might want
    /// to know of and need do nothing about, such as something having
    /// been started again after it stopped. Each is here for a short
    /// while and then is not.
    #[serde(default)]
    pub events: Vec<String>,
    /// The screens, in the order they stand as the configuration lists
    /// them, and then the rest. Empty from a compositor too old to say.
    #[serde(default)]
    pub screens: Vec<Screen>,
    /// Whether the session is locked: a screen locker holds it and a
    /// frame with nothing of the session in it has been shown on every
    /// screen. Until then a session that is being locked is not.
    #[serde(default)]
    pub locked: bool,
}

/// A screen, and the ways it can be driven.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Screen {
    /// Its name, such as `eDP-1`, which `config.yaml` lists it by.
    pub name: String,
    /// How far from the left of the space it begins, in units of layout.
    pub x: i32,
    /// How far from the top.
    pub y: i32,
    /// How wide it is, in units of layout.
    pub width: u32,
    /// How tall.
    pub height: u32,
    /// The mode it is driven in, as `config.yaml` writes one, such as
    /// `2560x1440@144`. Empty where the backend does not say, as inside
    /// another desktop.
    #[serde(default)]
    pub mode: String,
    /// The mode it says suits it best, written so. Empty if it says none.
    #[serde(default)]
    pub preferred: String,
    /// Every mode it can be driven in, written so, each once, the largest
    /// and fastest first.
    #[serde(default)]
    pub modes: Vec<String>,
}

/// A workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    /// What it is known by for as long as there is one, and never used
    /// again in a session. Its number changes as workspaces before it go;
    /// this does not. Zero from a compositor too old to say.
    #[serde(default)]
    pub id: u64,
    /// The name of the output it belongs to, such as `eDP-1`. Each output
    /// has its own workspaces, numbered alike. Empty while there is no
    /// output.
    #[serde(default)]
    pub output: String,
    /// Its number among that output's desktops, counting from 1 as key
    /// bindings and rules do. Zero for one made for a fullscreen window.
    pub number: u32,
    /// Its configured name, or its number if it was made and has none.
    pub name: String,
    /// Whether it is the one its output is showing.
    pub active: bool,
    /// Whether it is always there, as those the configuration names are.
    /// One that is not goes when it is empty and another is shown.
    #[serde(default = "kept")]
    pub kept: bool,
    /// The windows this workspace was made for, if it was made for
    /// windows: one that is fullscreen, or several that fill the screen
    /// between them, in the order of their parts. Such a workspace is not
    /// one of the desktops: its `number` is 0, no key goes to it by
    /// number, it is called what its windows are, and it goes when the
    /// last of them closes or leaves. Empty for a desktop.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub windows: Vec<u64>,
    /// Whether another window can join those it was made for, with the
    /// `join_workspace` action and its `id`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub room: bool,
}

/// Every workspace was always there before any could be made.
const fn kept() -> bool {
    true
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
    /// The name of the output whose workspaces it is on. Empty while there
    /// is no output.
    #[serde(default)]
    pub output: String,
    /// The number of the desktop it is on, among that output's, counting
    /// from 1. Zero for a window on a workspace made for a fullscreen
    /// window.
    pub workspace: u32,
    /// Whether it has keyboard focus.
    pub focused: bool,
    /// Whether it is the tab its frame is showing.
    pub shown: bool,
    /// Whether its frame is minimized.
    pub minimized: bool,
    /// How its frame stands apart from the others. Written as fields of
    /// the window itself.
    #[serde(flatten)]
    pub standing: Standing,
    /// What it is called in the list of windows a taskbar is given
    /// (`ext-foreign-toplevel-list-v1`), for a program that speaks that
    /// as well and wants to know which of those windows this is. Empty
    /// from a compositor that does not say.
    #[serde(default)]
    pub handle: String,
}

/// How a window's frame stands apart from the others, as the user or a
/// rule asked. Each is false from a compositor that does not say.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Standing {
    /// Whether it is rolled up into its titlebar.
    #[serde(default)]
    pub shaded: bool,
    /// Whether it is kept above the frames that are not.
    #[serde(default)]
    pub above: bool,
    /// Whether it is on every workspace of its output, whichever is shown.
    #[serde(default)]
    pub sticky: bool,
}

/// Where a window is on a screen: the part of it there, in the pixels of
/// that screen from its top left corner, as a picture of the screen has
/// them. Only what the window draws is counted, not its frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placed {
    /// The window's [`Window::id`].
    pub id: u64,
    /// The window's [`Window::handle`], by which a picture of it alone
    /// can be asked for.
    pub handle: String,
    /// The name of the screen.
    pub output: String,
    /// How far from the left of the screen it begins.
    pub x: u32,
    /// How far from the top of the screen it begins.
    pub y: u32,
    /// How wide it is there.
    pub width: u32,
    /// How tall it is there.
    pub height: u32,
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
        assert_eq!(encode(&Request::ShowProblems), "\"show_problems\"\n");
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
            Request::ShowProblems,
            Request::Reload,
            Request::Perform {
                action: json!("close"),
            },
            Request::Activate { window: u64::MAX },
            Request::Placed,
        ] {
            round_trip(&request);
        }
        let state = State {
            workspaces: vec![Workspace {
                output: "eDP-1".to_owned(),
                number: 1,
                name: "web".to_owned(),
                active: true,
                id: 0,
                kept: true,
                windows: Vec::new(),
                room: false,
            }],
            windows: vec![Window {
                id: 3,
                app_id: "foot".to_owned(),
                title: "a \"quoted\"\ntitle".to_owned(),
                output: "eDP-1".to_owned(),
                workspace: 1,
                focused: true,
                shown: true,
                minimized: false,
                standing: Standing::default(),
                handle: "3f2a".to_owned(),
            }],
            problems: vec!["keys.yaml:2:3: nope".to_owned()],
            events: vec!["something was started again".to_owned()],
            screens: vec![Screen {
                name: "eDP-1".to_owned(),
                x: 0,
                y: -864,
                width: 1536,
                height: 864,
                mode: "3840x2160@60".to_owned(),
                preferred: "3840x2160@60".to_owned(),
                modes: vec!["3840x2160@60".to_owned(), "3840x2160@48".to_owned()],
            }],
            locked: true,
        };
        for reply in [
            Reply::Version {
                protocol: PROTOCOL,
                compositor: "0.1.0".to_owned(),
            },
            Reply::State(state),
            Reply::State(State::default()),
            Reply::Placed(vec![Placed {
                id: 3,
                handle: "3f2a".to_owned(),
                output: "eDP-1".to_owned(),
                x: 10,
                y: 20,
                width: 640,
                height: 480,
            }]),
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

    #[test]
    fn a_workspace_from_before_they_could_be_made_is_one_that_is_kept() {
        let old = r#"{"output":"eDP-1","number":2,"name":"code","active":false}"#;
        let workspace: Workspace = serde_json::from_str(old).unwrap();
        assert!(workspace.kept);
        assert_eq!(workspace.id, 0);
    }

    #[test]
    fn a_state_from_before_it_said_whether_the_session_is_locked_says_it_is_not() {
        let old = r#"{"workspaces":[],"windows":[],"problems":[]}"#;
        let state: State = serde_json::from_str(old).unwrap();
        assert!(!state.locked);
    }

    #[test]
    fn how_a_window_stands_is_written_with_the_window_and_is_nothing_if_not_said() {
        let said = json!({
            "id": 7, "app_id": "notes", "title": "", "workspace": 1,
            "focused": false, "shown": true, "minimized": false, "above": true,
        });
        let window: Window = serde_json::from_value(said).unwrap();
        let above = Standing {
            above: true,
            ..Standing::default()
        };
        assert_eq!(window.standing, above);
        let written = serde_json::to_value(&window).unwrap();
        assert_eq!(written["above"], json!(true));
        assert_eq!(written["shaded"], json!(false));
        assert_eq!(written["sticky"], json!(false));
    }

    #[test]
    fn the_state_can_be_taken_from_an_answer_that_is_one() {
        let state = State {
            problems: vec!["something".to_owned()],
            ..State::default()
        };
        assert_eq!(Reply::State(state.clone()).into_state(), Some(state));
        assert_eq!(Reply::Done.into_state(), None);
        assert_eq!(Reply::Placed(Vec::new()).into_state(), None);
    }

    #[test]
    fn where_windows_are_can_be_taken_from_an_answer_that_says() {
        assert_eq!(Reply::Placed(Vec::new()).into_placed(), Some(Vec::new()));
        assert_eq!(encode(&Reply::Placed(Vec::new())), "{\"placed\":[]}\n");
        assert_eq!(encode(&Request::Placed), "\"placed\"\n");
        assert_eq!(Reply::State(State::default()).into_placed(), None);
        assert_eq!(Reply::Done.into_placed(), None);
        let version = Reply::Version {
            protocol: PROTOCOL,
            compositor: String::new(),
        };
        let error = Reply::Error {
            message: String::new(),
        };
        assert_eq!(version.into_placed(), None);
        assert_eq!(error.into_placed(), None);
    }
}
