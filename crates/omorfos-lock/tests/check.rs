//! The helper, as the lock screen starts it, asking a real PAM: one loaded
//! by `pam_wrapper` from services of the test's own, with `pam_matrix`
//! holding a made-up user and `pam_chatty` saying what modules say.
#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test support code fails the test on any error"
)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Where `pam_wrapper`'s modules are, which differs from one system to
/// another.
fn modules() -> PathBuf {
    let places = [
        "/usr/lib/x86_64-linux-gnu",
        "/usr/lib/aarch64-linux-gnu",
        "/usr/lib64",
        "/usr/lib",
    ];
    let found = places
        .iter()
        .map(|place| Path::new(place).join("pam_wrapper"))
        .find(|place| place.join("pam_matrix.so").exists());
    found.expect("pam_wrapper's modules, from libpam-wrapper")
}

/// PAM services of a test's own: `password`, which knows `someone` by
/// `secret`; `chatty`, which says something and something gone wrong
/// before it asks the same; and `who`, which asks who the user is first,
/// as Linux-PAM's own modules do of a user not named.
fn services() -> tempfile::TempDir {
    let directory = tempfile::TempDir::new().unwrap();
    let modules = modules();
    let passdb = directory.path().join("passdb");
    let known = ["password", "chatty", "who"].map(|service| format!("someone:secret:{service}\n"));
    std::fs::write(&passdb, known.concat()).unwrap();
    let matrix = format!(
        "auth required {} passdb={}\n",
        modules.join("pam_matrix.so").display(),
        passdb.display()
    );
    let chatty = format!(
        "auth optional {} info error num_lines=1\n",
        modules.join("pam_chatty.so").display()
    );
    std::fs::write(directory.path().join("password"), &matrix).unwrap();
    std::fs::write(directory.path().join("chatty"), chatty + &matrix).unwrap();
    let who = "auth requisite pam_succeed_if.so user = someone\n".to_owned();
    std::fs::write(directory.path().join("who"), who + &matrix).unwrap();
    directory
}

/// What the helper says when it asks `service` about `user`, given the
/// lines `typed`, and whether it said the user is who they say.
fn check(services: &Path, service: &str, user: Option<&str>, typed: &str) -> (String, bool) {
    let mut helper = Command::new(env!("CARGO_BIN_EXE_omorfos-lock"));
    helper.arg("check").arg(service).args(user);
    let mut child = helper
        .env("LD_PRELOAD", "libpam_wrapper.so")
        .env("PAM_WRAPPER", "1")
        .env("PAM_WRAPPER_SERVICE_DIR", services)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(typed.as_bytes())
        .unwrap();
    let ended = child.wait_with_output().unwrap();
    (
        String::from_utf8(ended.stdout).unwrap(),
        ended.status.success(),
    )
}

#[test]
fn the_right_secret_is_said_to_be_right_and_a_wrong_one_wrong() {
    let services = services();
    let (said, right) = check(services.path(), "password", Some("someone"), "secret\n");
    assert_eq!(said, "PAM_PROMPT_ECHO_OFF Password: \nSUCCESS\n");
    assert!(right);
    let (said, right) = check(services.path(), "password", Some("someone"), "guess\n");
    assert_eq!(said, "PAM_PROMPT_ECHO_OFF Password: \nFAILURE\n");
    assert!(!right);
    // Nor is what follows a NUL handed to PAM, which it cannot be.
    let (_, right) = check(
        services.path(),
        "password",
        Some("someone"),
        "secret\0more\n",
    );
    assert!(right, "cut at the NUL");
}

#[test]
fn what_modules_say_is_passed_on_without_ending_the_try() {
    let services = services();
    let (said, right) = check(services.path(), "chatty", Some("someone"), "secret\n");
    // What it says, in the order it says it, as often as it does.
    let mut lines: Vec<&str> = said.lines().collect();
    lines.dedup();
    assert_eq!(
        lines,
        [
            "PAM_TEXT_INFO Authentication succeeded",
            "PAM_ERROR_MSG Authentication generated an error",
            "PAM_PROMPT_ECHO_OFF Password: ",
            "SUCCESS",
        ]
    );
    assert!(right);
}

#[test]
fn with_no_user_named_pam_asks_who_it_is() {
    let services = services();
    let (said, right) = check(services.path(), "who", None, "someone\nsecret\n");
    let asked = "PAM_PROMPT_ECHO_ON login:\nPAM_PROMPT_ECHO_OFF Password: \nSUCCESS\n";
    assert_eq!(said, asked);
    assert!(right);
}

#[test]
fn a_way_that_is_not_set_up_says_it_cannot_be_asked() {
    let services = services();
    let (said, right) = check(services.path(), "absent", Some("someone"), "");
    assert_eq!(said, "UNAVAILABLE\n");
    assert!(!right);
    // Nor is one whose questions go unanswered right.
    let (said, right) = check(services.path(), "password", Some("someone"), "");
    assert!(said.starts_with("PAM_PROMPT_ECHO_OFF"), "{said}");
    assert!(!right);
}

#[test]
fn the_lock_screen_says_why_when_there_is_no_session() {
    // Only the session's variables are taken away: those that say where
    // coverage is written go on being there.
    let mut lock = Command::new(env!("CARGO_BIN_EXE_omorfos-lock"));
    let session = [
        "XDG_RUNTIME_DIR",
        "WAYLAND_DISPLAY",
        "WAYLAND_SOCKET",
        "HOME",
        "XDG_CONFIG_HOME",
    ];
    for variable in session {
        lock.env_remove(variable);
    }
    let ended = lock.output().unwrap();
    assert!(!ended.status.success());
    let said = String::from_utf8(ended.stderr).unwrap();
    assert!(said.contains("not an omorfos session"), "{said}");
}
