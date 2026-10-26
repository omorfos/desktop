//! The service on the bus that the portals pass applications' questions
//! to.
//!
//! Applications do not speak to this. They speak to `xdg-desktop-portal`,
//! which passes on what only the desktop can answer to whichever program
//! has the name its configuration gives for that desktop, by interfaces
//! the portal's own documentation sets out. This takes the name and
//! answers.
//!
//! Messages are read one at a time and answered here, by name, with no
//! machinery in between, so that every answer there is can be seen and
//! tested. They come from other programs: one that does not have the shape
//! it should is answered with an error and otherwise passed over.

use std::collections::HashMap;
use std::io::Write;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use rustix::pipe::{PipeFlags, pipe_with};
use zbus::blocking::connection::Builder;
use zbus::blocking::{Connection, MessageIterator};
use zbus::fdo::RequestNameFlags;
use zbus::message::{Message, Type};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Structure, Value};

use crate::inhibit::{self, Holds, INHIBIT, Keep, SCREENSAVER};
use crate::screencast::{self, Pointer, SCREENCAST, SESSION, Sessions};
use crate::{Appearance, Failure};

/// The name the portals pass questions for omorfos to.
pub const NAME: &str = "org.freedesktop.impl.portal.desktop.omorfos";

/// The object they are passed to.
pub const PATH: &str = "/org/freedesktop/portal/desktop";

/// The interface what the desktop looks like is asked by.
pub const SETTINGS: &str = "org.freedesktop.impl.portal.Settings";

/// The group of settings that say how the desktop looks.
pub const LOOKS: &str = "org.freedesktop.appearance";

/// The interface a picture of the screen is asked for by.
pub const SCREENSHOT: &str = "org.freedesktop.impl.portal.Screenshot";

/// The interface by which the portal says it no longer wants what it
/// asked for, on the object it named for that request.
pub const REQUEST: &str = "org.freedesktop.impl.portal.Request";

/// What can be read of each interface: which version of it is spoken,
/// and, of sharing the screen, that screens and windows can be shared,
/// with the pointer not shown, drawn in, or sent beside the pictures.
const PROPERTIES: [(&str, &str, u32); 7] = [
    (INHIBIT, "version", 3),
    (SETTINGS, "version", 1),
    (SCREENSHOT, "version", 2),
    (SCREENCAST, "version", 5),
    (SCREENCAST, "AvailableSourceTypes", 1 | 2),
    (SCREENCAST, "AvailableCursorModes", 1 | 2 | 4),
    (SESSION, "version", 1),
];

/// How a request is answered: given, refused by the user, or failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Response {
    /// What was asked for is given.
    Given = 0,
    /// The user said no.
    Refused = 1,
    /// It could not be done.
    Failed = 2,
}

/// What the portal asks of the desktop that waits on the user.
#[derive(Debug)]
pub enum Asked {
    /// A picture of the screen for an application, by its identifier,
    /// which is empty for one not in a sandbox. `request` is the object
    /// the portal may say it no longer wants it by.
    Screenshot {
        /// The call, to be answered once the user has.
        call: Message,
        /// The object the request is known by.
        request: OwnedObjectPath,
        /// The application, as the sandbox names it.
        app: String,
    },
    /// A colour picked from the screen for an application, by its
    /// identifier, as for a picture.
    Pick {
        /// The call, to be answered once the user has picked.
        call: Message,
        /// The object the request is known by.
        request: OwnedObjectPath,
        /// The application, as the sandbox names it.
        app: String,
    },
    /// The request known by this object is no longer wanted.
    Withdrawn(OwnedObjectPath),
    /// A screen to be shared with an application, in the session the
    /// portal made for it, with the pointer shown as asked.
    Share {
        /// The call, to be answered once the user has.
        call: Message,
        /// The object the request is known by.
        request: OwnedObjectPath,
        /// The object the session is known by.
        session: OwnedObjectPath,
        /// The application, as the sandbox names it.
        app: String,
        /// How the pointer is to be shown in what is shared.
        pointer: Pointer,
        /// Whether a window may be shared, and not only a screen.
        windows: bool,
    },
    /// The session known by this object has ended, and whatever it shares
    /// is to stop.
    Ended(OwnedObjectPath),
}

