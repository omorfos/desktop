//! The `omorfos-screenshot` executable itself.

use std::process::Command;

fn screenshot() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_omorfos-screenshot"));
    // Nothing from the real session may leak in. Only the variable that
    // tells an instrumented build where to record coverage is passed on.
    command.env_clear();
    command.envs(std::env::vars_os().filter(|(name, _)| name == "LLVM_PROFILE_FILE"));
    command
}

#[test]
fn outside_a_session_it_fails_with_an_explanation() {
    let output = screenshot().output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let said = String::from_utf8(output.stderr).unwrap();
    assert!(
        said.starts_with("omorfos-screenshot: this is not an omorfos session"),
        "{said}"
    );
}

#[test]
fn a_session_that_is_not_there_is_reported_too() {
    let directory = tempfile::TempDir::new().unwrap();
    let output = screenshot()
        .env("XDG_RUNTIME_DIR", directory.path())
        .env("WAYLAND_DISPLAY", "wayland-none")
        .env("HOME", directory.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let said = String::from_utf8(output.stderr).unwrap();
    assert!(said.starts_with("omorfos-screenshot: "), "{said}");
    assert!(said.contains("wayland-none"), "{said}");
    assert_eq!(output.stdout, b"");
}

#[test]
fn asked_what_it_is_omorfos_screenshot_says_so_and_starts_nothing() {
    // Outside a session, so that starting instead would fail.
    for (asked, said) in [
        ("--help", "Usage: omorfos-screenshot"),
        ("-h", "Usage: omorfos-screenshot"),
        ("--version", "omorfos-screenshot "),
        ("-V", "omorfos-screenshot "),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_omorfos-screenshot"));
        command.env_clear();
        command.envs(std::env::vars_os().filter(|(name, _)| name == "LLVM_PROFILE_FILE"));
        let output = command.arg(asked).output().unwrap();
        assert!(output.status.success(), "{asked}: {output:?}");
        let out = String::from_utf8_lossy(&output.stdout);
        assert!(out.contains(said), "{asked}: {out}");
    }
}
