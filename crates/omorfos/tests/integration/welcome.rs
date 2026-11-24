//! The welcome card, as a client of a real compositor, and the
//! compositor's part in showing it.
//!
//! What the card shows and what a key does to it is tested with the card
//! itself. Here it is the program: that it gets onto the screen, that the
//! keys reach it, that each choice is saved and that closed, it is marked.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use omorfos_config::Action;
use smithay::reexports::wayland_server::ListeningSocket;

use crate::common::{HEIGHT, Session, WIDTH, key};
use crate::windows::{BACKGROUND, RED};

/// A program that stands in for the card: it leaves a mark, saying what it
/// was started with, and stays until it is told to go.
fn stand_in() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/welcome")
}

fn opened(session: &Session) -> String {
    std::fs::read_to_string(session.config.path().join("welcome-opened")).unwrap_or_default()
}

#[test]
fn the_compositor_shows_the_card_at_a_login_and_when_asked_one_at_a_time() {
    let mut session = Session::new();
    session.listen();

    // Until a backend has said where the card is, there is none.
    session.compositor_mut().greet();
    session.compositor_mut().perform(&Action::ShowWelcome);
    session.compositor_mut().set_welcome(stand_in());
    session.compositor_mut().greet();
    session.eventually(|session| opened(session) == "--first;");

    // Asked again while it is open, the compositor leaves it be.
    session.compositor_mut().perform(&Action::ShowWelcome);
    session.advance(Duration::from_millis(100));
    assert_eq!(opened(&session), "--first;");

    // Once it has gone, asked for, it is shown as it is.
    std::fs::write(session.config.path().join("welcome-close"), "").unwrap();
    session.eventually(|session| {
        session.compositor_mut().perform(&Action::ShowWelcome);
        opened(session).len() > "--first;".len()
    });
    assert_eq!(opened(&session), "--first;;");
    std::fs::write(session.config.path().join("welcome-close"), "").unwrap();
    assert_eq!(session.compositor().problems(), [] as [&str; 0]);
}

/// Presses and lets go of `code`.
fn press(session: &mut Session, code: smithay::input::keyboard::Keycode) {
    for pressed in [true, false] {
        let input = omorfos::Input::Key { code, pressed };
        session.compositor_mut().input(input, 0);
    }
}

