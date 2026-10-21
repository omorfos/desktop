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
