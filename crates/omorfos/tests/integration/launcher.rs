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
use std::time::{Duration, Instant};

use omorfos::{Input, Scale};
use omorfos_config::{Action, Color, Part, State, States, Theme};
use omorfos_ipc::Client as Ipc;
use omorfos_launcher::{App, Launcher, Numbers, Picker, View};
use omorfos_ui::Text;
use smithay::input::keyboard::Keycode;
use smithay::reexports::wayland_server::ListeningSocket;

use crate::common::{Client, Event, HEIGHT, Session, WIDTH, between, half_way, key, wait};
use crate::windows::{BACKGROUND, RED, shaded};

const LEFT: u32 = 0x110;

const F: Keycode = Keycode::new(41);
const I: Keycode = Keycode::new(31);
const L: Keycode = Keycode::new(46);
const E: Keycode = Keycode::new(26);
const BACKSPACE: Keycode = Keycode::new(22);
const DOWN: Keycode = Keycode::new(116);
const UP: Keycode = Keycode::new(111);
const FIVE: Keycode = Keycode::new(14);
const TWO: Keycode = Keycode::new(11);
const MINUS: Keycode = Keycode::new(20);
const Z: Keycode = Keycode::new(52);
const DELETE: Keycode = Keycode::new(119);
const V: Keycode = Keycode::new(55);

fn argb(color: Color) -> u32 {
    u32::from_be_bytes([color.alpha, color.red, color.green, color.blue])
}

/// The colour of the launcher where nothing is on it: its glass, which
/// is solid in its colour where what is behind is not blurred.
fn surface() -> u32 {
    let glass = Theme::default()
        .style(Part::Launcher, States::NONE)
        .background;
    argb(Color {
        alpha: u8::MAX,
        ..glass
    })
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
    launcher_of(session, &["Files", "Firefox", "Lens"])
}

/// A launcher in the session offering programs called `names`, which each
/// leave a mark when started.
fn launcher_of(session: &mut Session, names: &[&str]) -> Launcher {
    let socket = session.listen();
    let apps: Vec<App> = names.iter().map(|name| marking(session, name)).collect();
    let (ours, theirs) = UnixStream::pair().unwrap();
    session
        .compositor_mut()
        .insert_client(ours, omorfos::Roles::OVERLAY)
        .unwrap();
    let commands = Ipc::connect(&socket).unwrap();
    let config = session.config.path().to_owned();
    let icons = omorfos_launcher::Icons::default();
    Launcher::new(
        theirs,
        commands,
        (&config, None),
        apps,
        icons,
        Numbers::Point,
    )
    .unwrap()
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

/// Where on the screen the launcher's top-left corner is: across, it is
/// in the middle; down, where it would be in the middle with as many rows
/// as the screen has room for, which is three, as many as the programs
/// `launcher` offers.
fn corner() -> (i32, i32) {
    let theme = Theme::default();
    let mut picker = Picker::new(
        Vec::new(),
        omorfos_config::Launcher::default(),
        Numbers::Point,
    );
    picker.fit(&theme, HEIGHT.unsigned_abs());
    let (width, _) = picker.size(&theme);
    let top = picker.top(&theme, HEIGHT.unsigned_abs());
    ((WIDTH - width.cast_signed()) / 2, top.cast_signed())
}

/// A point on the screen towards the right-hand end of a line of what is
/// found, clear of the program's name, with several found in the mode
/// that finds everything: the heading over the top hit, the top hit, the
/// heading over the rest, and the rest.
fn row(place: i32) -> (i32, i32) {
    line_in_sight(0, place)
}

/// The same for line `line` with the list scrolled to line `first`.
fn line_in_sight(first: i32, line: i32) -> (i32, i32) {
    let (left, top) = corner();
    // A border and padding, the text field and the room under it.
    let list = top + 9 + 56 + 8;
    let height = |line: i32| match line {
        0 | 2 => 24,
        _ => 56,
    };
    let above: i32 = (first..line).map(height).sum();
    (left + 400, list + above + height(line) / 2)
}

/// The middle of the launcher's field, across the screen's middle: on the
/// launcher whenever it is up, as short as it is with nothing typed.
fn field_middle() -> (i32, i32) {
    let (_, top) = corner();
    (WIDTH / 2, top + 13 + 28)
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
    let beside = picture.at(left - 2, top - 2);
    assert!(
        shaded(beside, RED),
        "the window is still there: {beside:#x}"
    );
    // Nothing typed, the field alone, and the window under the rest.
    assert_eq!(picture.at(left + 4, top + 13 + 28), surface());
    let under = picture.at(left + 4, top + 150);
    assert!(shaded(under, RED), "nothing under the field: {under:#x}");
}

#[test]
fn what_is_typed_reaches_the_launcher_and_enter_starts_the_program() {
    let mut session = Session::new();
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);

    // "fi" leaves Files and Firefox; Down lights Files, and again Firefox.
    press(&mut session, F);
    press(&mut session, I);
    press(&mut session, key::SHIFT);
    press(&mut session, DOWN);
    press(&mut session, DOWN);
    assert!(!settle(&mut session, &mut launcher));
    let picture = session.draw();
    // Under the heading over all but the first, the list scrolled to it,
    // as the screen has room for two lines.
    let (x, y) = line_in_sight(2, 3);
    assert_eq!(picture.at(x, y), lit());
    // Lens does not match, and the launcher ends above where it would be.
    let below = picture.at(x, line_in_sight(2, 4).1);
    assert!(![surface(), lit()].contains(&below), "{below:#x}");

    press(&mut session, key::RETURN);
    assert!(settle(&mut session, &mut launcher), "it is done with");
    session.eventually(|session| mark(session, "firefox") == "x");
    assert_eq!(mark(&session, "files"), "");

    // Gone, it leaves the screen.
    drop(launcher);
    session
        .eventually(|session| session.draw().at(field_middle().0, field_middle().1) == BACKGROUND);
}

#[test]
fn a_sum_typed_is_answered_and_enter_puts_the_answer_on_the_clipboard() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open(&mut client, "app", (200, 100), RED);
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);
    // Inside the text field, to the right of anything typed.
    let (left, top) = corner();
    let field = (left + 400, top + 13 + 16);
    // The field has no ground of its own: the launcher's glass shows.
    let input = surface();
    assert_eq!(session.draw().at(field.0, field.1), input);

    for code in [FIVE, MINUS, TWO] {
        press(&mut session, code);
    }
    assert!(!settle(&mut session, &mut launcher));
    // A row now, not three, and the field where it was.
    assert_eq!(session.draw().at(field.0, field.1), input);
    // The answer's row, and none of the programs, which do not match:
    // the launcher ends under it. Enter copies it, lit or not.
    let picture = session.draw();
    let x = row(0).0;
    assert_eq!(picture.at(x, row(1).1), surface());
    // Past its padding too.
    let below = picture.at(x, row(3).1);
    assert!(![surface(), lit()].contains(&below), "{below:#x}");

    press(&mut session, key::RETURN);
    assert!(settle(&mut session, &mut launcher), "it is done with");
    drop(launcher);
    // The launcher has the keyboard until the compositor sees it go, once
    // every copy of its end of the connection is closed, which a test
    // starting a program meanwhile holds for a moment: the window is
    // offered the clipboard when it has the keyboard back.
    session.eventually(|session| {
        session.roundtrip(&mut client);
        client.state.selection.is_some()
    });
    let pipe = client.paste();
    session.roundtrip(&mut client);
    let text = pipe.map_or_default(crate::common::read_text);
    assert_eq!(text, "3");
    for name in ["files", "firefox", "lens"] {
        assert_eq!(mark(&session, name), "", "{name} is not started");
    }
    drop(window);
}

