//! The launcher, as a client of a real compositor, and the compositor's
//! part in opening it.
//!
//! What the launcher offers and what a key does to it is tested with the
//! launcher itself. Here it is the program: that it gets onto the screen,
//! that the keys typed reach it, and that what is picked is started.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::ffi::OsString;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use omorfos::{Input, Scale};
use omorfos_config::{Action, Color, Part, State, States, Theme};
use omorfos_ipc::Client as Ipc;
use omorfos_launcher::{App, Launcher, Picker};
use smithay::input::keyboard::Keycode;
use smithay::reexports::wayland_server::ListeningSocket;

use crate::common::{Event, HEIGHT, Session, WIDTH, half_way, key};
use crate::windows::{BACKGROUND, RED};

const LEFT: u32 = 0x110;

const F: Keycode = Keycode::new(41);
const I: Keycode = Keycode::new(31);
const L: Keycode = Keycode::new(46);
const BACKSPACE: Keycode = Keycode::new(22);
const DOWN: Keycode = Keycode::new(116);
const UP: Keycode = Keycode::new(111);

fn argb(color: Color) -> u32 {
    u32::from_be_bytes([color.alpha, color.red, color.green, color.blue])
}

/// The colour of the launcher where nothing is on it.
fn surface() -> u32 {
    argb(
        Theme::default()
            .style(Part::Launcher, States::NONE)
            .background,
    )
}

/// The colour of the row of the program that would be started.
fn lit() -> u32 {
    let chosen = States::NONE.with(State::Active);
    argb(
        Theme::default()
            .style(Part::LauncherItem, chosen)
            .background,
    )
}

/// A program that leaves a mark in the session's directory when started.
fn marking(session: &Session, name: &str) -> App {
    let mark = session.config.path().join(name.to_lowercase());
    App {
        name: name.to_owned(),
        command: ["sh", "-c", &format!("printf x >> '{}'", mark.display())]
            .map(str::to_owned)
            .into(),
        terminal: false,
        description: String::new(),
        icon: String::new(),
    }
}

fn mark(session: &Session, name: &str) -> String {
    let path = session.config.path().join(name);
    std::fs::read_to_string(path).unwrap_or_default()
}

/// A launcher in the session offering Files, Firefox and Lens, which each
/// leave a mark when started.
fn launcher(session: &mut Session) -> Launcher {
    let socket = session.listen();
    let apps = ["Files", "Firefox", "Lens"].map(|name| marking(session, name));
    let (ours, theirs) = UnixStream::pair().unwrap();
    session.compositor_mut().insert_client(ours).unwrap();
    let commands = Ipc::connect(&socket).unwrap();
    let config = session.config.path().to_owned();
    let icons = omorfos_launcher::Icons::default();
    Launcher::new(theirs, commands, &config, apps.into(), icons).unwrap()
}

/// Lets the launcher and the compositor talk until they have nothing more
/// to say to each other. Returns whether the launcher is done with.
fn settle(session: &mut Session, launcher: &mut Launcher) -> bool {
    let mut done = false;
    for _ in 0..20 {
        done |= launcher.turn(Duration::ZERO).unwrap();
        session.step();
    }
    done
}

fn press(session: &mut Session, code: Keycode) {
    for pressed in [true, false] {
        let input = Input::Key { code, pressed };
        session.compositor_mut().input(input, 0);
    }
}

/// Where on the screen the launcher's top-left corner is: it is centred.
fn corner() -> (i32, i32) {
    let (width, height) = Picker::size(&Theme::default());
    (
        (WIDTH - width.cast_signed()) / 2,
        (HEIGHT - height.cast_signed()) / 2,
    )
}

/// A point on the screen towards the right-hand end of a row of the list,
/// clear of the program's name.
fn row(place: i32) -> (i32, i32) {
    let (left, top) = corner();
    // A border and padding, the text field and the room under it.
    let list = top + 9 + 32 + 8;
    (left + 400, list + place * 28 + 14)
}

