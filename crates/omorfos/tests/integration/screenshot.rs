//! The screenshot tool, run against a compositor as it is in a session.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;

use omorfos::Roles;
use smithay::reexports::wayland_server::ListeningSocket;

use crate::common::{Client, Event, HEIGHT, Session, WIDTH};
use crate::windows::{GREEN, RED};

/// What the tool said: where it saved pictures, or why it took none.
type Said = Result<Vec<PathBuf>, String>;

/// Starts the tool in a session whose socket gives `roles`, with `more`
/// in its environment besides. Its home folder is the session's own.
fn start(session: &mut Session, roles: Roles, more: &[(&str, OsString)]) -> JoinHandle<Said> {
    start_with(session, roles, more, &[])
}

/// The same, with words on its command line.
fn start_with(
    session: &mut Session,
    roles: Roles,
    more: &[(&str, OsString)],
    arguments: &[&str],
) -> JoinHandle<Said> {
    let runtime = session.config.path().to_owned();
    let name = format!("wayland-{}-{}", more.len(), arguments.len());
    let socket = ListeningSocket::bind_absolute(runtime.join(&name)).unwrap();
    session.inner().serve(socket, roles).unwrap();
    let mut variables = vec![
        (
            "XDG_RUNTIME_DIR".to_owned(),
            runtime.clone().into_os_string(),
        ),
        ("WAYLAND_DISPLAY".to_owned(), OsString::from(name)),
        ("HOME".to_owned(), runtime.into_os_string()),
    ];
    for (name, value) in more {
        variables.retain(|(known, _)| known != name);
        variables.push(((*name).to_owned(), value.clone()));
    }
    let arguments: Vec<OsString> = arguments.iter().map(OsString::from).collect();
    std::thread::spawn(move || {
        let variable = |name: &str| {
            let found = variables.iter().find(|(known, _)| known == name);
            found.map(|(_, value)| value.clone())
        };
        let mut out = Vec::new();
        let outcome = omorfos_screenshot::start(&variable, &arguments, &mut out);
        let out = String::from_utf8(out).unwrap();
        let saved = out.lines().map(PathBuf::from).collect();
        outcome
            .map(|()| saved)
            .map_err(|failure| failure.to_string())
    })
}

/// Draws the screens until the tool has finished, and returns what it
/// said.
fn finish(session: &mut Session, tool: JoinHandle<Said>) -> Said {
    session.eventually(|session| {
        session.draw();
        tool.is_finished()
    });
    tool.join().unwrap()
}

/// The size of a saved picture and the colour at a place in it, as
/// `0xAARRGGBB`.
fn saved(path: &Path, (x, y): (usize, usize)) -> ((u32, u32), u32) {
    let file = std::io::BufReader::new(std::fs::File::open(path).unwrap());
    let mut reader = png::Decoder::new(file).read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    let at = (y * usize::try_from(info.width).unwrap() + x) * 3;
    let color = u32::from_be_bytes([0xff, pixels[at], pixels[at + 1], pixels[at + 2]]);
    ((info.width, info.height), color)
}

#[test]
fn the_tool_saves_a_picture_of_the_screen_among_the_users_pictures() {
    let mut session = Session::new();
    let mut app = session.connect();
    session.open(&mut app, "app", (200, 100), RED);
    let home = session.config.path().to_owned();

    let tool = start(&mut session, Roles::CAPTURE, &[]);
    let paths = finish(&mut session, tool).unwrap();
    assert_eq!(paths.len(), 1);
    let name = paths[0].file_name().unwrap().to_str().unwrap();
    assert_eq!(
        paths[0].parent(),
        Some(home.join("Pictures/Screenshots").as_path())
    );
    // Named after when it was taken, and nothing else: there is one screen.
    assert!(
        name.starts_with("Screenshot 20") && paths[0].extension() == Some("png".as_ref()),
        "{name}"
    );
    assert_eq!(
        name.len(),
        "Screenshot 2026-10-13 14-03-22.png".len(),
        "{name}"
    );

    let size = (WIDTH.unsigned_abs(), HEIGHT.unsigned_abs());
    assert_eq!(saved(&paths[0], (10, 10)), (size, RED));
    assert_eq!(saved(&paths[0], (300, 300)).1, crate::windows::BACKGROUND);
}

#[test]
fn every_screen_is_saved_and_named_where_the_user_says_pictures_go() {
    let mut session = Session::new();
    session.add_output((320, 240));
    let mut app = session.connect();
    session.open(&mut app, "app", (200, 100), GREEN);
    let home = session.config.path().to_owned();
    let config = home.join("settings");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        config.join("user-dirs.dirs"),
        "XDG_PICTURES_DIR=\"$HOME/Bilder\"\n",
    )
    .unwrap();

    let more = [("XDG_CONFIG_HOME", config.into_os_string())];
    let tool = start(&mut session, Roles::CAPTURE, &more);
    let tool = {
        session.eventually(|session| {
            session.draw_on(1);
            session.draw();
            tool.is_finished()
        });
        tool.join().unwrap()
    };
    let mut paths = tool.unwrap();
    paths.sort();
    assert_eq!(paths.len(), 2);
    let names: Vec<&str> = paths.iter().map(|path| path.to_str().unwrap()).collect();
    assert!(
        names
            .iter()
            .all(|name| name.contains("/Bilder/Screenshots/")),
        "{names:?}"
    );
    assert!(names[0].contains(" test-0."), "{names:?}");
    assert!(names[1].contains(" test-1."), "{names:?}");
    assert_eq!(saved(&paths[0], (10, 10)).1, GREEN);
    assert_eq!(saved(&paths[1], (10, 10)).0, (320, 240));
}