#[test]
fn the_arrows_and_backspace_work_as_they_do_in_any_list() {
    let mut session = Session::new();
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);

    press(&mut session, L);
    settle(&mut session, &mut launcher);
    press(&mut session, BACKSPACE);
    // All three have an e; the first Down lights the first.
    press(&mut session, E);
    press(&mut session, DOWN);
    press(&mut session, DOWN);
    press(&mut session, DOWN);
    press(&mut session, UP);
    settle(&mut session, &mut launcher);
    let picture = session.draw();
    let x = row(0).0;
    // Three lines in sight, scrolled to the heading over the rest as
    // Down went to the last.
    assert_eq!(
        picture.at(x, line_in_sight(2, 3).1),
        lit(),
        "all three, the second chosen"
    );
    assert_eq!(picture.at(x, line_in_sight(2, 4).1), surface());

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
    let on_the_screen = (0..8).filter(|place| row(*place).1 < HEIGHT);
    on_the_screen.into_iter().find(|place| {
        let (x, y) = row(*place);
        picture.at(x, y) == lit()
    })
}

/// Whether line `line` is lit, with the list scrolled to line `first`.
fn lit_at(session: &mut Session, first: i32, line: i32) -> bool {
    let (x, y) = line_in_sight(first, line);
    session.draw().at(x, y) == lit()
}

fn hold(session: &mut Session, code: Keycode, pressed: bool) {
    session
        .compositor_mut()
        .input(Input::Key { code, pressed }, 0);
}

/// Which line Down lights from what is typed, if any matches: the top
/// hit's, under its heading; and the keyboard given back to what is
/// typed, with Up.
fn matched(session: &mut Session, launcher: &mut Launcher) -> Option<i32> {
    press(session, DOWN);
    settle(session, launcher);
    let lit = chosen(session);
    press(session, UP);
    settle(session, launcher);
    lit
}

#[test]
fn what_is_typed_is_edited_as_in_any_text_field() {
    let mut session = Session::new();
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);
    let chord = |session: &mut Session, held: &[Keycode], code: Keycode| {
        for key in held {
            hold(session, *key, true);
        }
        press(session, code);
        for key in held.iter().rev() {
            hold(session, *key, false);
        }
    };
    // "fl" matches nothing; an i put between its letters makes "fil".
    press(&mut session, F);
    press(&mut session, L);
    settle(&mut session, &mut launcher);
    assert_eq!(matched(&mut session, &mut launcher), None);
    // Super and Backspace, as Ctrl and Backspace, take out the word before
    // the cursor, which Right leaves at the end; all is listed, and typed
    // again.
    press(&mut session, key::RIGHT);
    chord(&mut session, &[key::SUPER], key::BACKSPACE);
    settle(&mut session, &mut launcher);
    assert_eq!(
        matched(&mut session, &mut launcher),
        None,
        "nothing typed, nothing"
    );
    press(&mut session, F);
    press(&mut session, L);
    press(&mut session, key::LEFT);
    press(&mut session, I);
    settle(&mut session, &mut launcher);
    assert_eq!(matched(&mut session, &mut launcher), Some(1), "Files");
    // Undone, and done again.
    chord(&mut session, &[key::SUPER], Z);
    settle(&mut session, &mut launcher);
    assert_eq!(matched(&mut session, &mut launcher), None, "fl");
    chord(&mut session, &[key::SUPER, key::SHIFT], Z);
    settle(&mut session, &mut launcher);
    assert_eq!(matched(&mut session, &mut launcher), Some(1), "fil");
    // The first character taken out from in front: "il", in Files still.
    press(&mut session, key::HOME);
    press(&mut session, DELETE);
    settle(&mut session, &mut launcher);
    assert_eq!(matched(&mut session, &mut launcher), Some(1));
    // All of it chosen and copied, the launcher staying; then cut.
    chord(&mut session, &[key::SUPER], key::A);
    chord(&mut session, &[key::SUPER], key::C);
    assert!(!settle(&mut session, &mut launcher), "still open");
    chord(&mut session, &[key::SUPER], key::X);
    assert!(!settle(&mut session, &mut launcher), "still open");
    // A word chosen back from the end, with Shift and Ctrl, and typed over.
    chord(&mut session, &[key::SUPER], Z);
    press(&mut session, key::END);
    chord(&mut session, &[key::CONTROL, key::SHIFT], key::LEFT);
    press(&mut session, L);
    press(&mut session, key::RETURN);
    settle(&mut session, &mut launcher);
    session.eventually(|session| mark(session, "lens") == "x");
}

#[test]
fn super_a_or_in_the_ordinary_way_ctrl_a_chooses_all_that_is_typed_for_the_next_key_to_take_its_place()
 {
    // Super, as a Mac's Cmd, or Ctrl where the keys are the ordinary way's
    // (ADR 101).
    let ordinary: &[(&str, &str)] = &[("keys.yaml", "defaults: ordinary\n")];
    for (chooser, files) in [(key::SUPER, &[][..]), (key::CONTROL, ordinary)] {
        let mut session = Session::with_config(files);
        let mut launcher = launcher(&mut session);
        settle(&mut session, &mut launcher);
        press(&mut session, F);
        press(&mut session, I);
        hold(&mut session, chooser, true);
        press(&mut session, key::A);
        hold(&mut session, chooser, false);
        // "l" in place of "fi": Lens first, as its name begins with it.
        press(&mut session, L);
        press(&mut session, key::RETURN);
        assert!(settle(&mut session, &mut launcher));
        session.eventually(|session| mark(session, "lens") == "x");
        assert_eq!(mark(&session, "files"), "");
    }
}

#[test]
fn super_with_backspace_takes_out_to_the_start_and_with_the_arrows_goes_to_the_ends() {
    // As a Mac's Cmd (#155): what is typed is taken out, and "f" goes in
    // before "i", to make "fi", as Files begins.
    let mut session = Session::new();
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);
    press(&mut session, L);
    command(&mut session, key::BACKSPACE);
    press(&mut session, I);
    command(&mut session, key::LEFT);
    command(&mut session, key::RIGHT);
    command(&mut session, key::UP);
    press(&mut session, F);
    command(&mut session, key::DOWN);
    press(&mut session, key::RETURN);
    assert!(settle(&mut session, &mut launcher));
    session.eventually(|session| mark(session, "files") == "x");
    assert_eq!(mark(&session, "lens"), "");
}

