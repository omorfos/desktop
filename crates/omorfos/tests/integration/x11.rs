//! Programs written for X11, which run through a real `Xwayland`.
//!
//! The program in these tests talks to the X server on a thread of its
//! own while the compositor carries on, since the X server answers some of
//! what it is asked only after asking the compositor.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::time::Duration;

use omorfos::Input;
use omorfos_config::Action;
use omorfos_core::geometry::Rect;
use x11rb::connection::Connection;
use x11rb::protocol::Event as Told;
use x11rb::protocol::xproto::{
    AtomEnum, ClientMessageEvent, ConfigureWindowAux, ConnectionExt, CreateWindowAux, EventMask,
    KeyButMask, PropMode, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use crate::common::{HEIGHT, Session, WIDTH, key};
use crate::windows::{BACKGROUND, BLUE, GREEN, RED};

const LEFT: u32 = 0x110;
const FOCUSED_TITLEBAR: u32 = 0xff23_262e;

/// Runs `work`, which talks to the X server, while the compositor carries
/// on.
fn meanwhile<T: Send>(session: &mut Session, work: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|scope| {
        let working = scope.spawn(work);
        while !working.is_finished() {
            session.step();
            std::thread::sleep(Duration::from_millis(1));
        }
        working.join().unwrap()
    })
}

/// What programs written for X11 are told to connect to.
fn display(session: &Session) -> Option<String> {
    let environment = session.compositor().session_environment();
    let mut environment = environment.into_iter();
    let display = environment.find(|(name, _)| *name == "DISPLAY");
    display.map(|(_, value)| value.into_string().unwrap())
}

/// A program connected to the session's X server.
fn connect(session: &mut Session) -> RustConnection {
    let display = display(session).unwrap();
    meanwhile(session, || {
        RustConnection::connect(Some(&display)).unwrap().0
    })
}

/// A session with an X server, and a program connected to it.
fn session() -> (Session, RustConnection) {
    let mut session = Session::new();
    session.compositor_mut().start_xwayland(None);
    let program = connect(&mut session);
    (session, program)
}

/// Has the program make a window of `size` filled with `color`, called
/// `name`, without showing it, and returns its number. One that is
/// `unmanaged` places itself, as a menu does.
fn create(
    session: &mut Session,
    program: &RustConnection,
    name: &str,
    (x, y, width, height): (i16, i16, u16, u16),
    (color, unmanaged): (u32, bool),
) -> u32 {
    meanwhile(session, || {
        let screen = &program.setup().roots[0];
        let window = program.generate_id().unwrap();
        let events = EventMask::STRUCTURE_NOTIFY
            | EventMask::KEY_PRESS
            | EventMask::BUTTON_PRESS
            | EventMask::FOCUS_CHANGE;
        let look = CreateWindowAux::new()
            .background_pixel(color)
            .override_redirect(u32::from(unmanaged))
            .event_mask(events);
        let (depth, class) = (screen.root_depth, WindowClass::INPUT_OUTPUT);
        let visual = screen.root_visual;
        program
            .create_window(
                depth,
                window,
                screen.root,
                x,
                y,
                width,
                height,
                0,
                class,
                visual,
                &look,
            )
            .unwrap();
        let (class, text) = (AtomEnum::WM_CLASS, AtomEnum::STRING);
        let named = format!("{name}\0{name}\0");
        program
            .change_property8(PropMode::REPLACE, window, class, text, named.as_bytes())
            .unwrap();
        program.flush().unwrap();
        window
    })
}

/// Gives a window its title.
fn entitle(session: &mut Session, program: &RustConnection, window: u32, title: &str) {
    meanwhile(session, || {
        let (name, text) = (AtomEnum::WM_NAME, AtomEnum::STRING);
        program
            .change_property8(PropMode::REPLACE, window, name, text, title.as_bytes())
            .unwrap();
        program.flush().unwrap();
    });
}

