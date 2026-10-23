//! Parsing the command line and carrying it out.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use omorfos_ipc::{Client, Reply, Request, State};
use serde_json::{Value, json};

const USAGE: &str = "\
Control a running omorfos compositor.

Usage: omorfosctl [--socket PATH] <COMMAND>

Commands:
  version               Print the compositor's version
  state                 Print workspaces, windows and problems as JSON
  screens               Print the screens, where they stand and their modes
  watch                 Print the state, one JSON line each time it changes
  action NAME [ARG]...  Perform an action, as named in keys.yaml
  activate ID           Switch to a window, by the id shown in `state`
  config check          Print configuration problems; fails if there are any
  config reload         Re-read the configuration, then check it

Options:
      --socket PATH     The compositor's socket [default: $OMORFOS_SOCKET, or
                        omorfos-$WAYLAND_DISPLAY.sock in $XDG_RUNTIME_DIR]
  -h, --help            Print this help
  -V, --version         Print omorfosctl's own version

Examples:
  omorfosctl action close
  omorfosctl action workspace 2
  omorfosctl action spawn foot --title scratch
";

/// What the user asked for.
#[derive(Debug, Clone, PartialEq)]
enum Command {
    Help,
    OwnVersion,
    Version,
    State,
    Screens,
    Watch,
    Perform(Value),
    Activate(u64),
    Check,
    Reload,
}

/// Reads the arguments: an optional `--socket PATH`, then a command.
fn parse(arguments: &[String]) -> Result<(Option<PathBuf>, Command), String> {
    let words: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let (socket, words) = match words.as_slice() {
        ["--socket", path, rest @ ..] => (Some(PathBuf::from(path)), rest),
        rest => (None, rest),
    };
    let command = match words {
        ["-h" | "--help"] => Command::Help,
        ["-V" | "--version"] => Command::OwnVersion,
        ["version"] => Command::Version,
        ["state"] => Command::State,
        ["screens"] => Command::Screens,
        ["watch"] => Command::Watch,
        ["config", "check"] => Command::Check,
        ["config", "reload"] => Command::Reload,
        ["activate", id] => {
            let id = id
                .parse()
                .map_err(|_| format!("`{id}` is not a window id"))?;
            Command::Activate(id)
        }
        ["action", name, arguments @ ..] => Command::Perform(action(name, arguments)),
        [] => return Err("no command given".to_owned()),
        [first, ..] => return Err(format!("unrecognised command `{first}`")),
    };
    Ok((socket, command))
}

/// Builds an action in the form `keys.yaml` uses, from words.
///
/// A name alone is the action itself (`close`). With one whole number it is
/// that action applied to the number (`workspace 2`). With anything else the
/// words are a list (`spawn foot --title scratch`).
fn action(name: &str, arguments: &[&str]) -> Value {
    match arguments {
        [] => json!(name),
        [number] if number.parse::<u64>().is_ok() => {
            json!({ name: number.parse::<u64>().unwrap_or_default() })
        }
        words => json!({ name: words }),
    }
}

