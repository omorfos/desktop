//! The `omorfos` executable.

use std::process::ExitCode;
use std::{env, io};

use omorfos::backend::Backend;

fn main() -> ExitCode {
    // As many descriptors as may be had: one is kept for each pool of
    // memory a program shares (ADR 93).
    omorfos_ipc::limits::raise();
    let arguments = env::args_os().skip(1);
    let backend = Backend::choose(
        env::var_os("WAYLAND_DISPLAY").as_deref(),
        env::var_os("DISPLAY").as_deref(),
    );
    omorfos::cli::run(
        arguments,
        &mut io::stdout(),
        &mut io::stderr(),
        backend.launcher(),
    )
}
