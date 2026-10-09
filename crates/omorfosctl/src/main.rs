//! The `omorfosctl` executable: command-line control for omorfos.

mod cli;

use std::process::ExitCode;
use std::{env, io};

fn main() -> ExitCode {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    let socket = omorfos_ipc::socket_path_from_env();
    cli::run(&arguments, socket, &mut io::stdout(), &mut io::stderr())
}