/// Presses Super and `code` together, as a Mac's Cmd, which copies and
/// pastes in the shell's fields (ADR 101).
fn command(session: &mut Session, code: Keycode) {
    hold(session, key::SUPER, true);
    press(session, code);
    hold(session, key::SUPER, false);
}

/// Pastes with Super and V, letting the launcher, the compositor and the
/// program that copied, if one did, talk until what was pasted has
/// arrived, and presses Enter.
fn paste_and_enter(session: &mut Session, launcher: &mut Launcher, from: Option<&mut Client>) {
    command(session, V);
    let mut from = from;
    for _ in 0..20 {
        launcher.turn(Duration::ZERO).unwrap();
        session.step();
        if let Some(from) = from.as_deref_mut() {
            session.roundtrip(from);
        }
    }
    // What the compositor copied itself, it hands over on a thread of its
    // own: the launcher reads it once that is done.
    session.eventually(|session| session.compositor_mut().handing_over() == 0);
    settle(session, launcher);
    press(session, key::RETURN);
    settle(session, launcher);
}

#[test]
fn what_is_copied_is_pasted_into_the_launcher_with_super_v_its_first_line_in_place_of_what_is_chosen()
 {
    let mut session = Session::new();
    let mut editor = session.connect();
    session.open(&mut editor, "editor", (200, 100), RED);
    editor.copy("lens\nand a second line, which is left out");
    session.roundtrip(&mut editor);
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);
    // "fi" typed and chosen, for what is pasted to take its place.
    press(&mut session, F);
    press(&mut session, I);
    command(&mut session, key::A);
    paste_and_enter(&mut session, &mut launcher, Some(&mut editor));
    session.eventually(|session| mark(session, "lens") == "x");
    assert_eq!(mark(&session, "files"), "");
}

#[test]
fn what_the_launcher_copied_itself_is_pasted_back() {
    let mut session = Session::new();
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);
    // "fi" copied, which the compositor gives from then on; then "l" typed
    // in its place, which would start Lens, and "fi" pasted over that.
    press(&mut session, F);
    press(&mut session, I);
    command(&mut session, key::A);
    command(&mut session, key::C);
    settle(&mut session, &mut launcher);
    press(&mut session, L);
    command(&mut session, key::A);
    paste_and_enter(&mut session, &mut launcher, None);
    session.eventually(|session| mark(session, "files") == "x");
    assert_eq!(mark(&session, "lens"), "");
}

#[test]
fn nothing_is_pasted_where_nothing_is_copied_or_nothing_that_is_text() {
    for copied in [None, Some("image/png")] {
        let mut session = Session::new();
        let mut editor = session.connect();
        session.open(&mut editor, "editor", (200, 100), RED);
        if let Some(kind) = copied {
            editor.copy_as("lens", &[kind]);
            session.roundtrip(&mut editor);
        }
        let mut launcher = launcher(&mut session);
        settle(&mut session, &mut launcher);
        paste_and_enter(&mut session, &mut launcher, Some(&mut editor));
        // Nothing typed, nothing is found, and Enter starts nothing.
        session.advance(Duration::from_millis(50));
        assert_eq!(mark(&session, "files"), "", "{copied:?}");
        assert_eq!(mark(&session, "lens"), "", "{copied:?}");
    }
}

#[test]
fn of_a_long_copy_only_so_much_is_read() {
    let mut session = Session::new();
    let mut editor = session.connect();
    session.open(&mut editor, "editor", (200, 100), RED);
    // Far more than a launcher holds, and more than is read of it.
    editor.copy(&format!("lens\n{}", "and on ".repeat(2000)));
    session.roundtrip(&mut editor);
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);
    paste_and_enter(&mut session, &mut launcher, Some(&mut editor));
    session.eventually(|session| mark(session, "lens") == "x");
}

#[test]
fn what_is_typed_is_chosen_by_dragging_over_it_with_the_pointer() {
    let mut session = Session::new();
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);
    press(&mut session, F);
    press(&mut session, I);
    // Pressed where what is typed begins, and dragged past its end.
    let (left, top) = corner();
    let at = |x: i32| Input::Motion {
        x: f64::from(left + x),
        y: f64::from(top + 29),
    };
    let left_button = |pressed| Input::Button {
        button: LEFT,
        pressed,
    };
    session.compositor_mut().input(at(21), 0);
    session.compositor_mut().input(left_button(true), 0);
    settle(&mut session, &mut launcher);
    session.compositor_mut().input(at(300), 0);
    settle(&mut session, &mut launcher);
    session.compositor_mut().input(left_button(false), 0);
    // "fi" chosen, and "l" typed in its place: Lens.
    press(&mut session, L);
    press(&mut session, key::RETURN);
    assert!(settle(&mut session, &mut launcher));
    session.eventually(|session| mark(session, "lens") == "x");
    assert_eq!(mark(&session, "files"), "");
}

#[test]
fn what_is_dragged_over_the_launcher_is_not_taken_and_goes_back() {
    let mut session = Session::new();
    let mut editor = session.connect();
    let window = session.open(&mut editor, "editor", (200, 100), RED);
    // Pressed on the editor's window, which starts dragging from there.
    let at = |x: f64, y: f64| Input::Motion { x, y };
    session.compositor_mut().input(at(100.0, 50.0), 0);
    let left = |pressed| Input::Button {
        button: LEFT,
        pressed,
    };
    session.compositor_mut().input(left(true), 0);
    session.step();
    session.roundtrip(&mut editor);
    editor.drag(&window.surface, None, "dragged");
    session.roundtrip(&mut editor);

    // The launcher opens, and what is dragged comes over its field, all
    // there is of it with nothing typed.
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);
    let (x, y) = field_middle();
    session
        .compositor_mut()
        .input(at(f64::from(x), f64::from(y)), 0);
    assert!(!settle(&mut session, &mut launcher));
    session.compositor_mut().input(left(false), 0);
    assert!(!settle(&mut session, &mut launcher), "it stays open");
    session.roundtrip(&mut editor);
    let told = editor.take_events();
    assert!(told.contains(&Event::SourceCancelled), "{told:?}");
}

/// Scrolls by `vertical`, downwards, where the pointer is.
fn scroll(session: &mut Session, vertical: f64) {
    let scrolled = Input::Scroll {
        horizontal: 0.0,
        vertical,
    };
    session.compositor_mut().input(scrolled, 0);
}

