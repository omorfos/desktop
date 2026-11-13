//! The `omorfos-screenshot` executable: takes a picture of every screen.

use std::ffi::OsString;
use std::process::ExitCode;
use std::{env, io};

use omorfos_ipc::help::answered;

fn main() -> ExitCode {
    let arguments: Vec<OsString> = env::args_os().skip(1).collect();
    if answered(&omorfos_screenshot::ABOUT, &arguments, &mut io::stdout()) {
        return ExitCode::SUCCESS;
    }
    let variable = |name: &str| env::var_os(name);
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    let (mut out, mut problems) = (io::stdout(), io::stderr());
    let failed = omorfos_screenshot::run(&variable, &arguments, &mut out, &mut problems);
    ExitCode::from(u8::from(failed))
}
