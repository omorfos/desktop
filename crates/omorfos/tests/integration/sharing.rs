//! Sharing the screen through the portal: an application asks, the user
//! is asked, and what it is given is a stream in PipeWire of what the
//! screen shows, which it reads for as long as the session lasts.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::OsString;
use std::io::Cursor;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::rc::Rc;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use omorfos::Roles;
use pipewire as pw;
use pw::properties::properties;
use pw::spa;
use pw::spa::param::format::{FormatProperties, MediaSubtype, MediaType};
use pw::spa::pod::{Pod, Value as PodValue, serialize::PodSerializer};
use smithay::reexports::wayland_server::ListeningSocket;
use xdg_desktop_portal_omorfos::{NAME, PATH, SCREENCAST, SESSION};
use zbus::blocking::Connection;
use zbus::blocking::connection::Builder;
use zbus::zvariant::{ObjectPath, OwnedValue, Value};

use std::io::Write;

use smithay::backend::allocator::{Format, Fourcc, Modifier};

use crate::common::{Event, HEIGHT, LayerShape, Session, WIDTH, key};
use crate::windows::{BACKGROUND, BLUE, GREEN, RED};
use omorfos::Input;
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::Shape;
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::Layer,
    zwlr_layer_surface_v1::{Anchor, KeyboardInteractivity},
};

/// A bus of the test's own, gone when the test is.
struct Daemon {
    child: Child,
    address: String,
    _directory: tempfile::TempDir,
}

impl Daemon {
    fn start() -> Self {
        use std::io::{BufRead, BufReader};
        let directory = tempfile::TempDir::new().unwrap();
        let listen = format!("--address=unix:path={}/bus", directory.path().display());
        let mut child = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1", &listen])
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut address = String::new();
        let stdout = child.stdout.take().unwrap();
        BufReader::new(stdout).read_line(&mut address).unwrap();
        Self {
            child,
            address: address.trim().to_owned(),
            _directory: directory,
        }
    }

