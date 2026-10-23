//! The `omorfos-lock` executable: the lock screen of the omorfos desktop,
//! and, as `omorfos-lock check SERVICE USER`, the helper it starts for each
//! way of unlocking.

use std::io;
use std::path::PathBuf;
use std::process::ExitCode;
use std::{env, fs};

use omorfos_lock::check::{Checked, check};
use omorfos_lock::{ended, who};

fn main() -> ExitCode {
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
    let user = who(&passwd, uid, env::var_os("USER").as_deref());
    let started = omorfos_lock::start(
        (
            env::var_os("XDG_RUNTIME_DIR").as_deref(),
            env::var_os("WAYLAND_DISPLAY").as_deref(),
        ),
        (
            env::var_os("XDG_CONFIG_HOME").as_deref(),
            env::var_os("HOME").as_deref(),
        ),
        (helper, user),
    );
    ended(started, &mut io::stderr())
}
