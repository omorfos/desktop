//! The command line.

use std::error::Error;
use std::ffi::OsString;
use std::io::Write;
use std::process::ExitCode;

const USAGE: &str = "\
A lightweight Wayland compositor.

Usage: omorfos [OPTIONS]

Options:
  -h, --help     Print this help
  -V, --version  Print the version
      --held     Take the socket from omorfos-hold, which started it: only
                 omorfos-hold runs omorfos so

Configuration is read from $XDG_CONFIG_HOME/omorfos/ (usually
~/.config/omorfos/) and reloaded when it changes.
";

/// How the compositor was started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Start {
    /// By the login screen or by hand, with nothing above it.
    Alone,
    /// By `omorfos-hold`, which hands it the socket and starts it again
    /// when it ends (ADR 60).
    Held,
}

/// Runs the program: handles the arguments and, unless one of them asks for
/// something else, starts the compositor by calling `launch` with how it
/// was started.
///
/// `launch` is passed in so that this function decides what happens without
/// depending on a display. Output goes to `out` and problems to `err`;
/// failures to write to either are ignored, as there is nowhere to report
/// them.
pub fn run<A, L>(arguments: A, out: &mut dyn Write, err: &mut dyn Write, launch: L) -> ExitCode
where
    A: IntoIterator<Item = OsString>,
    L: FnOnce(Start) -> Result<(), Box<dyn Error>>,
{
    let arguments: Vec<OsString> = arguments.into_iter().collect();
    let arguments: Vec<&str> = arguments
        .iter()
        .map(|a| a.to_str().unwrap_or("\u{fffd}"))
        .collect();
    let start = match arguments.as_slice() {
        [] => Start::Alone,
        ["--held"] => Start::Held,
        ["-h" | "--help"] => {
            let _ = out.write_all(USAGE.as_bytes());
            return ExitCode::SUCCESS;
        }
        ["-V" | "--version"] => {
            let _ = writeln!(out, "omorfos {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        [unexpected, ..] => {
            let _ = writeln!(
                err,
                "omorfos: unexpected argument `{unexpected}`\n\n{USAGE}"
            );
            return ExitCode::from(2);
        }
    };
    match launch(start) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(err, "omorfos: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs with `arguments`; returns the exit code, stdout, stderr and
    /// how the compositor was launched, if it was.
    fn invoke(
        arguments: &[&str],
        launch: Result<(), &'static str>,
    ) -> (ExitCode, String, String, Option<Start>) {
        invoke_raw(arguments.iter().map(OsString::from).collect(), launch)
    }

    fn invoke_raw(
        arguments: Vec<OsString>,
        launch: Result<(), &'static str>,
    ) -> (ExitCode, String, String, Option<Start>) {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let mut launched = None;
        let code = run(arguments, &mut out, &mut err, |start| {
            launched = Some(start);
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
        assert_eq!(launched, Some(Start::Alone));
        assert_eq!(
            (code, out.as_str(), err.as_str()),
            (ExitCode::SUCCESS, "", "")
        );
    }

    #[test]
    fn held_the_compositor_is_launched_to_take_the_socket_from_its_holder() {
        let (code, _, _, launched) = invoke(&["--held"], Ok(()));
        assert_eq!((code, launched), (ExitCode::SUCCESS, Some(Start::Held)));
        let (code, _, _, launched) = invoke(&["--held", "again"], Ok(()));
        assert_eq!((code, launched), (ExitCode::from(2), None));
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
            assert_eq!(launched, None);
            assert_eq!(code, ExitCode::SUCCESS);
            assert_eq!(out, format!("omorfos {}\n", env!("CARGO_PKG_VERSION")));
            assert_eq!(err, "");
        }
    }

    #[test]
    fn help_prints_usage_without_launching() {
        for flag in ["-h", "--help"] {
            let (code, out, err, launched) = invoke(&[flag], Ok(()));
            assert_eq!(launched, None);
            assert_eq!(code, ExitCode::SUCCESS);
            assert!(out.contains("Usage: omorfos [OPTIONS]"), "{out}");
            assert_eq!(err, "");
        }
    }

    #[test]
    fn an_unexpected_argument_is_a_usage_error() {
        for arguments in [&["--frobnicate"][..], &["--version", "extra"], &["run"]] {
            let (code, out, err, launched) = invoke(arguments, Ok(()));
            assert_eq!(launched, None);
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
        assert_eq!(launched, None);
        assert_eq!(code, ExitCode::from(2));
    }
}