/// What the service says of itself to a program that asks how to use it.
const INTROSPECTION: &str = r#"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node>
  <interface name="org.freedesktop.impl.portal.Settings">
    <method name="ReadAll">
      <arg direction="in" name="namespaces" type="as"/>
      <arg direction="out" name="value" type="a{sa{sv}}"/>
    </method>
    <method name="Read">
      <arg direction="in" name="namespace" type="s"/>
      <arg direction="in" name="key" type="s"/>
      <arg direction="out" name="value" type="v"/>
    </method>
    <signal name="SettingChanged">
      <arg name="namespace" type="s"/>
      <arg name="key" type="s"/>
      <arg name="value" type="v"/>
    </signal>
    <property name="version" type="u" access="read"/>
  </interface>
  <interface name="org.freedesktop.impl.portal.ScreenCast">
    <method name="CreateSession">
      <arg direction="in" name="handle" type="o"/>
      <arg direction="in" name="session_handle" type="o"/>
      <arg direction="in" name="app_id" type="s"/>
      <arg direction="in" name="options" type="a{sv}"/>
      <arg direction="out" name="response" type="u"/>
      <arg direction="out" name="results" type="a{sv}"/>
    </method>
    <method name="SelectSources">
      <arg direction="in" name="handle" type="o"/>
      <arg direction="in" name="session_handle" type="o"/>
      <arg direction="in" name="app_id" type="s"/>
      <arg direction="in" name="options" type="a{sv}"/>
      <arg direction="out" name="response" type="u"/>
      <arg direction="out" name="results" type="a{sv}"/>
    </method>
    <method name="Start">
      <arg direction="in" name="handle" type="o"/>
      <arg direction="in" name="session_handle" type="o"/>
      <arg direction="in" name="app_id" type="s"/>
      <arg direction="in" name="parent_window" type="s"/>
      <arg direction="in" name="options" type="a{sv}"/>
      <arg direction="out" name="response" type="u"/>
      <arg direction="out" name="results" type="a{sv}"/>
    </method>
    <property name="AvailableSourceTypes" type="u" access="read"/>
    <property name="AvailableCursorModes" type="u" access="read"/>
    <property name="version" type="u" access="read"/>
  </interface>
  <interface name="org.freedesktop.impl.portal.Inhibit">
    <method name="Inhibit">
      <arg direction="in" name="handle" type="o"/>
      <arg direction="in" name="app_id" type="s"/>
      <arg direction="in" name="window" type="s"/>
      <arg direction="in" name="flags" type="u"/>
      <arg direction="in" name="options" type="a{sv}"/>
    </method>
    <property name="version" type="u" access="read"/>
  </interface>
  <interface name="org.freedesktop.ScreenSaver">
    <method name="Inhibit">
      <arg direction="in" name="application_name" type="s"/>
      <arg direction="in" name="reason_for_inhibit" type="s"/>
      <arg direction="out" name="cookie" type="u"/>
    </method>
    <method name="UnInhibit">
      <arg direction="in" name="cookie" type="u"/>
    </method>
    <method name="GetActive">
      <arg direction="out" name="active" type="b"/>
    </method>
  </interface>
  <interface name="org.freedesktop.impl.portal.Screenshot">
    <method name="Screenshot">
      <arg direction="in" name="handle" type="o"/>
      <arg direction="in" name="app_id" type="s"/>
      <arg direction="in" name="parent_window" type="s"/>
      <arg direction="in" name="options" type="a{sv}"/>
      <arg direction="out" name="response" type="u"/>
      <arg direction="out" name="results" type="a{sv}"/>
    </method>
    <method name="PickColor">
      <arg direction="in" name="handle" type="o"/>
      <arg direction="in" name="app_id" type="s"/>
      <arg direction="in" name="parent_window" type="s"/>
      <arg direction="in" name="options" type="a{sv}"/>
      <arg direction="out" name="response" type="u"/>
      <arg direction="out" name="results" type="a{sv}"/>
    </method>
    <property name="version" type="u" access="read"/>
  </interface>
</node>
"#;

/// How the desktop looks just now, asked each time it is wanted.
pub type Look = Box<dyn Fn() -> Appearance + Send>;

/// The service on the bus.
///
/// Messages are answered on a thread of its own. The descriptor this is
/// becomes readable when the bus has gone, for an event loop to wait on.
#[derive(Debug)]
pub struct Portal {
    connection: Connection,
    /// Readable when the bus has gone.
    gone: OwnedFd,
    /// What has been asked that waits on the user, and what is readable
    /// when there is some.
    asked: Receiver<Asked>,
    asking: OwnedFd,
}

