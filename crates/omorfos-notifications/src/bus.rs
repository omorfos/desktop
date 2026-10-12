//! Hearing of notifications: the service that programs send them to.
//!
//! Programs send notifications over D-Bus, to whichever program has taken
//! the name `org.freedesktop.Notifications`, by a short list of methods
//! that the desktop notification specification sets out. This takes the
//! name, answers those methods, and hands what arrives to the rest of the
//! program.
//!
//! Messages are read one at a time and answered here, by name, with no
//! machinery in between, so that every answer there is can be seen and
//! tested. They come from other programs: one that does not have the shape
//! it should is answered with an error and otherwise passed over.

use std::collections::HashMap;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use omorfos_ui::Image;
use rustix::pipe::{PipeFlags, pipe_with};
use zbus::blocking::connection::Builder;
use zbus::blocking::{Connection, MessageIterator};
use zbus::fdo::RequestNameFlags;
use zbus::message::{Message, Type};
use zbus::zvariant::OwnedValue;

use crate::{Failure, Note, Picture, Reason};

/// The name programs send notifications to, which is also the interface
/// they send them by.
const NAME: &str = "org.freedesktop.Notifications";

/// The object they send them to.
const PATH: &str = "/org/freedesktop/Notifications";

/// The most pixels a picture sent with a notification may have along a
/// side.
const LARGEST_SIDE: i32 = 1024;

/// What the service can do, as programs are told when they ask: a body
/// besides the title, a picture that does not move, and telling a program
/// that its notification was clicked.
const CAPABILITIES: [&str; 3] = ["body", "icon-static", "actions"];

/// The action a click on a notification is, by the name the specification
/// gives it.
const DEFAULT_ACTION: &str = "default";

/// What the service says of itself to a program that asks how to use it.
const INTROSPECTION: &str = r#"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node>
  <interface name="org.freedesktop.Notifications">
    <method name="Notify">
      <arg direction="in" name="app_name" type="s"/>
      <arg direction="in" name="replaces_id" type="u"/>
      <arg direction="in" name="app_icon" type="s"/>
      <arg direction="in" name="summary" type="s"/>
      <arg direction="in" name="body" type="s"/>
      <arg direction="in" name="actions" type="as"/>
      <arg direction="in" name="hints" type="a{sv}"/>
      <arg direction="in" name="expire_timeout" type="i"/>
      <arg direction="out" name="id" type="u"/>
    </method>
    <method name="CloseNotification">
      <arg direction="in" name="id" type="u"/>
    </method>
    <method name="GetCapabilities">
      <arg direction="out" name="capabilities" type="as"/>
    </method>
    <method name="GetServerInformation">
      <arg direction="out" name="name" type="s"/>
      <arg direction="out" name="vendor" type="s"/>
      <arg direction="out" name="version" type="s"/>
      <arg direction="out" name="spec_version" type="s"/>
    </method>
    <signal name="NotificationClosed">
      <arg name="id" type="u"/>
      <arg name="reason" type="u"/>
    </signal>
    <signal name="ActionInvoked">
      <arg name="id" type="u"/>
      <arg name="action_key" type="s"/>
    </signal>
  </interface>
</node>
"#;

/// Something a program asked for, or that happened to the bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Show this notification.
    Show(Note),
    /// Take the notification of this number away.
    Close(u32),
    /// The bus has gone. Nothing more will come.
    Lost,
}

/// The arguments of `Notify`, in the order the specification gives them:
/// the program's name, the number of a notification to replace, an icon, a
/// title, a body, actions, hints and how long to stay.
type Sent = (
    String,
    u32,
    String,
    String,
    String,
    Vec<String>,
    HashMap<String, OwnedValue>,
    i32,
);

/// A picture as the specification sends one: its width and height, how
/// many bytes a row takes, whether it has an opacity, how many bits a
/// sample has, how many samples a pixel has, and the bytes.
type Raw = (i32, i32, i32, bool, i32, i32, Vec<u8>);

/// The service on the bus.
///
/// Messages are answered on a thread of its own. What they ask for is
/// queued, and the descriptor this is becomes readable when there is
/// something in the queue, for an event loop to wait on.
#[derive(Debug)]
pub struct Bus {
    connection: Connection,
    events: Receiver<Event>,
    /// Readable when there are events to take.
    wake: OwnedFd,
}

