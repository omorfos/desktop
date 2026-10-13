//! The `omorfos-screenshot` executable: takes a picture of every screen.

use std::process::ExitCode;
use std::{env, io};

fn main() -> ExitCode {
    let variable = |name: &str| env::var_os(name);
    let failed = omorfos_screenshot::run(&variable, &mut io::stdout(), &mut io::stderr());
    ExitCode::from(u8::from(failed))
}