impl Portal {
    /// Joins the bus at `address`, or the session's own if there is none
    /// given, and takes the name the portals pass questions for omorfos
    /// to, and the screensaver's if nobody else has it. `look` says how the
    /// desktop looks whenever that is asked, and `keep` is told what
    /// programs ask the session to be kept from whenever that changes.
    ///
    /// # Errors
    ///
    /// Returns an error if the bus cannot be joined, or if another program
    /// already has the name.
    pub fn open(address: Option<&str>, look: Look, keep: Keep) -> Result<Self, Failure> {
        let builder = address.map_or_else(Builder::session, Builder::address);
        let connection = builder.and_then(Builder::build).map_err(Failure::from);
        let pipe = || pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).map_err(Failure::from);
        let pipes = pipe().and_then(|gone| pipe().map(|asking| (gone, asking)));
        let joined = connection.and_then(|connection| pipes.map(|pipes| (connection, pipes)));
        joined.and_then(|(connection, ((gone, going), (asking, ask)))| {
            // Listening before asking for the name, so that nothing sent
            // to the name is missed.
            let messages = MessageIterator::from(&connection);
            // A copy left over from a session before this one, which did
            // not stop when that one did, gives the name up to this one;
            // and this one gives it up in turn to the next.
            let flags = RequestNameFlags::DoNotQueue
                | RequestNameFlags::ReplaceExisting
                | RequestNameFlags::AllowReplacement;
            // Programs that leave the bus let go of what they held; and
            // the screensaver is answered for, unless another program
            // already does, which is then left to. Both before the portal's
            // own name, so that once that is seen to be taken all is ready.
            let left = "type='signal',sender='org.freedesktop.DBus',member='NameOwnerChanged'";
            let bus = "org.freedesktop.DBus";
            let watched = (Some(bus), "/org/freedesktop/DBus", Some(bus), "AddMatch");
            let (to, at, on, asked) = watched;
            let _ = connection.call_method(to, at, on, asked, &(left,));
            let _ = connection.request_name_with_flags(SCREENSAVER, flags);
            let named = connection.request_name_with_flags(NAME, flags);
            let taken = |error: zbus::Error| {
                let message = format!(
                    "cannot take the name the portals speak to omorfos by ({error}); another \
                     program has it and will not give it up"
                );
                Failure::from(message)
            };
            named.map_err(taken).map(|_| {
                let answering = connection.clone();
                let (sender, asked) = mpsc::channel();
                thread::spawn(move || {
                    let waiting = Waiting { sender, ask };
                    let mut sessions = Sessions::new();
                    let mut holds = Holds::new(keep);
                    let messages = messages.map_while(Result::ok);
                    for message in messages.take_while(|message| !lost(message)) {
                        if let Some(left) = left_the_bus(&message) {
                            holds.gone(&left);
                        }
                        let held = (&waiting, &mut sessions, &mut holds);
                        answer(&answering, &message, &look, held);
                    }
                    // If nobody is waiting any more, there is nobody to tell.
                    let _ = rustix::io::write(&going, &[1]);
                });
                Self {
                    connection,
                    gone,
                    asked,
                    asking,
                }
            })
        })
    }

    /// What has been asked since this was last called that waits on the
    /// user, in order.
    #[must_use]
    pub fn asked(&self) -> Vec<Asked> {
        // Emptied first, so that what is asked meanwhile wakes again.
        let mut drained = [0; 64];
        while rustix::io::read(&self.asking, &mut drained).is_ok_and(|read| read > 0) {}
        self.asked.try_iter().collect()
    }

    /// What becomes readable when something has been asked that waits on
    /// the user.
    #[must_use]
    pub fn asking(&self) -> BorrowedFd<'_> {
        self.asking.as_fd()
    }

    /// Answers a call that waited on the user, with a picture saved at
    /// `uri` if it was given. If the bus has gone it cannot be answered,
    /// which shows by what this is becoming readable.
    pub fn respond(&self, call: &Message, response: Response, uri: Option<String>) {
        let results: HashMap<&str, Value<'_>> = uri
            .into_iter()
            .map(|uri| ("uri", Value::from(uri)))
            .collect();
        let _ = self
            .connection
            .reply(&call.header(), &(response as u32, results));
    }

    /// Answers a call to pick a colour that waited on the user, with the
    /// colour picked, red, green and blue, if one was. The portal is told
    /// each as a part of one.
    pub fn respond_colour(&self, call: &Message, response: Response, colour: Option<[u8; 3]>) {
        let part = |channel: u8| f64::from(channel) / f64::from(u8::MAX);
        let results: HashMap<&str, Value<'_>> = colour
            .into_iter()
            .map(|[red, green, blue]| ("color", Value::from((part(red), part(green), part(blue)))))
            .collect();
        let _ = self
            .connection
            .reply(&call.header(), &(response as u32, results));
    }

    /// Answers a call to start sharing that waited on the user, with the
    /// stream that shares the screen at `at`, or a window if there is no
    /// place, `size` large.
    pub fn respond_shared(
        &self,
        call: &Message,
        node: u32,
        at: Option<(i32, i32)>,
        size: (i32, i32),
    ) {
        let results = screencast::streams(node, at, size);
        let _ = self
            .connection
            .reply(&call.header(), &(Response::Given as u32, results));
    }

    /// Tells the portal that a session has ended of itself, as when the
    /// screen it shared has gone.
    pub fn close(&self, session: &OwnedObjectPath) {
        let nobody = None::<&str>;
        let _ = self
            .connection
            .emit_signal(nobody, session.as_str(), SESSION, "Closed", &());
    }

    /// Tells applications what of the desktop's look has changed, if
    /// anything has. If the bus has gone they cannot be told, which shows
    /// by what this is becoming readable.
    pub fn changed(&self, from: Appearance, to: Appearance) {
        let was = settings(from).into_iter();
        for ((key, now), (_, before)) in settings(to).into_iter().zip(was) {
            if now != before {
                let nobody = None::<&str>;
                let body = (LOOKS, key, now);
                let _ =
                    self.connection
                        .emit_signal(nobody, PATH, SETTINGS, "SettingChanged", &body);
            }
        }
    }
}

impl AsFd for Portal {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.gone.as_fd()
    }
}

/// Where what is asked that waits on the user goes, and how the rest of
/// the program is woken to it.
pub(crate) struct Waiting {
    sender: Sender<Asked>,
    ask: OwnedFd,
}

impl Waiting {
    pub(crate) fn tell(&self, asked: Asked) {
        // If nobody is listening any more, there is nobody to wake either.
        let _ = self.sender.send(asked);
        let _ = rustix::io::write(&self.ask, &[1]);
    }
}

impl Asked {
    /// The object the request is known by.
    #[must_use]
    pub const fn request(&self) -> &OwnedObjectPath {
        match self {
            Self::Screenshot { request, .. }
            | Self::Pick { request, .. }
            | Self::Share { request, .. }
            | Self::Withdrawn(request)
            | Self::Ended(request) => request,
        }
    }