/// Steps the session until `done` holds, by the clock on the wall: the
/// card is a thread of its own, saving to a file.
fn until(session: &mut Session, what: &str, done: impl Fn() -> bool) {
    let started = Instant::now();
    while !done() {
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "{what} never happened"
        );
        session.advance(Duration::from_millis(50));
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn the_card_runs_by_itself_saving_each_choice_and_marked_once_closed() {
    let mut session = Session::new();
    let runtime = session.config.path().to_owned();
    let socket = ListeningSocket::bind_absolute(runtime.join("wayland-welcome")).unwrap();
    session
        .inner()
        .serve(socket, omorfos::Roles::OVERLAY)
        .unwrap();
    // Two of the wallpapers that come with omorfos are installed.
    let there = runtime.join("data/backgrounds/omorfos");
    std::fs::create_dir_all(&there).unwrap();
    for name in ["prism-dark", "prism-light", "spill-dark", "spill-light"] {
        std::fs::write(there.join(format!("{name}.png")), "").unwrap();
    }
    let variables = [
        ("XDG_RUNTIME_DIR", runtime.clone().into_os_string()),
        ("WAYLAND_DISPLAY", OsString::from("wayland-welcome")),
        ("XDG_CONFIG_HOME", runtime.join("home").into_os_string()),
        ("XDG_STATE_HOME", runtime.join("state").into_os_string()),
        ("XDG_DATA_HOME", runtime.join("data").into_os_string()),
        ("XDG_DATA_DIRS", runtime.join("none").into_os_string()),
        ("HOME", runtime.clone().into_os_string()),
        ("LANG", OsString::from("pl_PL.UTF-8")),
    ];
    let card = std::thread::spawn(move || {
        let variable = |name: &str| {
            let found = variables.iter().find(|(known, _)| *known == name);
            found.map(|(_, value)| value.clone())
        };
        omorfos_welcome::start(&variable, false).map_err(|failure| failure.to_string())
    });

    // In the middle of the screen, over everything.
    let middle = (WIDTH / 2, HEIGHT / 2);
    session.eventually(|session| {
        assert!(!card.is_finished(), "the card stopped early");
        session.draw().at(middle.0, middle.1) != BACKGROUND
    });

    let config = runtime.join("home/omorfos/config.yaml");
    let written = || std::fs::read_to_string(&config).unwrap_or_default();
    // Past the welcome and the keys to the schemes; up from Next is the
    // scheme in use, dark; to its right, light.
    for code in [key::RETURN, key::RETURN, key::UP, key::RIGHT, key::RETURN] {
        press(&mut session, code);
    }
    until(&mut session, "light saved", || {
        written().contains("scheme: light")
    });
    // Down to Next, on to the wallpapers; up is the one in use, Prism; to
    // its right, Spill.
    for code in [key::DOWN, key::RETURN, key::UP, key::RIGHT, key::RETURN] {
        press(&mut session, code);
    }
    until(&mut session, "Spill saved", || {
        written().contains("spill-dark.png")
    });
    assert!(written().starts_with("appearance:\n  scheme: light\nwallpaper:\n  picture: { dark: "));

    press(&mut session, key::ESCAPE);
    until(&mut session, "the card closing", || card.is_finished());
    assert_eq!(card.join().unwrap(), Ok(()));
    assert!(runtime.join("state/omorfos/welcomed").exists());
}

/// A card with one row of keys, two wallpapers, and dark in use, drawn
/// under `theme` in the session, whose choices are put in `chosen`: saved,
/// or refused, as `refusing` says.
fn card_in(
    session: &mut Session,
    theme: &omorfos_config::Theme,
    marked: &Path,
    (chosen, refusing): (
        std::rc::Rc<std::cell::RefCell<Vec<omorfos_welcome::Choice>>>,
        bool,
    ),
) -> (omorfos_welcome::Welcome, (i32, i32)) {
    let (ours, theirs) = std::os::unix::net::UnixStream::pair().unwrap();
    session
        .compositor_mut()
        .insert_client(ours, omorfos::Roles::OVERLAY)
        .unwrap();
    let word = |word: &str| word.to_owned();
    let said = omorfos_welcome::Said {
        title: word("Welcome"),
        greeting: word("A few steps."),
        keys: word("Keys"),
        scheme: word("Light or dark"),
        wallpaper: word("Wallpaper"),
        applications: word("What opens what"),
        chosen_elsewhere: word("Chosen elsewhere"),
        none: word("None"),
        schemes: [word("Dark"), word("Light"), word("With the day")],
        skip: word("Skip"),
        back: word("Back"),
        next: word("Next"),
        finish: word("Finish"),
    };
    let picture = omorfos_ui::Image::new(16, 9, vec![255; 16 * 9 * 4]).unwrap();
    let shown = omorfos_welcome::Shown {
        mark: None,
        rows: vec![(word("Super+Return"), word("Open a terminal"))],
        schemes: vec![picture.clone(), picture.clone(), picture.clone()],
        wallpapers: vec![picture.clone(), picture],
        openers: Vec::new(),
    };
    let in_use = (Some(0), omorfos_config::SchemeChoice::Dark);
    let card = omorfos_welcome::Card::new(said, shown, in_use);
    let save: omorfos_welcome::Save = Box::new(move |choice| {
        chosen.borrow_mut().push(choice);
        if refusing {
            Err("config.yaml was left as it was".to_owned())
        } else {
            Ok(())
        }
    });
    let (width, height) = omorfos_welcome::Card::size(theme);
    let size = (width.cast_signed(), height.cast_signed());
    (
        omorfos_welcome::Welcome::new(theirs, card, (theme, save), marked).unwrap(),
        size,
    )
}

/// Lets the card and the compositor talk until they have nothing more to
/// say to each other. Returns whether the card is done with.
fn settle(session: &mut Session, card: &mut omorfos_welcome::Welcome) -> bool {
    let mut done = false;
    for _ in 0..20 {
        done |= card.turn().unwrap();
        session.step();
    }
    done
}

/// Whether two pictures of a screen `size` large differ anywhere.
fn differs(
    one: &crate::common::Picture,
    other: &crate::common::Picture,
    (width, height): (i32, i32),
) -> bool {
    (0..height).any(|y| (0..width).any(|x| one.at(x, y) != other.at(x, y)))
}

type Chosen = std::rc::Rc<std::cell::RefCell<Vec<omorfos_welcome::Choice>>>;

#[test]
fn the_keys_go_round_the_card_and_a_choice_refused_is_said_on_it() {
    let mut session = Session::new();
    let state = tempfile::TempDir::new().unwrap();
    let chosen = Chosen::default();
    let theme = omorfos_config::Theme::default();
    let (mut card, _) = card_in(
        &mut session,
        &theme,
        &state.path().join("welcomed"),
        (chosen.clone(), true),
    );
    assert!(!settle(&mut session, &mut card));
    let before = session.draw();

    // On past the welcome and the keys to the schemes; Shift+Tab from
    // Next is Back, and again the last scheme, and again the one before
    // it; Tab is the last again; left twice to the first; and a key that
    // means nothing to it does nothing.
    press(&mut session, key::RETURN);
    press(&mut session, key::RETURN);
    for _ in 0..3 {
        let shift = |pressed| omorfos::Input::Key {
            code: key::SHIFT,
            pressed,
        };
        session.compositor_mut().input(shift(true), 0);
        press(&mut session, key::TAB);
        session.compositor_mut().input(shift(false), 0);
    }
    for code in [key::TAB, key::LEFT, key::LEFT, key::A, key::RETURN] {
        press(&mut session, code);
    }
    assert!(!settle(&mut session, &mut card));
    assert_eq!(
        *chosen.borrow(),
        [omorfos_welcome::Choice::Scheme(
            omorfos_config::SchemeChoice::Dark
        )]
    );
    // Refused, it says so on a line more.
    assert!(differs(&session.draw(), &before, (WIDTH, HEIGHT)));
    press(&mut session, key::ESCAPE);
    assert!(settle(&mut session, &mut card));
    assert!(state.path().join("welcomed").exists());
}

#[test]
fn a_click_on_skip_closes_the_card_and_one_beside_it_does_nothing() {
    let mut session = Session::new();
    let state = tempfile::TempDir::new().unwrap();
    let chosen = Chosen::default();
    // A backdrop that dims what is behind, as a theme may have it.
    let dimmed = "launcher-backdrop { background: rgba(0, 0, 0, 0.4); }";
    let theme = omorfos_config::Theme::parse(dimmed).unwrap();
    let marked = state.path().join("welcomed");
    let (mut card, (width, height)) =
        card_in(&mut session, &theme, &marked, (chosen.clone(), false));
    assert!(!settle(&mut session, &mut card));
    assert_ne!(
        session.draw().at(4, 4),
        BACKGROUND,
        "what is behind is dimmed"
    );

    let click = |session: &mut Session, x: i32, y: i32| {
        let (x, y) = (f64::from(x), f64::from(y));
        session
            .compositor_mut()
            .input(omorfos::Input::Motion { x, y }, 0);
        session
            .compositor_mut()
            .input(omorfos::Input::Motion { x: x + 1.0, y }, 0);
        for pressed in [true, false] {
            let button = omorfos::Input::Button {
                button: 0x110,
                pressed,
            };
            session.compositor_mut().input(button, 0);
        }
    };
    // Beside the card, on what dims the screen.
    click(&mut session, 4, 4);
    assert!(!settle(&mut session, &mut card));
    // The card is centred; Skip is in its bottom left, inside its border
    // and padding of 13, 120 wide and 36 tall.
    let (left, top) = ((WIDTH - width) / 2, (HEIGHT - height) / 2);
    click(&mut session, left + 13 + 60, top + height - 13 - 18);
    assert!(settle(&mut session, &mut card));
    assert!(marked.exists());
    assert!(chosen.borrow().is_empty());
}

#[test]
fn the_card_is_drawn_again_for_a_denser_screen_and_done_with_when_its_screen_goes() {
    let mut session = Session::new();
    let other = session.add_output((800, 600));
    // The pointer is on the second screen, so that is where it opens.
    let (x, y) = (f64::from(WIDTH + 400), 300.0);
    session
        .compositor_mut()
        .input(omorfos::Input::Motion { x, y }, 0);
    let state = tempfile::TempDir::new().unwrap();
    let theme = omorfos_config::Theme::default();
    let marked = state.path().join("welcomed");
    let (mut card, _) = card_in(&mut session, &theme, &marked, (Chosen::default(), false));
    assert!(!settle(&mut session, &mut card));
    let before = session.draw_on(1);
    session
        .compositor_mut()
        .suggest_scale(&other, omorfos::Scale::new(2.0));
    assert!(!settle(&mut session, &mut card));
    assert!(differs(&session.draw_on(1), &before, (800, 600)));

    session.compositor_mut().remove_output(&other);
    assert!(settle(&mut session, &mut card));
    assert!(
        !marked.exists(),
        "gone with its screen, it is not marked closed"
    );
}

#[test]
fn a_card_started_after_the_theme_went_wrong_is_drawn_as_the_session_kept_it() {
    let mut session = Session::new();
    let runtime = session.config.path().to_owned();
    let socket = ListeningSocket::bind_absolute(runtime.join("wayland-kept")).unwrap();
    session
        .inner()
        .serve(socket, omorfos::Roles::OVERLAY)
        .unwrap();
    // The theme went wrong after the compositor last kept it, red.
    let config = runtime.join(".config/omorfos");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("theme.css"), "launcher { color: huge; }\n").unwrap();
    let kept = runtime.join("omorfos/last-good");
    std::fs::create_dir_all(&kept).unwrap();
    let red = "launcher { background: #ff0000; backdrop-filter: none; }\n";
    std::fs::write(kept.join("theme.css"), red).unwrap();
    let variables = [
        ("XDG_RUNTIME_DIR", runtime.clone().into_os_string()),
        ("WAYLAND_DISPLAY", OsString::from("wayland-kept")),
        ("HOME", runtime.into_os_string()),
    ];
    let card = std::thread::spawn(move || {
        let variable = |name: &str| {
            let found = variables.iter().find(|(known, _)| *known == name);
            found.map(|(_, value)| value.clone())
        };
        omorfos_welcome::start(&variable, false).map_err(|failure| failure.to_string())
    });
    // The card is centred; inside its left edge, in its padding, is its
    // background.
    let (width, _) = omorfos_welcome::Card::size(&omorfos_config::Theme::default());
    let left = (WIDTH - width.cast_signed()) / 2;
    session.eventually(|session| session.draw().at(left + 8, HEIGHT / 2) == RED);

    drop(session);

    assert!(card.join().unwrap().is_err());
}

#[test]
fn a_card_running_by_itself_gives_up_when_the_session_ends() {
    let mut session = Session::new();
    let runtime = session.config.path().to_owned();
    let socket = ListeningSocket::bind_absolute(runtime.join("wayland-ending")).unwrap();
    session
        .inner()
        .serve(socket, omorfos::Roles::OVERLAY)
        .unwrap();
    let variables = [
        ("XDG_RUNTIME_DIR", runtime.clone().into_os_string()),
        ("WAYLAND_DISPLAY", OsString::from("wayland-ending")),
        ("HOME", runtime.into_os_string()),
    ];
    let card = std::thread::spawn(move || {
        let variable = |name: &str| {
            let found = variables.iter().find(|(known, _)| *known == name);
            found.map(|(_, value)| value.clone())
        };
        omorfos_welcome::start(&variable, false).map_err(|failure| failure.to_string())
    });
    session.eventually(|session| session.draw().at(WIDTH / 2, HEIGHT / 2) != BACKGROUND);

    drop(session);

    assert!(card.join().unwrap().is_err());
}
