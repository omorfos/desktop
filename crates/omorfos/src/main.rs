//! The `omorfos` executable.

use std::process::ExitCode;
use std::{env, io};

fn main() -> ExitCode {
    let arguments = env::args_os().skip(1);
    omorfos::cli::run(
        arguments,
        &mut io::stdout(),
        &mut io::stderr(),
        omorfos::backend::winit::run,
    )
}