/// Runs the program: output goes to `out`, problems to `err`.
///
/// `socket` is where the environment says the compositor's socket is; a
/// `--socket` argument overrides it.
pub(crate) fn run(
    arguments: &[String],
    socket: Option<PathBuf>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> ExitCode {
    let (chosen, command) = match parse(arguments) {
        Ok(parsed) => parsed,
        Err(problem) => {
            let _ = writeln!(err, "omorfosctl: {problem}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match command {
        Command::Help => {
            let _ = out.write_all(USAGE.as_bytes());
            return ExitCode::SUCCESS;
        }
        Command::OwnVersion => {
            let _ = writeln!(out, "omorfosctl {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        _ => {}
    }
    let connected = chosen
        .or(socket)
        .ok_or_else(|| {
            "cannot tell where the compositor's socket is; is omorfos running? \
             Set OMORFOS_SOCKET or pass --socket"
                .to_owned()
        })
        .and_then(|path| {
            Client::connect(&path)
                .map_err(|error| format!("cannot connect to {}: {error}", path.display()))
        });
    match connected.and_then(|mut client| execute(&command, &mut client, out)) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(problem) => {
            let _ = writeln!(err, "omorfosctl: {problem}");
            ExitCode::FAILURE
        }
    }
}

/// Carries out a command that needs the compositor. Returns whether all is
/// well, which for `config check` means there were no problems.
fn execute(command: &Command, client: &mut Client, out: &mut dyn Write) -> Result<bool, String> {
    let request = match command {
        Command::Version => Request::Version,
        Command::Watch => Request::Subscribe,
        Command::Perform(action) => Request::Perform {
            action: action.clone(),
        },
        Command::Activate(window) => Request::Activate { window: *window },
        Command::Reload => Request::Reload,
        _ => Request::State,
    };
    let reply = client
        .request(&request)
        .map_err(|error| error.to_string())?;
    match (command, reply) {
        (_, Reply::Error { message }) => Err(message),
        (
            Command::Version,
            Reply::Version {
                protocol,
                compositor,
            },
        ) => {
            let _ = writeln!(out, "omorfos {compositor} (protocol {protocol})");
            Ok(true)
        }
        (Command::State, Reply::State(state)) => {
            let _ = writeln!(
                out,
                "{}",
                serde_json::to_string_pretty(&state).unwrap_or_default()
            );
            Ok(true)
        }
        (Command::Check, Reply::State(state)) => Ok(report(&state, out)),
        (Command::Screens, Reply::State(state)) => {
            screens(&state, out);
            Ok(true)
        }
        (Command::Watch, Reply::State(first)) => {
            let mut next = Ok(Reply::State(first));
            // Until the compositor goes away, or nobody is reading.
            while let Ok(Reply::State(state)) = next {
                if writeln!(out, "{}", serde_json::to_string(&state).unwrap_or_default()).is_err() {
                    break;
                }
                next = client.receive();
            }
            Ok(true)
        }
        (Command::Reload, Reply::Done) => {
            // Show what the reload made of the configuration.
            match client
                .request(&Request::State)
                .map_err(|error| error.to_string())?
            {
                Reply::State(state) => Ok(report(&state, out)),
                _ => Err("the compositor sent an unexpected answer".to_owned()),
            }
        }
        (Command::Perform(_) | Command::Activate(_), Reply::Done) => Ok(true),
        _ => Err("the compositor sent an unexpected answer".to_owned()),
    }
}

/// Prints each screen: where it stands, how large it is there and the mode
/// it is driven in, and then each mode it can be driven in, as
/// `config.yaml` writes one.
fn screens(state: &State, out: &mut dyn Write) {
    for screen in &state.screens {
        let driven = if screen.mode.is_empty() {
            String::new()
        } else {
            format!(", driven at {}", screen.mode)
        };
        let mirrors = if screen.mirrors.is_empty() {
            String::new()
        } else {
            format!(", mirroring {}", screen.mirrors)
        };
        let _ = writeln!(
            out,
            "{}: {} by {} units at {},{}{mirrors}{driven}",
            screen.name, screen.width, screen.height, screen.x, screen.y
        );
        for mode in &screen.modes {
            let best = if *mode == screen.preferred {
                ", which suits it best"
            } else {
                ""
            };
            let _ = writeln!(out, "  {mode}{best}");
        }
    }
}

/// Prints the configuration's problems, and returns whether there are none.
fn report(state: &State, out: &mut dyn Write) -> bool {
    for problem in &state.problems {
        let _ = writeln!(out, "{problem}");
    }
    state.problems.is_empty()
}

#[cfg(test)]
mod tests {
    use std::io::{self, BufRead, BufReader};
    use std::os::unix::net::UnixListener;
    use std::thread::{self, JoinHandle};

    use omorfos_ipc::{Window, Workspace, decode, encode};
    use tempfile::TempDir;

    use super::*;

    /// A stand-in compositor: answers each request with the next of
    /// `replies`, then hangs up. Returns the requests it was sent.
    struct Fake {
        _directory: TempDir,
        socket: PathBuf,
        served: JoinHandle<Vec<Request>>,
    }

    /// `extra` is how many of the replies are sent unprompted after the
    /// last request has been answered, as updates to a subscriber.
    fn fake(replies: Vec<Reply>, extra: usize) -> Fake {
        let directory = TempDir::new().unwrap();
        let socket = directory.path().join("omorfos.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let served = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut writer = stream.try_clone().unwrap();
            let mut lines = BufReader::new(stream).lines();
            let mut requests = Vec::new();
            let answered = replies.len() - extra;
            for (index, reply) in replies.iter().enumerate() {
                if index < answered {
                    let line = lines.next().unwrap().unwrap();
                    requests.push(decode::<Request>(&line).unwrap());
                }
                // The other end may have gone; that is its business.
                let _ = writer.write_all(encode(reply).as_bytes());
            }
            requests
        });
        Fake {
            _directory: directory,
            socket,
            served,
        }
    }

    struct Outcome {
        code: ExitCode,
        out: String,
        err: String,
        requests: Vec<Request>,
    }

    fn invoke(arguments: &[&str], replies: Vec<Reply>) -> Outcome {
        invoke_with_updates(arguments, replies, 0)
    }

    fn invoke_with_updates(arguments: &[&str], replies: Vec<Reply>, extra: usize) -> Outcome {
        let fake = fake(replies, extra);
        let arguments: Vec<String> = arguments.iter().map(ToString::to_string).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(&arguments, Some(fake.socket.clone()), &mut out, &mut err);
        // Connect once more in case the command never did, so the fake
        // compositor stops waiting.
        drop(Client::connect(&fake.socket));
        let requests = fake.served.join().unwrap();
        Outcome {
            code,
            out: String::from_utf8(out).unwrap(),
            err: String::from_utf8(err).unwrap(),
            requests,
        }
    }

    fn offline(arguments: &[&str]) -> (ExitCode, String, String) {
        let arguments: Vec<String> = arguments.iter().map(ToString::to_string).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(&arguments, None, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    fn state(problems: &[&str]) -> State {
        State {
            workspaces: vec![Workspace {
                output: "eDP-1".to_owned(),
                number: 1,
                name: "web".to_owned(),
                active: true,
                id: 0,
                kept: true,
                windows: Vec::new(),
                room: false,
            }],
            windows: vec![Window {
                id: 7,
                app_id: "foot".to_owned(),
                title: "~".to_owned(),
                output: "eDP-1".to_owned(),
                workspace: 1,
                focused: true,
                shown: true,
                minimized: false,
                standing: omorfos_ipc::Standing::default(),
                handle: String::new(),
            }],
            problems: problems.iter().map(ToString::to_string).collect(),
            events: Vec::new(),
            screens: Vec::new(),
            locked: false,
        }
    }

    #[test]
    fn help_and_its_own_version_need_no_compositor() {
        for flag in ["-h", "--help"] {
            let (code, out, err) = offline(&[flag]);
            assert_eq!((code, err.as_str()), (ExitCode::SUCCESS, ""));
            assert!(
                out.starts_with("Control a running omorfos compositor."),
                "{out}"
            );
        }
        for flag in ["-V", "--version"] {
            let (code, out, _) = offline(&[flag]);
            assert_eq!(code, ExitCode::SUCCESS);
            assert_eq!(out, format!("omorfosctl {}\n", env!("CARGO_PKG_VERSION")));
        }
    }

    #[test]
    fn a_missing_or_unknown_command_is_a_usage_error() {
        let (code, out, err) = offline(&[]);
        assert_eq!((code, out.as_str()), (ExitCode::from(2), ""));
        assert!(
            err.starts_with("omorfosctl: no command given\n\nControl a running"),
            "{err}"
        );

        for arguments in [
            &["frobnicate"][..],
            &["config"],
            &["config", "explode"],
            &["state", "extra"],
        ] {
            let (code, _, err) = offline(arguments);
            assert_eq!(code, ExitCode::from(2), "{arguments:?}");
            assert!(err.contains("unrecognised command"), "{err}");
        }
        let (code, _, err) = offline(&["activate", "seven"]);
        assert_eq!(code, ExitCode::from(2));
        assert!(
            err.starts_with("omorfosctl: `seven` is not a window id"),
            "{err}"
        );
    }

    #[test]
    fn without_a_socket_to_try_the_user_is_told_how_to_name_one() {
        let (code, out, err) = offline(&["state"]);
        assert_eq!((code, out.as_str()), (ExitCode::FAILURE, ""));
        assert!(err.contains("is omorfos running?"), "{err}");
    }

    #[test]
    fn a_socket_nobody_listens_on_is_reported() {
        let directory = TempDir::new().unwrap();
        let socket = directory.path().join("absent.sock");
        let arguments = [
            "--socket".to_owned(),
            socket.display().to_string(),
            "state".to_owned(),
        ];
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(&arguments, None, &mut out, &mut err);
        assert_eq!(code, ExitCode::FAILURE);
        let err = String::from_utf8(err).unwrap();
        assert!(
            err.starts_with(&format!(
                "omorfosctl: cannot connect to {}: ",
                socket.display()
            )),
            "{err}"
        );
    }

    #[test]
    fn the_socket_option_overrides_the_environment() {
        let fake = fake(vec![Reply::Done], 0);
        let arguments = [
            "--socket".to_owned(),
            fake.socket.display().to_string(),
            "action".to_owned(),
            "close".to_owned(),
        ];
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let elsewhere = Some(PathBuf::from("/nonexistent/omorfos.sock"));
        assert_eq!(
            run(&arguments, elsewhere, &mut out, &mut err),
            ExitCode::SUCCESS
        );
        assert_eq!(
            fake.served.join().unwrap(),
            [Request::Perform {
                action: json!("close")
            }]
        );
    }

    #[test]
    fn version_prints_what_the_compositor_reports() {
        let reply = Reply::Version {
            protocol: 1,
            compositor: "0.4.2".to_owned(),
        };
        let outcome = invoke(&["version"], vec![reply]);
        assert_eq!(outcome.code, ExitCode::SUCCESS);
        assert_eq!(outcome.out, "omorfos 0.4.2 (protocol 1)\n");
        assert_eq!(outcome.requests, [Request::Version]);
    }

    #[test]
    fn state_prints_readable_json() {
        let outcome = invoke(&["state"], vec![Reply::State(state(&[]))]);
        assert_eq!(outcome.code, ExitCode::SUCCESS);
        assert!(
            outcome.out.contains("\n      \"app_id\": \"foot\","),
            "{}",
            outcome.out
        );
        assert_eq!(
            serde_json::from_str::<State>(&outcome.out).unwrap(),
            state(&[])
        );
    }

    #[test]
    fn screens_prints_where_each_stands_and_its_modes() {
        let mut shown = state(&[]);
        shown.screens = vec![
            omorfos_ipc::Screen {
                name: "DP-1".to_owned(),
                x: 0,
                y: 0,
                width: 2560,
                height: 1440,
                mode: "2560x1440@144".to_owned(),
                preferred: "2560x1440@60".to_owned(),
                modes: vec!["2560x1440@144".to_owned(), "2560x1440@60".to_owned()],
                mirrors: String::new(),
            },
            omorfos_ipc::Screen {
                name: "nested".to_owned(),
                x: 2560,
                y: 0,
                width: 640,
                height: 480,
                ..omorfos_ipc::Screen::default()
            },
            omorfos_ipc::Screen {
                name: "HDMI-A-1".to_owned(),
                width: 2560,
                height: 1440,
                mode: "1920x1080@60".to_owned(),
                mirrors: "DP-1".to_owned(),
                ..omorfos_ipc::Screen::default()
            },
        ];
        let outcome = invoke(&["screens"], vec![Reply::State(shown)]);
        assert_eq!(outcome.code, ExitCode::SUCCESS);
        assert_eq!(
            outcome.out,
            "DP-1: 2560 by 1440 units at 0,0, driven at 2560x1440@144\n  2560x1440@144\n  \
             2560x1440@60, which suits it best\nnested: 640 by 480 units at 2560,0\nHDMI-A-1: 2560 \
             by 1440 units at 0,0, mirroring DP-1, driven at 1920x1080@60\n"
        );
    }

    #[test]
    fn actions_are_written_as_keys_yaml_writes_them() {
        assert_eq!(action("close", &[]), json!("close"));
        assert_eq!(action("workspace", &["2"]), json!({ "workspace": 2 }));
        assert_eq!(action("spawn", &["foot"]), json!({ "spawn": ["foot"] }));
        assert_eq!(
            action("spawn", &["foot", "-e", "top"]),
            json!({ "spawn": ["foot", "-e", "top"] })
        );
        assert_eq!(
            action("spawn", &["foot", "2"]),
            json!({ "spawn": ["foot", "2"] })
        );
    }

    #[test]
    fn an_action_is_sent_and_its_refusal_reported() {
        let outcome = invoke(&["action", "workspace", "2"], vec![Reply::Done]);
        assert_eq!(
            (outcome.code, outcome.out.as_str(), outcome.err.as_str()),
            (ExitCode::SUCCESS, "", "")
        );
        assert_eq!(
            outcome.requests,
            [Request::Perform {
                action: json!({ "workspace": 2 })
            }]
        );

        let refusal = Reply::Error {
            message: "not an action: unknown variant `explode`".to_owned(),
        };
        let outcome = invoke(&["action", "explode"], vec![refusal]);
        assert_eq!(outcome.code, ExitCode::FAILURE);
        assert_eq!(
            outcome.err,
            "omorfosctl: not an action: unknown variant `explode`\n"
        );
    }

    #[test]
    fn activate_names_the_window() {
        let outcome = invoke(&["activate", "7"], vec![Reply::Done]);
        assert_eq!(outcome.code, ExitCode::SUCCESS);
        assert_eq!(outcome.requests, [Request::Activate { window: 7 }]);
    }

    #[test]
    fn config_check_succeeds_quietly_when_there_are_no_problems() {
        let outcome = invoke(&["config", "check"], vec![Reply::State(state(&[]))]);
        assert_eq!(
            (outcome.code, outcome.out.as_str()),
            (ExitCode::SUCCESS, "")
        );
        assert_eq!(outcome.requests, [Request::State]);
    }

    #[test]
    fn config_check_lists_problems_and_fails() {
        let problems = [
            "theme.css:2:13: `nope` is not a color",
            "keys.yaml: `Super+Retrun` names no key",
        ];
        let outcome = invoke(&["config", "check"], vec![Reply::State(state(&problems))]);
        assert_eq!(outcome.code, ExitCode::FAILURE);
        assert_eq!(outcome.out, format!("{}\n{}\n", problems[0], problems[1]));
        assert_eq!(
            outcome.err, "",
            "the problems are the output, not an error of the tool"
        );
    }

    #[test]
    fn config_reload_reloads_and_then_reports_like_check() {
        let outcome = invoke(
            &["config", "reload"],
            vec![Reply::Done, Reply::State(state(&["bad"]))],
        );
        assert_eq!(outcome.code, ExitCode::FAILURE);
        assert_eq!(outcome.out, "bad\n");
        assert_eq!(outcome.requests, [Request::Reload, Request::State]);
    }

    #[test]
    fn watch_prints_a_line_per_update_until_the_compositor_goes_away() {
        let updates = vec![
            Reply::State(state(&[])),
            Reply::State(state(&["one"])),
            Reply::State(state(&[])),
        ];
        let outcome = invoke_with_updates(&["watch"], updates, 2);
        assert_eq!(outcome.code, ExitCode::SUCCESS);
        let lines: Vec<State> = outcome
            .out
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(lines, [state(&[]), state(&["one"]), state(&[])]);
        assert_eq!(outcome.requests, [Request::Subscribe]);
    }

    /// Accepts one write and fails the rest, like a pipe whose reader left.
    struct ClosingPipe(usize);

    impl Write for ClosingPipe {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0 += 1;
            if self.0 > 1 {
                Err(io::ErrorKind::BrokenPipe.into())
            } else {
                Ok(bytes.len())
            }
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn watch_stops_when_nobody_is_reading_its_output() {
        let updates = vec![Reply::State(state(&[])); 4];
        let fake = fake(updates, 3);
        let mut out = ClosingPipe(0);
        let code = run(
            &["watch".to_owned()],
            Some(fake.socket.clone()),
            &mut out,
            &mut Vec::new(),
        );
        assert_eq!(code, ExitCode::SUCCESS);
        assert!(out.0 >= 2, "it stopped at the failed write");
        out.flush().unwrap();
        fake.served.join().unwrap();
    }

    #[test]
    fn a_compositor_that_hangs_up_or_answers_oddly_is_reported() {
        let outcome = invoke(&["state"], vec![]);
        assert_eq!(outcome.code, ExitCode::FAILURE);
        assert!(outcome.err.starts_with("omorfosctl: "), "{}", outcome.err);

        let outcome = invoke(&["state"], vec![Reply::Done]);
        assert_eq!(
            outcome.err,
            "omorfosctl: the compositor sent an unexpected answer\n"
        );

        let outcome = invoke(&["config", "reload"], vec![Reply::Done]);
        assert_eq!(
            outcome.code,
            ExitCode::FAILURE,
            "it hung up before the check"
        );

        let outcome = invoke(&["config", "reload"], vec![Reply::Done, Reply::Done]);
        assert_eq!(
            outcome.err,
            "omorfosctl: the compositor sent an unexpected answer\n"
        );
    }
}
