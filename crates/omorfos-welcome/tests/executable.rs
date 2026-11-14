//! The `omorfos-welcome` executable itself.

use std::process::Command;

fn welcome() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_omorfos-welcome"));
    // Nothing from the real session may leak in. Only the variable that
    // tells an instrumented build where to record coverage is passed on.
    command.env_clear();
    command.envs(std::env::vars_os().filter(|(name, _)| name == "LLVM_PROFILE_FILE"));
    command
}

#[test]
fn outside_a_session_it_fails_with_an_explanation() {
    let output = welcome().output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let said = String::from_utf8(output.stderr).unwrap();
    assert!(
        said.starts_with("omorfos-welcome: this is not an omorfos session"),
        "{said}"
    );
}

#[test]
fn at_a_login_after_it_was_closed_it_shows_nothing_and_says_nothing() {
    let home = tempfile::TempDir::new().unwrap();
    let state = home.path().join(".local/state/omorfos");
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(state.join("welcomed"), "").unwrap();
    let output = welcome()
        .arg("--first")
        .env("HOME", home.path())
        .env("XDG_RUNTIME_DIR", home.path())
        .env("WAYLAND_DISPLAY", "wayland-none")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn asked_what_it_is_omorfos_welcome_says_so_and_shows_nothing() {
    // Outside a session, so that showing instead would fail.
    for (asked, said) in [
        ("--help", "Usage: omorfos-welcome"),
        ("-h", "Usage: omorfos-welcome"),
        ("--version", "omorfos-welcome "),
        ("-V", "omorfos-welcome "),
    ] {
        let output = welcome().arg(asked).output().unwrap();
        assert!(output.status.success(), "{asked}: {output:?}");
        let out = String::from_utf8_lossy(&output.stdout);
        assert!(out.contains(said), "{asked}: {out}");
    }
}
