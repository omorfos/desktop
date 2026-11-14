//! The `omorfos-recording` executable itself.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

#[test]
fn without_pipewire_it_says_nothing_records_and_stops_once_nobody_listens() {
    let folder = tempfile::TempDir::new().unwrap();
    let mut watcher = Command::new(env!("CARGO_BIN_EXE_omorfos-recording"));
    // Nothing from the real session may leak in: there is no PipeWire to be
    // found where it looks. Only what an instrumented build is told is
    // passed on.
    watcher.env_clear();
    watcher.envs(std::env::vars_os().filter(|(name, _)| instrumented(name)));
    watcher.env("XDG_RUNTIME_DIR", folder.path());
    let mut watcher = watcher.stdout(Stdio::piped()).spawn().unwrap();
    let mut said = BufReader::new(watcher.stdout.take().unwrap());
    let mut line = String::new();
    said.read_line(&mut line).unwrap();
    assert_eq!(line, ".\n");
    drop(said);
    assert!(watcher.wait().unwrap().success());
}

#[test]
fn asked_what_it_is_omorfos_recording_says_so_and_starts_nothing() {
    // Outside a session, so that starting instead would fail.
    for (asked, said) in [
        ("--help", "Usage: omorfos-recording"),
        ("-h", "Usage: omorfos-recording"),
        ("--version", "omorfos-recording "),
        ("-V", "omorfos-recording "),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_omorfos-recording"));
        command.env_clear();
        command.envs(std::env::vars_os().filter(|(name, _)| instrumented(name)));
        let output = command.arg(asked).output().unwrap();
        assert!(output.status.success(), "{asked}: {output:?}");
        let out = String::from_utf8_lossy(&output.stdout);
        assert!(out.contains(said), "{asked}: {out}");
    }
}

/// Whether the variable `name` tells an instrumented build what to do,
/// and so is passed on to the watcher where nothing else is: where to
/// record coverage, or what the address sanitizer is to look for, as the
/// nightly stress run asks of a sanitized build.
fn instrumented(name: &std::ffi::OsStr) -> bool {
    name == "LLVM_PROFILE_FILE" || name == "ASAN_OPTIONS"
}
