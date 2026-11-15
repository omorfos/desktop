//! The `omorfosctl` executable: command-line control for omorfos.

mod cli;
mod report;

use std::os::unix::fs::MetadataExt;
use std::process::ExitCode;
use std::{env, fs, io};

fn main() -> ExitCode {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    let socket = omorfos_ipc::socket_path_from_env();
    let mut places = report::Places::of(
        env::var_os("XDG_CONFIG_HOME").as_deref(),
        env::var_os("XDG_STATE_HOME").as_deref(),
        env::var_os("HOME").as_deref(),
    )
    .named(
        env::var_os("USER").as_deref(),
        env::var_os("LOGNAME").as_deref(),
    );
    // The process's own directory is the user's, whose number it has.
    let user = fs::metadata("/proc/self").ok().map(|own| own.uid());
    let session = env::var_os("XDG_SESSION_ID");
    places.whose = report::Whose::of(session.as_deref(), user);
    cli::run(
        &arguments,
        socket,
        &places,
        &mut io::stdout(),
        &mut io::stderr(),
    )
}
