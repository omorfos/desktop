//! `omorfos-relay` run as the holder runs it.

use std::os::fd::AsFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use omorfos_ipc::relay::{self, Hand};

const RELAY: &str = env!("CARGO_BIN_EXE_omorfos-relay");

#[test]
fn it_relays_what_the_holder_hands_it_until_the_holder_goes() {
    let (holder, relays) = relay::pair().unwrap();
    let mut started = Command::new(RELAY)
        .stdin(Stdio::from(relays))
        .spawn()
        .unwrap();
    let directory = tempfile::TempDir::new().unwrap();
    let path = directory.path().join("wayland-1");
    let socket = UnixListener::bind(&path).unwrap();
    let (towards, compositor) = relay::pair().unwrap();
    relay::send(holder.as_fd(), &Hand::Socket, &[socket.as_fd()]).unwrap();
    relay::send(holder.as_fd(), &Hand::Compositor, &[towards.as_fd()]).unwrap();
    drop((socket, towards));

    let _program = UnixStream::connect(&path).unwrap();
    let by = Instant::now() + Duration::from_secs(5);
    let handed = loop {
        if let Some(handed) = relay::receive(compositor.as_fd()).unwrap() {
            break handed;
        }
        assert!(Instant::now() < by, "the program was never handed on");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(matches!(handed.hand, Some(Hand::Program(_))));

    drop(holder);
    assert!(started.wait().unwrap().success());
}

#[test]
fn with_nothing_to_be_handed_by_it_ends_at_once() {
    let ended = Command::new(RELAY).stdin(Stdio::null()).status().unwrap();
    assert!(ended.success());
}

#[test]
fn it_says_what_it_is() {
    for asked in ["-V", "--version"] {
        let said = Command::new(RELAY).arg(asked).output().unwrap();
        let said = String::from_utf8(said.stdout).unwrap();
        assert_eq!(
            said,
            format!("omorfos-relay {}\n", env!("CARGO_PKG_VERSION"))
        );
    }
    for asked in ["-h", "--help"] {
        let said = Command::new(RELAY).arg(asked).output().unwrap();
        assert!(
            String::from_utf8(said.stdout)
                .unwrap()
                .starts_with("Stands between")
        );
    }
}

#[test]
fn it_refuses_what_it_does_not_know() {
    let refused = Command::new(RELAY).arg("--frobnicate").output().unwrap();
    assert_eq!(refused.status.code(), Some(2));
    let said = String::from_utf8(refused.stderr).unwrap();
    assert!(said.starts_with("omorfos-relay: unexpected argument `--frobnicate`"));
}
