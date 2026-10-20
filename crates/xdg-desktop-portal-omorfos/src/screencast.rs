//! Sharing the screen, as the portal asks for it: a session is made, what
//! is to be shared is chosen, and it is started, once the user says so,
//! as a stream of pictures that the application is told how to find.

use std::collections::HashMap;

use zbus::blocking::Connection;
use zbus::message::Message;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

use crate::bus::{Asked, Waiting};

/// The interface sharing the screen is asked for by.
pub const SCREENCAST: &str = "org.freedesktop.impl.portal.ScreenCast";

/// The interface by which the portal ends a session it made.
pub const SESSION: &str = "org.freedesktop.impl.portal.Session";

/// What a session was made for, until it ends.
#[derive(Debug, Default)]
pub(crate) struct Session {
    /// How the pointer is to be shown in what is shared.
    pointer: Pointer,
    /// Whether a window may be shared, and not only a screen.
    windows: bool,
}

/// The number the portal has for a screen, among the kinds of thing that
/// may be shared.
const SCREEN: u32 = 1;

/// The number it has for a window.
const WINDOW: u32 = 2;

/// How the pointer is shown in what is shared.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Pointer {
    /// Not at all, which is what the portal asks unless it says.
    #[default]
    Hidden,
    /// Drawn into the pictures.
    Drawn,
    /// Sent beside them, so that it moving makes no new picture and what
    /// reads them draws it where it is.
    Beside,
}

impl Pointer {
    /// How the pointer is to be shown, by the number the portal has for
    /// it: 1 not at all, 2 drawn in, 4 sent beside. Any other is not at all.
    fn of(mode: Option<u32>) -> Self {
        match mode {
            Some(2) => Self::Drawn,
            Some(4) => Self::Beside,
            _ => Self::Hidden,
        }
    }
}

/// The sessions there are, by the object each is known by.
pub(crate) type Sessions = HashMap<OwnedObjectPath, Session>;

/// What the portal sends to make a session: the request, the session, the
/// application, and how it would like it done.
type Made = (
    OwnedObjectPath,
    OwnedObjectPath,
    String,
    HashMap<String, OwnedValue>,
);

/// What it sends to start one: the same, and the window it asks from.
type Begun = (
    OwnedObjectPath,
    OwnedObjectPath,
    String,
    String,
    HashMap<String, OwnedValue>,
);

/// Answers a message about sharing the screen, if it is one, and returns
/// whether it could be answered. Nothing, if it is about something else.
pub(crate) fn answer(
    connection: &Connection,
    message: &Message,
    called: (&str, Option<&str>),
    sessions: &mut Sessions,
    waiting: &Waiting,
) -> Option<zbus::Result<()>> {
    let header = message.header();
    let body = message.body();
    let malformed = || {
        let name = "org.freedesktop.DBus.Error.InvalidArgs";
        connection.reply_error(&header, name, &"the arguments are not those of this method")
    };
    // A request about a session that was never made, or has ended, fails.
    let failed = || {
        let nothing: HashMap<&str, Value<'_>> = HashMap::new();
        connection.reply(&header, &(2_u32, nothing))
    };
    let given = || {
        let nothing: HashMap<&str, Value<'_>> = HashMap::new();
        connection.reply(&header, &(0_u32, nothing))
    };
    let sent = match called {
        (SCREENCAST, Some("CreateSession")) => match body.deserialize::<Made>() {
            Ok((_, session, _, _)) => {
                sessions.insert(session, Session::default());
                given()
            }
            Err(_) => malformed(),
        },
        (SCREENCAST, Some("SelectSources")) => match body.deserialize::<Made>() {
            Ok((_, session, _, options)) => {
                let cursor = options.get("cursor_mode");
                let cursor = cursor.and_then(|mode| mode.downcast_ref::<u32>().ok());
                // What may be shared, of the kinds the portal has numbers
                // for: 1 a screen, 2 a window. A screen, unless it says.
                let kinds = options.get("types");
                let kinds = kinds.and_then(|kinds| kinds.downcast_ref::<u32>().ok());
                let made = sessions.get_mut(&session);
                let known = made.map(|made| {
                    made.pointer = Pointer::of(cursor);
                    made.windows = kinds.is_some_and(|kinds| kinds & WINDOW != 0);
                });
                if known.is_some() { given() } else { failed() }
            }
            Err(_) => malformed(),
        },
        (SCREENCAST, Some("Start")) => match body.deserialize::<Begun>() {
            Ok((request, session, app, _, _)) => {
                let made = sessions
                    .get(&session)
                    .map(|made| (made.pointer, made.windows));
                if let Some((pointer, windows)) = made {
                    // Answered once the user has said.
                    waiting.tell(Asked::Share {
                        call: message.clone(),
                        request,
                        session,
                        app,
                        pointer,
                        windows,
                    });
                    Ok(())
                } else {
                    failed()
                }
            }
            Err(_) => malformed(),
        },
        (SESSION, Some("Close")) => {
            let session = header
                .path()
                .map(|path| OwnedObjectPath::from(path.to_owned()));
            let session = session.unwrap_or_default();
            sessions.remove(&session);
            waiting.tell(Asked::Ended(session));
            connection.reply(&header, &())
        }
        _ => return None,
    };
    Some(sent)
}

/// What the application is told of what is shared: the stream it is, how
/// large it is, and whether it is a screen, with where on the desktop that
/// is, or a window, which has no place to be told.
#[must_use]
pub fn streams(
    node: u32,
    at: Option<(i32, i32)>,
    (width, height): (i32, i32),
) -> HashMap<&'static str, Value<'static>> {
    let kind = if at.is_some() { SCREEN } else { WINDOW };
    let mut about = HashMap::from([
        ("size", Value::from((width, height))),
        ("source_type", Value::U32(kind)),
    ]);
    about.extend(at.map(|at| ("position", Value::from(at))));
    HashMap::from([("streams", Value::from(vec![(node, about)]))])
}
