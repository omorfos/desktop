//! The watcher, debouncer and store working together, as an event loop
//! would drive them.

use std::fs;
use std::time::{Duration, Instant};

use omorfos_config::{Color, Debouncer, Part, States, Store, Watcher};
use tempfile::TempDir;

const DELAY: Duration = Duration::from_millis(100);

/// The colour the theme in effect picks things out in.
fn accent(store: &Store) -> Color {
    let theme = &store.current().theme;
    theme.style(Part::Picker, States::NONE).border.top.color
}

#[test]
fn a_saved_change_is_applied_once_the_directory_goes_quiet() {
    let directory = TempDir::new().unwrap();
    let mut store = Store::open(directory.path());
    let mut watcher = Watcher::new(store.directory()).unwrap();
    let mut debouncer = Debouncer::new(DELAY);
    let start = Instant::now();

    // The user saves a broken theme, then fixes it a moment later.
    fs::write(
        directory.path().join("theme.css"),
        ":root { --accent: nope; }\n",
    )
    .unwrap();
    assert!(watcher.drain().unwrap());
    debouncer.trigger(start);

    fs::write(
        directory.path().join("theme.css"),
        ":root { --accent: #00ff00; }\n",
    )
    .unwrap();
    assert!(watcher.drain().unwrap());
    debouncer.trigger(start + DELAY / 2);

    // Nothing is reloaded while changes are still arriving.
    assert!(!debouncer.fire(start + DELAY));
    assert!(debouncer.fire(start + DELAY * 2));

    assert!(store.reload());
    assert_eq!(accent(&store), Color::rgb(0, 255, 0));
    assert_eq!(store.diagnostics(), []);
}

#[test]
fn a_broken_save_keeps_the_session_running_on_the_previous_configuration() {
    let directory = TempDir::new().unwrap();
    fs::write(
        directory.path().join("theme.css"),
        ":root { --accent: #00ff00; }\n",
    )
    .unwrap();
    let mut store = Store::open(directory.path());
    let mut watcher = Watcher::new(store.directory()).unwrap();

    fs::write(
        directory.path().join("theme.css"),
        ":root { --accent: nope; }\n",
    )
    .unwrap();
    assert!(watcher.drain().unwrap());

    assert!(!store.reload());
    assert_eq!(accent(&store), Color::rgb(0, 255, 0));
    let problem = store.diagnostics()[0].to_string();
    assert!(
        problem.contains("theme.css:1:19: `nope` is not a color"),
        "{problem}"
    );
}

#[test]
fn a_new_scheme_or_accent_changes_the_theme_though_the_theme_itself_did_not() {
    let directory = TempDir::new().unwrap();
    let mut store = Store::open(directory.path());
    assert_eq!(accent(&store), Color::rgb(0x22, 0xd3, 0xee));
    let mut watcher = Watcher::new(store.directory()).unwrap();

    fs::write(
        directory.path().join("config.yaml"),
        "appearance: { scheme: light, accent: teal }\n",
    )
    .unwrap();
    assert!(watcher.drain().unwrap());

    assert!(store.reload());
    assert_eq!(accent(&store), Color::rgb(0x0f, 0x76, 0x6e));
    assert!(!store.current().theme.dark());
}
