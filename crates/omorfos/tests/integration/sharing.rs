//! Sharing the screen through the portal: an application asks, the user
//! is asked, and what it is given is a stream in PipeWire of what the
//! screen shows, which it reads for as long as the session lasts.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::OsString;
use std::io::Cursor;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::rc::Rc;
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

use crate::common::{Event, HEIGHT, Session, WIDTH, key};
use crate::windows::{BACKGROUND, BLUE, GREEN, RED};
use omorfos::Input;
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::Shape;

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

/// Draws until the question is on the screen.
fn asked(session: &mut Session) {
    let looks = omorfos_config::Theme::default();
    let looks = looks
        .style(omorfos_config::Part::Launcher, omorfos_config::States::NONE)
        .background;
    let shown = u32::from_be_bytes([looks.alpha, looks.red, looks.green, looks.blue]);
    session.eventually(|session| session.draw().at(WIDTH / 2, HEIGHT / 2) == shown);
}

fn press(session: &mut Session, code: smithay::input::keyboard::Keycode) {
    for pressed in [true, false] {
        session
            .compositor_mut()
            .input(Input::Key { code, pressed }, 0);
    }
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
    seen: Rc<RefCell<Option<Vec<u8>>>>,
    /// Where it was last told the pointer is, where its position is in its
    /// picture, and the colour of that picture a little inside it.
    pointer: Rc<Cell<Option<Pointed>>>,
    link: Option<pw::link::Link>,
}

/// The pictures a viewer wants: blue first, of any size.
fn wanted() -> Vec<u8> {
    let wants = spa::pod::object!(
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
        let seen = Rc::new(RefCell::new(None));
        let pointer = Rc::new(Cell::new(None));
        let (named, kept, told) = (Rc::clone(&node), Rc::clone(&seen), Rc::clone(&pointer));
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
                    let cursor = buffer.find_meta::<spa::buffer::meta::MetaCursor>();
                    if let Some(cursor) = cursor.filter(|cursor| cursor.is_valid()) {
                        let (at, tip) = (cursor.position(), cursor.hotspot());
                        let bitmap = cursor.bitmap().and_then(|bitmap| bitmap.bitmap_data());
                        let colour = bitmap.map(|bytes| {
                            let at = (4 * 256 + 4) * 4;
                            u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
                        });
                        let was = told.get().map(|(_, _, colour)| colour);
                        let colour = colour.or(was).unwrap_or_default();
                        told.set(Some(((at.x, at.y), (tip.x, tip.y), colour)));
                    }
                    let data = &mut buffer.datas_mut()[0];
                    let size = usize::try_from(data.chunk().size()).unwrap();
                    let picture = data.data().map(|bytes| bytes[..size].to_vec());
                    if picture.as_ref().is_some_and(|picture| !picture.is_empty()) {
                        *kept.borrow_mut() = picture;
                    }
                }
            })
            .register()
            .unwrap();
        let bytes = wanted();
        let flags = pw::stream::StreamFlags::MAP_BUFFERS;
        stream
            .connect(
                spa::utils::Direction::Input,
                None,
                flags,
                &mut [Pod::from_bytes(&bytes).unwrap()],
            )
            .unwrap();
        Self {
            main,
            _context: context,
            core,
            stream,
            _listener: listener,
            node,
            seen,
            pointer,
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
    fn at(&self, width: i32, (x, y): (i32, i32)) -> Option<u32> {
        let seen = self.seen.borrow();
        let at = usize::try_from((y * width + x) * 4).unwrap();
        seen.as_ref()
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
    // What changes on the screen is what is read next, each time, as the
    // memory it is put in goes round.
    for colour in [GREEN, RED, GREEN, RED, GREEN, RED, GREEN] {
        app.fill(&window.surface, (200, 100), colour);
        session.roundtrip(&mut app);
        session.eventually(|session| {
            session.draw();
            viewer.turn(node);
            viewer.at(WIDTH, (10, 10)) == Some(colour)
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
        viewer.pointer.get() == Some(((300, 200), (1, 1), BLUE))
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
        viewer.pointer.get() == Some(((310, 205), (1, 1), BLUE))
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
        viewer.pointer.get() == Some(((50, 50), (1, 1), GREEN))
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
            .pointer
            .get()
            .is_some_and(|(_, _, colour)| colour == 0)
    });
    session.compositor_mut().input(over(320.0, 240.0), 0);
    session.eventually(|session| {
        session.draw();
        viewer.turn(node);
        viewer.pointer.get() == Some(((320, 240), (1, 1), BLUE))
    });

    // What reads it anew is told at once, though the pointer does not move.
    drop(viewer);
    let mut viewer = Viewer::new(&remote);
    session.eventually(|session| {
        session.draw();
        viewer.turn(node);
        viewer.pointer.get() == Some(((320, 240), (1, 1), BLUE))
    });
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
fn a_screen_that_changes_ends_its_sharing_and_the_session_is_closed() {
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
    assert_eq!(
        shared(&mut session, &daemon, call_session, key::RETURN).0,
        0
    );
    session.resize_output((320, 240));
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
