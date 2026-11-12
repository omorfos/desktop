//! The `xdg-desktop-portal-omorfos` executable itself.

use std::process::Command;

#[test]
fn where_it_cannot_begin_it_fails_with_an_explanation() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_xdg-desktop-portal-omorfos"));
    // Nothing from the real session may leak in. Only the variable that
    // tells an instrumented build where to record coverage is passed on.
    command.env_clear();
    command.envs(std::env::vars_os().filter(|(name, _)| name == "LLVM_PROFILE_FILE"));
    let output = command.output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let said = String::from_utf8(output.stderr).unwrap();
    assert!(
        said.starts_with("xdg-desktop-portal-omorfos: this is not an omorfos session"),
        "{said}"
    );
}

#[test]
fn asked_what_it_is_xdg_desktop_portal_omorfos_says_so_and_starts_nothing() {
    // Outside a session, so that starting instead would fail.
    for (asked, said) in [
        ("--help", "Usage: xdg-desktop-portal-omorfos"),
        ("-h", "Usage: xdg-desktop-portal-omorfos"),
        ("--version", "xdg-desktop-portal-omorfos "),
        ("-V", "xdg-desktop-portal-omorfos "),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_xdg-desktop-portal-omorfos"));
        command.env_clear();
        command.envs(std::env::vars_os().filter(|(name, _)| name == "LLVM_PROFILE_FILE"));
        let output = command.arg(asked).output().unwrap();
        assert!(output.status.success(), "{asked}: {output:?}");
        let out = String::from_utf8_lossy(&output.stdout);
        assert!(out.contains(said), "{asked}: {out}");
    }
}