#[test]
fn the_launcher_opens_in_the_middle_of_the_screen_over_everything() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (WIDTH, HEIGHT), RED);
    let mut launcher = launcher(&mut session);

    assert!(!settle(&mut session, &mut launcher));

    let picture = session.draw();
    let (left, top) = corner();
    assert_eq!(
        picture.at(left - 2, top - 2),
        RED,
        "the window is still there"
    );
    assert_eq!(picture.at(left + 4, top + 150), surface());
    // The first program is the one that would be started.
    let (x, first) = row(0);
    assert_eq!(picture.at(x, first), lit());
    assert_eq!(picture.at(x, row(1).1), surface());
}

#[test]
fn what_is_typed_reaches_the_launcher_and_enter_starts_the_program() {
    let mut session = Session::new();
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);

    // "fi" leaves Files and Firefox; down chooses Firefox.
    press(&mut session, F);
    press(&mut session, I);
    press(&mut session, key::SHIFT);
    press(&mut session, DOWN);
    assert!(!settle(&mut session, &mut launcher));
    let picture = session.draw();
    let x = row(0).0;
    assert_eq!(picture.at(x, row(1).1), lit());
    assert_eq!(picture.at(x, row(2).1), surface(), "Lens does not match");

    press(&mut session, key::RETURN);
    assert!(settle(&mut session, &mut launcher), "it is done with");
    session.eventually(|session| mark(session, "firefox") == "x");
    assert_eq!(mark(&session, "files"), "");

    // Gone, it leaves the screen.
    drop(launcher);
    session.eventually(|session| session.draw().at(WIDTH / 2, HEIGHT / 2) == BACKGROUND);
}

#[test]
fn the_arrows_and_backspace_work_as_they_do_in_any_list() {
    let mut session = Session::new();
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);

    press(&mut session, L);
    settle(&mut session, &mut launcher);
    press(&mut session, BACKSPACE);
    press(&mut session, DOWN);
    press(&mut session, DOWN);
    press(&mut session, UP);
    settle(&mut session, &mut launcher);
    let picture = session.draw();
    let x = row(0).0;
    assert_eq!(
        picture.at(x, row(1).1),
        lit(),
        "all three, the second chosen"
    );
    assert_eq!(picture.at(x, row(2).1), surface());

    press(&mut session, key::RETURN);
    settle(&mut session, &mut launcher);
    session.eventually(|session| mark(session, "firefox") == "x");
}

/// Lets the launcher and the compositor talk at the time `now`.
fn settle_at(session: &mut Session, launcher: &mut Launcher, now: Duration) {
    for _ in 0..20 {
        launcher.turn(now).unwrap();
        session.step();
    }
}

/// Which row of the list is lit: the program that would be started.
fn chosen(session: &mut Session) -> Option<i32> {
    let picture = session.draw();
    (0..8).find(|place| {
        let (x, y) = row(*place);
        picture.at(x, y) == lit()
    })
}

fn hold(session: &mut Session, code: Keycode, pressed: bool) {
    session
        .compositor_mut()
        .input(Input::Key { code, pressed }, 0);
}

