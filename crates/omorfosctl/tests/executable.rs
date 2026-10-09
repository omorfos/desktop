//! The `omorfosctl` executable itself.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::process::Command;
use std::thread;

fn omorfosctl() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_omorfosctl"));
    // Nothing from the real session may leak in. Only the variable that
    // tells an instrumented build where to record coverage is passed on.
    command.env_clear();
    command.envs(std::env::vars_os().filter(|(name, _)| name == "LLVM_PROFILE_FILE"));
    command
}

#[test]
fn its_own_version_is_printed() {
    let output = omorfosctl().arg("--version").output().unwrap();
    assert!(output.status.success());
    let expected = format!("omorfosctl {}\n", env!("CARGO_PKG_VERSION"));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
}

#[test]
fn without_a_session_it_fails_with_an_explanation() {
    let output = omorfosctl().arg("state").output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("is omorfos running?")
    );
}

#[test]
fn the_socket_is_found_through_the_environment() {
    let directory = tempfile::TempDir::new().unwrap();
    let listener = UnixListener::bind(directory.path().join("omorfos-wayland-7.sock")).unwrap();
    let compositor = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut writer = stream.try_clone().unwrap();
        let mut request = String::new();
        BufReader::new(stream).read_line(&mut request).unwrap();
        writer
            .write_all(b"{\"version\":{\"protocol\":1,\"compositor\":\"9.9.9\"}}\n")
            .unwrap();
        request
    });

    let output = omorfosctl()
        .arg("version")
        .env("XDG_RUNTIME_DIR", directory.path())
        .env("WAYLAND_DISPLAY", "wayland-7")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "omorfos 9.9.9 (protocol 1)\n"
    );
    assert_eq!(compositor.join().unwrap(), "\"version\"\n");
}
