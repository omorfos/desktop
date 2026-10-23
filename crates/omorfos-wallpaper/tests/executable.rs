//! The `omorfos-wallpaper` executable itself.

use std::process::Command;

fn wallpaper() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_omorfos-wallpaper"));
    // Nothing from the real session may leak in. Only the variable that
    // tells an instrumented build where to record coverage is passed on.
    command.env_clear();
    command.envs(std::env::vars_os().filter(|(name, _)| name == "LLVM_PROFILE_FILE"));
    command
}

#[test]
fn outside_a_session_it_fails_with_an_explanation() {
    let output = wallpaper().output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let said = String::from_utf8(output.stderr).unwrap();
    assert!(
        said.starts_with("omorfos-wallpaper: this is not an omorfos session"),
        "{said}"
    );
}

#[test]
fn a_connection_it_is_handed_is_the_one_it_uses() {
    use std::io::Read;
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    // The compositor starts its wallpaper with the connection already
    // made, as what it reads from, and says so. There is no socket of the
    // name given, so anything said to us came over what was handed.
    let directory = tempfile::TempDir::new().unwrap();
    let (mut ours, theirs) = UnixStream::pair().unwrap();
    let mut started = wallpaper()
        .env("XDG_RUNTIME_DIR", directory.path())
        .env("WAYLAND_DISPLAY", "wayland-none")
        .env("WAYLAND_SOCKET", "0")
        .env("HOME", directory.path())
        .stdin(OwnedFd::from(theirs))
        .spawn()
        .unwrap();

    ours.set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    // A read with a time limit is interrupted, not resumed, when the test
    // is stopped and continued, as a busy machine can do to it, and
    // `read_exact` reads again.
    let mut said = [0_u8; 1];
    let heard = ours.read_exact(&mut said);
    assert!(heard.is_ok(), "{heard:?}");

    // With the connection gone it has nothing to go on for, and stops of
    // its own accord.
    drop(ours);
    let stopped = (0..3000).find_map(|_| {
        std::thread::sleep(Duration::from_millis(10));
        started.try_wait().unwrap()
    });
    if stopped.is_none() {
        started.kill().unwrap();
    }
    assert_eq!(stopped.and_then(|status| status.code()), Some(1));
}
