//! The command line.

use std::error::Error;
use std::ffi::OsString;
use std::io::Write;
use std::process::ExitCode;

const USAGE: &str = "\
A lightweight Wayland compositor inspired by Fluxbox.

Usage: omorfos [OPTIONS]

Options:
  -h, --help     Print this help
  -V, --version  Print the version

Configuration is read from $XDG_CONFIG_HOME/omorfos/ (usually
~/.config/omorfos/) and reloaded when it changes.
";

/// Runs the program: handles the arguments and, unless one of them asks for
/// something else, starts the compositor by calling `launch`.
///
/// `launch` is passed in so that this function decides what happens without
/// depending on a display. Output goes to `out` and problems to `err`;
/// failures to write to either are ignored, as there is nowhere to report
/// them.
pub fn run<A, L>(arguments: A, out: &mut dyn Write, err: &mut dyn Write, launch: L) -> ExitCode
where
    A: IntoIterator<Item = OsString>,
    L: FnOnce() -> Result<(), Box<dyn Error>>,
{
    let arguments: Vec<OsString> = arguments.into_iter().collect();
    let arguments: Vec<&str> = arguments
        .iter()
        .map(|a| a.to_str().unwrap_or("\u{fffd}"))
        .collect();
    match arguments.as_slice() {
        [] => match launch() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                let _ = writeln!(err, "omorfos: {error}");
                ExitCode::FAILURE
            }
        },
        ["-h" | "--help"] => {
            let _ = out.write_all(USAGE.as_bytes());
            ExitCode::SUCCESS
        }
        ["-V" | "--version"] => {
            let _ = writeln!(out, "omorfos {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        [unexpected, ..] => {
            let _ = writeln!(
                err,
                "omorfos: unexpected argument `{unexpected}`\n\n{USAGE}"
            );
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs with `arguments`; returns the exit code, stdout, stderr and
    /// whether the compositor was launched.
    fn invoke(
        arguments: &[&str],
        launch: Result<(), &'static str>,
    ) -> (ExitCode, String, String, bool) {
        invoke_raw(arguments.iter().map(OsString::from).collect(), launch)
    }

    fn invoke_raw(
        arguments: Vec<OsString>,
        launch: Result<(), &'static str>,
    ) -> (ExitCode, String, String, bool) {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let mut launched = false;
        let code = run(arguments, &mut out, &mut err, || {
            launched = true;
            launch.map_err(Box::from)
        });
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
            launched,
        )
    }

    #[test]
    fn without_arguments_the_compositor_is_launched() {
        let (code, out, err, launched) = invoke(&[], Ok(()));
        assert!(launched);
        assert_eq!(
            (code, out.as_str(), err.as_str()),
            (ExitCode::SUCCESS, "", "")
        );
    }

    #[test]
    fn a_launch_failure_is_reported_and_fails_the_process() {
        let (code, out, err, _) = invoke(&[], Err("no display found"));
        assert_eq!((code, out.as_str()), (ExitCode::FAILURE, ""));
        assert_eq!(err, "omorfos: no display found\n");
    }

    #[test]
    fn version_prints_the_version_without_launching() {
        for flag in ["-V", "--version"] {
            let (code, out, err, launched) = invoke(&[flag], Ok(()));
            assert!(!launched);
            assert_eq!(code, ExitCode::SUCCESS);
            assert_eq!(out, format!("omorfos {}\n", env!("CARGO_PKG_VERSION")));
            assert_eq!(err, "");
        }
    }

    #[test]
    fn help_prints_usage_without_launching() {
        for flag in ["-h", "--help"] {
            let (code, out, err, launched) = invoke(&[flag], Ok(()));
            assert!(!launched);
            assert_eq!(code, ExitCode::SUCCESS);
            assert!(out.contains("Usage: omorfos [OPTIONS]"), "{out}");
            assert_eq!(err, "");
        }
    }

    #[test]
    fn an_unexpected_argument_is_a_usage_error() {
        for arguments in [&["--frobnicate"][..], &["--version", "extra"], &["run"]] {
            let (code, out, err, launched) = invoke(arguments, Ok(()));
            assert!(!launched);
            assert_eq!((code, out.as_str()), (ExitCode::from(2), ""));
            assert!(err.contains("Usage: omorfos"), "{err}");
        }
        let (_, _, err, _) = invoke(&["--frobnicate"], Ok(()));
        assert!(
            err.starts_with("omorfos: unexpected argument `--frobnicate`\n"),
            "{err}"
        );
    }

    #[test]
    fn an_argument_that_is_not_unicode_is_a_usage_error() {
        use std::os::unix::ffi::OsStringExt;
        let argument = OsString::from_vec(vec![0xff]);
        let (code, _, _, launched) = invoke_raw(vec![argument], Ok(()));
        assert!(!launched);
        assert_eq!(code, ExitCode::from(2));
    }
}