#[test]
fn a_key_held_down_is_taken_again_and_again_until_it_is_let_go() {
    let mut session = Session::new();
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);
    let ms = Duration::from_millis;

    hold(&mut session, DOWN, true);
    settle_at(&mut session, &mut launcher, ms(0));
    assert_eq!(chosen(&mut session), Some(1), "once, when it goes down");
    assert_eq!(launcher.deadline(), Some(ms(400)), "then after a pause");
    settle_at(&mut session, &mut launcher, ms(399));
    assert_eq!(chosen(&mut session), Some(1));
    settle_at(&mut session, &mut launcher, ms(400));
    assert_eq!(chosen(&mut session), Some(2), "and then steadily");

    hold(&mut session, DOWN, false);
    hold(&mut session, UP, true);
    settle_at(&mut session, &mut launcher, ms(500));
    assert_eq!(chosen(&mut session), Some(1));
    // Thirty times a second: one more by a little after the pause.
    settle_at(&mut session, &mut launcher, ms(500 + 400 + 5));
    assert_eq!(chosen(&mut session), Some(0));
    // Another key coming up does not stop the one that is held.
    hold(&mut session, key::SHIFT, true);
    hold(&mut session, key::SHIFT, false);
    settle_at(&mut session, &mut launcher, ms(950));
    assert!(launcher.deadline().is_some());
    hold(&mut session, UP, false);
    settle_at(&mut session, &mut launcher, ms(960));
    assert_eq!(launcher.deadline(), None, "let go, it stops");
}

#[test]
fn a_launcher_left_waiting_does_not_type_a_screenful_when_it_wakes() {
    let mut session = Session::new();
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);
    hold(&mut session, F, true);
    settle_at(&mut session, &mut launcher, Duration::ZERO);
    // A minute later, in one turn, only so many are taken, and it carries
    // on from then.
    let later = Duration::from_secs(60);
    launcher.turn(later).unwrap();
    let due = launcher.deadline().unwrap();
    assert!(
        due > later && due < later + Duration::from_millis(100),
        "{due:?}"
    );
}

#[test]
fn keys_do_not_repeat_when_the_keyboard_is_set_not_to() {
    let settings = "keyboard: { repeat_rate: 0 }\n";
    let mut session = Session::with_config(&[("config.yaml", settings)]);
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);
    hold(&mut session, DOWN, true);
    settle_at(&mut session, &mut launcher, Duration::ZERO);
    assert_eq!(chosen(&mut session), Some(1));
    assert_eq!(launcher.deadline(), None);
    settle_at(&mut session, &mut launcher, Duration::from_secs(5));
    assert_eq!(chosen(&mut session), Some(1));
}

#[test]
fn a_row_gets_to_the_colour_of_being_chosen_over_the_time_the_theme_gives() {
    let theme = "launcher-item { background: #ff0000; transition: 100ms linear; }\n\
                 launcher-item:active { background: #0000ff; }\n";
    let mut session = Session::with_config(&[("theme.css", theme)]);
    let mut launcher = launcher(&mut session);
    let at = Duration::from_millis;
    let (red, blue) = (0xffff_0000, 0xff00_00ff);
    let rows = |session: &mut Session| {
        let picture = session.draw();
        let color = |place| picture.at(row(place).0, row(place).1);
        (color(0), color(1))
    };
    settle_at(&mut session, &mut launcher, at(1000));
    assert_eq!(rows(&mut session), (blue, red), "as it opens, at once");
    assert_eq!(launcher.deadline(), None, "and nothing to wait for");

    press(&mut session, DOWN);
    settle_at(&mut session, &mut launcher, at(1000));
    assert_eq!(rows(&mut session), (blue, red), "setting out");
    assert_eq!(launcher.deadline(), Some(at(1016)));

    settle_at(&mut session, &mut launcher, at(1050));
    let (first, second) = rows(&mut session);
    assert!(half_way(first), "{first:#x}");
    assert!(half_way(second), "{second:#x}");

    settle_at(&mut session, &mut launcher, at(1100));
    assert_eq!(rows(&mut session), (red, blue), "arrived");
    assert_eq!(launcher.deadline(), None, "and at rest");
}

#[test]
fn escape_closes_the_launcher_and_starts_nothing() {
    let mut session = Session::new();
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);

    press(&mut session, F);
    press(&mut session, key::ESCAPE);

    assert!(settle(&mut session, &mut launcher));
    session.advance(Duration::from_millis(50));
    assert_eq!(mark(&session, "files"), "");
    assert_eq!(mark(&session, "firefox"), "");
}

