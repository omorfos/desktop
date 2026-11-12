//! What each program says when asked what it is, the same way for all:
//! `--help` or `-h` for what it is and how it is started, `--version` or
//! `-V` for its version, and nothing else done. Asked anywhere among its
//! arguments, so that a program that takes words of its own does not take
//! these for one, and never starts instead: a lock screen asked for help
//! that locked the session would cost the user more than a wrong answer.

use std::ffi::OsString;
use std::fmt::Write as _;
use std::io::Write;

/// What a program says of itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct About {
    /// Its name, as it is run.
    pub name: &'static str,
    /// What it is, in a sentence.
    pub what: &'static str,
    /// How it is started, and what it takes: what follows `Usage: `.
    pub usage: &'static str,
    /// The commands it takes, each as it is said and what it does; none
    /// for a program that takes none.
    pub commands: &'static [(&'static str, &'static str)],
    /// Its options, but for help and version, which every program has, each
    /// as it is said and what it does.
    pub options: &'static [(&'static str, &'static str)],
    /// Whole command lines, to show how it is run.
    pub examples: &'static [&'static str],
    /// Anything more someone running it should know, as who starts it, in
    /// paragraphs parted by a blank line; nothing for nothing.
    pub notes: &'static str,
    /// Its version.
    pub version: &'static str,
}

/// The options every program has.
const ASKED: &[(&str, &str)] = &[
    ("-h, --help", "Print this help"),
    ("-V, --version", "Print the version"),
];

/// How wide help is kept, as a terminal is at its narrowest.
const WIDTH: usize = 80;

/// Answers `arguments` if any asks what the program is, writing its help or
/// its version to `out`; says whether it did, for the program to end there
/// with success. Help is given where both are asked. A failure to write is
/// ignored, as there is nowhere to say it.
pub fn answered(about: &About, arguments: &[OsString], out: &mut dyn Write) -> bool {
    let asked = |words: [&str; 2]| {
        arguments
            .iter()
            .any(|argument| words.map(OsString::from).contains(argument))
    };
    if asked(["--help", "-h"]) {
        let _ = out.write_all(help(about).as_bytes());
        return true;
    }
    if asked(["--version", "-V"]) {
        let _ = writeln!(out, "{} {}", about.name, about.version);
        return true;
    }
    false
}

/// The help of the program `about` tells of, laid out as `omorfosctl`'s
/// is: what it is, how it is used, its commands and options in a column
/// each, examples, and notes, kept within `WIDTH`.
fn help(about: &About) -> String {
    let About {
        name,
        what,
        usage,
        commands,
        options,
        examples,
        notes,
        ..
    } = about;
    let mut help = String::new();
    wrapped(&mut help, what, 0);
    let _ = write!(help, "\nUsage: {name}{usage}\n");
    // One column for what each command and option does, after the widest
    // of them, so that the two lists read as one.
    let entries = || commands.iter().chain(*options).chain(ASKED);
    let widest = entries().map(|(said, _)| shown(said).chars().count()).max();
    let column = 2 + widest.unwrap_or_default() + 2;
    let mut listed = |title: &str, list: &mut dyn Iterator<Item = &(&str, &str)>| {
        let _ = write!(help, "\n{title}:\n");
        for (said, does) in list {
            let said = shown(said);
            let _ = write!(help, "  {said:width$}  ", width = column - 4);
            wrapped(&mut help, does, column);
        }
    };
    if !commands.is_empty() {
        listed("Commands", &mut commands.iter());
    }
    listed("Options", &mut options.iter().chain(ASKED));
    if !examples.is_empty() {
        help.push_str("\nExamples:\n");
        for example in *examples {
            let _ = writeln!(help, "  {example}");
        }
    }
    for paragraph in notes
        .split("\n\n")
        .filter(|paragraph| !paragraph.is_empty())
    {
        help.push('\n');
        wrapped(&mut help, paragraph, 0);
    }
    help
}

/// An option as it is listed: one with no short form under the long forms
/// of those that have one, as `--help` stands in `-h, --help`.
fn shown(said: &str) -> String {
    if said.starts_with("--") {
        format!("    {said}")
    } else {
        said.to_owned()
    }
}

