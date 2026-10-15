//! The `omorfos-recording` executable itself.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

#[test]
fn without_pipewire_it_says_nothing_records_and_stops_once_nobody_listens() {
    let folder = tempfile::TempDir::new().unwrap();
    let mut watcher = Command::new(env!("CARGO_BIN_EXE_omorfos-recording"));
    // Nothing from the real session may leak in: there is no PipeWire to be
    // found where it looks. Only the variable that tells an instrumented
    // build where to record coverage is passed on.
    watcher.env_clear();
    watcher.envs(std::env::vars_os().filter(|(name, _)| name == "LLVM_PROFILE_FILE"));
    watcher.env("XDG_RUNTIME_DIR", folder.path());
    let mut watcher = watcher.stdout(Stdio::piped()).spawn().unwrap();
    let mut said = BufReader::new(watcher.stdout.take().unwrap());
    let mut line = String::new();
    said.read_line(&mut line).unwrap();
    assert_eq!(line, ".\n");
    drop(said);
    assert!(watcher.wait().unwrap().success());
}
