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