/// `text` added to `help` in lines no wider than `WIDTH`, the first going
/// on from where `help` ends, and each after it starting at `indent`; the
/// last ended.
fn wrapped(help: &mut String, text: &str, indent: usize) {
    let line = help.rfind('\n').map_or(0, |end| end + 1);
    let mut at = help[line..].chars().count();
    for (index, word) in text.split_whitespace().enumerate() {
        let width = word.chars().count();
        if index > 0 && at + 1 + width > WIDTH {
            help.push('\n');
            help.push_str(&" ".repeat(indent));
            at = indent;
        } else if index > 0 {
            help.push(' ');
            at += 1;
        }
        help.push_str(word);
        at += width;
    }
    help.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;

    const PANEL: About = About {
        name: "omorfos-panel",
        what: "The panel of the omorfos desktop.",
        usage: "",
        commands: &[],
        options: &[],
        examples: &[],
        notes: "Started by the compositor.",
        version: "1.2.3",
    };

    fn asked(about: &About, arguments: &[&str]) -> Option<String> {
        let arguments: Vec<OsString> = arguments.iter().map(OsString::from).collect();
        let mut out = Vec::new();
        answered(about, &arguments, &mut out).then(|| String::from_utf8_lossy(&out).into_owned())
    }

    #[test]
    fn asked_for_help_it_says_what_it_is_how_it_is_started_and_what_it_takes() {
        let help = "The panel of the omorfos desktop.\n\nUsage: omorfos-panel\n\nOptions:\n  -h, --help     Print this help\n  -V, --version  Print the version\n\nStarted by the compositor.\n";
        assert_eq!(asked(&PANEL, &["--help"]).as_deref(), Some(help));
        assert_eq!(asked(&PANEL, &["-h"]).as_deref(), Some(help));
        // Wherever it is asked, and before the version.
        assert_eq!(
            asked(&PANEL, &["--to", "file", "-V", "--help"]).as_deref(),
            Some(help)
        );
        let plain = About {
            usage: " [--to WHERE]",
            notes: "",
            ..PANEL
        };
        let said = asked(&plain, &["-h"]).unwrap_or_default();
        assert!(
            said.contains("Usage: omorfos-panel [--to WHERE]\n"),
            "{said}"
        );
        assert!(
            said.ends_with("Print the version\n"),
            "nothing after: {said}"
        );
    }

    #[test]
    fn its_commands_and_options_are_listed_in_one_column_within_eighty_and_then_examples() {
        let tool = About {
            name: "omorfos-tool",
            what: "A tool of the omorfos desktop, said at such length that it does not fit in \
                   one line of a terminal.",
            usage: " [COMMAND]",
            commands: &[
                ("louder", "Turn it up"),
                (
                    "sleep|restart|shut-down",
                    "Ask for the machine to sleep, start again or turn off, as long as whoever \
                     decides lets it be done",
                ),
            ],
            options: &[("--to WHERE", "Where it goes")],
            examples: &["omorfos-tool louder", "omorfos-tool sleep"],
            notes: "A first paragraph.\n\nA second.",
            version: "1.2.3",
        };
        let help = "\
A tool of the omorfos desktop, said at such length that it does not fit in one
line of a terminal.

Usage: omorfos-tool [COMMAND]

Commands:
  louder                   Turn it up
  sleep|restart|shut-down  Ask for the machine to sleep, start again or turn
                           off, as long as whoever decides lets it be done

Options:
      --to WHERE           Where it goes
  -h, --help               Print this help
  -V, --version            Print the version

Examples:
  omorfos-tool louder
  omorfos-tool sleep

A first paragraph.

A second.
";
        assert_eq!(asked(&tool, &["--help"]).as_deref(), Some(help));
        assert!(help.lines().all(|line| line.chars().count() <= WIDTH));
    }

    #[test]
    fn asked_for_its_version_it_says_it_and_otherwise_nothing_is_answered() {
        assert_eq!(
            asked(&PANEL, &["--version"]).as_deref(),
            Some("omorfos-panel 1.2.3\n")
        );
        assert_eq!(
            asked(&PANEL, &["check", "-V"]).as_deref(),
            Some("omorfos-panel 1.2.3\n")
        );
        assert_eq!(asked(&PANEL, &[]), None);
        assert_eq!(asked(&PANEL, &["check", "--helpful", "-x"]), None);
    }
}