    /// The call to answer, if it is one that waits for an answer.
    #[must_use]
    pub const fn call(&self) -> Option<&Message> {
        match self {
            Self::Screenshot { call, .. } | Self::Pick { call, .. } | Self::Share { call, .. } => {
                Some(call)
            }
            Self::Withdrawn(_) | Self::Ended(_) => None,
        }
    }
}

/// What an application asking for a picture of the screen sends: the
/// object its request is known by, the application, the window it asks
/// from, and how it would like it done.
type Shoot = (OwnedObjectPath, String, String, HashMap<String, OwnedValue>);

/// Whether a message says that the name has gone to another program,
/// which is a copy of this one that has taken over.
fn lost(message: &Message) -> bool {
    let header = message.header();
    let member = header.member().map(ToString::to_string);
    let lost = header.message_type() == Type::Signal && member.as_deref() == Some("NameLost");
    lost && message
        .body()
        .deserialize::<String>()
        .is_ok_and(|name| name == NAME)
}

/// How the desktop looks, as the settings applications read: whether it
/// is dark, the colour that picks things out, and whether contrast is to
/// be higher than usual, which it is not.
fn settings(look: Appearance) -> [(&'static str, Value<'static>); 3] {
    let (red, green, blue) = look.accent;
    [
        ("color-scheme", Value::U32(look.scheme())),
        (
            "accent-color",
            Value::Structure(Structure::from((red, green, blue))),
        ),
        ("contrast", Value::U32(0)),
    ]
}

/// Whether a list of groups of settings asks for how the desktop looks:
/// one of them is that group, or stands for it by ending in `*`, or the
/// list is empty, which asks for everything.
fn asked(groups: &[String]) -> bool {
    let stands = |group: &String| {
        let start = group.strip_suffix('*');
        group == LOOKS || start.is_some_and(|start| LOOKS.starts_with(start))
    };
    groups.is_empty() || groups.iter().any(stands)
}

/// The program that a message says has left the bus, by its own name
/// there, if it says one has.
fn left_the_bus(message: &Message) -> Option<String> {
    let header = message.header();
    let member = header.member().map(ToString::to_string);
    let changed =
        header.message_type() == Type::Signal && member.as_deref() == Some("NameOwnerChanged");
    let said = changed.then(|| {
        message
            .body()
            .deserialize::<(String, String, String)>()
            .ok()
    });
    let (name, _, now) = said.flatten()?;
    (name.starts_with(':') && now.is_empty()).then_some(name)
}

/// Answers one message.
fn answer(
    connection: &Connection,
    message: &Message,
    look: &Look,
    (waiting, sessions, holds): (&Waiting, &mut Sessions, &mut Holds),
) {
    let header = message.header();
    // Replies and signals pass by on the same connection.
    if header.message_type() != Type::MethodCall {
        return;
    }
    let interface = header.interface().map(ToString::to_string);
    let member = header.member().map(ToString::to_string);
    let called = (interface.as_deref().unwrap_or(SETTINGS), member.as_deref());
    let kept = inhibit::answer(connection, message, called, holds);
    let sent = kept.or_else(|| screencast::answer(connection, message, called, sessions, waiting));
    if let Some(sent) = sent {
        // As below.
        let _ = sent;
        return;
    }
    let body = message.body();
    let refuse = |name: &str, why: &str| connection.reply_error(&header, name, &why);
    let malformed = || {
        let name = "org.freedesktop.DBus.Error.InvalidArgs";
        refuse(name, "the arguments are not those of this method")
    };
    let sent = match called {
        (SETTINGS, Some("ReadAll")) => match body.deserialize::<Vec<String>>() {
            Ok(groups) => {
                let looks = asked(&groups).then(|| (LOOKS, HashMap::from(settings(look()))));
                let all: HashMap<&str, HashMap<&str, Value<'_>>> = looks.into_iter().collect();
                connection.reply(&header, &all)
            }
            Err(_) => malformed(),
        },
        (SETTINGS, Some("Read")) => match body.deserialize::<(String, String)>() {
            Ok((group, key)) => {
                let mut settings = settings(look()).into_iter();
                let setting = settings.find(|(name, _)| group == LOOKS && *name == key);
                if let Some((_, value)) = setting {
                    connection.reply(&header, &value)
                } else {
                    let name = "org.freedesktop.portal.Error.NotFound";
                    refuse(name, "there is no such setting")
                }
            }
            Err(_) => malformed(),
        },
        (SCREENSHOT, Some("Screenshot")) => match body.deserialize::<Shoot>() {
            Ok((request, app, _, _)) => {
                // Answered once the user has said.
                waiting.tell(Asked::Screenshot {
                    call: message.clone(),
                    request,
                    app,
                });
                Ok(())
            }
            Err(error) => {
                let said = format!(
                    "xdg-desktop-portal-omorfos: was asked for a picture in a way it does not \
                     understand: {error}"
                );
                let _ = writeln!(std::io::stderr(), "{said}");
                malformed()
            }
        },
        (SCREENSHOT, Some("PickColor")) => match body.deserialize::<Shoot>() {
            Ok((request, app, _, _)) => {
                // Answered once the user has picked.
                waiting.tell(Asked::Pick {
                    call: message.clone(),
                    request,
                    app,
                });
                Ok(())
            }
            Err(_) => malformed(),
        },
        (REQUEST, Some("Close")) => {
            let request = header
                .path()
                .map(|path| OwnedObjectPath::from(path.to_owned()));
            waiting.tell(Asked::Withdrawn(request.unwrap_or_default()));
            connection.reply(&header, &())
        }
        ("org.freedesktop.DBus.Properties", Some(member)) => property(connection, message, member),
        ("org.freedesktop.DBus.Introspectable", Some("Introspect")) => {
            connection.reply(&header, &INTROSPECTION)
        }
        ("org.freedesktop.DBus.Peer", Some("Ping")) => connection.reply(&header, &()),
        (interface, member) => {
            // Said in the session's log, so that a portal asking for what
            // is not answered can be seen.
            let member = member.unwrap_or_default();
            let said = format!(
                "xdg-desktop-portal-omorfos: was asked {interface}.{member}, which it does not answer"
            );
            let _ = writeln!(std::io::stderr(), "{said}");
            let name = "org.freedesktop.DBus.Error.UnknownMethod";
            refuse(name, "there is no such method here")
        }
    };
    // An answer that cannot be sent means the bus is going, which shows
    // when the next message is waited for.
    let _ = sent;
}

