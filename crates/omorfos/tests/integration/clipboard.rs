//! The clipboard: copying in one application and pasting in another.

use std::io::Read;

use omorfos_ipc::copied::{LONGEST_KIND, MOST_KINDS};

use crate::common::{Session, read_text};
use crate::windows::{BLUE, RED};

#[test]
fn text_copied_in_one_application_can_be_pasted_in_another() {
    let mut session = Session::new();
    let mut editor = session.connect();
    session.open(&mut editor, "editor", (200, 100), RED);
    editor.copy("hello from the editor");
    session.roundtrip(&mut editor);

    // A second application opens and takes the keyboard, and with it the
    // offer of the clipboard.
    let mut terminal = session.connect();
    session.open(&mut terminal, "terminal", (200, 100), BLUE);
    assert_eq!(terminal.state.formats, ["text/plain;charset=utf-8"]);

    // The terminal asks, the editor answers, and the text arrives.
    let pipe = terminal.paste().unwrap();
    session.roundtrip(&mut terminal);
    session.roundtrip(&mut editor);
    let pasted = read_text(pipe);
    assert_eq!(pasted, "hello from the editor");
}

#[test]
fn a_program_that_copies_in_very_many_kinds_does_not_end_the_program_next_given_the_keyboard() {
    let mut session = Session::new();
    let mut flood = session.connect();
    session.open(&mut flood, "flood", (200, 100), RED);
    // It copies as any program does, and only then says the rest: 400
    // kinds of 4000 bytes, more than a program's socket holds, a few at a
    // time as it is read.
    let source = flood.copy_as("too much", &["text/plain"]);
    session.roundtrip(&mut flood);
    for kind in 0..400 {
        source.offer(format!("flood/{kind:0>3994}"));
        if kind % 16 == 15 {
            session.roundtrip(&mut flood);
        }
    }

    // The program the user goes to next is not told of them, and so is
    // still there: it is offered nothing.
    let mut editor = session.connect();
    session.open(&mut editor, "editor", (200, 100), BLUE);
    assert_eq!(editor.state.formats.len(), 0);
    session.roundtrip(&mut editor);
}

#[test]
fn a_program_that_copies_in_more_kinds_than_any_does_each_short_is_not_offered() {
    let mut session = Session::new();
    let mut flood = session.connect();
    session.open(&mut flood, "flood", (200, 100), RED);
    // Kinds as short as kinds are, but far more of them than any program
    // offers: each is within how long a kind may be, and only how many
    // there are refuses them.
    let source = flood.copy_as("too many", &["text/plain"]);
    session.roundtrip(&mut flood);
    for kind in 0..2000 {
        source.offer(format!("flood/{kind}"));
        if kind % 64 == 63 {
            session.roundtrip(&mut flood);
        }
    }
    session.roundtrip(&mut flood);

    let mut editor = session.connect();
    session.open(&mut editor, "editor", (200, 100), BLUE);
    assert_eq!(editor.state.formats.len(), 0, "offered nothing");
    session.roundtrip(&mut editor);
}

#[test]
fn a_program_may_copy_in_as_many_kinds_as_any_does_and_each_as_long() {
    let mut session = Session::new();
    let mut editor = session.connect();
    session.open(&mut editor, "editor", (200, 100), RED);
    let source = editor.copy_as("much", &[]);
    // The most, each as long as it may be.
    let width = LONGEST_KIND - "much/".len();
    for kind in 0..MOST_KINDS {
        source.offer(format!("much/{kind:0>width$}"));
    }
    session.roundtrip(&mut editor);

    let mut terminal = session.connect();
    session.open(&mut terminal, "terminal", (200, 100), BLUE);
    assert_eq!(terminal.state.formats.len(), MOST_KINDS);
    assert_eq!(terminal.state.formats[MOST_KINDS - 1].len(), LONGEST_KIND);
}

