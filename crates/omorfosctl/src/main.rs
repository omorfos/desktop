//! The `omorfosctl` executable: command-line control for omorfos.

mod cli;
mod report;

use std::process::ExitCode;
use std::{env, io};

fn main() -> ExitCode {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    let socket = omorfos_ipc::socket_path_from_env();
    let places = report::Places::of(
        env::var_os("XDG_CONFIG_HOME").as_deref(),
        env::var_os("XDG_STATE_HOME").as_deref(),
        env::var_os("HOME").as_deref(),
    );
    cli::run(
        &arguments,
        socket,
        &places,
        &mut io::stdout(),
        &mut io::stderr(),
    )
}