/// Has the program show a window, or take it from the screen.
fn show(session: &mut Session, program: &RustConnection, window: u32, shown: bool) {
    meanwhile(session, || {
        if shown {
            program.map_window(window).unwrap();
        } else {
            program.unmap_window(window).unwrap();
        }
        program.flush().unwrap();
    });
}

/// Has the program open a window of `size` filled with `color`, called
/// `name`, and waits for it to be part of the desktop.
fn open(
    session: &mut Session,
    program: &RustConnection,
    name: &str,
    size: (u16, u16),
    color: u32,
) -> u32 {
    let already = windows(session).len();
    let area = (0, 0, size.0, size.1);
    let window = create(session, program, name, area, (color, false));
    entitle(session, program, window, &format!("{name} window"));
    show(session, program, window, true);
    session.eventually(|session| windows(session).len() > already);
    window
}

/// The applications and titles of the windows on the desktop.
fn windows(session: &Session) -> Vec<(String, String)> {
    let frames = session.compositor().desktop().frames();
    frames
        .map(|frame| {
            let window = frame.window();
            (window.app_id.clone(), window.title.clone())
        })
        .collect()
}

/// Where the frame of the first window is, and whether it is maximized
/// and whether it is fullscreen.
fn frame(session: &Session) -> (Rect, bool, bool) {
    let frame = session.compositor().desktop().frames().next().unwrap();
    (frame.geometry, frame.maximized, frame.fullscreen)
}

/// How large the program finds its window to be.
fn size_of(session: &mut Session, program: &RustConnection, window: u32) -> (u16, u16) {
    meanwhile(session, || {
        let geometry = program.get_geometry(window).unwrap().reply().unwrap();
        (geometry.width, geometry.height)
    })
}

/// The number the X server knows a name by.
fn atom(session: &mut Session, program: &RustConnection, name: &str) -> u32 {
    meanwhile(session, || {
        let atom = program.intern_atom(false, name.as_bytes()).unwrap();
        atom.reply().unwrap().atom
    })
}

/// Has the program ask the window manager for something about a window,
/// the way programs written for X11 do.
fn ask(session: &mut Session, program: &RustConnection, window: u32, what: &str, how: [u32; 5]) {
    let kind = atom(session, program, what);
    meanwhile(session, || {
        let root = program.setup().roots[0].root;
        let message = ClientMessageEvent::new(32, window, kind, how);
        let whom = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
        program.send_event(false, root, whom, message).unwrap();
        program.flush().unwrap();
    });
}

