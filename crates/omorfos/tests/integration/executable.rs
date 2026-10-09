//! The `omorfos` executable itself.

use std::process::Command;

fn omorfos() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_omorfos"));
    // Whatever the test finds, it must not open a window or touch the real
    // configuration directory. Only the variable that tells an instrumented
    // build where to record coverage is passed on.
    command.env_clear();
    command.envs(std::env::vars_os().filter(|(name, _)| name == "LLVM_PROFILE_FILE"));
    command
}

#[test]
fn version_is_printed() {
    let output = omorfos().arg("--version").output().unwrap();
    assert!(output.status.success());
    let expected = format!("omorfos {}\n", env!("CARGO_PKG_VERSION"));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
}

#[test]
fn an_unknown_argument_fails_with_usage() {
    let output = omorfos().arg("--frobnicate").output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("Usage: omorfos")
    );
}

#[test]
fn starting_without_a_home_fails_with_an_explanation() {
    let output = omorfos().output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "omorfos: cannot find the configuration directory: set HOME or XDG_CONFIG_HOME\n"
    );
}
