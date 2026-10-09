//! The list of windows offered to taskbars (ext-foreign-toplevel-list).

use crate::common::{Client, Session};
use crate::windows::{BLUE, RED};

/// What a taskbar would show: each open window's application and title.
fn listing(taskbar: &Client) -> Vec<(String, String)> {
    let open = taskbar.state.taskbar.iter().filter(|task| !task.closed);
    open.map(|task| (task.app_id.clone(), task.title.clone()))
        .collect()
}

fn entry(app_id: &str, title: &str) -> (String, String) {
    (app_id.to_owned(), title.to_owned())
}

#[test]
fn a_taskbar_is_told_about_windows_that_are_already_open() {
    let mut session = Session::new();
    let mut apps = session.connect();
    session.open(&mut apps, "editor", (200, 100), RED);
    session.open(&mut apps, "terminal", (200, 100), BLUE);

    let taskbar = session.connect();

    assert_eq!(
        listing(&taskbar),
        [
            entry("editor", "editor window"),
            entry("terminal", "terminal window")
        ]
    );
}

#[test]
fn a_taskbar_hears_of_windows_opening_changing_title_and_closing() {
    let mut session = Session::new();
    let mut taskbar = session.connect();
    let mut apps = session.connect();
    assert_eq!(listing(&taskbar), []);

    let window = session.open(&mut apps, "editor", (200, 100), RED);
    session.roundtrip(&mut taskbar);
    assert_eq!(listing(&taskbar), [entry("editor", "editor window")]);

    window.role.set_title("notes.txt".to_owned());
    session.roundtrip(&mut apps);
    session.roundtrip(&mut taskbar);
    assert_eq!(listing(&taskbar), [entry("editor", "notes.txt")]);

    window.role.destroy();
    window.xdg_surface.destroy();
    window.surface.destroy();
    session.roundtrip(&mut apps);
    session.roundtrip(&mut taskbar);
    assert_eq!(listing(&taskbar), []);
}

#[test]
fn a_title_set_before_a_window_shows_anything_is_not_announced() {
    let mut session = Session::new();
    let mut taskbar = session.connect();
    let mut apps = session.connect();

    let window = apps.toplevel("slow", "loading");
    window.role.set_title("still loading".to_owned());
    session.roundtrip(&mut apps);
    session.roundtrip(&mut taskbar);

    assert_eq!(
        listing(&taskbar),
        [],
        "a window with no content yet is not in the list"
    );
}
