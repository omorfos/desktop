//! The `omorfos-session` executable itself, with a stand-in for `wpctl`
//! and no bus, so that nothing of the real session is touched.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::path::Path;
use std::process::{Command, Output};

fn run(arguments: &[&str], wpctl: &str) -> Output {
    let fake = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fake")
        .join(wpctl);
    let mut session = Command::new(env!("CARGO_BIN_EXE_omorfos-session"));
    // Only the variable that tells an instrumented build where to record
    // coverage is passed on.
    session.env_clear();
    session.envs(std::env::vars_os().filter(|(name, _)| name == "LLVM_PROFILE_FILE"));
    session.env("PATH", fake).args(arguments).output().unwrap()
}

#[test]
fn what_is_asked_is_done_and_nothing_is_said() {
    let done = run(&["volume", "up"], "wpctl");
    assert!(done.status.success());
    assert_eq!(String::from_utf8_lossy(&done.stderr), "");
}

#[test]
fn what_could_not_be_done_is_said_and_fails() {
    let done = run(&["volume", "up"], "broken");
    assert_eq!(done.status.code(), Some(1));
    let said = String::from_utf8_lossy(&done.stderr);
    // What wpctl said of why comes first, as it said it.
    let expected = "Could not connect to PipeWire\n\
        omorfos-session: wpctl set-volume -l 1.0 @DEFAULT_AUDIO_SINK@ 5%+ failed\n";
    assert_eq!(said, expected);
}

#[test]
fn what_is_not_understood_is_answered_with_how_to_ask() {
    let done = run(&["louder"], "wpctl");
    assert_eq!(done.status.code(), Some(2));
    let said = String::from_utf8_lossy(&done.stderr);
    assert!(
        said.starts_with("omorfos-session: usage: omorfos-session volume up"),
        "{said}"
    );
}
