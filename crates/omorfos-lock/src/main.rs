//! The `omorfos-lock` executable: the lock screen of the omorfos desktop,
//! and, as `omorfos-lock check SERVICE USER`, the helper it starts for each
//! way of unlocking.

use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;
use std::{env, fs};

use omorfos_ipc::help::{About, answered};
use omorfos_lock::check::{Checked, check};
use omorfos_lock::{ended, who};

/// What it says when asked what it is.
const ABOUT: About = About {
    name: "omorfos-lock",
    what: "The lock screen of the omorfos desktop, which asks PAM for each way of unlocking at \
           once.",
    usage: " [COMMAND]",
    commands: &[(
        "check SERVICE USER",
        "Ask PAM's SERVICE whether USER may unlock, as the helper the lock \
         screen starts for each way of unlocking; not for running by hand",
    )],
    options: &[],
    examples: &[],
    notes: "Without a command it locks the session at once. The `lock` action starts it so, \
            as `locker` in config.yaml names it, and so does the session before the machine \
            sleeps.",
    version: env!("CARGO_PKG_VERSION"),
};

fn main() -> ExitCode {
    let arguments: Vec<OsString> = env::args_os().skip(1).collect();
    if answered(&ABOUT, &arguments, &mut io::stdout()) {
        return ExitCode::SUCCESS;
    }
    let mut arguments = env::args_os().skip(1);
    let asked = arguments.next();
    if asked.as_deref() == Some("check".as_ref()) {
        let service = arguments.next().unwrap_or_default();
        let user = arguments.next().unwrap_or_default();
        let checked = check(&service, &user, io::stdin().lock(), io::stdout().lock());
        return match checked {
            Checked::Right => ExitCode::SUCCESS,
            Checked::Wrong | Checked::Unavailable => ExitCode::FAILURE,
        };
    }
    // The helpers are this program, started again, which the kernel always
    // knows the way to.
    let helper = PathBuf::from("/proc/self/exe");
    let passwd = fs::read_to_string("/etc/passwd").unwrap_or_default();
    let uid = rustix::process::getuid().as_raw();
    let home = env::var_os("HOME");
    let user = who(
        &passwd,
        uid,
        env::var_os("USER").as_deref(),
        home.as_deref(),
    );
    let started = omorfos_lock::start(
        (
            env::var_os("XDG_RUNTIME_DIR").as_deref(),
            env::var_os("WAYLAND_DISPLAY").as_deref(),
        ),
        (env::var_os("XDG_CONFIG_HOME").as_deref(), home.as_deref()),
        (helper, user),
    );
    ended(started, &mut io::stderr())
}
