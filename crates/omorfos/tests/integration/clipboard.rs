//! The clipboard: copying in one application and pasting in another.

use std::io::Read;

use crate::common::Session;
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
    let mut pipe = terminal.paste().unwrap();
    session.roundtrip(&mut terminal);
    session.roundtrip(&mut editor);
    let mut pasted = String::new();
    pipe.read_to_string(&mut pasted).unwrap();
    assert_eq!(pasted, "hello from the editor");
}

#[test]
fn an_application_can_paste_what_it_copied_itself() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "editor", (200, 100), RED);
    client.copy("round trip");
    session.roundtrip(&mut client);

    let mut pipe = client.paste().unwrap();
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    let mut text = String::new();
    pipe.read_to_string(&mut text).unwrap();
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

    let mut pipe = terminal.paste_selection().unwrap();
    session.roundtrip(&mut terminal);
    session.roundtrip(&mut editor);
    let mut pasted = String::new();
    pipe.read_to_string(&mut pasted).unwrap();
    assert_eq!(pasted, "selected in the editor");
}

#[test]
fn with_nothing_selected_there_is_nothing_to_paste() {
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    assert!(client.paste_selection().is_none());
}