#[test]
fn the_wheel_and_the_touchpad_scroll_the_list_and_the_pointer_chooses_in_it() {
    let mut session = Session::new();
    // Six programs, where the screen has room for two lines at once.
    let six = ["Alpha", "Bravo", "Charlie", "Delta", "Echo", "Foxtrot"];
    let mut launcher = launcher_of(&mut session, &six);
    settle(&mut session, &mut launcher);
    let point = |session: &mut Session, (x, y): (i32, i32)| {
        let (x, y) = (f64::from(x), f64::from(y));
        session.compositor_mut().input(Input::Motion { x, y }, 0);
    };
    // Over the text field, where the pointer chooses nothing, and what is
    // typed has the keyboard: nothing is lit.
    let (left, top) = corner();
    point(&mut session, (left + 40, top + 20));
    // Four have an a: the heading, Alpha the top hit, the heading, Bravo,
    // Charlie and Delta.
    press(&mut session, key::A);
    settle(&mut session, &mut launcher);
    assert_eq!(chosen(&mut session), None);

    // A notch of the wheel scrolls the list a line, and lights nothing;
    // two, and in sight are the heading and Bravo.
    for _ in 0..2 {
        scroll(&mut session, 15.0);
        settle(&mut session, &mut launcher);
        assert_eq!(chosen(&mut session), None);
    }
    // The pointer chooses Bravo, the second in sight, and the list stays.
    point(&mut session, line_in_sight(2, 3));
    settle(&mut session, &mut launcher);
    assert!(lit_at(&mut session, 2, 3));
    // A touchpad goes by less at a time, and gets there as it adds up;
    // Bravo stays chosen while it is in sight.
    for _ in 0..2 {
        scroll(&mut session, 7.0);
        settle(&mut session, &mut launcher);
        assert!(lit_at(&mut session, 2, 3));
    }
    scroll(&mut session, 7.0);
    settle(&mut session, &mut launcher);
    assert!(lit_at(&mut session, 3, 3), "Bravo, a line higher");
    // Moving the pointer on chooses what is under it, Charlie, and does
    // not scroll the list back.
    let (x, y) = line_in_sight(3, 4);
    point(&mut session, (x - 10, y));
    settle(&mut session, &mut launcher);
    assert!(lit_at(&mut session, 3, 4));
    // Back up by as much, and no further than the first, however far; the
    // chosen program out of sight gives way to the nearest in sight.
    scroll(&mut session, -21.0);
    settle(&mut session, &mut launcher);
    assert!(lit_at(&mut session, 2, 3), "Bravo");
    scroll(&mut session, -1.0e9);
    settle(&mut session, &mut launcher);
    assert!(lit_at(&mut session, 0, 1), "Alpha");
    // Nor is what was beyond the first kept, to be scrolled back first.
    scroll(&mut session, 15.0);
    settle(&mut session, &mut launcher);
    assert!(lit_at(&mut session, 1, 1), "Alpha, a line higher");
    press(&mut session, key::RETURN);
    settle(&mut session, &mut launcher);
    session.eventually(|session| mark(session, "alpha") == "x");
}

#[test]
fn scrolling_beside_the_launcher_scrolls_what_is_behind_it_and_not_its_list() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (WIDTH, HEIGHT), RED);
    let six = ["Alpha", "Bravo", "Charlie", "Delta", "Echo", "Foxtrot"];
    let mut launcher = launcher_of(&mut session, &six);
    settle(&mut session, &mut launcher);
    // Four found, Alpha the top hit.
    press(&mut session, key::A);
    settle(&mut session, &mut launcher);
    // Beside it, where the window underneath shows.
    let beside = Input::Motion { x: 5.0, y: 5.0 };
    session.compositor_mut().input(beside, 0);
    settle(&mut session, &mut launcher);
    session.roundtrip(&mut client);
    client.take_events();

    scroll(&mut session, 15.0);
    settle(&mut session, &mut launcher);
    session.roundtrip(&mut client);
    let told = client.take_events();
    let scrolled = Event::PointerScroll {
        vertical: true,
        value: 15.0,
    };
    assert!(
        told.contains(&scrolled),
        "the window was scrolled: {told:?}"
    );
    // The list was not: its first program is still the one chosen.
    press(&mut session, key::RETURN);
    settle(&mut session, &mut launcher);
    session.eventually(|session| mark(session, "alpha") == "x");
}

#[test]
fn a_key_held_down_is_taken_again_and_again_until_it_is_let_go() {
    let mut session = Session::new();
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);
    let ms = Duration::from_millis;
    // All three found, Files the top hit, under its heading; two lines
    // in sight on this screen.
    press(&mut session, E);

    // Down from the field lights the first.
    hold(&mut session, DOWN, true);
    settle_at(&mut session, &mut launcher, ms(0));
    assert!(lit_at(&mut session, 0, 1), "once, when it goes down");
    assert_eq!(launcher.deadline(), Some(ms(400)), "then after a pause");
    settle_at(&mut session, &mut launcher, ms(399));
    assert!(lit_at(&mut session, 0, 1));
    settle_at(&mut session, &mut launcher, ms(400));
    // Firefox, past the heading, the list scrolled to show the two.
    assert!(lit_at(&mut session, 2, 3), "and then steadily");

    // Down is taken once more as it is let go, before the launcher hears
    // that it was: Lens, the list a line further, and Up back to Firefox.
    hold(&mut session, DOWN, false);
    hold(&mut session, UP, true);
    settle_at(&mut session, &mut launcher, ms(500));
    assert!(lit_at(&mut session, 3, 3));
    // Thirty times a second: one more by a little after the pause, back to
    // the top hit with its heading over it.
    settle_at(&mut session, &mut launcher, ms(500 + 400 + 5));
    assert!(lit_at(&mut session, 0, 1));
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
    press(&mut session, E);
    hold(&mut session, DOWN, true);
    settle_at(&mut session, &mut launcher, Duration::ZERO);
    assert!(lit_at(&mut session, 0, 1));
    assert_eq!(launcher.deadline(), None);
    settle_at(&mut session, &mut launcher, Duration::from_secs(5));
    assert!(lit_at(&mut session, 0, 1));
}

