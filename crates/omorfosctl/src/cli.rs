//! Parsing the command line and carrying it out.

use std::fmt::Write as _;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use omorfos_ipc::help::{About, help};
use omorfos_ipc::{Client, Drawing, Reply, Request, State};
use serde_json::{Value, json};

use crate::report::{self, Places};

/// What it says of itself, when asked and in its manual page.
pub(crate) const ABOUT: About = About {
    name: "omorfosctl",
    what: "Control a running omorfos compositor.",
    usage: " [--socket PATH] <COMMAND>",
    commands: &[
        ("version", "Print the running compositor's version"),
        ("state", "Print workspaces, windows and problems as JSON"),
        (
            "screens",
            "Print the screens, where they stand and their modes",
        ),
        (
            "frames",
            "Print how long each screen's last frame took to draw",
        ),
        (
            "watch",
            "Print the state, one JSON line each time it changes",
        ),
        (
            "action NAME [ARG]...",
            "Perform an action, as named in keys.yaml; `action` alone lists them",
        ),
        (
            "activate ID",
            "Switch to a window, by the id shown in `state`",
        ),
        (
            "config check",
            "Print configuration problems; fails if there are any",
        ),
        ("config reload", "Re-read the configuration, then check it"),
        (
            "report [--whole]",
            "Print what a report of a fault needs, for you to look over before handing it on: \
             versions, graphics cards, screens, state, configuration and the session's logs. \
             Window titles and paths in your home are left out unless --whole is given. Works \
             without a running compositor",
        ),
    ],
    options: &[(
        "--socket PATH",
        "The compositor's socket [default: $OMORFOS_SOCKET, or \
         omorfos-$WAYLAND_DISPLAY.sock in $XDG_RUNTIME_DIR]",
    )],
    examples: &[
        "omorfosctl action close",
        "omorfosctl action workspace 2",
        "omorfosctl action spawn foot --title scratch",
        "omorfosctl report > omorfos-report.txt",
    ],
    notes: "It exits with 0 on success, 1 on failure or if `config check` found problems, and 2 \
            for a usage error.",
    version: env!("CARGO_PKG_VERSION"),
};

/// What the user asked for.
#[derive(Debug, Clone, PartialEq)]
enum Command {
    Help,
    Actions,
    OwnVersion,
    Version,
    State,
    Screens,
    Frames,
    Watch,
    Perform(Value),
    Activate(u64),
    Check,
    Reload,
    Report { whole: bool },
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
        ["frames"] => Command::Frames,
        ["watch"] => Command::Watch,
        ["config", "check"] => Command::Check,
        ["config", "reload"] => Command::Reload,
        ["report"] => Command::Report { whole: false },
        ["report", "--whole"] => Command::Report { whole: true },
        ["activate", id] => {
            let id = id
                .parse()
                .map_err(|_| format!("`{id}` is not a window id"))?;
            Command::Activate(id)
        }
        ["action"] | ["action", "-h" | "--help"] => Command::Actions,
        ["action", name, arguments @ ..] => Command::Perform(action(name, arguments)),
        [] => return Err("no command given".to_owned()),
        [first, ..] => return Err(format!("unrecognised command `{first}`")),
    };
    Ok((socket, command))
}

/// Every action there is, as `omorfosctl action` alone lists them: in
/// their groups, each by its name and what it takes, beside what it does.
fn actions() -> String {
    let told = omorfos_ipc::actions::TOLD;
    let named = |told: &omorfos_ipc::actions::Told| {
        let space = if told.takes.is_empty() { "" } else { " " };
        format!("{}{space}{}", told.name, told.takes)
    };
    let wide = told
        .iter()
        .map(|told| named(told).len())
        .max()
        .unwrap_or_default();
    let mut list = String::from(
        "Actions, for `omorfosctl action NAME [ARG]...`, and to bind in keys.yaml\n\
         and menu.yaml by the same names:\n",
    );
    let mut group = "";
    for told in told {
        if told.group != group {
            group = told.group;
            let _ = write!(list, "\n{group}:\n");
        }
        let _ = writeln!(list, "  {:wide$}  {}", named(told), told.does);
    }
    list.push_str(
        "\nExamples:\n  omorfosctl action overview\n  omorfosctl action workspace 2\n  \
         omorfosctl action tile left\n  omorfosctl action spawn foot --title scratch\n",
    );
    list
}

