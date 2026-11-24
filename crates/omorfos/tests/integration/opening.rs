//! The root menu's Browser, Files, Editor, Terminal and Settings: each
//! opens the application the system has as the default for it, or says
//! none is set; the terminal is `foot` where none is named.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::fs;
use std::path::PathBuf;

use omorfos_config::{Action, Application};

use crate::common::{Session, runnable};

/// A session whose places of configuration and data are of the test's
/// own, with a `mimeapps.list` that has `browser.desktop` open web links,
/// and that entry run a script that writes what it was given to the file
/// returned.
fn session() -> (Session, tempfile::TempDir, PathBuf) {
    let directory = tempfile::TempDir::new().unwrap();
    let (config, data) = (
        directory.path().join("config"),
        directory.path().join("data"),
    );
    let applications = data.join("applications");
    fs::create_dir_all(&config).unwrap();
    fs::create_dir_all(&applications).unwrap();
    let opened = directory.path().join("opened");
    let program = directory.path().join("browser");
    runnable(
        &program,
        &format!("#!/bin/sh\necho \"$@\" >> '{}'\n", opened.display()),
    );
    let list = "[Default Applications]\nx-scheme-handler/https=browser.desktop\n";
    fs::write(config.join("mimeapps.list"), list).unwrap();
    let entry = format!(
        "[Desktop Entry]\nName=Browser\nExec={} --new-window %u\n",
        program.display()
    );
    fs::write(applications.join("browser.desktop"), entry).unwrap();
    let mut session = Session::new();
    session
        .compositor_mut()
        .set_config_directories(vec![config]);
    session.compositor_mut().set_data_directories(vec![data]);
    (session, directory, opened)
}

#[test]
fn the_default_browser_is_opened_as_its_desktop_entry_says() {
    let (mut session, _directory, opened) = session();
    session
        .compositor_mut()
        .perform(&Action::Open(Application::Browser));
    let said = || fs::read_to_string(&opened).unwrap_or_default();
    session.eventually(|_| said() == "--new-window\n");
}

#[test]
fn with_nothing_set_to_open_it_the_user_is_told_each_time() {
    let (mut session, _directory, _opened) = session();
    for kind in [
        Application::Files,
        Application::Editor,
        Application::Settings,
    ] {
        session.compositor_mut().perform(&Action::Open(kind));
    }
    session.step();
    let said: Vec<String> = session.compositor().events().map(str::to_owned).collect();
    assert_eq!(said.len(), 3, "{said:?}");
    assert!(said[0].starts_with("No file manager is set"), "{said:?}");
    assert!(said[1].starts_with("No text editor is set"), "{said:?}");
    assert!(said[2].contains("no settings of its own yet"), "{said:?}");

    session
        .compositor_mut()
        .perform(&Action::Open(Application::Settings));
    session.step();
    let again = session
        .compositor()
        .events()
        .filter(|said| said.contains("no settings"))
        .count();
    assert_eq!(again, 2, "told again");
}

#[test]
fn the_terminal_opened_is_the_first_an_xdg_terminals_list_names_as_its_entry_says() {
    let (mut session, directory, _opened) = session();
    let (config, data) = (
        directory.path().join("config"),
        directory.path().join("data"),
    );
    let started = directory.path().join("started");
    let program = directory.path().join("terminal");
    runnable(
        &program,
        &format!("#!/bin/sh\necho \"$@\" >> '{}'\n", started.display()),
    );
    let entry = format!(
        "[Desktop Entry]\nName=Terminal\nExec={} --login\n",
        program.display()
    );
    fs::write(data.join("applications").join("terminal.desktop"), entry).unwrap();
    let list = "# not there\ngone.desktop\nterminal.desktop\n";
    fs::write(config.join("xdg-terminals.list"), list).unwrap();
    session
        .compositor_mut()
        .perform(&Action::Open(Application::Terminal));
    let said = || fs::read_to_string(&started).unwrap_or_default();
    session.eventually(|_| said() == "--login\n");
}
