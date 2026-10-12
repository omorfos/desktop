//! The `omorfos-notifications` executable itself.

use std::process::Command;

#[test]
fn outside_a_session_it_fails_with_an_explanation() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_omorfos-notifications"));
    // Nothing from the real session may leak in. Only the variable that
    // tells an instrumented build where to record coverage is passed on.
    command.env_clear();
    command.envs(std::env::vars_os().filter(|(name, _)| name == "LLVM_PROFILE_FILE"));
    let output = command.output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let said = String::from_utf8(output.stderr).unwrap();
    assert!(
        said.starts_with("omorfos-notifications: this is not an omorfos session"),
        "{said}"
    );
}

#[test]
fn a_session_that_is_not_there_is_reported_too() {
    let directory = tempfile::TempDir::new().unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_omorfos-notifications"));
    command.env_clear();
    command.envs(std::env::vars_os().filter(|(name, _)| name == "LLVM_PROFILE_FILE"));
    let output = command
        .env("XDG_RUNTIME_DIR", directory.path())
        .env("WAYLAND_DISPLAY", "wayland-none")
        .env("HOME", directory.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let said = String::from_utf8(output.stderr).unwrap();
    assert!(said.starts_with("omorfos-notifications: "), "{said}");
    assert!(!said.contains("not an omorfos session"), "{said}");
}