#[test]
fn the_pointer_chooses_a_program_and_a_click_starts_it() {
    let mut session = Session::new();
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);
    let (x, y) = row(2);
    let motion = |session: &mut Session, x: i32, y: i32| {
        let (x, y) = (f64::from(x), f64::from(y));
        session.compositor_mut().input(Input::Motion { x, y }, 0);
    };

    motion(&mut session, x, y - 2);
    motion(&mut session, x, y);
    assert!(!settle(&mut session, &mut launcher));
    assert_eq!(session.draw().at(x, y), lit());
    assert_eq!(session.draw().at(x, row(0).1), surface());

    for pressed in [true, false] {
        let button = Input::Button {
            button: LEFT,
            pressed,
        };
        session.compositor_mut().input(button, 0);
    }
    assert!(settle(&mut session, &mut launcher));
    session.eventually(|session| mark(session, "lens") == "x");
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
fn a_click_beside_the_launcher_closes_it_and_starts_nothing() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (WIDTH, HEIGHT), RED);
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);

    // On the launcher but on no program: it stays.
    let (left, top) = corner();
    click(&mut session, left + 4, top + 4);
    assert!(!settle(&mut session, &mut launcher));

    // Beside it, where the window underneath shows.
    click(&mut session, 5, 5);
    assert!(settle(&mut session, &mut launcher));
    session.advance(Duration::from_millis(50));
    for name in ["files", "firefox", "lens"] {
        assert_eq!(mark(&session, name), "");
    }
}

#[test]
fn a_click_closes_the_launcher_though_the_pointer_has_not_moved_since_it_opened() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (WIDTH, HEIGHT), RED);
    // The pointer rests on the window, which the launcher then covers.
    let rest = Input::Motion { x: 5.0, y: 5.0 };
    session.compositor_mut().input(rest, 0);
    session.roundtrip(&mut client);
    client.take_events();
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);

    for pressed in [true, false] {
        let button = Input::Button {
            button: LEFT,
            pressed,
        };
        session.compositor_mut().input(button, 0);
    }

    assert!(settle(&mut session, &mut launcher), "closed by the click");
    session.roundtrip(&mut client);
    let told = client.take_events();
    let pressed = told
        .iter()
        .any(|event| matches!(event, Event::PointerButton { .. }));
    assert!(!pressed, "the window underneath saw none of it: {told:?}");
}

#[test]
fn the_rest_of_the_screen_can_be_dimmed_while_the_launcher_is_open() {
    let theme = "launcher-backdrop { background: #0000ff; }\n";
    let mut session = Session::with_config(&[("theme.css", theme)]);
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);

    let picture = session.draw();
    assert_eq!(picture.at(5, 5), 0xff00_00ff);
    assert_eq!(picture.at(WIDTH - 5, HEIGHT - 5), 0xff00_00ff);
    let (left, top) = corner();
    assert_eq!(
        picture.at(left + 4, top + 150),
        surface(),
        "the box is over it"
    );
}

#[test]
fn the_launcher_shows_icons_and_descriptions_when_the_configuration_asks() {
    let settings = "launcher: { icons: true, descriptions: true }\n";
    let mut session = Session::with_config(&[("config.yaml", settings)]);
    let socket = session.listen();
    // One program, with a green picture for an icon and something to say.
    let icon = session.config.path().join("green.svg");
    let green = "<svg xmlns='http://www.w3.org/2000/svg' width='8' height='8'>\
                 <rect width='8' height='8' fill='#00ff00'/></svg>";
    std::fs::write(&icon, green).unwrap();
    let app = App {
        description: "Looks at things".to_owned(),
        icon: icon.to_str().unwrap().to_owned(),
        ..marking(&session, "Lens")
    };
    let (ours, theirs) = UnixStream::pair().unwrap();
    session.compositor_mut().insert_client(ours).unwrap();
    let commands = Ipc::connect(&socket).unwrap();
    let config = session.config.path().to_owned();
    let icons = omorfos_launcher::Icons::default();
    let mut launcher = Launcher::new(theirs, commands, &config, vec![app], icons).unwrap();

    settle(&mut session, &mut launcher);

    let picture = session.draw();
    let y = row(0).1;
    let (left, _) = corner();
    let across = left..left + 480;
    let green = across.clone().filter(|x| picture.at(*x, y) == 0xff00_ff00);
    assert_eq!(
        green.count(),
        21,
        "the icon, three quarters of the row across"
    );
    // More is written on the row than the four letters of the name.
    let written = across.filter(|x| picture.at(*x, y) != lit());
    assert!(written.count() > 21 + 40);
}

