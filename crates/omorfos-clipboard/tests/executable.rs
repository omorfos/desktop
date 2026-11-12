//! The `omorfos-clipboard` executable itself.

use std::process::Command;

#[test]
fn outside_a_session_it_fails_with_an_explanation() {
    let mut keeper = Command::new(env!("CARGO_BIN_EXE_omorfos-clipboard"));
    // Nothing from the real session may leak in. Only the variable that
    // tells an instrumented build where to record coverage is passed on.
    keeper.env_clear();
    keeper.envs(std::env::vars_os().filter(|(name, _)| name == "LLVM_PROFILE_FILE"));
    let output = keeper.output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let said = String::from_utf8(output.stderr).unwrap();
    assert!(
        said.starts_with("omorfos-clipboard: this is not an omorfos session"),
        "{said}"
    );
}

#[test]
fn asked_what_it_is_omorfos_clipboard_says_so_and_starts_nothing() {
    // Outside a session, so that starting instead would fail.
    for (asked, said) in [
        ("--help", "Usage: omorfos-clipboard"),
        ("-h", "Usage: omorfos-clipboard"),
        ("--version", "omorfos-clipboard "),
        ("-V", "omorfos-clipboard "),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_omorfos-clipboard"));
        command.env_clear();
        command.envs(std::env::vars_os().filter(|(name, _)| name == "LLVM_PROFILE_FILE"));
        let output = command.arg(asked).output().unwrap();
        assert!(output.status.success(), "{asked}: {output:?}");
        let out = String::from_utf8_lossy(&output.stdout);
        assert!(out.contains(said), "{asked}: {out}");
    }
}