#[test]
fn text_selected_in_a_kind_longer_than_any_is_not_offered_with_the_middle_button() {
    let mut session = Session::new();
    let mut flood = session.connect();
    session.open(&mut flood, "flood", (200, 100), RED);
    // What it copied, in a kind as kinds are, is none the worse for it.
    flood.copy("copied");
    let source = flood.select("selected");
    // One byte longer than a kind may be.
    let width = LONGEST_KIND + 1 - "flood/".len();
    source.offer(format!("flood/{:0>width$}", 0));
    session.roundtrip(&mut flood);

    let mut editor = session.connect();
    session.open(&mut editor, "editor", (200, 100), BLUE);
    assert!(editor.paste_selection().is_none());
    assert_eq!(editor.state.formats, ["text/plain;charset=utf-8"]);
}

#[test]
fn a_program_that_pastes_through_a_pipe_already_full_does_not_stop_the_compositor() {
    use std::io::Write;
    use std::os::fd::AsFd;

    use rustix::fs::{OFlags, fcntl_setfl};

    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    // Text of the compositor's own is on the clipboard, as after a sum is
    // copied from the launcher.
    let socket = session.listen();
    let mut peer = omorfos_ipc::Client::connect(&socket).unwrap();
    let text = "from the compositor".to_owned();
    let copy = omorfos_ipc::Request::Copy { text: text.clone() };
    session.ask(&mut peer, &copy);
    session.roundtrip(&mut client);
    let offer = client.state.selection.clone().unwrap();

    // The program asks for it through a pipe with no room left, which
    // waits when it is written to.
    let (mut reader, mut writer) = std::io::pipe().unwrap();
    fcntl_setfl(&writer, OFlags::NONBLOCK).unwrap();
    let mut filled = 0;
    while let Ok(more) = writer.write(&[b'.'; 4096]) {
        filled += more;
    }
    fcntl_setfl(&writer, OFlags::empty()).unwrap();
    offer.receive(crate::common::TEXT.to_owned(), writer.as_fd());
    drop(writer);

    // The compositor goes on, with the program not reading.
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    assert_eq!(session.compositor_mut().handing_over(), 1);
    // And once the program does read, what it asked for is there.
    let mut read = vec![0; filled];
    reader.read_exact(&mut read).unwrap();
    let pasted = read_text(reader);
    assert_eq!(pasted, text);
    session.eventually(|session| session.compositor_mut().handing_over() == 0);
}

#[test]
fn an_application_can_paste_what_it_copied_itself() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "editor", (200, 100), RED);
    client.copy("round trip");
    session.roundtrip(&mut client);

    let pipe = client.paste().unwrap();
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    let text = read_text(pipe);
    assert_eq!(text, "round trip");
}

#[test]
fn an_application_without_the_keyboard_is_not_offered_the_clipboard() {
    let mut session = Session::new();
    let mut background = session.connect();
    session.open(&mut background, "background", (200, 100), RED);
    let mut focused = session.connect();
    session.open(&mut focused, "focused", (200, 100), BLUE);

    focused.copy("private to whoever is focused");
    session.roundtrip(&mut focused);
    session.roundtrip(&mut background);

    assert!(background.state.selection.is_none());
    assert!(focused.state.selection.is_some());
}

#[test]
fn an_empty_clipboard_offers_nothing() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    assert!(client.paste().is_none());
}

#[test]
fn selected_text_can_be_pasted_elsewhere_with_the_middle_button() {
    let mut session = Session::new();
    let mut editor = session.connect();
    session.open(&mut editor, "editor", (200, 100), RED);
    editor.select("selected in the editor");
    editor.copy("copied, which is a different thing");
    session.roundtrip(&mut editor);

    let mut terminal = session.connect();
    session.open(&mut terminal, "terminal", (200, 100), BLUE);

    let pipe = terminal.paste_selection().unwrap();
    session.roundtrip(&mut terminal);
    session.roundtrip(&mut editor);
    let pasted = read_text(pipe);
    assert_eq!(pasted, "selected in the editor");
}

#[test]
fn with_nothing_selected_there_is_nothing_to_paste() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    assert!(client.paste_selection().is_none());
}