#[test]
fn the_launcher_is_drawn_with_as_many_pixels_as_its_screen_has() {
    let mut session = Session::new();
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);

    session.suggest_scale(Scale::new(2.0));
    settle(&mut session, &mut launcher);

    // Twice the size on a screen of the same pixels, it reaches the edges.
    let picture = session.draw();
    assert_eq!(picture.at(4, HEIGHT / 2), surface());
    assert_eq!(picture.at(WIDTH - 5, HEIGHT / 2), surface());
}

#[test]
fn the_launcher_is_done_with_when_its_screen_goes() {
    let mut session = Session::new();
    let other = session.add_output((800, 600));
    // The pointer is on the second screen, so that is where it opens.
    let (x, y) = (f64::from(WIDTH + 400), 300.0);
    session.compositor_mut().input(Input::Motion { x, y }, 0);
    let mut launcher = launcher(&mut session);
    assert!(!settle(&mut session, &mut launcher));
    assert_eq!(session.draw_on(1).at(400, 300), surface());

    session.compositor_mut().remove_output(&other);

    assert!(settle(&mut session, &mut launcher));
}

#[test]
fn the_launcher_gives_up_when_the_compositor_has_gone() {
    let mut session = Session::new();
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);

    drop(session);

    // The connection closing can take a moment to be seen.
    let gone = (0..5000).any(|_| {
        std::thread::sleep(Duration::from_millis(1));
        launcher.turn(Duration::ZERO).is_err()
    });
    assert!(gone);
}

/// A program that stands in for the launcher: it leaves a mark in the
/// session's directory and stays until it is told to go.
fn stand_in_launcher() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/launcher")
}

#[test]
fn the_compositor_opens_the_launcher_when_asked_and_only_one_at_a_time() {
    let mut session = Session::new();
    session.listen();
    let show = Action::ShowLauncher;

    // Until a backend has said where the launcher is, there is none.
    session.compositor_mut().perform(&show);
    session.compositor_mut().set_launcher(stand_in_launcher());
    session.compositor_mut().perform(&show);
    session.eventually(|session| mark(session, "launcher-opened") == "x");

    // Asked again while it is open, the compositor leaves it be.
    session.compositor_mut().perform(&show);
    session.advance(Duration::from_millis(100));
    assert_eq!(mark(&session, "launcher-opened"), "x");

    // Once it has gone, it can be opened again.
    std::fs::write(session.config.path().join("launcher-close"), "").unwrap();
    session.eventually(|session| {
        session.compositor_mut().perform(&Action::ShowLauncher);
        mark(session, "launcher-opened").len() > 1
    });
    assert_eq!(session.compositor().problems(), [] as [&str; 0]);
}

#[test]
fn the_launcher_is_opened_by_its_key_and_from_the_menu_by_default() {
    let mut session = Session::new();
    session.listen();
    session.compositor_mut().set_launcher(stand_in_launcher());
    std::fs::write(session.config.path().join("launcher-close"), "").unwrap();

    // Super+d.
    let d = Keycode::new(40);
    for (code, pressed) in [
        (key::SUPER, true),
        (d, true),
        (d, false),
        (key::SUPER, false),
    ] {
        session
            .compositor_mut()
            .input(Input::Key { code, pressed }, 0);
    }

    session.eventually(|session| mark(session, "launcher-opened") == "x");
    let menu = &session.compositor().config().menu.items;
    let offered = menu.iter().any(|item| {
        matches!(item, omorfos_config::MenuItem::Entry { action, .. } if *action == Action::ShowLauncher)
    });
    assert!(offered);
}

