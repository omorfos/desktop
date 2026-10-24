//! The `omorfos-brand` executable: `omorfos-brand OUT` draws the pictures
//! of the brand and writes them under `OUT` as they are installed under
//! `share`. The Makefile runs it as omorfos is built.

use std::path::Path;
use std::process::ExitCode;
use std::{env, io};

fn main() -> ExitCode {
    let out = env::args_os().nth(1);
    let pictures = omorfos_brand::everything();
    let written = out.map(|out| omorfos_brand::write(Path::new(&out), &pictures));
    omorfos_brand::ended(written, &mut io::stderr())
}