#[test]
fn started_as_an_ordinary_program_it_says_how_it_could_be_let() {
    let mut session = Session::new();
    let tool = start(&mut session, Roles::NONE, &[]);
    let said = finish(&mut session, tool).unwrap_err();
    assert!(said.contains("may: [capture]"), "{said}");
    assert!(!session.config.path().join("Pictures").exists());
}

#[test]
fn in_a_sandbox_it_says_that_pictures_are_not_offered() {
    let mut session = Session::new();
    let mut flatpak = session.connect_plain();
    let path = session.config.path().join("sandbox");
    let _open = flatpak.sandbox(&path);
    session.roundtrip(&mut flatpak);

    // A whole path in place of a name is where the tool connects.
    let more = [("WAYLAND_DISPLAY", path.into_os_string())];
    let tool = start(&mut session, Roles::CAPTURE, &more);
    let said = finish(&mut session, tool).unwrap_err();
    assert!(said.contains("does not offer pictures"), "{said}");
}

#[test]
fn a_picture_that_cannot_be_taken_is_reported() {
    let mut session = Session::new();
    let mut locker = session.connect();
    let lock = locker.lock();
    let surface = locker.lock_surface(&lock);
    session.roundtrip(&mut locker);
    locker.fill(&surface, (WIDTH, HEIGHT), GREEN);
    session.roundtrip(&mut locker);
    session.draw();
    session.send_frames(std::time::Duration::from_millis(16));
    session.roundtrip(&mut locker);
    assert!(locker.take_events().contains(&Event::Locked));

    let tool = start(&mut session, Roles::CAPTURE, &[]);
    let said = finish(&mut session, tool).unwrap_err();
    assert!(said.contains("could not be taken"), "{said}");
}

#[test]
fn a_picture_that_cannot_be_saved_is_reported() {
    let mut session = Session::new();
    // Where pictures go is a file, so no folder can be made in it.
    std::fs::write(session.config.path().join("Pictures"), "").unwrap();
    let tool = start(&mut session, Roles::CAPTURE, &[]);
    let said = finish(&mut session, tool).unwrap_err();
    assert!(said.contains("Pictures/Screenshots: "), "{said}");
}

#[test]
fn screens_that_come_and_go_while_it_waits_do_not_confuse_it() {
    let mut session = Session::new();
    let tool = start(&mut session, Roles::CAPTURE, &[]);
    // It has asked, and waits for the screen to be drawn.
    session.eventually(|session| session.compositor().captured(&session.output()));
    let other = session.add_output((320, 240));
    session.step();
    session.compositor_mut().remove_output(&other);
    session.step();

    let paths = finish(&mut session, tool).unwrap();
    assert_eq!(paths.len(), 1, "of the screen there was when it began");
}

#[test]
fn it_gives_up_when_the_session_ends() {
    let mut session = Session::new();
    let tool = start(&mut session, Roles::CAPTURE, &[]);
    session.eventually(|session| session.compositor().captured(&session.output()));
    drop(session);
    let said = tool.join().unwrap().unwrap_err();
    assert!(said.contains("the session ended"), "{said}");
}

#[test]
fn it_says_where_it_saved_or_why_it_did_not() {
    let mut session = Session::new();
    let runtime = session.config.path().to_owned();
    let socket = ListeningSocket::bind_absolute(runtime.join("wayland-said")).unwrap();
    session.inner().serve(socket, Roles::CAPTURE).unwrap();
    let home = runtime.clone();
    let tool = std::thread::spawn(move || {
        let variable = |name: &str| match name {
            "XDG_RUNTIME_DIR" | "HOME" => Some(home.clone().into_os_string()),
            "WAYLAND_DISPLAY" => Some(OsString::from("wayland-said")),
            _ => None,
        };
        let (mut out, mut problems) = (Vec::new(), Vec::new());
        let failed = omorfos_screenshot::run(&variable, &[], &mut out, &mut problems);
        (failed, String::from_utf8(out).unwrap(), problems)
    });
    session.eventually(|session| {
        session.draw();
        tool.is_finished()
    });
    let (failed, out, problems) = tool.join().unwrap();
    assert!(!failed);
    assert_eq!(problems, b"");
    let line = out.strip_suffix('\n').unwrap();
    assert!(
        line.starts_with(
            runtime
                .join("Pictures/Screenshots/Screenshot ")
                .to_str()
                .unwrap()
        )
    );
    assert!(Path::new(line).exists());

    let (mut out, mut problems) = (Vec::new(), Vec::new());
    assert!(omorfos_screenshot::run(
        &|_| None,
        &[],
        &mut out,
        &mut problems
    ));
    let problems = String::from_utf8(problems).unwrap();
    assert!(problems.starts_with("omorfos-screenshot: this is not an omorfos session"));
    assert_eq!(out, b"");
}