impl Bus {
    /// Joins the bus at `address`, or the session's own if there is none
    /// given, and takes the name notifications are sent to.
    ///
    /// # Errors
    ///
    /// Returns an error if the bus cannot be joined, or if another program
    /// already has the name: only one can show notifications.
    pub fn open(address: Option<&str>) -> Result<Self, Failure> {
        let builder = address.map_or_else(Builder::session, Builder::address);
        let connection = builder.and_then(Builder::build).map_err(Failure::from);
        let pipe = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).map_err(Failure::from);
        let joined = connection.and_then(|connection| pipe.map(|pipe| (connection, pipe)));
        joined.and_then(|(connection, (wake, waker))| {
            // Listening before asking for the name, so that nothing sent
            // to the name is missed.
            let messages = MessageIterator::from(&connection);
            let flags = RequestNameFlags::DoNotQueue.into();
            let named = connection.request_name_with_flags(NAME, flags);
            // Asked not to wait its turn, it is refused the name outright
            // when another program has it.
            let taken = |error: zbus::Error| {
                let message = format!(
                    "cannot take the name notifications are sent to ({error}); another \
                     program is probably already showing notifications"
                );
                Failure::from(message)
            };
            named.map_err(taken).map(|_| {
                let (sender, events) = mpsc::channel();
                let answering = connection.clone();
                thread::spawn(move || listen(messages, &answering, &sender, &waker));
                Self {
                    connection,
                    events,
                    wake,
                }
            })
        })
    }

    /// What has been asked for since this was last called, in order.
    #[must_use]
    pub fn events(&self) -> Vec<Event> {
        // Emptied first, so that an event queued meanwhile wakes again.
        let mut drained = [0; 64];
        while rustix::io::read(&self.wake, &mut drained).is_ok_and(|read| read > 0) {}
        self.events.try_iter().collect()
    }

    /// Tells every program that a notification was clicked, for the one
    /// that sent it to do what it does then.
    ///
    /// # Errors
    ///
    /// Returns an error if the bus has gone.
    pub fn clicked(&self, id: u32) -> Result<(), Failure> {
        let body = (id, DEFAULT_ACTION);
        let nobody = None::<&str>;
        let told = self
            .connection
            .emit_signal(nobody, PATH, NAME, "ActionInvoked", &body);
        told.map_err(Failure::from)
    }

    /// Tells every program that a notification is no longer showing, and
    /// why.
    ///
    /// # Errors
    ///
    /// Returns an error if the bus has gone.
    pub fn closed(&self, id: u32, reason: Reason) -> Result<(), Failure> {
        let body = (id, reason as u32);
        let nobody = None::<&str>;
        let told = self
            .connection
            .emit_signal(nobody, PATH, NAME, "NotificationClosed", &body);
        told.map_err(Failure::from)
    }
}

impl AsFd for Bus {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.wake.as_fd()
    }
}

/// Answers messages until the bus goes.
fn listen(
    messages: MessageIterator,
    connection: &Connection,
    events: &Sender<Event>,
    wake: &OwnedFd,
) {
    let mut last = 0;
    let tell = |event: Event| {
        // If nobody is listening any more, there is nobody to wake either.
        let _ = events.send(event);
        let _ = rustix::io::write(wake, &[1]);
    };
    for message in messages.map_while(Result::ok) {
        answer(connection, &message, &mut last)
            .into_iter()
            .for_each(&tell);
    }
    tell(Event::Lost);
}