#[test]
fn a_row_gets_to_the_colour_of_being_chosen_over_the_time_the_theme_gives() {
    let theme = "launcher-item, launcher-top { background: #ff0000; transition: 100ms linear; }\n\
                 launcher-item:active, launcher-top:active { background: #0000ff; }\n";
    let mut session = Session::with_config(&[("theme.css", theme)]);
    let mut launcher = launcher(&mut session);
    let at = Duration::from_millis;
    let (red, blue) = (0xffff_0000, 0xff00_00ff);
    // Line `line`'s colour, the list scrolled to `first`.
    let line = |session: &mut Session, first: i32, line: i32| {
        let (x, y) = line_in_sight(first, line);
        session.draw().at(x, y)
    };
    // All three have an e. Down lights the first, which the field had
    // kept unlit.
    settle_at(&mut session, &mut launcher, at(0));
    press(&mut session, E);
    settle_at(&mut session, &mut launcher, at(1000));
    assert_eq!(line(&mut session, 0, 1), red, "as it opens, none lit");
    assert_eq!(launcher.deadline(), None, "and nothing to wait for");
    press(&mut session, DOWN);
    settle_at(&mut session, &mut launcher, at(1000));
    settle_at(&mut session, &mut launcher, at(1100));
    assert_eq!(line(&mut session, 0, 1), blue, "the first lit");
    // Firefox, past the heading, the list scrolled to it and its heading,
    // the two lines this screen has room for: it comes into sight lit, as
    // it was not drawn before to change from.
    press(&mut session, DOWN);
    settle_at(&mut session, &mut launcher, at(1500));
    settle_at(&mut session, &mut launcher, at(1700));
    assert_eq!(line(&mut session, 2, 3), blue);

    // Lens, the list a line further, Firefox and Lens in sight: Lens comes
    // lit, and Firefox goes back over the time the theme gives.
    press(&mut session, DOWN);
    settle_at(&mut session, &mut launcher, at(2000));
    let rows = |session: &mut Session| (line(session, 3, 3), line(session, 3, 4));
    assert_eq!(rows(&mut session), (blue, blue), "setting out");
    assert_eq!(launcher.deadline(), Some(at(2016)));

    settle_at(&mut session, &mut launcher, at(2050));
    let (first, second) = rows(&mut session);
    assert!(half_way(first), "{first:#x}");
    assert_eq!(second, blue);

    settle_at(&mut session, &mut launcher, at(2100));
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
fn the_pointer_over_the_empty_launcher_offers_the_modes_and_a_click_goes_to_one() {
    let mut session = Session::new();
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);
    let motion = |session: &mut Session, (x, y): (i32, i32)| {
        let (x, y) = (f64::from(x), f64::from(y));
        session.compositor_mut().input(Input::Motion { x, y }, 0);
    };
    // Where the launcher's own drawing has the button for every program,
    // once the pointer is over it.
    let theme = Theme::default();
    let mut picker = Picker::new(
        Vec::new(),
        omorfos_config::Launcher::default(),
        Numbers::Point,
    );
    // In the language the launcher speaks, as wide as its words.
    picker.speak(&omorfos_i18n::Words::of(&omorfos_i18n::LAUNCHER));
    let (x, y) = field_middle();
    let (left, top) = corner();
    let text = &mut Text::new();
    picker.hover(Some((x - left, y - top)), &theme, text);
    let programs = View::Mode(omorfos_config::Mode::Programs);
    let (across, down, wide, tall) = picker.mode_area(programs, &theme, text).unwrap();
    let button = (
        left + across + wide.cast_signed() / 2,
        top + down + tall.cast_signed() / 2,
    );
    // Not there until the pointer comes over the launcher: the launcher's
    // shadow is there, and then the button.
    let before = session.draw().at(button.0, button.1);
    motion(&mut session, (x, y));
    session.eventually(|session| {
        settle(session, &mut launcher);
        session.draw().at(button.0, button.1) != before
    });
    // Gone as the pointer leaves it, and back as it comes again.
    motion(&mut session, (2, 2));
    session.eventually(|session| {
        settle(session, &mut launcher);
        session.draw().at(button.0, button.1) == before
    });
    // Over the launcher again, which grows the row under the pointer, and
    // down onto the button.
    motion(&mut session, (x, y));
    settle(&mut session, &mut launcher);
    motion(&mut session, button);
    settle(&mut session, &mut launcher);
    for pressed in [true, false] {
        let click = Input::Button {
            button: LEFT,
            pressed,
        };
        session.compositor_mut().input(click, 0);
    }
    settle(&mut session, &mut launcher);
    // Every program, with nothing typed, one of them lit, by Right or by
    // the pointer now over its tile: Enter starts it, where the mode that
    // finds everything has nothing to start with nothing typed. Not by
    // Down, which from a tile lit on the grid's one line goes on to the
    // row of modes.
    press(&mut session, key::RIGHT);
    press(&mut session, key::RETURN);
    assert!(settle(&mut session, &mut launcher));
    let names = ["files", "firefox", "lens"];
    session.eventually(|session| names.iter().any(|name| mark(session, name) == "x"));
}

#[test]
fn the_pointer_chooses_a_program_and_a_click_starts_it() {
    let mut session = Session::new();
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);
    // All three have an e: Files the top hit, under its heading.
    press(&mut session, E);
    settle(&mut session, &mut launcher);
    let (x, y) = row(1);
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
    session.eventually(|session| mark(session, "files") == "x");
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
        picture.at(left + 4, top + 13 + 28),
        surface(),
        "the box is over it"
    );
}

#[test]
fn the_dim_behind_the_launcher_fades_in_and_out_with_it() {
    let theme = "launcher-backdrop { background: #0000ff; transition: 100ms linear; }\n";
    let mut session = Session::with_config(&[("theme.css", theme)]);
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);
    let blue = 0xff00_00ff;
    let corner = |session: &mut Session| session.draw().at(5, 5);
    assert_eq!(corner(&mut session), BACKGROUND, "not there yet");
    session.advance(Duration::from_millis(50));
    let half = corner(&mut session);
    assert!(between(half, BACKGROUND, blue), "{half:#x}");
    session.advance(Duration::from_millis(50));
    assert_eq!(corner(&mut session), blue);

    // Gone once the launcher is, which is once its connection is closed:
    // seen to set out, without the clock moving on.
    press(&mut session, key::ESCAPE);
    assert!(settle(&mut session, &mut launcher), "closed");
    assert_eq!(session.compositor().deadline(), None, "at rest");
    drop(launcher);
    wait(&mut session, |session| {
        session.compositor().deadline().is_some()
    });
    assert_eq!(corner(&mut session), blue, "setting out");
    session.advance(Duration::from_millis(50));
    let half = corner(&mut session);
    assert!(between(half, BACKGROUND, blue), "{half:#x}");
    session.advance(Duration::from_millis(50));
    assert_eq!(corner(&mut session), BACKGROUND, "gone");
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
    session
        .compositor_mut()
        .insert_client(ours, omorfos::Roles::OVERLAY)
        .unwrap();
    let commands = Ipc::connect(&socket).unwrap();
    let config = session.config.path().to_owned();
    let icons = omorfos_launcher::Icons::default();
    let mut launcher = Launcher::new(
        theirs,
        commands,
        (&config, None),
        vec![app],
        icons,
        Numbers::Point,
    )
    .unwrap();

    settle(&mut session, &mut launcher);
    // Found, the top hit: outlined, its icon inside its border.
    press(&mut session, L);
    settle(&mut session, &mut launcher);

    let picture = session.draw();
    let y = row(1).1;
    let (left, _) = corner();
    let across = left..left + 480;
    let green = across.clone().filter(|x| picture.at(*x, y) == 0xff00_ff00);
    assert_eq!(
        green.count(),
        38,
        "the icon, as tall as the row's two lines"
    );
    // Something is written in the bottom half of the row, under the name,
    // as only what the program says of itself is.
    let under = across.flat_map(|x| (y + 4..y + 18).map(move |y| (x, y)));
    let written = under.filter(|(x, y)| picture.at(*x, *y) != surface());
    assert!(written.count() > 20, "what it says of itself");
}

