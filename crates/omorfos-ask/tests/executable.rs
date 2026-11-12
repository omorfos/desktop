//! The `omorfos-ask` executable itself, where there is no compositor.

use std::process::Command;

#[test]
fn without_a_compositor_it_says_why_it_cannot_ask_and_fails() {
    let mut asker = Command::new(env!("CARGO_BIN_EXE_omorfos-ask"));
    // Only the variable that tells an instrumented build where to record
    // coverage is passed on.
    asker.env_clear();
    asker.envs(std::env::vars_os().filter(|(name, _)| name == "LLVM_PROFILE_FILE"));
    let done = asker.output().unwrap();
    assert_eq!(done.status.code(), Some(1));
    let said = String::from_utf8_lossy(&done.stderr);
    assert!(
        said.starts_with("omorfos-ask: this is not an omorfos session"),
        "{said}"
    );
}

#[test]
fn asked_what_it_is_omorfos_ask_says_so_and_starts_nothing() {
    // Outside a session, so that starting instead would fail.
    for (asked, said) in [
        ("--help", "Usage: omorfos-ask"),
        ("-h", "Usage: omorfos-ask"),
        ("--version", "omorfos-ask "),
        ("-V", "omorfos-ask "),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_omorfos-ask"));
        command.env_clear();
        command.envs(std::env::vars_os().filter(|(name, _)| name == "LLVM_PROFILE_FILE"));
        let output = command.arg(asked).output().unwrap();
        assert!(output.status.success(), "{asked}: {output:?}");
        let out = String::from_utf8_lossy(&output.stdout);
        assert!(out.contains(said), "{asked}: {out}");
    }
}

#[test]
fn asked_what_it_is_omorfos_pinentry_says_so_and_starts_nothing() {
    // Outside a session, so that starting instead would fail.
    for (asked, said) in [
        ("--help", "Usage: omorfos-pinentry"),
        ("-h", "Usage: omorfos-pinentry"),
        ("--version", "omorfos-pinentry "),
        ("-V", "omorfos-pinentry "),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_omorfos-pinentry"));
        command.env_clear();
        command.envs(std::env::vars_os().filter(|(name, _)| name == "LLVM_PROFILE_FILE"));
        let output = command.arg(asked).output().unwrap();
        assert!(output.status.success(), "{asked}: {output:?}");
        let out = String::from_utf8_lossy(&output.stdout);
        assert!(out.contains(said), "{asked}: {out}");
    }
}