/// Answers one message, and returns what it asks of the rest of the
/// program, if anything. `last` is the number the last new notification
/// was given.
fn answer(connection: &Connection, message: &Message, last: &mut u32) -> Option<Event> {
    let header = message.header();
    // Replies and signals pass by on the same connection.
    if header.message_type() != Type::MethodCall {
        return None;
    }
    let interface = header.interface().map(ToString::to_string);
    let member = header.member().map(ToString::to_string);
    let called = (interface.as_deref().unwrap_or(NAME), member.as_deref());
    let body = message.body();
    let mut event = None;
    let malformed = || {
        let name = "org.freedesktop.DBus.Error.InvalidArgs";
        connection.reply_error(&header, name, &"the arguments are not those of this method")
    };
    let sent = match called {
        (NAME, Some("Notify")) => match body.deserialize::<Sent>() {
            Ok((_, replaces, icon, title, body, actions, hints, stays)) => {
                // A notification that replaces one keeps its number.
                *last = last.wrapping_add(u32::from(replaces == 0)).max(1);
                let id = if replaces == 0 { *last } else { replaces };
                event = Some(Event::Show(Note {
                    id,
                    title,
                    body,
                    picture: picture(&hints, &icon),
                    urgent: urgency(&hints) == Some(2),
                    stays,
                    // Actions come as a name and then what to call it.
                    acts: actions.iter().step_by(2).any(|name| name == DEFAULT_ACTION),
                }));
                connection.reply(&header, &id)
            }
            Err(_) => malformed(),
        },
        (NAME, Some("CloseNotification")) => match body.deserialize::<u32>() {
            Ok(id) => {
                event = Some(Event::Close(id));
                connection.reply(&header, &())
            }
            Err(_) => malformed(),
        },
        (NAME, Some("GetCapabilities")) => connection.reply(&header, &CAPABILITIES.to_vec()),
        (NAME, Some("GetServerInformation")) => {
            let version = env!("CARGO_PKG_VERSION");
            connection.reply(&header, &("omorfos", "omorfos", version, "1.2"))
        }
        ("org.freedesktop.DBus.Introspectable", Some("Introspect")) => {
            connection.reply(&header, &INTROSPECTION)
        }
        ("org.freedesktop.DBus.Peer", Some("Ping")) => connection.reply(&header, &()),
        _ => {
            let name = "org.freedesktop.DBus.Error.UnknownMethod";
            connection.reply_error(&header, name, &"there is no such method here")
        }
    };
    // An answer that cannot be sent means the bus is going, which shows
    // when the next message is waited for.
    let _ = sent;
    event
}

/// How pressing a notification is: 0 for low, 1 for usual, 2 for critical.
fn urgency(hints: &HashMap<String, OwnedValue>) -> Option<u8> {
    let hint = hints.get("urgency");
    hint.and_then(|value| value.downcast_ref::<u8>().ok())
}

/// The picture a notification came with: the pixels themselves if it sent
/// them, or else the picture it gave the path of, or else its program's
/// icon.
fn picture(hints: &HashMap<String, OwnedValue>, icon: &str) -> Picture {
    let hint = |names: [&str; 2]| names.into_iter().find_map(|name| hints.get(name));
    let raw = hint(["image-data", "image_data"]).or_else(|| hints.get("icon_data"));
    let raw = raw.and_then(|value| value.try_clone().ok());
    let raw = raw.and_then(|value| Raw::try_from(value).ok());
    let path = hint(["image-path", "image_path"]);
    let path = path.and_then(|value| value.downcast_ref::<&str>().ok());
    let named = path.into_iter().chain([icon]).find(|name| !name.is_empty());
    // A path may be given as an address of a file.
    let named = named.map(|name| name.strip_prefix("file://").unwrap_or(name).to_owned());
    let pixels = raw.and_then(pixels).map(Picture::Pixels);
    pixels
        .or_else(|| named.map(Picture::Named))
        .unwrap_or_default()
}

/// A picture from the bytes a program sent, if they are a picture: eight
/// bits a sample, three or four samples a pixel, of a size that makes
/// sense, and as many bytes as that takes.
fn pixels((width, height, stride, opacity, bits, samples, bytes): Raw) -> Option<Image> {
    let sane = (1..=LARGEST_SIDE).contains(&width) && (1..=LARGEST_SIDE).contains(&height);
    let known = bits == 8 && samples == if opacity { 4 } else { 3 };
    let each = usize::try_from(samples).ok().filter(|_| sane && known);
    let stride = usize::try_from(stride).ok();
    let sides = u32::try_from(width).ok().zip(u32::try_from(height).ok());
    each.zip(stride)
        .zip(sides)
        .and_then(|((each, stride), (width, height))| {
            let across = usize::try_from(width).unwrap_or(usize::MAX);
            let rows = bytes
                .chunks(stride.max(1))
                .take(usize::try_from(height).unwrap_or(0));
            let rgba = rows.flat_map(|row| {
                let pixels = row.chunks_exact(each).take(across);
                pixels.flat_map(|pixel| {
                    let sample = |place: usize| pixel.get(place).copied();
                    [
                        sample(0),
                        sample(1),
                        sample(2),
                        Some(sample(3).unwrap_or(u8::MAX)),
                    ]
                })
            });
            // A row that was cut short leaves too few bytes, which is no
            // picture.
            let rgba: Option<Vec<u8>> = rgba.collect();
            rgba.and_then(|rgba| Image::new(width, height, rgba))
        })
}