#[test]
fn the_launcher_is_drawn_with_as_many_pixels_as_its_screen_has() {
    let mut session = Session::new();
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);

    session.suggest_scale(Scale::new(2.0));
    settle(&mut session, &mut launcher);

    // Twice the size on a screen of the same pixels, it reaches the edges.
    // Half-way down its field, clear of its rounded corners: the launcher,
    // wider than the screen, is put against its left edge, so its padding
    // is there. Where it stands is worked out in units of a screen half as
    // tall as it has pixels.
    let mut placed = Picker::new(
        Vec::new(),
        omorfos_config::Launcher::default(),
        Numbers::Point,
    );
    let units = (HEIGHT / 2).unsigned_abs();
    placed.fit(&Theme::default(), units);
    let top = placed.top(&Theme::default(), units).cast_signed();
    let y = 2 * (top + 13 + 28);
    let picture = session.draw();
    for x in [4, WIDTH - 5] {
        let there = picture.at(x, y);
        assert_eq!(there, surface(), "{x}: {there:#x}");
    }
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
    // The launcher's field, where it stands on that screen.
    let theme = Theme::default();
    let mut placed = Picker::new(
        Vec::new(),
        omorfos_config::Launcher::default(),
        Numbers::Point,
    );
    placed.fit(&theme, 600);
    let field = placed.top(&theme, 600).cast_signed() + 13 + 28;
    let drawn = session.draw_on(1).at(400, field);
    assert!([surface(), lit()].contains(&drawn), "{drawn:#x}");

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
fn every_program_is_asked_for_by_starting_the_launcher_onto_its_grid() {
    let mut session = Session::new();
    session.listen();
    session.compositor_mut().set_launcher(stand_in_launcher());
    session.compositor_mut().perform(&Action::ShowPrograms);
    session.eventually(|session| mark(session, "launcher-arguments") == "--programs;");
    // While it is open, the launcher asked for alone leaves it as it is.
    session.compositor_mut().perform(&Action::ShowLauncher);
    session.advance(Duration::from_millis(100));
    assert_eq!(mark(&session, "launcher-opened"), "x");
    std::fs::write(session.config.path().join("launcher-close"), "").unwrap();
    session.eventually(|session| {
        session.compositor_mut().perform(&Action::ShowLauncher);
        mark(session, "launcher-opened").len() > 1
    });
    assert_eq!(mark(&session, "launcher-arguments"), "--programs;;");
}

#[test]
fn the_launcher_is_opened_by_its_key_and_from_the_menu_by_default() {
    let mut session = Session::new();
    session.listen();
    session.compositor_mut().set_launcher(stand_in_launcher());
    std::fs::write(session.config.path().join("launcher-close"), "").unwrap();

    // Super+Space.
    let space = Keycode::new(65);
    for (code, pressed) in [
        (key::SUPER, true),
        (space, true),
        (space, false),
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
        [
            "cannot start `/no/such/launcher`, which is not installed: install it, or name another in its place in the configuration"
        ]
    );
}

#[test]
fn a_launcher_running_by_itself_gives_up_when_the_session_ends() {
    let mut session = Session::new();
    let runtime = session.config.path().to_owned();
    let socket = ListeningSocket::bind_absolute(runtime.join("wayland-test")).unwrap();
    session
        .inner()
        .serve(socket, omorfos::Roles::OVERLAY)
        .unwrap();
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
        omorfos_launcher::start(&variable, false).map_err(|failure| failure.to_string())
    });
    session
        .eventually(|session| session.draw().at(field_middle().0, field_middle().1) == surface());

    drop(session);

    assert!(launcher.join().unwrap().is_err());
}

#[test]
fn a_launcher_started_after_the_theme_went_wrong_is_drawn_as_the_session_kept_it() {
    let mut session = Session::new();
    let runtime = session.config.path().to_owned();
    let socket = ListeningSocket::bind_absolute(runtime.join("wayland-test")).unwrap();
    session
        .inner()
        .serve(socket, omorfos::Roles::OVERLAY)
        .unwrap();
    let ipc = session.listen();
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
        ("WAYLAND_DISPLAY", OsString::from("wayland-test")),
        ("OMORFOS_SOCKET", ipc.into_os_string()),
        ("HOME", runtime.into_os_string()),
    ];
    let launcher = std::thread::spawn(move || {
        let variable = |name: &str| {
            let found = variables.iter().find(|(known, _)| *known == name);
            found.map(|(_, value)| value.clone())
        };
        omorfos_launcher::start(&variable, false).map_err(|failure| failure.to_string())
    });
    session.eventually(|session| session.draw().at(field_middle().0, field_middle().1) == RED);

    drop(session);

    assert!(launcher.join().unwrap().is_err());
}

#[test]
fn the_launcher_runs_by_itself_offering_what_is_installed() {
    let mut session = Session::new();
    let runtime = session.config.path().to_owned();
    let socket = ListeningSocket::bind_absolute(runtime.join("wayland-test")).unwrap();
    session
        .inner()
        .serve(socket, omorfos::Roles::OVERLAY)
        .unwrap();
    let ipc = session.listen();
    // One program is installed, which leaves a mark, named in the
    // language the session asks for as well as in English.
    let applications = runtime.join("data/applications");
    std::fs::create_dir_all(&applications).unwrap();
    let marker = runtime.join("started");
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Marker\nName[pl]=Znacznik\n\
         Exec=sh -c \"printf x > '{}'\"\n",
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
        ("LANG", OsString::from("pl_PL.UTF-8")),
    ];
    let launcher = std::thread::spawn(move || {
        let variable = |name: &str| {
            let found = variables.iter().find(|(known, _)| *known == name);
            found.map(|(_, value)| value.clone())
        };
        omorfos_launcher::start(&variable, false).map_err(|failure| failure.to_string())
    });

    let (x, y) = field_middle();
    session.eventually(|session| {
        assert!(!launcher.is_finished(), "the launcher stopped early");
        session.draw().at(x, y) == surface()
    });
    // Its name in Polish, as the session asks.
    press(&mut session, Z);
    // A key held for a while is taken again by the launcher's own clock,
    // with nothing prodding it. There is one program, lit at the first
    // Down, so it stays chosen.
    hold(&mut session, DOWN, true);
    for _ in 0..60 {
        session.step();
        std::thread::sleep(Duration::from_millis(10));
    }
    hold(&mut session, DOWN, false);
    press(&mut session, key::RETURN);
    // The launcher asks and ends at once, so its socket may close before
    // what it asked is read; the compositor reads it all the same. The wait
    // is long enough by the clock on the wall, not a count of turns, for a
    // busy machine to start a program, and if it fails, what the launcher
    // and the compositor made of it is said, which a bare wait would not.
    let started = Instant::now();
    while !marker.exists() {
        let waited = started.elapsed();
        if waited > Duration::from_secs(30) {
            let ended = launcher.is_finished().then(|| launcher.join().unwrap());
            let problems = session.compositor().problems().to_vec();
            panic!(
                "the program picked was never started in {waited:?}: the launcher ended with \
                 {ended:?} (nothing, if it is still running), and the compositor says {problems:?}"
            );
        }
        session.advance(Duration::from_millis(200));
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(launcher.join().unwrap(), Ok(()));
}