/// May take pictures and set the clipboard, as the built-in keys start
/// the tool.
const BOTH: Roles = Roles::CAPTURE.with(Roles::CLIPBOARD);

/// Has a program with the keyboard paste the picture on the clipboard,
/// and returns the colour at a place in it.
fn pasted(session: &mut Session, app: &mut Client, (x, y): (usize, usize)) -> u32 {
    use std::io::Read;
    session.eventually(|session| {
        session.draw();
        session.roundtrip(app);
        app.state.formats.iter().any(|format| format == "image/png")
    });
    let mut reader = app.paste_as("image/png").unwrap();
    session.roundtrip(app);
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).unwrap();
    let mut decoded = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; decoded.output_buffer_size().unwrap()];
    let info = decoded.next_frame(&mut pixels).unwrap();
    let at = (y * usize::try_from(info.width).unwrap() + x) * 3;
    u32::from_be_bytes([0xff, pixels[at], pixels[at + 1], pixels[at + 2]])
}

#[test]
fn told_to_it_puts_the_picture_on_the_clipboard_and_holds_it_there() {
    let mut session = Session::new();
    let mut app = session.connect();
    session.open(&mut app, "app", (200, 100), RED);
    let home = session.config.path().to_owned();

    let tool = start_with(&mut session, BOTH, &[], &["--to", "clipboard"]);
    assert_eq!(pasted(&mut session, &mut app, (10, 10)), RED);
    assert_eq!(
        pasted(&mut session, &mut app, (300, 300)),
        crate::windows::BACKGROUND
    );
    assert!(
        !tool.is_finished(),
        "it is what gives the picture when pasted"
    );
    assert!(!home.join("Pictures").exists(), "and nothing was saved");

    // Something else is copied, and it has nothing more to do.
    app.copy("something else");
    session.roundtrip(&mut app);
    let paths = finish(&mut session, tool).unwrap();
    assert_eq!(paths, Vec::<PathBuf>::new());
}

#[test]
fn the_configuration_says_where_a_picture_goes_when_nothing_else_does() {
    let mut session = Session::new();
    let mut app = session.connect();
    session.open(&mut app, "app", (200, 100), GREEN);
    let home = session.config.path().to_owned();
    std::fs::create_dir_all(home.join(".config/omorfos")).unwrap();
    let config = "screenshots: { destination: both }\n";
    std::fs::write(home.join(".config/omorfos/config.yaml"), config).unwrap();

    let tool = start(&mut session, BOTH, &[]);
    assert_eq!(pasted(&mut session, &mut app, (10, 10)), GREEN);
    app.copy("something else");
    session.roundtrip(&mut app);
    let paths = finish(&mut session, tool).unwrap();
    assert_eq!(paths.len(), 1, "saved as well");
    assert_eq!(saved(&paths[0], (10, 10)).1, GREEN);
}

#[test]
fn without_leave_to_set_the_clipboard_it_says_how_it_could_be_let() {
    let mut session = Session::new();
    let tool = start_with(&mut session, Roles::CAPTURE, &[], &["--to", "clipboard"]);
    let said = finish(&mut session, tool).unwrap_err();
    assert!(said.contains("may: [capture, clipboard]"), "{said}");
}

#[test]
fn with_no_screen_there_is_nothing_to_put_on_the_clipboard() {
    let mut session = Session::new();
    let only = session.output();
    session.compositor_mut().remove_output(&only);
    let tool = start_with(&mut session, BOTH, &[], &["--to", "clipboard"]);
    session.eventually(|_| tool.is_finished());
    let said = tool.join().unwrap().unwrap_err();
    assert!(said.contains("no screen"), "{said}");
}

#[test]
fn a_picture_held_on_the_clipboard_is_let_go_when_the_session_ends() {
    let mut session = Session::new();
    let mut app = session.connect();
    session.open(&mut app, "app", (200, 100), RED);
    let tool = start_with(&mut session, BOTH, &[], &["--to", "clipboard"]);
    pasted(&mut session, &mut app, (10, 10));
    drop(session);
    assert_eq!(tool.join().unwrap(), Ok(Vec::new()));
}

#[test]
fn told_something_it_does_not_understand_it_says_how_it_is_used() {
    let (mut out, mut problems) = (Vec::new(), Vec::new());
    let arguments = [OsString::from("--help")];
    assert!(omorfos_screenshot::run(
        &|_| None,
        &arguments,
        &mut out,
        &mut problems
    ));
    let problems = String::from_utf8(problems).unwrap();
    assert!(
        problems.starts_with("omorfos-screenshot: usage: "),
        "{problems}"
    );
}
