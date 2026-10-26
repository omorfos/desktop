//! The `omorfos-hold` executable itself: copied, with a stand-in for the
//! compositor beside it, into a place of its own, as it is installed.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::fs;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// The holder at `program`, with nothing of the test's environment but
/// what tells an instrumented build where to record coverage, and the
/// runtime directory `runtime`.
fn holder(program: &Path, runtime: Option<&Path>) -> Command {
    let mut command = Command::new(program);
    command.env_clear();
    command.envs(std::env::vars_os().filter(|(name, _)| name == "LLVM_PROFILE_FILE"));
    command.envs(runtime.map(|runtime| ("XDG_RUNTIME_DIR", runtime)));
    command
}

/// The holder as it was built.
fn built() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_omorfos-hold"))
}

/// Puts `script` at `path`, as a new file in the place of any old one,
/// written by a process of its own: one written by this process could not
/// be run while a process another test is starting holds it open, as it
/// does for the moment before it becomes its own program.
fn install(path: &Path, script: &str) {
    let mut writer = Command::new("sh")
        .args([
            "-c",
            "cat > \"$1.new\" && chmod 755 \"$1.new\" && mv \"$1.new\" \"$1\"",
            "sh",
        ])
        .arg(path)
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = writer.stdin.take().unwrap();
    std::io::Write::write_all(&mut input, script.as_bytes()).unwrap();
    drop(input);
    assert!(writer.wait().unwrap().success());
}

/// A place with the holder installed in it, and where it is.
fn installed() -> (tempfile::TempDir, PathBuf) {
    let place = tempfile::TempDir::new().unwrap();
    let program = place.path().join("omorfos-hold");
    // Copied by a process of its own, for the reason `install` says.
    let copied = Command::new("cp").arg(built()).arg(&program).status();
    assert!(copied.unwrap().success());
    (place, program)
}

/// A stand-in for the compositor at `place`: counts its runs, writes what
/// it was told and its parent's number, and does `then`, knowing its run
/// as `$n`.
fn compositor(place: &Path, then: &str) {
    let marks = place.display();
    let script = format!(
        "#!/bin/sh\n\
         marks='{marks}'\n\
         n=$(cat \"$marks/runs\" 2>/dev/null || echo 0); n=$((n + 1)); echo $n > \"$marks/runs\"\n\
         told=''\n\
         while read -r line && [ -n \"$line\" ]; do told=\"$told$line,\"; done\n\
         printf '%s;' \"$told\" >> \"$marks/told\"\n\
         printf '%s;' \"$PPID\" >> \"$marks/parents\"\n\
         {then}\n"
    );
    install(&place.join("omorfos"), &script);
}

fn said(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

#[test]
fn its_version_is_printed_and_asked_to_check_it_says_the_same() {
    for flag in ["--version", "-V", "--check"] {
        let output = holder(&built(), None).arg(flag).output().unwrap();
        assert!(output.status.success());
        let expected = format!("omorfos-hold {}\n", env!("CARGO_PKG_VERSION"));
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }
}

#[test]
fn help_is_printed_and_anything_else_is_a_usage_error() {
    let output = holder(&built(), None).arg("--help").output().unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("Usage: omorfos-hold")
    );
    let output = holder(&built(), None).arg("--frobnicate").output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(said(&output).contains("unexpected argument `--frobnicate`"));
}

#[test]
fn without_a_runtime_directory_or_a_place_in_it_there_is_nothing_to_hold() {
    let output = holder(&built(), None).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(said(&output), "omorfos-hold: XDG_RUNTIME_DIR is not set\n");
    let nowhere = Path::new("/nonexistent/omorfos-hold");
    let output = holder(&built(), Some(nowhere)).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        said(&output).contains("cannot hold the session"),
        "{}",
        said(&output)
    );
}

#[test]
fn it_holds_the_session_for_the_compositor_beside_it_until_that_ends() {
    let (place, program) = installed();
    compositor(place.path(), "printf 'up\\n' >&0; exit 0");
    let output = holder(&program, Some(place.path())).output().unwrap();
    assert!(output.status.success(), "{}", said(&output));
    assert_eq!(fs::read_to_string(place.path().join("told")).unwrap(), ";");
    assert!(
        !place.path().join("wayland-1").exists(),
        "the name is given back"
    );
}

#[test]
fn a_session_whose_compositor_keeps_failing_fails() {
    let (place, program) = installed();
    compositor(place.path(), "exit 1");
    let output = holder(&program, Some(place.path())).output().unwrap();
    assert_eq!(output.status.code(), Some(1), "{}", said(&output));
    assert!(said(&output).contains("keeps failing"), "{}", said(&output));
}

#[test]
fn its_own_new_version_takes_its_place_between_one_compositor_and_the_next() {
    let (place, program) = installed();
    // The first compositor installs a new version of the holder as it
    // runs, as installing omorfos does, and steps down; the next comes up
    // under the new one, and ends the session.
    let then = format!(
        "if [ $n = 1 ]; then cp '{built}' '{program}.next' && mv '{program}.next' '{program}'; \
         printf 'up\\nlocked\\nstepping down\\n' >&0; exit 0; fi; printf 'up\\n' >&0; exit 0",
        built = built().display(),
        program = program.display(),
    );
    compositor(place.path(), &then);
    let output = holder(&program, Some(place.path())).output().unwrap();
    assert!(output.status.success(), "{}", said(&output));
    let told = fs::read_to_string(place.path().join("told")).unwrap();
    assert_eq!(told, ";again,locked,;", "{}", said(&output));
    // It is the same process, which the login screen watches, all along.
    let parents = fs::read_to_string(place.path().join("parents")).unwrap();
    let parents: Vec<&str> = parents.split_terminator(';').collect();
    assert_eq!(parents.len(), 2);
    assert_eq!(parents[0], parents[1]);
    assert!(
        !place.path().join("wayland-1").exists(),
        "the name is given back"
    );
}

/// Runs `program` as a version taking, or taking `back`, what is handed
/// to it as `text` and `fds`.
fn taking(program: &Path, flag: &str, text: &str, fds: &[std::os::fd::BorrowedFd<'_>]) -> Output {
    let (ours, theirs) = UnixStream::pair().unwrap();
    omorfos_ipc::hold::give(&ours, text, fds).unwrap();
    holder(program, None)
        .arg(flag)
        .stdin(OwnedFd::from(theirs))
        .output()
        .unwrap()
}

#[test]
fn a_version_that_cannot_take_what_it_is_handed_goes_back_to_the_one_before() {
    // Nothing to go back to.
    let output = taking(&built(), "--taking", "nonsense\n\n", &[]);
    assert_eq!(output.status.code(), Some(1));
    let first = said(&output);
    assert!(first.contains("going back to it"), "{first}");
    assert!(first.contains("the version before is not known"), "{first}");

    // The one before, given it back, cannot take it either.
    let (_place, old) = installed();
    let old = fs::File::open(old).unwrap();
    let output = taking(&built(), "--taking", "nonsense\n\n", &[old.as_fd()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        said(&output).contains("cannot take back"),
        "{}",
        said(&output)
    );
}

#[test]
fn a_version_handed_nothing_says_so() {
    let output = holder(&built(), None)
        .arg("--taking")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        said(&output).contains("nothing was handed over"),
        "{}",
        said(&output)
    );
}