#[test]
fn in_the_grid_the_arrows_walk_across_and_enter_starts_the_one_chosen() {
    let mut session = Session::new();
    let mut launcher = launcher(&mut session).in_grid(true);
    settle(&mut session, &mut launcher);
    // Nothing typed, Left and Right are the grid's, not the field's: the
    // first lights the first.
    press(&mut session, key::RIGHT);
    press(&mut session, key::RIGHT);
    press(&mut session, key::RIGHT);
    press(&mut session, key::LEFT);
    press(&mut session, key::RIGHT);
    press(&mut session, key::RETURN);
    assert!(settle(&mut session, &mut launcher));
    session.eventually(|session| mark(session, "lens") == "x");
    assert_eq!(mark(&session, "files"), "");
}

#[test]
fn down_gives_the_programs_the_keyboard_so_that_left_walks_back_among_them() {
    let mut session = Session::new();
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);
    press(&mut session, F);
    press(&mut session, DOWN);
    press(&mut session, key::LEFT);
    press(&mut session, key::RETURN);
    assert!(settle(&mut session, &mut launcher));
    session.eventually(|session| mark(session, "files") == "x");
    assert_eq!(mark(&session, "firefox"), "");
}

#[test]
fn down_from_what_is_typed_lights_the_first_program_rather_than_the_next() {
    let mut session = Session::new();
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);
    press(&mut session, F);
    press(&mut session, DOWN);
    press(&mut session, key::RETURN);
    assert!(settle(&mut session, &mut launcher));
    session.eventually(|session| mark(session, "files") == "x");
    assert_eq!(mark(&session, "firefox"), "");
}

#[test]
fn the_cursor_blinks_from_the_last_key_unless_nothing_is_to_move() {
    // The tests' sessions keep still unless a part says otherwise, as the
    // launcher does here.
    let moving = ("theme.css", "launcher { transition: 180ms; }\n");
    let mut session = Session::with_config(&[moving]);
    let mut launcher = launcher_of(&mut session, &["Files"]);
    let at = Duration::from_millis;
    let input = Theme::default().style(Part::LauncherInput, States::NONE);
    // The cursor at the start of the empty field, past the mode's sign,
    // in the field's colour: the border and padding, the field's own, the
    // sign half as tall as the field, and the room beside it.
    let cursor = |session: &mut Session| {
        let picture = session.draw();
        let ink = argb(input.color);
        let (left, top) = corner();
        let x = left + 13 + 8 + 28 + 10;
        let field = top..top + 13 + 56;
        (x - 1..=x + 1).any(|x| field.clone().any(|y| picture.at(x, y) == ink))
    };
    settle_at(&mut session, &mut launcher, at(0));
    // The compositor brings it in as the theme says, by its own clock.
    session.advance(Duration::from_millis(300));
    assert!(cursor(&mut session));
    assert_eq!(launcher.deadline(), Some(at(500)));
    settle_at(&mut session, &mut launcher, at(500));
    assert!(!cursor(&mut session), "off");
    // A key shows it at once.
    press(&mut session, DOWN);
    press(&mut session, UP);
    settle_at(&mut session, &mut launcher, at(700));
    assert!(cursor(&mut session));
    assert_eq!(launcher.deadline(), Some(at(1200)));
    // Left alone, it stops, shown.
    settle_at(&mut session, &mut launcher, at(20_000));
    assert!(cursor(&mut session));
    assert_eq!(launcher.deadline(), None);

    // Not where motion is reduced, nor where the theme moves nothing.
    let reduced = ("config.yaml", "appearance: { reduce_motion: true }\n");
    for files in [&[moving, reduced][..], &[]] {
        let mut still = Session::with_config(files);
        let mut held = launcher_of(&mut still, &["Files"]);
        settle_at(&mut still, &mut held, at(500));
        still.advance(Duration::from_millis(300));
        assert!(cursor(&mut still), "{files:?}");
        assert_eq!(held.deadline(), None, "{files:?}");
    }
}

#[test]
fn the_launcher_grows_to_what_it_lists_over_a_while_unless_nothing_is_to_move() {
    let moving = ("theme.css", "launcher { transition: 180ms; }\n");
    let reduced = ("config.yaml", "appearance: { reduce_motion: true }\n");
    let at = Duration::from_millis;
    // Of the three that have an e, the heading over all but the top hit,
    // the last of the three lines the test's screen has room for: well
    // under the field alone, and further than the launcher has grown half
    // way.
    let (x, y) = row(2);
    for (files, moves) in [
        (&[moving][..], true),
        (&[moving, reduced], false),
        (&[], false),
    ] {
        let mut session = Session::with_config(files);
        let mut launcher = launcher(&mut session);
        settle_at(&mut session, &mut launcher, at(0));
        session.advance(Duration::from_millis(300));
        let behind = session.draw().at(x, y);
        assert_ne!(behind, surface(), "{files:?}");
        press(&mut session, E);
        settle_at(&mut session, &mut launcher, at(1000));
        let glides = launcher.deadline() == Some(at(1016));
        assert_eq!(glides, moves, "{files:?}");
        if moves {
            // Setting out, it is as short as it was, and half way there
            // in half the time.
            assert_ne!(session.draw().at(x, y), surface());
            settle_at(&mut session, &mut launcher, at(1100));
            assert_ne!(session.draw().at(x, y), surface(), "not that far yet");
            settle_at(&mut session, &mut launcher, at(1200));
        }
        assert_eq!(session.draw().at(x, y), surface(), "{files:?}");
        assert_ne!(launcher.deadline(), Some(at(1216)), "at rest");
    }
}

#[test]
fn the_lit_rows_ground_slides_to_the_next_row_lit_unless_nothing_is_to_move() {
    let moving = ("theme.css", "launcher { transition: 180ms; }\n");
    let reduced = ("config.yaml", "appearance: { reduce_motion: true }\n");
    let at = Duration::from_millis;
    for (files, moves) in [(&[moving][..], true), (&[moving, reduced], false)] {
        let mut session = Session::with_config(files);
        let mut launcher = launcher(&mut session);
        settle_at(&mut session, &mut launcher, at(0));
        session.advance(Duration::from_millis(300));
        press(&mut session, E);
        settle_at(&mut session, &mut launcher, at(1000));
        settle_at(&mut session, &mut launcher, at(1300));
        // The first lit is lit where it is, with nothing to slide from.
        press(&mut session, DOWN);
        settle_at(&mut session, &mut launcher, at(2000));
        assert!(lit_at(&mut session, 0, 1), "{files:?}");
        assert_ne!(launcher.deadline(), Some(at(2016)), "{files:?}");
        // The next, Firefox, past the heading, the list scrolled to it and
        // its heading, the two lines this screen has room for: the ground
        // sets out from the top hit, above what is in sight, comes in over
        // the heading, and gets there.
        press(&mut session, DOWN);
        settle_at(&mut session, &mut launcher, at(3000));
        if moves {
            assert_eq!(launcher.deadline(), Some(at(3016)));
            assert!(!lit_at(&mut session, 2, 2), "setting out, out of sight");
            assert!(!lit_at(&mut session, 2, 3));
            settle_at(&mut session, &mut launcher, at(3075));
            assert!(lit_at(&mut session, 2, 2), "over the heading");
            settle_at(&mut session, &mut launcher, at(3150));
        }
        assert!(lit_at(&mut session, 2, 3), "{files:?}");
        assert!(!lit_at(&mut session, 2, 2), "{files:?}");
        assert_ne!(launcher.deadline(), Some(at(3166)), "{files:?}");
    }
}