/// Waits for the program to be told something that `wanted` picks out.
fn told<T: Send>(
    session: &mut Session,
    program: &RustConnection,
    wanted: impl Fn(&Told) -> Option<T> + Send + Sync,
) -> T {
    meanwhile(session, || {
        for _ in 0..5000 {
            while let Some(event) = program.poll_for_event().unwrap() {
                if let Some(found) = wanted(&event) {
                    return found;
                }
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        unreachable!("the program was never told what was expected");
    })
}

fn press(session: &mut Session, code: smithay::input::keyboard::Keycode, pressed: bool) {
    session
        .compositor_mut()
        .input(Input::Key { code, pressed }, 0);
}

fn click(session: &mut Session, x: i32, y: i32) {
    let (x, y) = (f64::from(x), f64::from(y));
    session.compositor_mut().input(Input::Motion { x, y }, 0);
    for pressed in [true, false] {
        let button = Input::Button {
            button: LEFT,
            pressed,
        };
        session.compositor_mut().input(button, 0);
    }
}

#[test]
fn a_window_of_a_program_written_for_x11_is_a_window_like_any_other() {
    let (mut session, program) = session();
    assert_eq!(session.draw().at(100, 80), BACKGROUND);

    open(&mut session, &program, "xapp", (200, 100), 0x00ff_0000);

    assert_eq!(
        windows(&session),
        [("xapp".to_owned(), "xapp window".to_owned())]
    );
    session.eventually(|session| session.draw().at(100, 80) == RED);
    let picture = session.draw();
    assert_eq!(picture.at(190, 14), FOCUSED_TITLEBAR, "in a frame of ours");
}

#[test]
fn what_is_typed_goes_to_the_window_of_a_program_written_for_x11_that_has_focus() {
    let (mut session, program) = session();
    open(&mut session, &program, "xapp", (200, 100), 0x00ff_0000);
    let pressed = |event: &Told| match event {
        Told::KeyPress(key) => Some((key.detail, key.state)),
        _ => None,
    };

    press(&mut session, key::A, true);
    press(&mut session, key::A, false);
    let (which, held) = told(&mut session, &program, pressed);
    assert_eq!(
        (u32::from(which), held),
        (key::A.raw(), KeyButMask::default())
    );

    // What is held down with it is told too.
    press(&mut session, key::SHIFT, true);
    told(&mut session, &program, pressed);
    press(&mut session, key::A, true);
    let (which, held) = told(&mut session, &program, pressed);
    assert_eq!((u32::from(which), held), (key::A.raw(), KeyButMask::SHIFT));
    press(&mut session, key::A, false);
    press(&mut session, key::SHIFT, false);

    // A window of another program takes the keyboard from it.
    let mut other = session.connect();
    session.open(&mut other, "app", (50, 50), GREEN);
    let lost = |event: &Told| matches!(event, Told::FocusOut(_)).then_some(());
    told(&mut session, &program, lost);
    press(&mut session, key::A, true);
    session.roundtrip(&mut other);
    assert!(!other.take_events().is_empty(), "typed into the other");
}

#[test]
fn a_program_written_for_x11_is_asked_to_close_its_window_like_any_other() {
    let (mut session, program) = session();
    open(&mut session, &program, "xapp", (200, 100), 0x00ff_0000);

    session.compositor_mut().perform(&Action::Close);

    // It has not said it would like to be asked, so the window is taken.
    session.eventually(|session| windows(session).is_empty());

    // By the time the window manager hears of the program's next window,
    // it has heard that the first is gone for good.
    open(&mut session, &program, "next", (200, 100), 0x0000_ff00);
    assert_eq!(windows(&session)[0].0, "next");
}

#[test]
fn a_window_a_program_takes_from_the_screen_leaves_the_desktop_and_may_return() {
    let (mut session, program) = session();
    let window = open(&mut session, &program, "xapp", (200, 100), 0x00ff_0000);

    show(&mut session, &program, window, false);
    session.eventually(|session| windows(session).is_empty());

    show(&mut session, &program, window, true);
    session.eventually(|session| windows(session).len() == 1);
    session.eventually(|session| session.draw().at(100, 80) == RED);
}

#[test]
fn a_window_is_the_size_its_program_asked_for_before_it_was_shown_and_stays_so() {
    let (mut session, program) = session();
    let area = (0, 0, 200, 100);
    let window = create(&mut session, &program, "xapp", area, (0x00ff_0000, false));
    let resize = |session: &mut Session, wide: u32, tall: Option<u32>| {
        meanwhile(session, || {
            let size = ConfigureWindowAux::new().width(wide).height(tall);
            program.configure_window(window, &size).unwrap();
            program.flush().unwrap();
        });
    };
    resize(&mut session, 300, Some(150));
    resize(&mut session, 320, None);

    show(&mut session, &program, window, true);
    session.eventually(|session| windows(session).len() == 1);
    session.eventually(|session| session.draw().at(310, 160) == RED);
    assert_eq!(size_of(&mut session, &program, window), (320, 150));

    // Once it is on the desktop, the desktop says how large it is.
    resize(&mut session, 50, Some(50));
    for _ in 0..50 {
        session.step();
    }
    assert_eq!(size_of(&mut session, &program, window), (320, 150));
}

#[test]
fn a_window_of_a_program_written_for_x11_is_maximized_and_made_fullscreen_like_any_other() {
    let (mut session, program) = session();
    let window = open(&mut session, &program, "xapp", (200, 100), 0x00ff_0000);
    let (wide, tall) = (
        u16::try_from(WIDTH).unwrap(),
        u16::try_from(HEIGHT).unwrap(),
    );

    session.compositor_mut().perform(&Action::Maximize);
    session.step();
    assert!(frame(&session).1);
    // All of the screen, less the frame around it.
    let filling = (wide - 2, tall - 29);
    assert_eq!(size_of(&mut session, &program, window), filling);
    // And the program draws it that large, once it is told that what it
    // drew before has been shown.
    session.eventually(|session| {
        session.send_frames(Duration::from_millis(16));
        session.draw().at(WIDTH - 20, HEIGHT - 20) == RED
    });

    session.compositor_mut().perform(&Action::Fullscreen);
    session.step();
    assert_eq!(size_of(&mut session, &program, window), (wide, tall));

    session.compositor_mut().perform(&Action::Fullscreen);
    session.compositor_mut().perform(&Action::Maximize);
    session.step();
    assert_eq!(size_of(&mut session, &program, window), (200, 100));
}

#[test]
fn a_program_written_for_x11_can_ask_for_its_window_to_be_maximized_or_fullscreen() {
    let (mut session, program) = session();
    let window = open(&mut session, &program, "xapp", (200, 100), 0x00ff_0000);
    let upright = atom(&mut session, &program, "_NET_WM_STATE_MAXIMIZED_VERT");
    let level = atom(&mut session, &program, "_NET_WM_STATE_MAXIMIZED_HORZ");
    let whole = atom(&mut session, &program, "_NET_WM_STATE_FULLSCREEN");
    let (add, remove) = (1, 0);
    let state = |session: &mut Session, how: u32, first: u32, second: u32| {
        let how = [how, first, second, 1, 0];
        ask(session, &program, window, "_NET_WM_STATE", how);
    };

    state(&mut session, add, upright, level);
    session.eventually(|session| frame(session).1);
    state(&mut session, remove, upright, level);
    session.eventually(|session| !frame(session).1);

    state(&mut session, add, whole, 0);
    session.eventually(|session| frame(session).2);
    state(&mut session, remove, whole, 0);
    session.eventually(|session| !frame(session).2);
}

/// Moves the pointer on, a little at a time, until the first window's
/// frame is no longer `before`: until a drag that a program asked for has
/// begun and taken hold.
fn drag_until_changed(session: &mut Session, from: (i32, i32), before: Rect) {
    for step in 0..5000 {
        let (x, y) = (f64::from(from.0 + step), f64::from(from.1 + step));
        session.compositor_mut().input(Input::Motion { x, y }, 0);
        session.step();
        if frame(session).0 != before {
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    unreachable!("the frame was never dragged");
}

#[test]
fn a_program_written_for_x11_can_ask_for_its_window_to_be_moved_or_resized() {
    let (mut session, program) = session();
    let window = open(&mut session, &program, "xapp", (200, 100), 0x00ff_0000);
    let (before, ..) = frame(&session);
    let release = Input::Button {
        button: LEFT,
        pressed: false,
    };

    // Held by the middle of the window and carried off.
    let moving = [100, 60, 8, 1, 1];
    ask(&mut session, &program, window, "_NET_WM_MOVERESIZE", moving);
    drag_until_changed(&mut session, (100, 60), before);
    let (moved, ..) = frame(&session);
    assert_ne!(moved.origin, before.origin);
    assert_eq!(moved.size, before.size);
    session.compositor_mut().input(release, 0);

    // Held near its bottom right-hand corner, which is the one dragged.
    let corner = (
        moved.origin.x + moved.size.width.cast_signed() - 5,
        moved.origin.y + moved.size.height.cast_signed() - 5,
    );
    let (x, y) = (f64::from(corner.0), f64::from(corner.1));
    session.compositor_mut().input(Input::Motion { x, y }, 0);
    session.step();
    let sizing = [0, 0, 4, 1, 1];
    ask(&mut session, &program, window, "_NET_WM_MOVERESIZE", sizing);
    drag_until_changed(&mut session, corner, moved);
    let (sized, ..) = frame(&session);
    assert_eq!(sized.origin, moved.origin);
    assert!(sized.size.width > moved.size.width, "{sized:?}");
    assert!(sized.size.height > moved.size.height, "{sized:?}");
    session.compositor_mut().input(release, 0);

    // A program that asks about a window that is not on the desktop is
    // not listened to.
    show(&mut session, &program, window, false);
    session.eventually(|session| windows(session).is_empty());
    ask(&mut session, &program, window, "_NET_WM_MOVERESIZE", moving);
    let upright = atom(&mut session, &program, "_NET_WM_STATE_MAXIMIZED_VERT");
    let level = atom(&mut session, &program, "_NET_WM_STATE_MAXIMIZED_HORZ");
    let maximize = [1, upright, level, 1, 0];
    ask(&mut session, &program, window, "_NET_WM_STATE", maximize);
    for _ in 0..50 {
        session.step();
    }
    assert_eq!(windows(&session), []);
}

#[test]
fn a_window_goes_by_the_title_its_program_gives_it_as_that_changes() {
    let (mut session, program) = session();
    let window = open(&mut session, &program, "xapp", (200, 100), 0x00ff_0000);

    entitle(&mut session, &program, window, "another title");

    let renamed = [("xapp".to_owned(), "another title".to_owned())];
    session.eventually(|session| windows(session) == renamed);

    // Something else about it changing is no change of title.
    meanwhile(&mut session, || {
        let (class, text) = (AtomEnum::WM_CLASS, AtomEnum::STRING);
        program
            .change_property8(PropMode::REPLACE, window, class, text, b"other\0other\0")
            .unwrap();
        program.flush().unwrap();
    });
    for _ in 0..50 {
        session.step();
    }
    assert_eq!(windows(&session), renamed);
}

#[test]
fn a_window_that_is_not_being_shown_is_left_where_it_was() {
    let (mut session, program) = session();
    let window = open(&mut session, &program, "xapp", (200, 100), 0x00ff_0000);

    session.compositor_mut().perform(&Action::NextWorkspace);
    session.step();
    assert_eq!(session.draw().at(100, 80), BACKGROUND);
    assert_eq!(size_of(&mut session, &program, window), (200, 100));

    session.compositor_mut().perform(&Action::PreviousWorkspace);
    session.step();
    assert_eq!(session.draw().at(100, 80), RED);
}

#[test]
fn a_menu_of_a_program_written_for_x11_is_shown_where_the_program_puts_it() {
    let (mut session, program) = session();
    open(&mut session, &program, "xapp", (400, 300), 0x00ff_0000);
    session.eventually(|session| session.draw().at(310, 210) == RED);

    // A menu places itself, and is over the window it belongs to.
    let area = (300, 200, 50, 40);
    let menu = create(&mut session, &program, "menu", area, (0x0000_00ff, true));
    show(&mut session, &program, menu, true);
    session.eventually(|session| session.draw().at(310, 210) == BLUE);
    assert_eq!(windows(&session).len(), 1, "it is no window of the desktop");
    assert_eq!(session.draw().at(360, 210), RED, "beside it");

    // The pointer reaches it.
    click(&mut session, 310, 210);
    let clicked = |event: &Told| match event {
        Told::ButtonPress(press) => Some(press.event),
        _ => None,
    };
    assert_eq!(told(&mut session, &program, clicked), menu);

    show(&mut session, &program, menu, false);
    session.eventually(|session| session.draw().at(310, 210) == RED);
}

#[test]
fn programs_written_for_x11_do_not_run_when_that_is_turned_off() {
    let mut session = Session::with_config(&[("config.yaml", "xwayland: { enabled: false }\n")]);
    session.compositor_mut().start_xwayland(None);
    session.step();
    assert_eq!(display(&session), None);
    assert_eq!(session.compositor().problems(), [] as [String; 0]);
}

#[test]
fn the_x_server_is_started_once_and_a_number_that_is_taken_is_a_problem() {
    let (mut session, _program) = session();
    let first = display(&session).unwrap();
    session.compositor_mut().start_xwayland(None);
    assert_eq!(display(&session), Some(first.clone()), "the same one");

    let taken = first.trim_start_matches(':').parse().unwrap();
    let mut other = Session::new();
    other.compositor_mut().start_xwayland(Some(taken));
    other.step();
    assert_eq!(display(&other), None);
    let problems = other.compositor().problems().join("\n");
    assert!(
        problems.contains("cannot run programs written for X11"),
        "{problems}"
    );
}

/// The number of the process that is the session's X server: the one
/// started as `Xwayland` with the session's number.
fn server(session: &Session) -> Option<u32> {
    let number = display(session)?;
    let ours = |process: &std::fs::DirEntry| {
        let line = std::fs::read(process.path().join("cmdline")).unwrap_or_default();
        let mut words = line.split(|byte| *byte == 0);
        let server = words.next().is_some_and(|word| word.ends_with(b"Xwayland"));
        server && words.next() == Some(number.as_bytes())
    };
    let mut processes = std::fs::read_dir("/proc").unwrap().flatten();
    let process = processes.find(ours)?;
    process.file_name().to_str()?.parse().ok()
}

/// Kills the session's X server, and waits for it to be gone.
fn kill_server(session: &mut Session) {
    session.eventually(|session| server(session).is_some());
    let process = server(session).unwrap();
    let killed = std::process::Command::new("kill")
        .args(["-9", &process.to_string()])
        .status();
    assert!(killed.unwrap().success());
    session.eventually(|session| server(session) != Some(process));
}

/// Kills the session's X server, and waits for another to have been
/// started in its place.
fn kill_and_recover(session: &mut Session) {
    kill_server(session);
    session.eventually(|session| server(session).is_some());
}

#[test]
fn an_x_server_that_dies_is_started_again_where_programs_expect_to_find_it() {
    let (mut session, program) = session();
    open(&mut session, &program, "xapp", (200, 100), 0x00ff_0000);
    let mut other = session.connect();
    session.open(&mut other, "app", (50, 50), GREEN);
    assert_eq!(windows(&session).len(), 2);
    let number = display(&session);

    kill_and_recover(&mut session);

    // Its windows went with it, and those of other programs did not.
    session.eventually(|session| windows(session).len() == 1);
    assert_eq!(windows(&session)[0].0, "app");
    session.roundtrip(&mut other);
    let problems = session.compositor().problems().join("\n");
    assert!(problems.contains("has been started again"), "{problems}");

    // A program started now finds an X server where the last one was.
    assert_eq!(display(&session), number);
    let again = connect(&mut session);
    open(&mut session, &again, "back", (200, 100), 0x0000_00ff);
    assert_eq!(windows(&session).len(), 2);
}

#[test]
fn an_x_server_that_dies_before_it_is_ready_is_started_again_too() {
    let mut session = Session::new();
    session.compositor_mut().start_xwayland(None);

    kill_and_recover(&mut session);

    let program = connect(&mut session);
    open(&mut session, &program, "xapp", (200, 100), 0x00ff_0000);
    assert_eq!(windows(&session).len(), 1);
}

#[test]
fn an_x_server_that_keeps_dying_is_given_up_on() {
    let (mut session, _program) = session();
    for _ in 0..3 {
        kill_and_recover(&mut session);
    }

    kill_server(&mut session);
    session.eventually(|session| display(session).is_none());
    let problems = session.compositor().problems().join("\n");
    assert!(problems.contains("keeps stopping"), "{problems}");

    // Once it has gone a while without stopping, it is trusted again.
    session.advance(Duration::from_secs(120));
    session.compositor_mut().start_xwayland(None);
    assert!(display(&session).is_some());
    kill_and_recover(&mut session);
    let program = connect(&mut session);
    open(&mut session, &program, "xapp", (200, 100), 0x00ff_0000);
}