#[test]
fn a_launcher_that_is_not_installed_is_a_problem_the_user_is_told_of() {
    let mut session = Session::new();
    session.compositor_mut().set_launcher("/no/such/launcher");
    session.compositor_mut().perform(&Action::ShowLauncher);
    assert_eq!(
        session.compositor().problems(),
        ["cannot start `/no/such/launcher`: No such file or directory (os error 2)"]
    );
}

#[test]
fn a_launcher_running_by_itself_gives_up_when_the_session_ends() {
    let mut session = Session::new();
    let runtime = session.config.path().to_owned();
    let socket = ListeningSocket::bind_absolute(runtime.join("wayland-test")).unwrap();
    session.inner().serve(socket).unwrap();
    let ipc = session.listen();
    let variables = [
        ("XDG_RUNTIME_DIR", runtime.clone().into_os_string()),
        ("WAYLAND_DISPLAY", OsString::from("wayland-test")),
        ("OMORFOS_SOCKET", ipc.into_os_string()),
        ("HOME", runtime.into_os_string()),
    ];
    let launcher = std::thread::spawn(move || {
        let variable = |name: &str| {
            let found = variables.iter().find(|(known, _)| *known == name);
            found.map(|(_, value)| value.clone())
        };
        omorfos_launcher::start(&variable).map_err(|failure| failure.to_string())
    });
    session.eventually(|session| session.draw().at(WIDTH / 2, HEIGHT / 2) == surface());

    drop(session);

    assert!(launcher.join().unwrap().is_err());
}

#[test]
fn the_launcher_runs_by_itself_offering_what_is_installed() {
    let mut session = Session::new();
    let runtime = session.config.path().to_owned();
    let socket = ListeningSocket::bind_absolute(runtime.join("wayland-test")).unwrap();
    session.inner().serve(socket).unwrap();
    let ipc = session.listen();
    // One program is installed, which leaves a mark.
    let applications = runtime.join("data/applications");
    std::fs::create_dir_all(&applications).unwrap();
    let marker = runtime.join("started");
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Marker\nExec=sh -c \"printf x > '{}'\"\n",
        marker.display()
    );
    std::fs::write(applications.join("marker.desktop"), entry).unwrap();

    let variables = [
        ("XDG_RUNTIME_DIR", runtime.clone().into_os_string()),
        ("WAYLAND_DISPLAY", OsString::from("wayland-test")),
        ("OMORFOS_SOCKET", ipc.into_os_string()),
        ("XDG_CONFIG_HOME", runtime.clone().into_os_string()),
        ("XDG_DATA_HOME", runtime.join("data").into_os_string()),
        ("XDG_DATA_DIRS", runtime.join("none").into_os_string()),
    ];
    let launcher = std::thread::spawn(move || {
        let variable = |name: &str| {
            let found = variables.iter().find(|(known, _)| *known == name);
            found.map(|(_, value)| value.clone())
        };
        omorfos_launcher::start(&variable).map_err(|failure| failure.to_string())
    });

    let (x, y) = row(0);
    session.eventually(|session| session.draw().at(x, y) == lit());
    // A key held for a while is taken again by the launcher's own clock,
    // with nothing prodding it. There is one program, so it stays chosen.
    hold(&mut session, DOWN, true);
    for _ in 0..60 {
        session.step();
        std::thread::sleep(Duration::from_millis(10));
    }
    hold(&mut session, DOWN, false);
    press(&mut session, key::RETURN);
    session.eventually(|_| marker.exists());
    assert_eq!(launcher.join().unwrap(), Ok(()));
}