/// Answers a question about what can be read of an interface.
fn property(connection: &Connection, message: &Message, member: &str) -> zbus::Result<()> {
    let header = message.header();
    let body = message.body();
    let refuse = |name: &str, why: &str| connection.reply_error(&header, name, &why);
    // Nothing can be set, so it is reading one or reading them all.
    if member != "Get" {
        let interface = body.deserialize::<String>().unwrap_or_default();
        let known = PROPERTIES.iter().filter(|(of, ..)| *of == interface);
        let all: HashMap<&str, Value<'_>> = known
            .map(|(_, name, value)| (*name, Value::U32(*value)))
            .collect();
        return connection.reply(&header, &all);
    }
    let Ok((interface, property)) = body.deserialize::<(String, String)>() else {
        let name = "org.freedesktop.DBus.Error.InvalidArgs";
        return refuse(name, "the arguments are not those of this method");
    };
    let mut known = PROPERTIES.iter();
    let known = known.find(|(of, name, _)| *of == interface && *name == property);
    if let Some((.., value)) = known {
        connection.reply(&header, &Value::U32(*value))
    } else {
        let name = "org.freedesktop.DBus.Error.UnknownProperty";
        refuse(name, "there is no such property")
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::io::{BufRead, BufReader};
    use std::process::{Child, Command, Stdio};

    use zbus::zvariant::{ObjectPath, OwnedValue};

    use super::*;

    /// A bus of the test's own, gone when the test is.
    pub(crate) struct Daemon {
        child: Child,
        pub(crate) address: String,
        _directory: tempfile::TempDir,
    }

    impl Daemon {
        pub(crate) fn start() -> Self {
            let directory = tempfile::TempDir::new().unwrap();
            let listen = format!("--address=unix:path={}/bus", directory.path().display());
            // Killed by the kernel once the thread that started it ends, so
            // that a test killed before it could stop it leaves none running.
            let mut child = Command::new("setpriv")
                .args(["--pdeathsig", "KILL", "--", "dbus-daemon", "--session"])
                .args(["--nofork", "--print-address=1", &listen])
                .stdout(Stdio::piped())
                .spawn()
                .expect("setpriv and dbus-daemon are needed to test the portals");
            let mut address = String::new();
            let stdout = child.stdout.take().unwrap();
            BufReader::new(stdout).read_line(&mut address).unwrap();
            Self {
                child,
                address: address.trim().to_owned(),
                _directory: directory,
            }
        }

        /// A program on the bus that is not the service, as the portal
        /// that passes questions on is.
        pub(crate) fn peer(&self) -> Connection {
            Builder::address(self.address.as_str())
                .unwrap()
                .build()
                .unwrap()
        }
    }

    impl Drop for Daemon {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    /// What the service is to tell of what programs hold, and where it is
    /// told.
    pub(crate) fn keeper() -> (Keep, Receiver<(bool, bool)>) {
        let (tell, told) = mpsc::channel();
        let keep: Keep = Box::new(move |idle, sleep| {
            let _ = tell.send((idle, sleep));
        });
        (keep, told)
    }

    /// Everything about how the desktop looks, as a peer is told it.
    pub(crate) type Told = HashMap<String, HashMap<String, OwnedValue>>;

    /// Asks the service for the settings in some groups.
    pub(crate) fn read_all(peer: &Connection, groups: &[&str]) -> Told {
        let reply = peer
            .call_method(Some(NAME), PATH, Some(SETTINGS), "ReadAll", &(groups,))
            .unwrap();
        reply.body().deserialize().unwrap()
    }

    /// Whether the desktop is dark, as a peer is told: 1 for dark, 2 for
    /// light.
    pub(crate) fn scheme(told: &Told) -> u32 {
        let value = &told[LOOKS]["color-scheme"];
        value.downcast_ref::<u32>().unwrap()
    }

    /// Has a peer hear the signals of the service from now on.
    pub(crate) fn listen_for_changes(peer: &Connection) -> MessageIterator {
        let rule = format!("type='signal',interface='{SETTINGS}'");
        let bus = "org.freedesktop.DBus";
        peer.call_method(
            Some(bus),
            "/org/freedesktop/DBus",
            Some(bus),
            "AddMatch",
            &(rule,),
        )
        .unwrap();
        MessageIterator::from(peer)
    }

    /// The next setting the service says has changed, and what to.
    pub(crate) fn next_change(heard: &mut MessageIterator) -> (String, OwnedValue) {
        let signals = heard.map_while(Result::ok);
        let mut changes = signals.filter(|message| {
            let member = message.header().member().map(ToString::to_string);
            member.as_deref() == Some("SettingChanged")
        });
        let change = changes.next().unwrap();
        let (group, key, value): (String, String, OwnedValue) =
            change.body().deserialize().unwrap();
        assert_eq!(group, LOOKS);
        (key, value)
    }

    const DARK: Appearance = Appearance {
        dark: true,
        accent: (0.25, 0.5, 1.0),
    };

    /// The service on a bus, saying the desktop looks as given.
    fn service(daemon: &Daemon, look: Appearance) -> Portal {
        Portal::open(Some(&daemon.address), Box::new(move || look), keeper().0).unwrap()
    }

    #[test]
    fn an_application_is_told_how_the_desktop_looks() {
        let daemon = Daemon::start();
        let _portal = service(&daemon, DARK);
        let peer = daemon.peer();

        let told = read_all(&peer, &[LOOKS]);
        assert_eq!(scheme(&told), 1);
        let accent = &told[LOOKS]["accent-color"];
        let accent: (f64, f64, f64) = accent.try_clone().unwrap().try_into().unwrap();
        assert_eq!(accent, (0.25, 0.5, 1.0));
        assert_eq!(told[LOOKS]["contrast"].downcast_ref::<u32>().unwrap(), 0);

        // One setting by itself is the same.
        let reply = peer
            .call_method(
                Some(NAME),
                PATH,
                Some(SETTINGS),
                "Read",
                &(LOOKS, "color-scheme"),
            )
            .unwrap();
        let value: OwnedValue = reply.body().deserialize().unwrap();
        assert_eq!(value.downcast_ref::<u32>().unwrap(), 1);
    }

    #[test]
    fn what_is_asked_for_by_group_is_what_is_given() {
        let daemon = Daemon::start();
        let _portal = service(&daemon, DARK);
        let peer = daemon.peer();
        // Nothing named asks for everything, and a group can be named
        // by how it begins.
        assert!(read_all(&peer, &[]).contains_key(LOOKS));
        assert!(read_all(&peer, &["org.freedesktop.*"]).contains_key(LOOKS));
        assert!(read_all(&peer, &["org.gnome.desktop.interface", LOOKS]).contains_key(LOOKS));
        assert_eq!(read_all(&peer, &["org.gnome.*"]), Told::new());
        assert_eq!(
            read_all(&peer, &["org.gnome.desktop.interface"]),
            Told::new()
        );
    }

    #[test]
    fn what_does_not_exist_or_is_asked_wrongly_is_answered_with_an_error() {
        let daemon = Daemon::start();
        let _portal = service(&daemon, DARK);
        let peer = daemon.peer();
        let named = |result: zbus::Result<Message>| result.unwrap_err().to_string();
        let settings = Some(SETTINGS);
        let said = named(peer.call_method(Some(NAME), PATH, settings, "Read", &(LOOKS, "font")));
        assert!(said.contains("NotFound"), "{said}");
        let said =
            named(peer.call_method(Some(NAME), PATH, settings, "Read", &("other", "contrast")));
        assert!(said.contains("NotFound"), "{said}");
        let said = named(peer.call_method(Some(NAME), PATH, settings, "Read", &(7_u32,)));
        assert!(said.contains("InvalidArgs"), "{said}");
        let said = named(peer.call_method(Some(NAME), PATH, settings, "ReadAll", &("all",)));
        assert!(said.contains("InvalidArgs"), "{said}");
        let said = named(peer.call_method(Some(NAME), PATH, settings, "Write", &()));
        assert!(said.contains("UnknownMethod"), "{said}");
        let shots = Some(SCREENSHOT);
        let said = named(peer.call_method(Some(NAME), PATH, shots, "PickColor", &(7_u32,)));
        assert!(
            said.contains("InvalidArgs"),
            "a colour asked for wrongly: {said}"
        );

        let properties = Some("org.freedesktop.DBus.Properties");
        let said =
            named(peer.call_method(Some(NAME), PATH, properties, "Get", &(SETTINGS, "name")));
        assert!(said.contains("UnknownProperty"), "{said}");
        let said = named(peer.call_method(Some(NAME), PATH, properties, "Get", &(1_u32,)));
        assert!(said.contains("InvalidArgs"), "{said}");
    }

    #[test]
    fn it_says_which_version_it_speaks_and_how_it_is_used() {
        let daemon = Daemon::start();
        let _portal = service(&daemon, DARK);
        let peer = daemon.peer();
        let properties = Some("org.freedesktop.DBus.Properties");
        let reply = peer
            .call_method(Some(NAME), PATH, properties, "Get", &(SETTINGS, "version"))
            .unwrap();
        let version: OwnedValue = reply.body().deserialize().unwrap();
        assert_eq!(version.downcast_ref::<u32>().unwrap(), 1);
        let reply = peer
            .call_method(Some(NAME), PATH, properties, "GetAll", &(SETTINGS,))
            .unwrap();
        let all: HashMap<String, OwnedValue> = reply.body().deserialize().unwrap();
        assert_eq!(all["version"].downcast_ref::<u32>().unwrap(), 1);

        let how = Some("org.freedesktop.DBus.Introspectable");
        let reply = peer
            .call_method(Some(NAME), PATH, how, "Introspect", &())
            .unwrap();
        let said: String = reply.body().deserialize().unwrap();
        assert!(said.contains("SettingChanged"), "{said}");
        // A method called without saying of which interface is taken to
        // be of this one.
        let none = None::<&str>;
        let reply = peer.call_method(Some(NAME), PATH, none, "ReadAll", &(vec![LOOKS],));
        assert!(reply.is_ok());
        let alive = Some("org.freedesktop.DBus.Peer");
        assert!(
            peer.call_method(Some(NAME), PATH, alive, "Ping", &())
                .is_ok()
        );
    }

    #[test]
    fn the_version_of_each_interface_is_its_own() {
        let daemon = Daemon::start();
        let _portal = service(&daemon, DARK);
        let peer = daemon.peer();
        let properties = Some("org.freedesktop.DBus.Properties");
        let get = |interface: &str| {
            let reply =
                peer.call_method(Some(NAME), PATH, properties, "Get", &(interface, "version"));
            let version: OwnedValue = reply.unwrap().body().deserialize().unwrap();
            version.downcast_ref::<u32>().unwrap()
        };
        assert_eq!((get(SETTINGS), get(SCREENSHOT)), (1, 2));
        let all = |interface: &str| {
            let reply = peer.call_method(Some(NAME), PATH, properties, "GetAll", &(interface,));
            reply
                .unwrap()
                .body()
                .deserialize::<HashMap<String, OwnedValue>>()
                .unwrap()
        };
        assert_eq!(all(SCREENSHOT)["version"].downcast_ref::<u32>().unwrap(), 2);
        assert!(all("org.example.Other").is_empty());
        let said = peer
            .call_method(
                Some(NAME),
                PATH,
                properties,
                "Get",
                &("org.example.Other", "version"),
            )
            .unwrap_err()
            .to_string();
        assert!(said.contains("UnknownProperty"), "{said}");
    }

    /// Waits for the service to have been asked something that waits on
    /// the user, and returns it.
    pub(crate) fn heard(portal: &Portal) -> Vec<Asked> {
        use rustix::event::{PollFd, PollFlags, poll};
        let fd = portal.asking();
        let mut asking = [PollFd::new(&fd, PollFlags::IN)];
        poll(&mut asking, None).unwrap();
        portal.asked()
    }

    /// Asks for a picture of the screen, as the portal does for an
    /// application, and waits for the answer on a thread of its own.
    pub(crate) fn ask_for_picture(
        peer: &Connection,
        request: &'static str,
    ) -> thread::JoinHandle<(u32, HashMap<String, OwnedValue>)> {
        ask_for(peer, request, "Screenshot")
    }

    /// The same, by `method` of the Screenshot portal.
    pub(crate) fn ask_for(
        peer: &Connection,
        request: &'static str,
        method: &'static str,
    ) -> thread::JoinHandle<(u32, HashMap<String, OwnedValue>)> {
        let peer = peer.clone();
        thread::spawn(move || {
            let path = ObjectPath::try_from(request).unwrap();
            let options: HashMap<&str, Value<'_>> = HashMap::new();
            let body = (path, "org.example.App", "", options);
            let reply = peer.call_method(Some(NAME), PATH, Some(SCREENSHOT), method, &body);
            reply.unwrap().body().deserialize().unwrap()
        })
    }

    #[test]
    fn a_colour_asked_for_waits_on_the_user_and_is_answered_with_it() {
        let daemon = Daemon::start();
        let portal = service(&daemon, DARK);
        let peer = daemon.peer();
        let request = "/org/freedesktop/portal/desktop/request/1/colour";
        let asking = ask_for(&peer, request, "PickColor");
        let asked = heard(&portal).pop().unwrap();
        assert_eq!(asked.request().as_str(), request);
        assert!(matches!(&asked, Asked::Pick { app, .. } if app == "org.example.App"));
        portal.respond_colour(asked.call().unwrap(), Response::Given, Some([255, 51, 0]));
        let (response, results) = asking.join().unwrap();
        let colour = <(f64, f64, f64)>::try_from(results["color"].clone()).unwrap();
        assert_eq!((response, colour), (0, (1.0, 0.2, 0.0)));
    }

    #[test]
    fn a_picture_asked_for_waits_on_the_user_and_is_answered_when_they_have() {
        let daemon = Daemon::start();
        let portal = service(&daemon, DARK);
        let peer = daemon.peer();
        let asking = ask_for_picture(&peer, "/org/freedesktop/portal/desktop/request/1/a");

        let asked = heard(&portal).pop().unwrap();
        let call = asked.call().unwrap();
        let request = asked.request();
        assert_eq!(
            request.as_str(),
            "/org/freedesktop/portal/desktop/request/1/a"
        );
        assert!(matches!(&asked, Asked::Screenshot { app, .. } if app == "org.example.App"));
        assert!(!asking.is_finished(), "it waits");
        portal.respond(
            call,
            Response::Given,
            Some("file:///pictures/one.png".to_owned()),
        );
        let (response, results) = asking.join().unwrap();
        assert_eq!(response, 0);
        assert_eq!(
            results["uri"].downcast_ref::<&str>().unwrap(),
            "file:///pictures/one.png"
        );

        // One refused is answered so, with nothing more.
        let asking = ask_for_picture(&peer, "/org/freedesktop/portal/desktop/request/1/b");
        let asked = heard(&portal).pop().unwrap();
        portal.respond(asked.call().unwrap(), Response::Refused, None);
        let (response, results) = asking.join().unwrap();
        assert_eq!((response, results.len()), (1, 0));
    }

    #[test]
    fn the_portal_can_say_it_no_longer_wants_what_it_asked_for() {
        let daemon = Daemon::start();
        let portal = service(&daemon, DARK);
        let peer = daemon.peer();
        let request = "/org/freedesktop/portal/desktop/request/1/c";
        peer.call_method(Some(NAME), request, Some(REQUEST), "Close", &())
            .unwrap();
        let withdrawn = heard(&portal).pop().unwrap();
        assert_eq!(withdrawn.request().as_str(), request);
        assert!(
            withdrawn.call().is_none(),
            "nothing waits for an answer to it"
        );

        let said = peer
            .call_method(
                Some(NAME),
                PATH,
                Some(SCREENSHOT),
                "Screenshot",
                &("not a path",),
            )
            .unwrap_err()
            .to_string();
        assert!(said.contains("InvalidArgs"), "{said}");
    }

    #[test]
    fn a_screen_to_share_is_asked_about_and_its_session_ended() {
        let daemon = Daemon::start();
        let portal = service(&daemon, DARK);
        let peer = daemon.peer();
        let none: HashMap<&str, Value<'_>> = HashMap::new();
        let request = ObjectPath::try_from("/org/freedesktop/portal/desktop/request/1/s").unwrap();
        let session = ObjectPath::try_from("/org/freedesktop/portal/desktop/session/1/s").unwrap();
        let made = (
            request.clone(),
            session.clone(),
            "org.example.Call",
            none.clone(),
        );
        peer.call_method(Some(NAME), PATH, Some(SCREENCAST), "CreateSession", &made)
            .unwrap();
        let begun = (request, session.clone(), "org.example.Call", "", none);
        let starting = peer.clone();
        let asking = thread::spawn(move || {
            starting
                .call_method(Some(NAME), PATH, Some(SCREENCAST), "Start", &begun)
                .unwrap();
        });
        let asked = heard(&portal).pop().unwrap();
        assert_eq!(
            asked.request().as_str(),
            "/org/freedesktop/portal/desktop/request/1/s"
        );
        portal.respond_shared(asked.call().unwrap(), 7, Some((0, 0)), (640, 480));
        asking.join().unwrap();

        peer.call_method(Some(NAME), session.as_str(), Some(SESSION), "Close", &())
            .unwrap();
        let ended = heard(&portal).pop().unwrap();
        assert_eq!(ended.request().as_str(), session.as_str());
        assert!(ended.call().is_none());
    }

    #[test]
    fn applications_are_told_what_has_changed_and_nothing_that_has_not() {
        let daemon = Daemon::start();
        let portal = service(&daemon, DARK);
        let peer = daemon.peer();
        let mut heard = listen_for_changes(&peer);

        let light = Appearance {
            dark: false,
            ..DARK
        };
        // Nothing changed, nothing said; then light, which is all that is.
        portal.changed(DARK, DARK);
        portal.changed(DARK, light);
        let (key, value) = next_change(&mut heard);
        assert_eq!(
            (key.as_str(), value.downcast_ref::<u32>().unwrap()),
            ("color-scheme", 2)
        );

        let red = Appearance {
            accent: (1.0, 0.0, 0.0),
            ..light
        };
        portal.changed(light, red);
        let (key, value) = next_change(&mut heard);
        assert_eq!(key, "accent-color");
        let accent: (f64, f64, f64) = value.try_into().unwrap();
        assert_eq!(accent, (1.0, 0.0, 0.0));
    }

    #[test]
    fn a_copy_that_is_left_over_gives_the_name_up_to_a_new_one() {
        use rustix::event::{PollFd, PollFlags, poll};

        let daemon = Daemon::start();
        let first = service(&daemon, DARK);
        let light = Appearance {
            dark: false,
            ..DARK
        };
        let _second = service(&daemon, light);
        // The first has stopped answering, which shows as it does when
        // the bus has gone, and the second answers in its place.
        let mut gone = [PollFd::new(&first, PollFlags::IN)];
        poll(&mut gone, None).unwrap();
        assert_eq!(scheme(&read_all(&daemon.peer(), &[LOOKS])), 2);

        // A program that holds the name and will not give it up keeps it.
        let other = Daemon::start();
        let holder = other.peer();
        holder
            .request_name_with_flags(NAME, RequestNameFlags::DoNotQueue.into())
            .unwrap();
        let refused = Portal::open(Some(&other.address), Box::new(|| DARK), keeper().0);
        let said = refused.unwrap_err().to_string();
        assert!(said.contains("will not give it up"), "{said}");
    }

    #[test]
    fn a_bus_that_is_not_there_cannot_be_joined() {
        let nowhere = Portal::open(
            Some("unix:path=/nowhere/bus"),
            Box::new(|| DARK),
            keeper().0,
        );
        assert!(nowhere.is_err());
    }
}
