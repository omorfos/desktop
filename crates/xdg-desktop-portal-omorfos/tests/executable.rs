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