    fn peer(&self) -> Connection {
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

/// A PipeWire of the test's own, gone when the test is. Its socket is in
/// a folder of a short name: one may be no longer than 108 bytes.
struct Pipewire {
    child: Child,
    folder: tempfile::TempDir,
}

impl Pipewire {
    fn start() -> Self {
        let folder = tempfile::Builder::new()
            .prefix("pw")
            .tempdir_in("/tmp")
            .unwrap();
        let child = Command::new("pipewire")
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("XDG_RUNTIME_DIR", folder.path())
            .env("PIPEWIRE_RUNTIME_DIR", folder.path())
            .env("HOME", folder.path())
            // Its complaints, of having no session bus above all, are not
            // to go to the journal of whoever runs the tests.
            .env("PIPEWIRE_LOG_SYSTEMD", "false")
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let socket = folder.path().join("pipewire-0");
        for _ in 0..500 {
            if socket.exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Self { child, folder }
    }

    fn socket(&self) -> PathBuf {
        self.folder.path().join("pipewire-0")
    }
}

impl Drop for Pipewire {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// What a call was answered with.
type Answered = (u32, HashMap<String, OwnedValue>);

/// May ask over everything and take pictures, as the compositor starts it.
const ITS_OWN: Roles = Roles::PANEL.with(Roles::OVERLAY).with(Roles::CAPTURE);

/// Starts the program answering the portals in the session, with PipeWire
/// at `remote`, and waits until it answers on the bus.
fn start(session: &mut Session, daemon: &Daemon, roles: Roles, remote: &str) -> JoinHandle<String> {
    let runtime = session.config.path().to_owned();
    let socket = ListeningSocket::bind_absolute(runtime.join("wayland-sharing")).unwrap();
    session.inner().serve(socket, roles).unwrap();
    let variables = [
        ("XDG_RUNTIME_DIR", runtime.clone().into_os_string()),
        ("WAYLAND_DISPLAY", OsString::from("wayland-sharing")),
        ("HOME", runtime.into_os_string()),
        ("DBUS_SESSION_BUS_ADDRESS", OsString::from(&daemon.address)),
        ("PIPEWIRE_REMOTE", OsString::from(remote)),
    ];
    let running = std::thread::spawn(move || {
        let variable = |name: &str| {
            let found = variables.iter().find(|(known, _)| *known == name);
            found.map(|(_, value)| value.clone())
        };
        xdg_desktop_portal_omorfos::start(&variable).to_string()
    });
    let peer = daemon.peer();
    let bus = "org.freedesktop.DBus";
    session.eventually(|session| {
        session.step();
        let asked = peer.call_method(Some(bus), "/", Some(bus), "NameHasOwner", &(NAME,));
        asked.unwrap().body().deserialize::<bool>().unwrap() || running.is_finished()
    });
    running
}

/// Calls the backend as the portal does, for sharing the screen.
fn call(
    peer: &Connection,
    method: &str,
    body: &(impl serde::Serialize + zbus::zvariant::DynamicType),
) -> zbus::Result<Answered> {
    let reply = peer.call_method(Some(NAME), PATH, Some(SCREENCAST), method, body)?;
    Ok(reply.body().deserialize().unwrap())
}

fn path(text: &str) -> ObjectPath<'_> {
    ObjectPath::try_from(text).unwrap()
}

/// Makes a session and chooses what to share in it, with the pointer
/// drawn in or not.
/// How a call asks for the pointer to be shown, by the portal's numbers.
const HIDDEN: u32 = 1;
const DRAWN: u32 = 2;
const BESIDE: u32 = 4;

fn make(peer: &Connection, session: &str, cursor: u32) {
    let none: HashMap<&str, Value<'_>> = HashMap::new();
    let request = "/org/freedesktop/portal/desktop/request/1_1/make";
    let made = call(
        peer,
        "CreateSession",
        &(path(request), path(session), "org.example.Call", none),
    );
    assert_eq!(made.unwrap().0, 0);
    let options = HashMap::from([
        ("types", Value::U32(1)),
        ("cursor_mode", Value::U32(cursor)),
    ]);
    let chosen = call(
        peer,
        "SelectSources",
        &(path(request), path(session), "org.example.Call", options),
    );
    assert_eq!(chosen.unwrap().0, 0);
}

/// Asks to start sharing in a session, and waits for the answer on a
/// thread of its own.
fn begin(daemon: &Daemon, session: &'static str) -> JoinHandle<Answered> {
    let peer = daemon.peer();
    std::thread::spawn(move || {
        let none: HashMap<&str, Value<'_>> = HashMap::new();
        let request = "/org/freedesktop/portal/desktop/request/1_1/start";
        let body = (path(request), path(session), "org.example.Call", "", none);
        call(&peer, "Start", &body).unwrap()
    })
}

/// The colour of a question on the screen, where it is.
fn question() -> u32 {
    let looks = omorfos_config::Theme::default();
    let looks = looks
        .style(omorfos_config::Part::Launcher, omorfos_config::States::NONE)
        .background;
    u32::from_be_bytes([looks.alpha, looks.red, looks.green, looks.blue])
}

/// Draws until the question is on the screen.
fn asked(session: &mut Session) {
    session.eventually(|session| session.draw().at(WIDTH / 2, HEIGHT / 2) == question());
}

/// Draws until the question is off the screen. The application is
/// answered before the question is taken away, so a question asked
/// straight after could otherwise be taken to be up while the last still
/// is.
fn unasked(session: &mut Session) {
    session.eventually(|session| session.draw().at(WIDTH / 2, HEIGHT / 2) != question());
}

fn press(session: &mut Session, code: smithay::input::keyboard::Keycode) {
    for pressed in [true, false] {
        session
            .compositor_mut()
            .input(Input::Key { code, pressed }, 0);
    }
}

/// Clicks at `at` until the call has been answered, as a screen to share
/// is picked once the picture of each is up, and returns how.
fn picked_screen(
    session: &mut Session,
    asking: JoinHandle<Answered>,
    (x, y): (i32, i32),
) -> Answered {
    let at = Input::Motion {
        x: f64::from(x),
        y: f64::from(y),
    };
    session.eventually(|session| {
        session.compositor_mut().input(at, 0);
        session.step();
        for pressed in [true, false] {
            let button = Input::Button {
                button: 0x110,
                pressed,
            };
            session.compositor_mut().input(button, 0);
        }
        drop(session.draw_on(1));
        session.draw();
        asking.is_finished()
    });
    asking.join().unwrap()
}

/// Draws until a call has been answered, and returns how.
fn answered(session: &mut Session, asking: JoinHandle<Answered>) -> Answered {
    session.eventually(|session| {
        session.draw();
        asking.is_finished()
    });
    asking.join().unwrap()
}

/// The stream an application is told to read, and how large it is.
fn stream(results: &HashMap<String, OwnedValue>) -> (u32, (i32, i32)) {
    let streams = results["streams"].try_clone().unwrap();
    let streams: Vec<(u32, HashMap<String, OwnedValue>)> = streams.try_into().unwrap();
    let (node, about) = &streams[0];
    let size: (i32, i32) = about["size"].try_clone().unwrap().try_into().unwrap();
    assert_eq!(
        about["source_type"].downcast_ref::<u32>().unwrap(),
        1,
        "a screen"
    );
    (*node, size)
}

/// An application reading a stream in PipeWire, as one sharing the screen
/// in a call does.
struct Viewer {
    main: pw::main_loop::MainLoopRc,
    _context: pw::context::ContextRc,
    core: pw::core::CoreRc,
    stream: pw::stream::StreamRc,
    _listener: pw::stream::StreamListener<()>,
    node: Rc<Cell<Option<u32>>>,
    seen: Arc<Mutex<Seen>>,
    link: Option<pw::link::Link>,
}

/// What a viewer has been given. PipeWire hands a picture over on the
/// thread that turns the viewer, or, in some versions, on a thread of its
/// own while the viewer is not being turned, so it is behind a lock.
#[derive(Default)]
struct Seen {
    /// The last picture.
    picture: Option<Vec<u8>>,
    /// Where it was last told the pointer is, where its position is in its
    /// picture, and the colour of that picture a little inside it.
    pointer: Option<Pointed>,
    /// Whether the last picture was in memory of the graphics card.
    card: bool,
    /// Whether it was given memory of the card that said it held a picture
    /// and could not be read, as a reader that reads such memory whatever
    /// its size says fails on.
    unreadable: bool,
}

/// The pictures a viewer wants: blue first, of any size, in memory of the
/// graphics card laid out in `layout` if there is one.
fn wanted(layout: Option<u64>) -> Vec<u8> {
    let mut wants = spa::pod::object!(
        spa::utils::SpaTypes::ObjectParamFormat,
        spa::param::ParamType::EnumFormat,
        spa::pod::property!(FormatProperties::MediaType, Id, MediaType::Video),
        spa::pod::property!(FormatProperties::MediaSubtype, Id, MediaSubtype::Raw),
        spa::pod::property!(
            FormatProperties::VideoFormat,
            Id,
            spa::param::video::VideoFormat::BGRx
        ),
        spa::pod::property!(
            FormatProperties::VideoSize,
            Choice,
            Range,
            Rectangle,
            spa::utils::Rectangle {
                width: 640,
                height: 480
            },
            spa::utils::Rectangle {
                width: 1,
                height: 1
            },
            spa::utils::Rectangle {
                width: 8192,
                height: 8192
            }
        ),
    );
    wants
        .properties
        .extend(layout.map(|layout| spa::pod::Property {
            key: FormatProperties::VideoModifier.as_raw(),
            flags: spa::pod::PropertyFlags::MANDATORY,
            value: PodValue::Long(layout.cast_signed()),
        }));
    let bytes = PodSerializer::serialize(Cursor::new(Vec::new()), &PodValue::Object(wants));
    bytes.unwrap().0.into_inner()
}

/// Where a viewer was told the pointer is, where its position is in its
/// picture, and the colour of that picture a little inside it.
type Pointed = ((i32, i32), (i32, i32), u32);

/// What the viewer asks to be told of the pointer: as much room as a call
/// asks for, beside each picture.
fn told_of_the_pointer() -> Vec<u8> {
    let room = |side: i32| 28 + 20 + side * side * 4;
    let wants = spa::pod::Object {
        type_: spa::utils::SpaTypes::ObjectParamMeta.as_raw(),
        id: spa::param::ParamType::Meta.as_raw(),
        properties: vec![
            spa::pod::Property::new(
                spa::sys::SPA_PARAM_META_type,
                PodValue::Id(spa::utils::Id(spa::sys::SPA_META_Cursor)),
            ),
            spa::pod::Property::new(
                spa::sys::SPA_PARAM_META_size,
                PodValue::Choice(spa::pod::ChoiceValue::Int(spa::utils::Choice(
                    spa::utils::ChoiceFlags::empty(),
                    spa::utils::ChoiceEnum::Range {
                        default: room(64),
                        min: room(1),
                        max: room(384),
                    },
                ))),
            ),
        ],
    };
    let bytes = PodSerializer::serialize(Cursor::new(Vec::new()), &PodValue::Object(wants));
    bytes.unwrap().0.into_inner()
}

impl Viewer {
    fn new(socket: &std::path::Path) -> Self {
        Self::reading(socket, false)
    }

    /// A viewer that would rather take the pictures in memory of the
    /// graphics card laid out a row at a time, if `card`, which it can map
    /// to read.
    fn reading(socket: &std::path::Path, takes_card: bool) -> Self {
        pw::init();
        let main = pw::main_loop::MainLoopRc::new(None).unwrap();
        let context = pw::context::ContextRc::new(&main, None).unwrap();
        let remote = socket.to_str().unwrap();
        let core = context
            .connect_rc(Some(properties! { *pw::keys::REMOTE_NAME => remote }))
            .unwrap();
        let kind = properties! { *pw::keys::MEDIA_TYPE => "Video", *pw::keys::MEDIA_CATEGORY => "Capture" };
        let stream = pw::stream::StreamRc::new(core.clone(), "viewer", kind).unwrap();
        let node = Rc::new(Cell::new(None));
        let seen = Arc::new(Mutex::new(Seen::default()));
        let (named, kept) = (Rc::clone(&node), Arc::clone(&seen));
        let listener = stream
            .add_local_listener::<()>()
            .state_changed(move |stream, (), _, now| {
                if matches!(now, pw::stream::StreamState::Paused) {
                    named.set(Some(stream.node_id()));
                }
            })
            .param_changed(|stream, (), id, param| {
                if id == spa::param::ParamType::Format.as_raw() && param.is_some() {
                    let wants = told_of_the_pointer();
                    let wants = Pod::from_bytes(&wants).unwrap();
                    stream.update_params(&mut [wants]).unwrap();
                }
            })
            .process(move |stream, ()| {
                if let Some(mut buffer) = stream.dequeue_buffer() {
                    let mut seen = kept.lock().unwrap();
                    let cursor = buffer.find_meta::<spa::buffer::meta::MetaCursor>();
                    if let Some(cursor) = cursor.filter(|cursor| cursor.is_valid()) {
                        let (at, tip) = (cursor.position(), cursor.hotspot());
                        let bitmap = cursor.bitmap().and_then(|bitmap| bitmap.bitmap_data());
                        let colour = bitmap.map(|bytes| {
                            let at = (4 * 256 + 4) * 4;
                            u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
                        });
                        let was = seen.pointer.map(|(_, _, colour)| colour);
                        let colour = colour.or(was).unwrap_or_default();
                        seen.pointer = Some(((at.x, at.y), (tip.x, tip.y), colour));
                    }
                    let data = &mut buffer.datas_mut()[0];
                    let card = data.type_() == spa::buffer::DataType::DmaBuf;
                    seen.card = card;
                    // Memory of the card is read whatever its size says,
                    // unless it is said to be spoilt; read so, it has to say
                    // how far apart its rows are.
                    let chunk = data.chunk();
                    let spoilt = chunk.flags().contains(spa::buffer::ChunkFlags::CORRUPTED);
                    if card && !spoilt && chunk.stride() == 0 {
                        seen.unreadable = true;
                    }
                    let size = usize::try_from(chunk.size()).unwrap();
                    let picture = data.data().map(|bytes| bytes[..size].to_vec());
                    if picture.as_ref().is_some_and(|picture| !picture.is_empty()) {
                        seen.picture = picture;
                    }
                }
            })
            .register()
            .unwrap();
        let wants = [takes_card.then(|| wanted(Some(0))), Some(wanted(None))];
        let wants = wants.iter().flatten();
        let mut wants: Vec<_> = wants.map(|wants| Pod::from_bytes(wants).unwrap()).collect();
        let flags = pw::stream::StreamFlags::MAP_BUFFERS;
        stream
            .connect(spa::utils::Direction::Input, None, flags, &mut wants)
            .unwrap();
        Self {
            main,
            _context: context,
            core,
            stream,
            _listener: listener,
            node,
            seen,
            link: None,
        }
    }

    /// Does what there is to do, and links to `shared` once it can.
    fn turn(&mut self, shared: u32) {
        self.main
            .loop_()
            .iterate(pw::loop_::Timeout::Finite(Duration::ZERO));
        if self.link.is_none()
            && let Some(own) = self.node.get()
        {
            let ends = properties! {
                "link.output.node" => shared.to_string(),
                "link.input.node" => own.to_string(),
                "object.linger" => "false",
            };
            self.link = Some(
                self.core
                    .create_object::<pw::link::Link>("link-factory", &ends)
                    .unwrap(),
            );
        }
    }

    /// The colour at a place in the last picture seen, as `0xAARRGGBB`,
    /// for a picture `width` pixels across.
    /// What it has been given so far.
    fn seen(&self) -> MutexGuard<'_, Seen> {
        self.seen.lock().unwrap()
    }

    /// How many pixels the last picture seen had.
    fn pixels(&self) -> usize {
        let seen = self.seen();
        seen.picture.as_ref().map_or(0, |seen| seen.len() / 4)
    }

    fn at(&self, width: i32, (x, y): (i32, i32)) -> Option<u32> {
        let seen = self.seen();
        let at = usize::try_from((y * width + x) * 4).unwrap();
        seen.picture
            .as_ref()
            .map(|seen| u32::from_be_bytes([0xff, seen[at + 2], seen[at + 1], seen[at]]))
    }
}

impl Drop for Viewer {
    fn drop(&mut self) {
        let _ = self.stream.disconnect();
    }
}

#[test]
fn a_call_is_given_the_screen_to_read_once_the_user_says_so() {
    let daemon = Daemon::start();
    let pipewire = Pipewire::start();
    let mut session = Session::new();
    let mut app = session.connect();
    let window = session.open(&mut app, "app", (200, 100), RED);
    let remote = pipewire.socket();
    let _running = start(&mut session, &daemon, ITS_OWN, remote.to_str().unwrap());

    let peer = daemon.peer();
    let call_session = "/org/freedesktop/portal/desktop/session/1_1/call";
    make(&peer, call_session, HIDDEN);
    let starting = begin(&daemon, call_session);
    asked(&mut session);
    press(&mut session, key::RETURN);
    let (response, results) = answered(&mut session, starting);
    assert_eq!(response, 0);
    let (node, size) = stream(&results);
    assert_eq!(size, (WIDTH, HEIGHT));

    // What the screen shows is what is read, the question not in it.
    let mut viewer = Viewer::new(&remote);
    session.eventually(|session| {
        session.draw();
        viewer.turn(node);
        viewer.at(WIDTH, (10, 10)) == Some(RED)
    });
    assert_eq!(viewer.at(WIDTH, (WIDTH / 2, HEIGHT / 2)), Some(BACKGROUND));
    // A line round the screen says it is shared, and is not in what is.
    let line = omorfos_config::Theme::default()
        .style(
            omorfos_config::Part::CaptureBorder,
            omorfos_config::States::NONE,
        )
        .border
        .top
        .color;
    let line = u32::from_be_bytes([line.alpha, line.red, line.green, line.blue]);
    // Once the next picture has been asked for: the first could be a
    // screenshot's.
    session.eventually(|session| {
        viewer.turn(node);
        session.draw().at(0, HEIGHT / 2) == line
    });
    let picture = session.draw();
    for (x, y) in [
        (0, HEIGHT / 2),
        (WIDTH - 1, HEIGHT / 2),
        (WIDTH / 2, HEIGHT - 1),
    ] {
        assert_eq!(picture.at(x, y), line, "the line, at {x}, {y}");
    }
    assert_eq!(picture.at(2, HEIGHT / 2), BACKGROUND, "two wide");
    assert_eq!(viewer.at(WIDTH, (0, HEIGHT / 2)), Some(BACKGROUND));
    // What changes on the screen is what is read next, each time, as the
    // memory it is put in goes round: a change in one place and then in
    // another, so that memory that missed the first while it was being read
    // is brought up to date too.
    let corner = LayerShape {
        layer: Layer::Top,
        anchor: Anchor::Bottom | Anchor::Right,
        size: (40, 40),
        exclusive_zone: 0,
        keyboard: KeyboardInteractivity::None,
    };
    let (square, _layer) = session.open_layer(&mut app, &corner, (40, 40), RED);
    let mut shown = (RED, RED);
    for step in 0..8 {
        let colour = if step % 4 < 2 { GREEN } else { RED };
        if step % 2 == 0 {
            app.fill(&window.surface, (200, 100), colour);
            shown.0 = colour;
        } else {
            app.fill(&square, (40, 40), colour);
            shown.1 = colour;
        }
        session.roundtrip(&mut app);
        session.eventually(|session| {
            session.draw();
            viewer.turn(node);
            let read = (
                viewer.at(WIDTH, (10, 10)),
                viewer.at(WIDTH, (WIDTH - 20, HEIGHT - 20)),
            );
            read == (Some(shown.0), Some(shown.1))
        });
    }
    let output = session.output();
    assert!(
        session.compositor().captured(&output),
        "the screen is marked as shared"
    );
    // A screen that comes and goes meanwhile, not the one shared, leaves
    // it shared.
    let other = session.add_output((320, 240));
    session.step();
    session.compositor_mut().remove_output(&other);
    session.advance(Duration::from_millis(50));
    session.draw();
    assert!(session.compositor().captured(&output));

    // The portal ends the session, and the screen is no longer shared.
    peer.call_method(Some(NAME), call_session, Some(SESSION), "Close", &())
        .unwrap();
    session.eventually(|session| {
        session.draw();
        !session.compositor().captured(&output)
    });
    assert_eq!(
        session.draw().at(0, HEIGHT / 2),
        BACKGROUND,
        "and the line has gone"
    );
}

/// Shares the screen with a call in a new session, as the user says yes,
/// and returns the session and what the call is to read.
fn sharing(
    session: &mut Session,
    daemon: &Daemon,
    call_session: &'static str,
) -> (Connection, u32) {
    sharing_with(session, daemon, call_session, HIDDEN)
}

/// The same, with the pointer shown as `cursor` asks.
fn sharing_with(
    session: &mut Session,
    daemon: &Daemon,
    call_session: &'static str,
    cursor: u32,
) -> (Connection, u32) {
    let peer = daemon.peer();
    make(&peer, call_session, cursor);
    let (response, results) = shared(session, daemon, call_session, key::RETURN);
    assert_eq!(response, 0);
    (peer, stream(&results).0)
}

/// Has the session go on for a while, doing `also` meanwhile, as the
/// backend paces itself by the clock, so that what it does in that time is
/// done.
fn wait(session: &mut Session, long: Duration, mut also: impl FnMut()) {
    let until = std::time::Instant::now() + long;
    while std::time::Instant::now() < until {
        session.step();
        also();
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn a_call_that_begins_reading_again_is_shown_the_screen_at_once() {
    let daemon = Daemon::start();
    let pipewire = Pipewire::start();
    let mut session = Session::new();
    let mut app = session.connect();
    let window = session.open(&mut app, "app", (200, 100), RED);
    let remote = pipewire.socket();
    let _running = start(&mut session, &daemon, ITS_OWN, remote.to_str().unwrap());
    let (_peer, node) = sharing(
        &mut session,
        &daemon,
        "/org/freedesktop/portal/desktop/session/1_1/again",
    );

    let mut viewer = Viewer::new(&remote);
    session.eventually(|session| {
        session.draw();
        viewer.turn(node);
        viewer.at(WIDTH, (10, 10)) == Some(RED)
    });
    // The next picture has been asked for, and waits on the screen to
    // change, when what reads the stream goes; a call does so to show a
    // preview, and then reads it again to share it.
    wait(&mut session, Duration::from_millis(200), || {
        viewer.turn(node);
    });
    drop(viewer);
    // What the screen shows meanwhile goes to nobody.
    app.fill(&window.surface, (200, 100), GREEN);
    session.roundtrip(&mut app);
    session.draw();
    wait(&mut session, Duration::from_millis(100), || {});

    // What reads it next is shown the screen as it is, though nothing on
    // it changes any more.
    let mut viewer = Viewer::new(&remote);
    session.eventually(|session| {
        session.draw();
        viewer.turn(node);
        viewer.at(WIDTH, (10, 10)) == Some(GREEN)
    });
}

#[test]
fn a_call_that_asks_is_told_where_the_pointer_is_and_what_it_looks_like_beside_the_pictures() {
    let daemon = Daemon::start();
    let pipewire = Pipewire::start();
    let mut session = Session::new();
    let themes = session.install_cursors("default", &[("default", 8, BLUE), ("text", 8, GREEN)]);
    session.compositor_mut().set_cursor_themes(themes);
    session.compositor_mut().show_pointer();
    let over = |x: f64, y: f64| Input::Motion { x, y };
    session.compositor_mut().input(over(300.0, 200.0), 0);
    let remote = pipewire.socket();
    let _running = start(&mut session, &daemon, ITS_OWN, remote.to_str().unwrap());
    let called = "/org/freedesktop/portal/desktop/session/1_1/beside";
    let (_peer, node) = sharing_with(&mut session, &daemon, called, BESIDE);
    // It was over the question, which has gone.
    session.compositor_mut().input(over(300.0, 200.0), 0);

    // The pointer is not in the pictures; what reads them is told where it
    // is and what it looks like.
    let mut viewer = Viewer::new(&remote);
    session.eventually(|session| {
        session.draw();
        viewer.turn(node);
        viewer.seen().pointer == Some(((300, 200), (1, 1), BLUE))
    });
    session.eventually(|session| {
        session.draw();
        viewer.turn(node);
        viewer.at(WIDTH, (10, 10)).is_some()
    });
    assert_eq!(viewer.at(WIDTH, (303, 203)), Some(BACKGROUND));

    // It moving is told, with nothing on the screen changing.
    session.compositor_mut().input(over(310.0, 205.0), 0);
    session.eventually(|session| {
        session.step();
        viewer.turn(node);
        viewer.seen().pointer == Some(((310, 205), (1, 1), BLUE))
    });
    // And so is it looking different.
    let mut app = session.connect();
    session.open(&mut app, "app", (200, 100), RED);
    session.compositor_mut().input(over(50.0, 50.0), 0);
    session.roundtrip(&mut app);
    app.set_cursor_shape(Shape::Text);
    session.roundtrip(&mut app);
    session.eventually(|session| {
        session.draw();
        viewer.turn(node);
        viewer.seen().pointer == Some(((50, 50), (1, 1), GREEN))
    });

    // Away from the screen shared, it is not to be seen, and back over it
    // it is again.
    session.add_output((WIDTH, HEIGHT));
    session
        .compositor_mut()
        .input(over(f64::from(WIDTH + 10), 10.0), 0);
    session.eventually(|session| {
        session.draw();
        viewer.turn(node);
        viewer
            .seen()
            .pointer
            .is_some_and(|(_, _, colour)| colour == 0)
    });
    session.compositor_mut().input(over(320.0, 240.0), 0);
    session.eventually(|session| {
        session.draw();
        viewer.turn(node);
        viewer.seen().pointer == Some(((320, 240), (1, 1), BLUE))
    });

    // What reads it anew is told at once, though the pointer does not move.
    drop(viewer);
    let mut viewer = Viewer::new(&remote);
    session.eventually(|session| {
        session.draw();
        viewer.turn(node);
        viewer.seen().pointer == Some(((320, 240), (1, 1), BLUE))
    });
}

/// What the software renderer the tests draw with can draw into in the
/// memory of a card: pictures laid out a row at a time.
fn drawable() -> [Format; 1] {
    [Format {
        code: Fourcc::Xrgb8888,
        modifier: Modifier::Linear,
    }]
}

/// Shares the first screen, a window on it red, with a viewer that would
/// rather take memory of the graphics card, the compositor saying the card
/// it draws with is the one numbered `card`, and returns the viewer once it
/// has seen the window.
fn shared_from_card(card: u64) -> Viewer {
    let daemon = Daemon::start();
    let pipewire = Pipewire::start();
    let mut session = Session::new();
    session
        .compositor_mut()
        .offer_gpu_buffers(card, drawable())
        .unwrap();
    let mut app = session.connect();
    session.open(&mut app, "app", (200, 100), RED);
    let remote = pipewire.socket();
    let _running = start(&mut session, &daemon, ITS_OWN, remote.to_str().unwrap());
    let called = "/org/freedesktop/portal/desktop/session/1_1/card";
    let (_peer, node) = sharing_with(&mut session, &daemon, called, BESIDE);
    let mut viewer = Viewer::reading(&remote, true);
    session.eventually(|session| {
        session.draw();
        viewer.turn(node);
        viewer.at(WIDTH, (10, 10)) == Some(RED)
    });
    assert_eq!(viewer.at(WIDTH, (400, 400)), Some(BACKGROUND));
    // The pointer moving alone is told with nothing to read but where it
    // is.
    let over = |x: f64, y: f64| Input::Motion { x, y };
    for (x, y) in [(300, 200), (310, 210)] {
        session
            .compositor_mut()
            .input(over(f64::from(x), f64::from(y)), 0);
        session.eventually(|session| {
            session.step();
            viewer.turn(node);
            viewer.seen().pointer.is_some_and(|(at, ..)| at == (x, y))
        });
    }
    assert_eq!(
        viewer.at(WIDTH, (10, 10)),
        Some(RED),
        "the picture is as it was"
    );
    assert!(!viewer.seen().unreadable, "a buffer no reader could read");
    viewer
}

#[test]
fn a_call_that_would_take_memory_of_the_card_is_given_shared_memory_where_it_cannot_be_had() {
    // No card has this number.
    let viewer = shared_from_card(0);
    assert!(!viewer.seen().card, "shared memory");
}

#[test]
fn a_screen_is_shared_in_the_memory_of_the_card_where_there_is_one() {
    let Some(card) = render_node() else {
        let said = "not tried: there is no graphics card this user may use";
        let _ = writeln!(std::io::stderr(), "{said}");
        return;
    };
    let viewer = shared_from_card(card);
    assert!(viewer.seen().card, "memory of the card");
}

/// The device number of the graphics card to try with, if this machine has
/// one this user may use.
fn render_node() -> Option<u64> {
    crate::common::graphics_card().map(|(_, number)| number)
}

#[test]
fn a_screen_shared_while_the_session_is_locked_is_shown_again_once_it_is_not() {
    let daemon = Daemon::start();
    let pipewire = Pipewire::start();
    let mut session = Session::new();
    let mut app = session.connect();
    let window = session.open(&mut app, "app", (200, 100), RED);
    let remote = pipewire.socket();
    let _running = start(&mut session, &daemon, ITS_OWN, remote.to_str().unwrap());
    let (_peer, node) = sharing(
        &mut session,
        &daemon,
        "/org/freedesktop/portal/desktop/session/1_1/locked",
    );
    let mut viewer = Viewer::new(&remote);
    session.eventually(|session| {
        session.draw();
        viewer.turn(node);
        viewer.at(WIDTH, (10, 10)) == Some(RED)
    });

    // Nothing is shown of the session while it is locked.
    let mut locker = session.connect();
    let lock = locker.lock();
    let surface = locker.lock_surface(&lock);
    session.roundtrip(&mut locker);
    locker.fill(&surface, (WIDTH, HEIGHT), GREEN);
    session.roundtrip(&mut locker);
    session.draw();
    session.send_frames(Duration::from_millis(16));
    session.roundtrip(&mut locker);
    assert!(locker.take_events().contains(&Event::Locked));
    wait(&mut session, Duration::from_millis(100), || {
        viewer.turn(node);
    });
    assert_eq!(viewer.at(WIDTH, (10, 10)), Some(RED));

    // Once it is not, what changes is shown, the call none the wiser.
    lock.unlock_and_destroy();
    session.roundtrip(&mut locker);
    app.fill(&window.surface, (200, 100), GREEN);
    session.roundtrip(&mut app);
    session.eventually(|session| {
        session.draw();
        viewer.turn(node);
        viewer.at(WIDTH, (10, 10)) == Some(GREEN)
    });
    assert!(session.compositor().captured(&session.output()));
}

/// Starts sharing in a session already made, has the user answer with a
/// key, and returns the answer.
fn shared(
    session: &mut Session,
    daemon: &Daemon,
    call_session: &'static str,
    answer: smithay::input::keyboard::Keycode,
) -> Answered {
    let starting = begin(daemon, call_session);
    asked(session);
    press(session, answer);
    answered(session, starting)
}

#[test]
fn a_call_is_refused_the_screen_when_the_user_says_no() {
    let daemon = Daemon::start();
    let pipewire = Pipewire::start();
    let mut session = Session::new();
    let remote = pipewire.socket();
    let _running = start(&mut session, &daemon, ITS_OWN, remote.to_str().unwrap());
    let peer = daemon.peer();
    let call_session = "/org/freedesktop/portal/desktop/session/1_1/no";
    make(&peer, call_session, HIDDEN);
    let refused = shared(&mut session, &daemon, call_session, key::ESCAPE);
    assert_eq!(refused, (1, HashMap::new()));
    assert!(
        !session.compositor().captured(&session.output()),
        "nothing was shared"
    );
    unasked(&mut session);

    // The session ends with nothing shared, and the next is asked about
    // as the first was.
    peer.call_method(Some(NAME), call_session, Some(SESSION), "Close", &())
        .unwrap();
    let again = "/org/freedesktop/portal/desktop/session/1_1/again";
    make(&peer, again, HIDDEN);
    let starting = begin(&daemon, again);
    asked(&mut session);
    // Another asked while that question is up fails at once.
    let other = "/org/freedesktop/portal/desktop/session/1_1/other";
    make(&peer, other, HIDDEN);
    assert_eq!(answered(&mut session, begin(&daemon, other)).0, 2);
    // And the portal withdraws what it asked, which is a no.
    let request = "/org/freedesktop/portal/desktop/request/1_1/start";
    let withdraw = Some("org.freedesktop.impl.portal.Request");
    peer.call_method(Some(NAME), request, withdraw, "Close", &())
        .unwrap();
    assert_eq!(answered(&mut session, starting).0, 1);
}

#[test]
fn what_is_asked_of_a_session_that_was_never_made_fails() {
    let daemon = Daemon::start();
    let mut session = Session::new();
    let _running = start(&mut session, &daemon, ITS_OWN, "");
    let peer = daemon.peer();
    let none: HashMap<&str, Value<'_>> = HashMap::new();
    let request = "/org/freedesktop/portal/desktop/request/1_1/r";
    let nowhere = "/org/freedesktop/portal/desktop/session/1_1/nowhere";
    let chosen = call(
        &peer,
        "SelectSources",
        &(
            path(request),
            path(nowhere),
            "org.example.Call",
            none.clone(),
        ),
    );
    assert_eq!(chosen.unwrap().0, 2);
    let started = call(
        &peer,
        "Start",
        &(path(request), path(nowhere), "org.example.Call", "", none),
    );
    assert_eq!(started.unwrap().0, 2);
    // Ending it is nothing either, and what is not understood is said so.
    peer.call_method(Some(NAME), nowhere, Some(SESSION), "Close", &())
        .unwrap();
    for method in ["CreateSession", "SelectSources", "Start"] {
        let said = call(&peer, method, &("not a request",))
            .unwrap_err()
            .to_string();
        assert!(said.contains("InvalidArgs"), "{said}");
    }
    // It says what it can share, and how the pointer can be shown.
    let properties = Some("org.freedesktop.DBus.Properties");
    let reply = peer
        .call_method(
            Some(NAME),
            PATH,
            properties,
            "Get",
            &(SCREENCAST, "AvailableSourceTypes"),
        )
        .unwrap();
    let types: OwnedValue = reply.body().deserialize().unwrap();
    assert_eq!(types.downcast_ref::<u32>().unwrap(), 1, "screens");
}

#[test]
fn without_pipewire_or_leave_to_take_pictures_the_call_is_told_it_failed() {
    let daemon = Daemon::start();
    let mut session = Session::new();
    let _running = start(&mut session, &daemon, ITS_OWN, "/nowhere/pipewire-0");
    let call_session = "/org/freedesktop/portal/desktop/session/1_1/pw";
    make(&daemon.peer(), call_session, HIDDEN);
    assert_eq!(
        shared(&mut session, &daemon, call_session, key::RETURN).0,
        2,
        "no PipeWire"
    );

    let daemon = Daemon::start();
    let pipewire = Pipewire::start();
    let mut session = Session::new();
    let remote = pipewire.socket();
    let not_let = Roles::PANEL.with(Roles::OVERLAY);
    let _running = start(&mut session, &daemon, not_let, remote.to_str().unwrap());
    let call_session = "/org/freedesktop/portal/desktop/session/1_1/role";
    make(&daemon.peer(), call_session, HIDDEN);
    assert_eq!(
        shared(&mut session, &daemon, call_session, key::RETURN).0,
        2,
        "no pictures"
    );
}

#[test]
fn a_screen_that_goes_ends_its_sharing_and_the_session_is_closed() {
    let daemon = Daemon::start();
    let pipewire = Pipewire::start();
    let mut session = Session::new();
    // A second screen, which is not the one shared.
    session.add_output((320, 240));
    let remote = pipewire.socket();
    let _running = start(&mut session, &daemon, ITS_OWN, remote.to_str().unwrap());
    let peer = daemon.peer();
    let rule = format!("type='signal',interface='{SESSION}'");
    let bus = "org.freedesktop.DBus";
    peer.call_method(
        Some(bus),
        "/org/freedesktop/DBus",
        Some(bus),
        "AddMatch",
        &(rule,),
    )
    .unwrap();
    let mut heard = zbus::blocking::MessageIterator::from(&peer);

    let call_session = "/org/freedesktop/portal/desktop/session/1_1/gone";
    // With the pointer in it, as a call that shows where one points asks.
    make(&peer, call_session, DRAWN);
    let starting = begin(&daemon, call_session);
    asked(&mut session);
    press(&mut session, key::RETURN);
    // With two screens, the one to share is picked.
    assert_eq!(picked_screen(&mut session, starting, (10, 10)).0, 0);
    let output = session.output();
    session.compositor_mut().remove_output(&output);
    session.eventually(|session| {
        session.draw();
        session.step();
        !session.compositor().captured(&session.output())
    });
    let closed = heard.by_ref().map_while(Result::ok).find(|message| {
        let member = message.header().member().map(ToString::to_string);
        member.as_deref() == Some("Closed")
    });
    let path = closed.unwrap().header().path().map(ToString::to_string);
    assert_eq!(path.as_deref(), Some(call_session));
}

/// Starts the portal with two screens, the second 320 by 240, asks to
/// share in a new session, says yes, and returns the session's path, the
/// call and what it waits for.
fn picking_a_screen(
    session: &mut Session,
    daemon: &Daemon,
    roles: Roles,
    remote: &str,
    call_session: &'static str,
) -> (Connection, JoinHandle<Answered>) {
    session.add_output((320, 240));
    // It runs until the session ends, and is not waited for.
    drop(start(session, daemon, roles, remote));
    let peer = daemon.peer();
    make(&peer, call_session, HIDDEN);
    let starting = begin(daemon, call_session);
    asked(session);
    press(session, key::RETURN);
    (peer, starting)
}

/// Moves the pointer to a place.
fn point(session: &mut Session, (x, y): (i32, i32)) {
    let at = Input::Motion {
        x: f64::from(x),
        y: f64::from(y),
    };
    session.compositor_mut().input(at, 0);
    session.step();
}

#[test]
fn with_two_screens_the_one_the_user_picks_is_shared() {
    let daemon = Daemon::start();
    let pipewire = Pipewire::start();
    let mut session = Session::new();
    let remote = pipewire.socket();
    let call_session = "/org/freedesktop/portal/desktop/session/1_1/picked";
    let (_peer, starting) = picking_a_screen(
        &mut session,
        &daemon,
        ITS_OWN,
        remote.to_str().unwrap(),
        call_session,
    );
    // The screen under the pointer is bright, and the other dimmed.
    session.eventually(|session| {
        point(session, (WIDTH + 100, 100));
        let other = session.draw().at(5, 5);
        let under = session.draw_on(1).at(100, 100);
        other != BACKGROUND && under == BACKGROUND
    });
    let (response, results) = picked_screen(&mut session, starting, (WIDTH + 100, 100));
    assert_eq!(response, 0);
    assert_eq!(stream(&results).1, (320, 240), "the second screen");
}

#[test]
fn enter_shares_the_screen_the_pointer_is_on() {
    let daemon = Daemon::start();
    let pipewire = Pipewire::start();
    let mut session = Session::new();
    let remote = pipewire.socket();
    let call_session = "/org/freedesktop/portal/desktop/session/1_1/entered";
    let (_peer, starting) = picking_a_screen(
        &mut session,
        &daemon,
        ITS_OWN,
        remote.to_str().unwrap(),
        call_session,
    );
    session.eventually(|session| {
        point(session, (WIDTH + 100, 100));
        press(session, key::RETURN);
        drop(session.draw_on(1));
        session.draw();
        starting.is_finished()
    });
    let (response, results) = starting.join().unwrap();
    assert_eq!(response, 0);
    assert_eq!(stream(&results).1, (320, 240));
}

#[test]
fn escape_while_a_screen_is_picked_shares_nothing() {
    let daemon = Daemon::start();
    let pipewire = Pipewire::start();
    let mut session = Session::new();
    let remote = pipewire.socket();
    let call_session = "/org/freedesktop/portal/desktop/session/1_1/escaped";
    let (_peer, starting) = picking_a_screen(
        &mut session,
        &daemon,
        ITS_OWN,
        remote.to_str().unwrap(),
        call_session,
    );
    session.eventually(|session| {
        press(session, key::ESCAPE);
        drop(session.draw_on(1));
        session.draw();
        starting.is_finished()
    });
    assert_eq!(starting.join().unwrap(), (1, HashMap::new()));
    assert!(!session.compositor().captured(&session.output()));
}

#[test]
fn a_session_that_ends_while_its_screen_is_picked_is_told_no_at_once() {
    let daemon = Daemon::start();
    let pipewire = Pipewire::start();
    let mut session = Session::new();
    let remote = pipewire.socket();
    let call_session = "/org/freedesktop/portal/desktop/session/1_1/ended";
    let (peer, starting) = picking_a_screen(
        &mut session,
        &daemon,
        ITS_OWN,
        remote.to_str().unwrap(),
        call_session,
    );
    // Up: the first screen is dimmed.
    session.eventually(|session| {
        drop(session.draw_on(1));
        session.draw().at(5, 5) != BACKGROUND
    });
    peer.call_method(Some(NAME), call_session, Some(SESSION), "Close", &())
        .unwrap();
    assert_eq!(answered(&mut session, starting), (1, HashMap::new()));
    // The picker stays until the user is done, and nothing comes of it.
    let screen = (WIDTH + 100, 100);
    session.eventually(|session| {
        point(session, screen);
        press(session, key::RETURN);
        drop(session.draw_on(1));
        session.draw().at(5, 5) == BACKGROUND
    });
    // Once the portal has heard what was picked, which it says on its own
    // time, the next is asked about as the first was, and not refused as
    // asked while another is.
    wait(&mut session, Duration::from_millis(300), || {});
    let next = "/org/freedesktop/portal/desktop/session/1_1/next";
    make(&peer, next, HIDDEN);
    let starting = begin(&daemon, next);
    session.eventually(|session| {
        let looks = omorfos_config::Theme::default();
        let looks = looks.style(omorfos_config::Part::Launcher, omorfos_config::States::NONE);
        let shown = looks.background;
        let shown = u32::from_be_bytes([shown.alpha, shown.red, shown.green, shown.blue]);
        // On the screen the pointer is on.
        let up = session.draw_on(1).at(160, 120) == shown;
        up || session.draw().at(WIDTH / 2, HEIGHT / 2) == shown || starting.is_finished()
    });
    assert!(!starting.is_finished(), "asked about, not refused");
    assert!(!session.compositor().captured(&session.output()));
}

#[test]
fn without_leave_to_take_pictures_two_screens_cannot_be_picked_from() {
    let daemon = Daemon::start();
    let pipewire = Pipewire::start();
    let mut session = Session::new();
    let remote = pipewire.socket();
    let call_session = "/org/freedesktop/portal/desktop/session/1_1/unseen";
    let roles = Roles::PANEL.with(Roles::OVERLAY);
    let (_peer, starting) = picking_a_screen(
        &mut session,
        &daemon,
        roles,
        remote.to_str().unwrap(),
        call_session,
    );
    assert_eq!(answered(&mut session, starting).0, 2);
}

#[test]
fn a_screen_that_changes_size_goes_on_being_shared_at_its_new_size() {
    let daemon = Daemon::start();
    let pipewire = Pipewire::start();
    let mut session = Session::new();
    let mut app = session.connect();
    session.open(&mut app, "app", (200, 100), RED);
    let remote = pipewire.socket();
    let _running = start(&mut session, &daemon, ITS_OWN, remote.to_str().unwrap());
    let call_session = "/org/freedesktop/portal/desktop/session/1_1/resized";
    let (_peer, node) = sharing(&mut session, &daemon, call_session);
    let mut viewer = Viewer::new(&remote);
    session.eventually(|session| {
        session.draw();
        viewer.turn(node);
        viewer.at(WIDTH, (10, 10)) == Some(RED)
    });
    assert_eq!(viewer.pixels(), 640 * 480);

    session.resize_output((320, 240));
    session.eventually(|session| {
        session.draw();
        viewer.turn(node);
        viewer.pixels() == 320 * 240 && viewer.at(320, (10, 10)) == Some(RED)
    });
    assert_eq!(viewer.at(320, (300, 200)), Some(BACKGROUND));
    assert!(
        session.compositor().captured(&session.output()),
        "and it is still shared"
    );
}

#[test]
fn a_screen_shared_in_the_memory_of_the_card_goes_on_at_its_new_size() {
    let Some(card) = render_node() else {
        let said = "not tried: there is no graphics card this user may use";
        let _ = writeln!(std::io::stderr(), "{said}");
        return;
    };
    let daemon = Daemon::start();
    let pipewire = Pipewire::start();
    let mut session = Session::new();
    session
        .compositor_mut()
        .offer_gpu_buffers(card, drawable())
        .unwrap();
    let mut app = session.connect();
    session.open(&mut app, "app", (200, 100), RED);
    let remote = pipewire.socket();
    let _running = start(&mut session, &daemon, ITS_OWN, remote.to_str().unwrap());
    let called = "/org/freedesktop/portal/desktop/session/1_1/card_resized";
    let (_peer, node) = sharing(&mut session, &daemon, called);
    let mut viewer = Viewer::reading(&remote, true);
    session.eventually(|session| {
        session.draw();
        viewer.turn(node);
        viewer.at(WIDTH, (10, 10)) == Some(RED)
    });
    session.resize_output((320, 240));
    session.eventually(|session| {
        session.draw();
        viewer.turn(node);
        viewer.pixels() == 320 * 240 && viewer.at(320, (10, 10)) == Some(RED)
    });
    assert_eq!(viewer.at(320, (300, 200)), Some(BACKGROUND));
    assert!(viewer.seen().card, "memory of the card still");
    assert!(!viewer.seen().unreadable, "a buffer no reader could read");
}