#[test]
fn the_fields_sign_turns_into_the_next_modes_unless_nothing_is_to_move() {
    // Tiles narrow enough that the grid is no wider than the list, so
    // that the sign stays where it is on the screen.
    let moving = (
        "theme.css",
        "launcher { transition: 180ms; }\nlauncher-tile { min-width: 70px; }\n",
    );
    let reduced = ("config.yaml", "appearance: { reduce_motion: true }\n");
    let at = Duration::from_millis;
    // The square of the field the sign is in: the launcher's border and
    // padding and the field's, and half the field's height, in its middle.
    let sign = |session: &mut Session| {
        let picture = session.draw();
        let (left, top) = corner();
        let (x, y) = (left + 13 + 8, top + 13 + 14);
        let square = (x..x + 28).flat_map(|x| (y..y + 28).map(move |y| (x, y)));
        square.map(|(x, y)| picture.at(x, y)).collect::<Vec<u32>>()
    };
    // Down the first program's tile, its name under where its icon would
    // be, under the field and its margin: a sixth of the room inside the
    // launcher's border and padding.
    let tile = |session: &mut Session| {
        let picture = session.draw();
        let (left, top) = corner();
        let x = left + 13 + (480 - 26) / 12;
        let ys = top + 13 + 56 + 8..top + 13 + 56 + 8 + 80;
        ys.map(|y| picture.at(x, y)).collect::<Vec<u32>>()
    };
    for (files, moves) in [(&[moving][..], true), (&[moving, reduced], false)] {
        let mut session = Session::with_config(files);
        let mut launcher = launcher(&mut session);
        settle_at(&mut session, &mut launcher, at(0));
        session.advance(Duration::from_millis(300));
        let everything = sign(&mut session);
        // Every program's mode, the first.
        chord(&mut session, &[key::SUPER], key::ONE);
        settle_at(&mut session, &mut launcher, at(1000));
        if moves {
            assert_eq!(sign(&mut session), everything, "setting out");
            settle_at(&mut session, &mut launcher, at(1110));
            assert_ne!(sign(&mut session), everything, "on its way");
            // What the mode shows fading in as it turns: once the
            // launcher has grown past the first program's name, and before
            // the name is all there.
            settle_at(&mut session, &mut launcher, at(1140));
            let coming = tile(&mut session);
            settle_at(&mut session, &mut launcher, at(1300));
            let there = tile(&mut session);
            let glass = surface();
            // The name, the first ink down the tile, up to the glass after.
            let first = (0..there.len()).find(|at| there[*at] != glass);
            let name = (first.unwrap_or(0)..there.len()).take_while(|at| there[*at] != glass);
            let name = name.collect::<Vec<usize>>();
            let past = name.last().map_or(0, |last| last + 1);
            assert_eq!(coming[past], glass, "grown past it");
            assert!(name.iter().any(|at| coming[*at] != there[*at]), "fading in");
        }
        let programs = sign(&mut session);
        assert_ne!(programs, everything, "{files:?}");
        assert_ne!(launcher.deadline(), Some(at(1316)), "{files:?}");
    }
}

#[test]
fn super_with_a_digit_is_the_launchers_while_it_has_the_keyboard() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);
    // With the launcher open, Super and 2 is no workspace.
    hold(&mut session, key::SUPER, true);
    press(&mut session, key::TWO);
    hold(&mut session, key::SUPER, false);
    assert!(!settle(&mut session, &mut launcher), "still open");
    assert_eq!(session.compositor().desktop().active_workspace(), 0);
    // With a window's keyboard it is the workspace's, as ever:
    // `input::shift_selects_a_different_binding_on_the_same_key`.
}

/// Presses `code` with `held` held, in the order a hand would.
fn chord(session: &mut Session, held: &[Keycode], code: Keycode) {
    for key in held {
        hold(session, *key, true);
    }
    press(session, code);
    for key in held.iter().rev() {
        hold(session, *key, false);
    }
}

#[test]
fn with_the_launcher_up_super_with_shift_and_a_digit_is_still_its_binding() {
    // As the pictures of the screen are, on Super, Shift and a digit.
    let keys = "bindings:\n  - { keys: Super+Shift+2, action: { workspace: 2 } }\n";
    let mut session = Session::with_config(&[("keys.yaml", keys)]);
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);
    chord(&mut session, &[key::SUPER, key::SHIFT], key::TWO);
    settle(&mut session, &mut launcher);
    assert_eq!(session.compositor().desktop().active_workspace(), 1);
}

#[test]
fn in_the_ordinary_way_super_and_a_digit_bound_is_its_binding_with_the_launcher_up() {
    // There the launcher's modes are on Ctrl, and Super is the system's.
    let keys = "defaults: ordinary\nbindings:\n  - { keys: Super+2, action: { workspace: 2 } }\n";
    let mut session = Session::with_config(&[("keys.yaml", keys)]);
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);
    chord(&mut session, &[key::SUPER], key::TWO);
    settle(&mut session, &mut launcher);
    assert_eq!(session.compositor().desktop().active_workspace(), 1);
}

#[test]
fn super_and_1_goes_to_the_programs_grid_and_escape_back() {
    let mut session = Session::new();
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);
    hold(&mut session, key::SUPER, true);
    press(&mut session, key::ONE);
    hold(&mut session, key::SUPER, false);
    // Escape goes back to everything and keeps the launcher; Super and 1
    // goes to the grid again, where the arrows walk across.
    press(&mut session, key::ESCAPE);
    assert!(!settle(&mut session, &mut launcher), "still open");
    hold(&mut session, key::SUPER, true);
    press(&mut session, key::ONE);
    hold(&mut session, key::SUPER, false);
    press(&mut session, key::RIGHT);
    press(&mut session, key::RIGHT);
    press(&mut session, key::RETURN);
    assert!(settle(&mut session, &mut launcher));
    session.eventually(|session| mark(session, "firefox") == "x");
}

#[test]
fn the_generic_mode_starts_the_top_hit_on_enter() {
    let mut session = Session::new();
    let mut launcher = launcher(&mut session);
    settle(&mut session, &mut launcher);
    press(&mut session, F);
    press(&mut session, I);
    press(&mut session, key::RETURN);
    assert!(settle(&mut session, &mut launcher));
    session.eventually(|session| mark(session, "files") == "x");
    assert_eq!(mark(&session, "firefox"), "");
}