/// Builds an action in the form `keys.yaml` uses, from words.
///
/// A name alone is the action itself (`close`). With one whole number it is
/// that action applied to the number (`workspace 2`), and with one word
/// that action applied to the word (`tile left`). A program to start, and
/// anything else, is a list (`spawn foot --title scratch`).
fn action(name: &str, arguments: &[&str]) -> Value {
    match arguments {
        [] => json!(name),
        [number] if number.parse::<u64>().is_ok() => {
            json!({ name: number.parse::<u64>().unwrap_or_default() })
        }
        [word] if name != "spawn" => json!({ name: word }),
        words => json!({ name: words }),
    }
}

/// Runs the program: output goes to `out`, problems to `err`.
///
/// `socket` is where the environment says the compositor's socket is; a
/// `--socket` argument overrides it. A report reads from `places`.
pub(crate) fn run(
    arguments: &[String],
    socket: Option<PathBuf>,
    places: &Places,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> ExitCode {
    let (chosen, command) = match parse(arguments) {
        Ok(parsed) => parsed,
        Err(problem) => {
            let _ = writeln!(err, "omorfosctl: {problem}\n\n{}", help(&ABOUT));
            return ExitCode::from(2);
        }
    };
    match command {
        Command::Help => {
            let _ = out.write_all(help(&ABOUT).as_bytes());
            return ExitCode::SUCCESS;
        }
        Command::Actions => {
            let _ = out.write_all(actions().as_bytes());
            return ExitCode::SUCCESS;
        }
        Command::OwnVersion => {
            let _ = writeln!(out, "omorfosctl {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        // Made with or without a compositor, which is most wanted when there
        // is none.
        Command::Report { whole } => {
            let asked = connect(chosen.or(socket)).and_then(|mut client| ask(&mut client));
            report::write(places, asked, whole, out);
            return ExitCode::SUCCESS;
        }
        _ => {}
    }
    let connected = connect(chosen.or(socket));
    match connected.and_then(|mut client| execute(&command, &mut client, out)) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(problem) => {
            let _ = writeln!(err, "omorfosctl: {problem}");
            ExitCode::FAILURE
        }
    }
}

/// Connects to the compositor's socket at `path`, saying why not where it
/// cannot.
fn connect(path: Option<PathBuf>) -> Result<Client, String> {
    path.ok_or_else(|| {
        "cannot tell where the compositor's socket is; is omorfos running? \
         Set OMORFOS_SOCKET or pass --socket"
            .to_owned()
    })
    .and_then(|path| {
        Client::connect(&path)
            .map_err(|error| format!("cannot connect to {}: {error}", path.display()))
    })
}

/// Asks the compositor its version, as `version` prints it, and its state.
fn ask(client: &mut Client) -> Result<(String, State), String> {
    let unexpected = || "the compositor sent an unexpected answer".to_owned();
    let version = match client
        .request(&Request::Version)
        .map_err(|error| error.to_string())?
    {
        Reply::Version {
            protocol,
            compositor,
        } => format!("omorfos {compositor} (protocol {protocol})"),
        _ => return Err(unexpected()),
    };
    match client
        .request(&Request::State)
        .map_err(|error| error.to_string())?
    {
        Reply::State(state) => Ok((version, state)),
        _ => Err(unexpected()),
    }
}

/// Carries out a command that needs the compositor. Returns whether all is
/// well, which for `config check` means there were no problems.
fn execute(command: &Command, client: &mut Client, out: &mut dyn Write) -> Result<bool, String> {
    let request = match command {
        Command::Version => Request::Version,
        Command::Frames => Request::Frames,
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
        (Command::Frames, Reply::Frames(drawn)) => {
            frames(&drawn, out);
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
pub(crate) fn screens(state: &State, out: &mut dyn Write) {
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
        if screen.off {
            let _ = writeln!(out, "{}: off", screen.name);
        } else {
            let _ = writeln!(
                out,
                "{}: {} by {} units at {},{}{mirrors}{driven}",
                screen.name, screen.width, screen.height, screen.x, screen.y
            );
        }
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

/// Prints how many frames each screen has drawn, and how long the last
/// took to draw and how many blurs it made.
fn frames(drawn: &[Drawing], out: &mut dyn Write) {
    let counted = |count: u64, what: &str| {
        let plural = if count == 1 { "" } else { "s" };
        format!("{count} {what}{plural}")
    };
    for drawing in drawn {
        if drawing.frames == 0 {
            let _ = writeln!(out, "{}: no frames drawn", drawing.output);
            continue;
        }
        let micros = drawing.took;
        let _ = writeln!(
            out,
            "{}: {}, the last drawn in {}.{:02}ms with {}",
            drawing.output,
            counted(drawing.frames, "frame"),
            micros / 1000,
            micros % 1000 / 10,
            counted(drawing.blurs, "blur"),
        );
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
        let code = run(
            &arguments,
            Some(fake.socket.clone()),
            &nowhere(),
            &mut out,
            &mut err,
        );
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

    /// Places a report finds nothing in.
    fn nowhere() -> Places {
        Places {
            config: None,
            logs: None,
            home: None,
            root: PathBuf::from("/nonexistent"),
        }
    }

    fn offline(arguments: &[&str]) -> (ExitCode, String, String) {
        let arguments: Vec<String> = arguments.iter().map(ToString::to_string).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(&arguments, None, &nowhere(), &mut out, &mut err);
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
            awake: None,
            radios: Vec::new(),
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

    /// How the manual pages are made again.
    const AGAIN: &str =
        "make it again with `OMORFOS_BLESS=1 cargo test --workspace --all-features manual_page`";

    /// Kept here, as the other programs' pages are kept by the compositor's
    /// tests, through their libraries, which omorfosctl has none of.
    #[test]
    fn its_manual_page_is_made_from_what_it_says() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../man/omorfosctl.1");
        let made = omorfos_ipc::manual::page(&ABOUT);
        let bless = std::env::var_os("OMORFOS_BLESS").is_some();
        let kept = omorfos_ipc::manual::kept(&path, &made, bless).unwrap();
        assert!(kept, "man/omorfosctl.1 is out of date; {AGAIN}");
    }

    #[test]
    fn the_action_command_alone_lists_every_action_in_groups_and_needs_no_compositor() {
        for arguments in [&["action"][..], &["action", "--help"], &["action", "-h"]] {
            let (code, out, err) = offline(arguments);
            assert_eq!((code, err.as_str()), (ExitCode::SUCCESS, ""));
            assert!(out.starts_with("Actions, for `omorfosctl action"), "{out}");
            for told in omorfos_ipc::actions::TOLD {
                let name = format!("{} ", told.name);
                let line = out
                    .lines()
                    .find(|line| line.trim_start().starts_with(&name));
                assert!(
                    line.is_some_and(|line| line.ends_with(told.does)),
                    "{told:?}\n{out}"
                );
            }
            assert!(out.contains("\nWindows:\n  close "), "{out}");
            assert!(out.lines().all(|line| line.len() <= 80), "{out}");
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
        let code = run(&arguments, None, &nowhere(), &mut out, &mut err);
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
            run(&arguments, elsewhere, &nowhere(), &mut out, &mut err),
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
    fn a_report_asks_the_compositor_its_version_and_state_and_is_made_without_one() {
        let version = Reply::Version {
            protocol: 3,
            compositor: "0.1.0".to_owned(),
        };
        let outcome = invoke(
            &["report"],
            vec![version.clone(), Reply::State(state(&["odd"]))],
        );
        assert_eq!(outcome.code, ExitCode::SUCCESS);
        assert_eq!(outcome.requests, [Request::Version, Request::State]);
        assert!(
            outcome
                .out
                .contains("compositor: omorfos 0.1.0 (protocol 3)\n"),
            "{}",
            outcome.out
        );
        assert!(
            outcome.out.contains("\"title\": \"(left out)\""),
            "{}",
            outcome.out
        );
        let outcome = invoke(
            &["report", "--whole"],
            vec![version.clone(), Reply::State(state(&[]))],
        );
        assert!(outcome.out.contains("\"title\": \"~\""), "{}", outcome.out);

        let refused = Reply::Error {
            message: "busy".to_owned(),
        };
        for replies in [vec![Reply::Done], vec![version.clone(), Reply::Done]] {
            let outcome = invoke(&["report"], replies);
            let said = "compositor: did not answer: the compositor sent an unexpected answer\n";
            assert!(outcome.out.contains(said), "{}", outcome.out);
        }
        let outcome = invoke(&["report"], vec![refused]);
        assert!(
            outcome
                .out
                .contains("did not answer: the compositor sent an unexpected answer"),
            "{}",
            outcome.out
        );

        // Gone before it answered either.
        for replies in [Vec::new(), vec![version.clone()]] {
            let outcome = invoke(&["report"], replies);
            assert!(
                outcome.out.contains("compositor: did not answer: "),
                "{}",
                outcome.out
            );
        }

        let (code, out, err) = offline(&["report"]);
        assert_eq!((code, err.as_str()), (ExitCode::SUCCESS, ""));
        assert!(
            out.contains("did not answer: cannot tell where the compositor's socket is"),
            "{out}"
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
                off: false,
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
            omorfos_ipc::Screen {
                name: "DP-2".to_owned(),
                off: true,
                modes: vec!["1920x1080@60".to_owned()],
                ..omorfos_ipc::Screen::default()
            },
        ];
        let outcome = invoke(&["screens"], vec![Reply::State(shown)]);
        assert_eq!(outcome.code, ExitCode::SUCCESS);
        assert_eq!(
            outcome.out,
            "DP-1: 2560 by 1440 units at 0,0, driven at 2560x1440@144\n  2560x1440@144\n  \
             2560x1440@60, which suits it best\nnested: 640 by 480 units at 2560,0\nHDMI-A-1: 2560 \
             by 1440 units at 0,0, mirroring DP-1, driven at 1920x1080@60\nDP-2: off\n  \
             1920x1080@60\n"
        );
    }

    #[test]
    fn frames_prints_how_each_screen_drew_its_last_frame() {
        let drawing = |output: &str, frames, took, blurs| Drawing {
            output: output.to_owned(),
            frames,
            took,
            blurs,
        };
        let drawn = vec![
            drawing("eDP-1", 1200, 850, 1),
            drawing("HDMI-A-1", 1, 16_204, 2),
            drawing("DP-2", 0, 0, 0),
        ];
        let outcome = invoke(&["frames"], vec![Reply::Frames(drawn)]);
        assert_eq!(outcome.code, ExitCode::SUCCESS);
        assert_eq!(outcome.requests, [Request::Frames]);
        assert_eq!(
            outcome.out,
            "eDP-1: 1200 frames, the last drawn in 0.85ms with 1 blur
\
             HDMI-A-1: 1 frame, the last drawn in 16.20ms with 2 blurs
\
             DP-2: no frames drawn
"
        );
    }

    #[test]
    fn actions_are_written_as_keys_yaml_writes_them() {
        assert_eq!(action("close", &[]), json!("close"));
        assert_eq!(action("workspace", &["2"]), json!({ "workspace": 2 }));
        assert_eq!(action("spawn", &["foot"]), json!({ "spawn": ["foot"] }));
        assert_eq!(action("tile", &["left"]), json!({ "tile": "left" }));
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
    fn every_action_listed_is_built_from_words_as_the_configuration_takes_it() {
        for told in omorfos_ipc::actions::TOLD {
            // What it takes, as someone would type it: the first of the
            // choices, a number, or a program and its arguments.
            let words: Vec<&str> = match told.takes {
                "" => Vec::new(),
                "N" | "ID" | "SECONDS" => vec!["2"],
                "PROGRAM [ARG]..." => vec!["foot", "--title", "scratch"],
                choices => vec![choices.split('|').next().unwrap_or_default()],
            };
            let built = action(told.name, &words);
            let taken = serde_json::from_value::<omorfos_config::Action>(built.clone());
            assert!(taken.is_ok(), "{} {words:?}: {built} {taken:?}", told.name);
        }
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
            &nowhere(),
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
