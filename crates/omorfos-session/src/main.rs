//! The `omorfos-session` executable: does what a key for the volume, the
//! brightness or what is playing asks.

use std::io::Write;
use std::process::ExitCode;
use std::{env, io};

use omorfos_session::{Places, Request};

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let done = Request::parse(&arguments).map_err(|unknown| (unknown.into(), ExitCode::from(2)));
    let places = Places::from(&|name| env::var_os(name));
    let done = done.and_then(|request| {
        omorfos_session::run(request, &places).map_err(|failure| (failure, ExitCode::FAILURE))
    });
    match done {
        Ok(()) => ExitCode::SUCCESS,
        Err((failure, code)) => {
            // If even that cannot be said, there is nobody to say it to.
            let _ = writeln!(io::stderr(), "omorfos-session: {failure}");
            code
        }
    }
}
