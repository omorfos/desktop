//! The `omorfos-launcher` executable itself.

use std::process::Command;

fn launcher() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_omorfos-launcher"));
    // Nothing from the real session may leak in. Only the variable that
    // tells an instrumented build where to record coverage is passed on.
    command.env_clear();
    command.envs(std::env::vars_os().filter(|(name, _)| name == "LLVM_PROFILE_FILE"));
    command
}

#[test]
fn outside_a_session_it_fails_with_an_explanation() {
    let output = launcher().output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let said = String::from_utf8(output.stderr).unwrap();
    assert!(
        said.starts_with("omorfos-launcher: this is not an omorfos session"),
        "{said}"
    );
}

#[test]
fn asked_what_it_is_omorfos_launcher_says_so_and_starts_nothing() {
    // Outside a session, so that starting instead would fail.
    for (asked, said) in [
        ("--help", "Usage: omorfos-launcher"),
        ("-h", "Usage: omorfos-launcher"),
        ("--version", "omorfos-launcher "),
        ("-V", "omorfos-launcher "),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_omorfos-launcher"));
        command.env_clear();
        command.envs(std::env::vars_os().filter(|(name, _)| name == "LLVM_PROFILE_FILE"));
        let output = command.arg(asked).output().unwrap();
        assert!(output.status.success(), "{asked}: {output:?}");
        let out = String::from_utf8_lossy(&output.stdout);
        assert!(out.contains(said), "{asked}: {out}");
    }
}