#[cfg(test)]
pub(crate) mod tests {
    use std::io::{BufRead, BufReader};
    use std::process::{Child, Command, Stdio};
    use std::time::Duration;

    use rustix::event::{PollFd, PollFlags, Timespec, poll};
    use zbus::zvariant::Value;

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
            let mut child = Command::new("dbus-daemon")
                .args(["--session", "--nofork", "--print-address=1", &listen])
                .stdout(Stdio::piped())
                .spawn()
                .expect("dbus-daemon is needed to test notifications");
            let mut address = String::new();
            let stdout = child.stdout.take().unwrap();
            BufReader::new(stdout).read_line(&mut address).unwrap();
            Self {
                child,
                address: address.trim().to_owned(),
                _directory: directory,
            }
        }

        /// A program on the bus that is not the service.
        pub(crate) fn peer(&self) -> Connection {
            Builder::address(self.address.as_str())
                .unwrap()
                .build()
                .unwrap()
        }

        pub(crate) fn stop(&mut self) {
            self.child.kill().unwrap();
            self.child.wait().unwrap();
        }
    }

    impl Drop for Daemon {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    /// Waits for the service to have heard something, and returns it.
    pub(crate) fn heard(bus: &Bus) -> Vec<Event> {
        let mut sources = [PollFd::new(bus, PollFlags::IN)];
        let patience = Timespec::try_from(Duration::from_secs(20)).unwrap();
        poll(&mut sources, Some(&patience)).unwrap();
        bus.events()
    }

    type Hints<'a> = HashMap<&'a str, Value<'a>>;

    /// Sends a notification as a program would, and returns its number.
    pub(crate) fn notify(peer: &Connection, replaces: u32, title: &str, hints: Hints<'_>) -> u32 {
        let actions: Vec<&str> = Vec::new();
        let body = (
            "tester",
            replaces,
            "tester-icon",
            title,
            "the <b>body</b>",
            actions,
            hints,
            1500,
        );
        let reply = peer.call_method(Some(NAME), PATH, Some(NAME), "Notify", &body);
        reply.unwrap().body().deserialize().unwrap()
    }

    /// What is heard of a notification sent by [`notify`].
    fn sent(id: u32, title: &str, urgent: bool, picture: Picture) -> Event {
        Event::Show(Note {
            id,
            title: title.to_owned(),
            body: "the <b>body</b>".to_owned(),
            picture,
            urgent,
            stays: 1500,
            acts: false,
        })
    }

    fn icon() -> Picture {
        Picture::Named("tester-icon".to_owned())
    }

    #[test]
    fn a_notification_sent_by_a_program_is_heard_with_all_it_says() {
        let daemon = Daemon::start();
        let bus = Bus::open(Some(&daemon.address)).unwrap();
        let peer = daemon.peer();

        let id = notify(&peer, 0, "A title", Hints::new());

        assert_eq!(id, 1);
        let events = heard(&bus);
        assert_eq!(events, [sent(1, "A title", false, icon())]);
        assert_eq!(bus.events(), [], "and only once");
    }

    #[test]
    fn each_notification_gets_a_number_and_one_that_replaces_another_keeps_it() {
        let daemon = Daemon::start();
        let bus = Bus::open(Some(&daemon.address)).unwrap();
        let peer = daemon.peer();
        assert_eq!(notify(&peer, 0, "one", Hints::new()), 1);
        assert_eq!(notify(&peer, 0, "two", Hints::new()), 2);
        assert_eq!(notify(&peer, 1, "one again", Hints::new()), 1);
        assert_eq!(notify(&peer, 0, "three", Hints::new()), 3);
        let mut events = Vec::new();
        while events.len() < 4 {
            events.extend(heard(&bus));
        }
        let expected = [(1, "one"), (2, "two"), (1, "one again"), (3, "three")];
        let expected = expected.map(|(id, title)| sent(id, title, false, icon()));
        assert_eq!(events, expected);
    }

    #[test]
    fn a_notification_may_be_critical_and_bring_a_picture() {
        let daemon = Daemon::start();
        let bus = Bus::open(Some(&daemon.address)).unwrap();
        let peer = daemon.peer();
        // Two pixels, red and blue, in rows padded to eight bytes.
        let raw: Raw = (2, 1, 8, false, 8, 3, vec![255, 0, 0, 0, 0, 255, 9, 9]);
        let hints = Hints::from([("urgency", Value::U8(2)), ("image-data", Value::from(raw))]);
        notify(&peer, 0, "pictured", hints);
        let pixels = vec![255, 0, 0, 255, 0, 0, 255, 255];
        let pixels = Picture::Pixels(Image::new(2, 1, pixels).unwrap());
        assert_eq!(heard(&bus), [sent(1, "pictured", true, pixels)]);

        // A path is taken as given, with or without `file://` before it,
        // and before the program's icon.
        let hints = Hints::from([
            ("urgency", Value::U8(1)),
            ("image-path", Value::from("file:///tmp/picture.png")),
        ]);
        notify(&peer, 0, "by path", hints);
        let path = Picture::Named("/tmp/picture.png".to_owned());
        assert_eq!(heard(&bus), [sent(2, "by path", false, path)]);
    }

    #[test]
    fn pixels_that_do_not_make_a_picture_are_passed_over() {
        let picture = |raw: Raw| pixels(raw);
        let two = vec![1, 2, 3, 4, 5, 6, 7, 8];
        assert!(picture((2, 1, 8, true, 8, 4, two.clone())).is_some());
        assert!(picture((1, 2, 4, true, 8, 4, two.clone())).is_some());
        for wrong in [
            (0, 1, 8, true, 8, 4, two.clone()),
            (2, 0, 8, true, 8, 4, two.clone()),
            (2000, 1, 8, true, 8, 4, two.clone()),
            (2, 1, 8, true, 16, 4, two.clone()),
            (2, 1, 8, true, 8, 3, two.clone()),
            (2, 1, 8, false, 8, 4, two.clone()),
            (2, 1, -8, true, 8, 4, two.clone()),
            (3, 1, 12, true, 8, 4, two.clone()),
            (2, 2, 8, true, 8, 4, two.clone()),
            (2, 1, 8, true, 8, 4, Vec::new()),
        ] {
            assert_eq!(picture(wrong.clone()), None, "{wrong:?}");
        }
        // With nothing sent and no icon, there is no picture.
        assert_eq!(super::picture(&HashMap::new(), ""), Picture::None);
    }

    #[test]
    fn a_program_can_take_a_notification_back_and_hears_what_became_of_one() {
        let daemon = Daemon::start();
        let bus = Bus::open(Some(&daemon.address)).unwrap();
        let peer = daemon.peer();
        let signals = MessageIterator::for_match_rule(
            zbus::MatchRule::builder()
                .msg_type(Type::Signal)
                .interface(NAME)
                .unwrap()
                .build(),
            &peer,
            None,
        );
        let mut signals = signals.unwrap();

        let closed = peer.call_method(Some(NAME), PATH, Some(NAME), "CloseNotification", &7_u32);
        closed.unwrap();
        assert_eq!(heard(&bus), [Event::Close(7)]);

        bus.closed(7, Reason::Dismissed).unwrap();
        let signal = signals.next().unwrap().unwrap();
        assert_eq!(
            signal.header().member().unwrap().as_str(),
            "NotificationClosed"
        );
        assert_eq!(signal.body().deserialize::<(u32, u32)>().unwrap(), (7, 2));
    }

    #[test]
    fn a_program_can_ask_to_hear_of_a_click_and_is_told_of_one() {
        let daemon = Daemon::start();
        let bus = Bus::open(Some(&daemon.address)).unwrap();
        let peer = daemon.peer();
        let rule = zbus::MatchRule::builder()
            .msg_type(Type::Signal)
            .interface(NAME)
            .unwrap()
            .build();
        let mut signals = MessageIterator::for_match_rule(rule, &peer, None).unwrap();
        let send = |actions: Vec<&str>| {
            let body = ("tester", 0_u32, "", "Open me", "", actions, Hints::new(), 0);
            peer.call_method(Some(NAME), PATH, Some(NAME), "Notify", &body)
                .unwrap();
            let events = heard(&bus);
            matches!(&events[0], Event::Show(note) if note.acts)
        };
        assert!(send(vec!["default", "Open"]));
        assert!(send(vec!["other", "Other", "default", ""]));
        // A button of another name is not the click, nor is a label that
        // happens to read "default".
        assert!(!send(vec!["reply", "Reply"]));
        assert!(!send(vec!["reply", "default"]));
        assert!(!send(Vec::new()));

        bus.clicked(4).unwrap();
        let signal = signals.next().unwrap().unwrap();
        assert_eq!(signal.header().member().unwrap().as_str(), "ActionInvoked");
        let said: (u32, String) = signal.body().deserialize().unwrap();
        assert_eq!(said, (4, "default".to_owned()));
    }

    #[test]
    fn a_program_can_ask_what_the_service_is_and_does() {
        let daemon = Daemon::start();
        let _bus = Bus::open(Some(&daemon.address)).unwrap();
        let peer = daemon.peer();
        let call = |interface: &str, method: &str| {
            peer.call_method(Some(NAME), PATH, Some(interface), method, &())
        };
        let capabilities = call(NAME, "GetCapabilities").unwrap();
        let capabilities: Vec<String> = capabilities.body().deserialize().unwrap();
        assert_eq!(capabilities, ["body", "icon-static", "actions"]);
        let information = call(NAME, "GetServerInformation").unwrap();
        let information: (String, String, String, String) =
            information.body().deserialize().unwrap();
        assert_eq!(information.0, "omorfos");
        assert_eq!(information.3, "1.2");
        let described = call("org.freedesktop.DBus.Introspectable", "Introspect").unwrap();
        let described: String = described.body().deserialize().unwrap();
        assert!(described.contains("<method name=\"Notify\">"));
        call("org.freedesktop.DBus.Peer", "Ping").unwrap();
    }

    #[test]
    fn what_is_not_understood_is_answered_with_an_error() {
        let daemon = Daemon::start();
        let bus = Bus::open(Some(&daemon.address)).unwrap();
        let peer = daemon.peer();
        let failure = |interface: &str, method: &str, body: &str| {
            let reply = peer.call_method(Some(NAME), PATH, Some(interface), method, &body);
            reply.unwrap_err().to_string()
        };
        assert!(failure(NAME, "Notify", "not the arguments").contains("InvalidArgs"));
        assert!(failure(NAME, "CloseNotification", "not a number").contains("InvalidArgs"));
        assert!(failure(NAME, "Explode", "").contains("UnknownMethod"));
        assert!(failure("org.example.Other", "Notify", "").contains("UnknownMethod"));
        // None of which reached the rest of the program, and the service
        // still answers.
        assert_eq!(notify(&peer, 0, "still here", Hints::new()), 1);
        assert_eq!(heard(&bus).len(), 1);
    }

    #[test]
    fn only_one_program_can_show_notifications() {
        let daemon = Daemon::start();
        let _first = Bus::open(Some(&daemon.address)).unwrap();
        let second = Bus::open(Some(&daemon.address)).unwrap_err();
        assert!(second.to_string().contains("already showing notifications"));
    }

    #[test]
    fn a_bus_that_is_not_there_cannot_be_joined() {
        assert!(Bus::open(Some("unix:path=/nowhere/at/all")).is_err());
        assert!(Bus::open(Some("not an address")).is_err());
    }

    #[test]
    fn the_service_says_when_the_bus_has_gone() {
        let mut daemon = Daemon::start();
        let bus = Bus::open(Some(&daemon.address)).unwrap();
        daemon.stop();
        assert_eq!(heard(&bus), [Event::Lost]);
        assert!(bus.closed(1, Reason::Expired).is_err());
    }
}
