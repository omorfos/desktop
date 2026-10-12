//! The programs there are to start: what the desktop entries installed on
//! the system say.
//!
//! A desktop entry is a small file an application installs to say what it
//! is called and how it is started. They are other people's files and are
//! read as such: only so much of each, only so many of them, and whatever
//! does not make sense is passed over.

use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

/// The most of a desktop entry that is read, in bytes. Real ones are a few
/// kilobytes, most of it translations.
const LONGEST: u64 = 256 * 1024;

/// The most desktop entries that are read from one directory.
const MOST: usize = 4096;

/// A program that can be started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    /// What it is called.
    pub name: String,
    /// The program and its arguments.
    pub command: Vec<String>,
    /// Whether it has to be run in a terminal.
    pub terminal: bool,
    /// What it says of itself, in a few words. Empty if it says nothing.
    pub description: String,
    /// Its icon: a name to look up among the icons installed, or the path
    /// of a picture. Empty if it has none.
    pub icon: String,
}

impl App {
    /// Reads a desktop entry. Returns nothing for one that is not a
    /// program to offer: it is of another type, is hidden, or lacks a name
    /// or a command.
    #[must_use]
    pub fn parse(entry: &str) -> Option<Self> {
        let mut inside = false;
        let (mut name, mut exec) = (None, None);
        let (mut description, mut icon) = ("", "");
        let (mut application, mut shown, mut terminal) = (false, true, false);
        for line in entry.lines().map(str::trim) {
            if line.starts_with('[') {
                inside = line == "[Desktop Entry]";
            }
            let Some((key, value)) = line.split_once('=').filter(|_| inside) else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "Type" => application = value == "Application",
                "Name" => name = Some(value),
                "Exec" => exec = Some(value),
                "Comment" => description = value,
                "Icon" => icon = value,
                "NoDisplay" | "Hidden" => shown &= value != "true",
                "Terminal" => terminal = value == "true",
                _ => {}
            }
        }
        let command = exec.map(arguments).filter(|command| !command.is_empty());
        let name = name.filter(|name| !name.is_empty());
        let offered = name.zip(command).filter(|_| application && shown);
        offered.map(|(name, command)| Self {
            name: name.to_owned(),
            command,
            terminal,
            description: description.to_owned(),
            icon: icon.to_owned(),
        })
    }
}

/// Splits the `Exec` line of a desktop entry into a program and its
/// arguments, as the specification has it: words apart from one another by
/// spaces, a word in double quotes kept whole with `\` before a character
/// that is meant as itself. The places where files to open would go (`%f`,
/// `%U` and the like) are left out, since there are none.
fn arguments(exec: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word: Option<String> = None;
    let mut quoted = false;
    let mut characters = exec.chars();
    while let Some(character) = characters.next() {
        match character {
            '"' => {
                quoted = !quoted;
                // Even nothing between quotes is a word.
                word.get_or_insert_default();
            }
            '\\' if quoted => word.get_or_insert_default().extend(characters.next()),
            ' ' if !quoted => words.extend(word.take()),
            '%' => {
                let code = characters.next();
                let literal = code.filter(|code| *code == '%');
                word.get_or_insert_default().extend(literal);
            }
            other => word.get_or_insert_default().push(other),
        }
    }
    words.extend(word);
    // A place for a file that stood alone leaves an empty word behind.
    words.retain(|word| !word.is_empty());
    words
}

/// A variable of the environment that was given a value.
fn set(value: Option<&OsStr>) -> Option<&OsStr> {
    value.filter(|value| !value.is_empty())
}

/// Where desktop entries are installed, the user's own first: the
/// `applications` directory of `data_home` (`XDG_DATA_HOME`, or
/// `.local/share` in `home`) and of each of `data_dirs` (`XDG_DATA_DIRS`,
/// or `/usr/local/share` and `/usr/share`).
#[must_use]
pub fn directories(
    data_home: Option<&OsStr>,
    home: Option<&OsStr>,
    data_dirs: Option<&OsStr>,
) -> Vec<PathBuf> {
    let own = set(data_home).map(PathBuf::from);
    let own = own.or_else(|| set(home).map(|home| Path::new(home).join(".local/share")));
    let shared = set(data_dirs).unwrap_or(OsStr::new("/usr/local/share:/usr/share"));
    let shared = std::env::split_paths(shared).filter(|path| path.is_absolute());
    let all = own.into_iter().chain(shared);
    all.map(|path| path.join("applications")).collect()
}

/// The programs installed in `directories`, by name. Where two directories
/// have an entry of the same file name, the one in the earlier directory
/// is the one that counts, hidden or not, which is how a user's own entry
/// replaces or removes the system's.
#[must_use]
pub fn installed(directories: &[PathBuf]) -> Vec<App> {
    let mut seen: Vec<std::ffi::OsString> = Vec::new();
    let mut apps = Vec::new();
    for directory in directories {
        let entries = fs::read_dir(directory).into_iter().flatten().flatten();
        let entries = entries.map(|entry| entry.path());
        let entries = entries.filter(|path| path.extension() == Some(OsStr::new("desktop")));
        let mut entries: Vec<PathBuf> = entries.take(MOST).collect();
        // The order a directory lists its files in is not one to rely on.
        entries.sort();
        for path in entries {
            let id = path.file_name().unwrap_or_default().to_owned();
            if seen.contains(&id) {
                continue;
            }
            seen.push(id);
            apps.extend(read(&path).as_deref().and_then(App::parse));
        }
    }
    apps.sort_by_key(|app| app.name.to_lowercase());
    apps
}

/// Reads the beginning of a file, if it can be read and is text.
fn read(path: &Path) -> Option<String> {
    let mut text = String::new();
    let file = File::open(path).ok();
    let read = file.map(|file| file.take(LONGEST).read_to_string(&mut text));
    read.and_then(Result::ok).map(|_| text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(lines: &[&str]) -> String {
        format!("[Desktop Entry]\n{}\n", lines.join("\n"))
    }

    fn app(name: &str, command: &[&str]) -> App {
        App {
            name: name.to_owned(),
            command: command.iter().map(|word| (*word).to_owned()).collect(),
            terminal: false,
            description: String::new(),
            icon: String::new(),
        }
    }

    #[test]
    fn a_desktop_entry_says_what_a_program_is_called_and_how_it_is_started() {
        let text = entry(&[
            "Type=Application",
            "Name=Web Browser",
            "Name[pl]=Przegl\u{105}darka",
            "Exec=browser --new-window %U",
            "Icon=browser",
        ]);
        let expected = App {
            icon: "browser".to_owned(),
            ..app("Web Browser", &["browser", "--new-window"])
        };
        assert_eq!(App::parse(&text), Some(expected));
    }

    #[test]
    fn an_entry_may_say_what_the_program_is_for() {
        let text = entry(&[
            "Type=Application",
            "Name=Notes",
            "Comment=Write things down",
            "Comment[pl]=Zapisuj",
            "Exec=notes",
        ]);
        let notes = App::parse(&text).unwrap();
        assert_eq!(notes.description, "Write things down");
        assert_eq!(notes.icon, "");
    }

    #[test]
    fn only_the_main_group_of_an_entry_counts() {
        let text = "# a comment\n\n[Desktop Entry]\nType = Application\nName = Editor \n\
                    Exec = edit\n\n[Desktop Action new]\nName=New Window\nExec=edit --new\n";
        assert_eq!(App::parse(text), Some(app("Editor", &["edit"])));
        let stray = "Name=Stray\nExec=stray\nType=Application\n[Other]\nName=Other\n";
        assert_eq!(App::parse(stray), None, "nothing before the group counts");
    }

    #[test]
    fn what_is_not_a_program_to_offer_is_passed_over() {
        let offered = ["Type=Application", "Name=Tool", "Exec=tool"];
        assert!(App::parse(&entry(&offered)).is_some());
        let with = |extra: &str| {
            let mut lines = offered.to_vec();
            lines.push(extra);
            App::parse(&entry(&lines))
        };
        assert_eq!(with("NoDisplay=true"), None);
        assert_eq!(with("Hidden=true"), None);
        assert!(with("NoDisplay=false").is_some());
        assert_eq!(
            App::parse(&entry(&["Type=Link", "Name=Site", "Exec=x"])),
            None
        );
        assert_eq!(App::parse(&entry(&["Name=Tool", "Exec=tool"])), None);
        assert_eq!(App::parse(&entry(&["Type=Application", "Exec=tool"])), None);
        assert_eq!(App::parse(&entry(&["Type=Application", "Name=Tool"])), None);
        let empty = ["Type=Application", "Name=", "Exec=tool"];
        assert_eq!(App::parse(&entry(&empty)), None);
        let nothing = ["Type=Application", "Name=Tool", "Exec=%f"];
        assert_eq!(App::parse(&entry(&nothing)), None);
        assert_eq!(App::parse(""), None);
        assert_eq!(App::parse("not an entry at all"), None);
    }

    #[test]
    fn a_program_may_need_a_terminal() {
        let text = entry(&["Type=Application", "Name=Top", "Exec=top", "Terminal=true"]);
        assert!(App::parse(&text).unwrap().terminal);
        let text = entry(&["Type=Application", "Name=Top", "Exec=top", "Terminal=false"]);
        assert!(!App::parse(&text).unwrap().terminal);
    }

    #[test]
    fn a_command_is_split_into_words_with_quoted_ones_kept_whole() {
        assert_eq!(arguments("tool"), ["tool"]);
        assert_eq!(
            arguments("  tool   --flag  value "),
            ["tool", "--flag", "value"]
        );
        assert_eq!(
            arguments(r#"sh -c "printf x > '/tmp/a file'""#),
            ["sh", "-c", "printf x > '/tmp/a file'"]
        );
        assert_eq!(
            arguments(r#"say "a \"quoted\" word" \n"#),
            ["say", "a \"quoted\" word", "\\n"]
        );
        assert_eq!(arguments(r#"tool "" end"#), ["tool", "end"]);
        assert_eq!(arguments(r#"pre"fix ed"post"#), ["prefix edpost"]);
        assert_eq!(arguments(""), [""; 0]);
        // A quote left open takes the rest, and a `\` at the very end is
        // nothing.
        assert_eq!(arguments(r#"tool "the rest"#), ["tool", "the rest"]);
        assert_eq!(arguments("tool \"x\\"), ["tool", "x"]);
    }

    #[test]
    fn places_for_files_to_open_are_left_out_of_a_command() {
        assert_eq!(arguments("view %f"), ["view"]);
        assert_eq!(arguments("view %U --all"), ["view", "--all"]);
        assert_eq!(arguments("view --file=%f"), ["view", "--file="]);
        assert_eq!(arguments("discount 50%% off"), ["discount", "50%", "off"]);
        assert_eq!(arguments("odd %"), ["odd"]);
    }

    #[test]
    fn desktop_entries_are_looked_for_in_the_users_directory_first() {
        let os = |text: &'static str| Some(OsStr::new(text));
        assert_eq!(
            directories(
                os("/home/me/data"),
                os("/home/me"),
                os("/opt/share:/usr/share")
            ),
            [
                PathBuf::from("/home/me/data/applications"),
                PathBuf::from("/opt/share/applications"),
                PathBuf::from("/usr/share/applications"),
            ]
        );
        assert_eq!(
            directories(None, os("/home/me"), None),
            [
                PathBuf::from("/home/me/.local/share/applications"),
                PathBuf::from("/usr/local/share/applications"),
                PathBuf::from("/usr/share/applications"),
            ]
        );
        // Without a home there are still the system's, and a directory
        // that is not a whole path is not one to look in.
        assert_eq!(
            directories(os(""), None, os("relative:/usr/share:")),
            [PathBuf::from("/usr/share/applications")]
        );
    }

    #[test]
    fn installed_programs_are_listed_by_name_with_the_users_entries_first() {
        let root = tempfile::TempDir::new().unwrap();
        let (own, system) = (root.path().join("own"), root.path().join("system"));
        fs::create_dir_all(&own).unwrap();
        fs::create_dir_all(&system).unwrap();
        let write = |directory: &Path, file: &str, lines: &[&str]| {
            fs::write(directory.join(file), entry(lines)).unwrap();
        };
        let tool = |name: &str| format!("Name={name}");
        write(
            &system,
            "zed.desktop",
            &["Type=Application", &tool("zed"), "Exec=zed"],
        );
        write(
            &system,
            "Alpha.desktop",
            &["Type=Application", &tool("Alpha"), "Exec=alpha"],
        );
        write(
            &system,
            "beta.desktop",
            &["Type=Application", &tool("beta"), "Exec=beta"],
        );
        write(
            &system,
            "gone.desktop",
            &["Type=Application", &tool("Gone"), "Exec=gone"],
        );
        write(
            &system,
            "notes.txt",
            &["Type=Application", &tool("Notes"), "Exec=notes"],
        );
        // The user's own replaces one of the system's and hides another.
        write(
            &own,
            "beta.desktop",
            &["Type=Application", &tool("Beta mine"), "Exec=mine"],
        );
        write(
            &own,
            "gone.desktop",
            &[
                "Type=Application",
                &tool("Gone"),
                "Exec=gone",
                "Hidden=true",
            ],
        );
        fs::write(own.join("binary.desktop"), [0xff, 0xfe, 0x00]).unwrap();

        let missing = root.path().join("missing");
        let apps = installed(&[own, missing, system]);

        let names: Vec<&str> = apps.iter().map(|app| app.name.as_str()).collect();
        assert_eq!(names, ["Alpha", "Beta mine", "zed"]);
        assert_eq!(apps[1].command, ["mine"]);
    }

    #[test]
    fn only_the_beginning_of_an_enormous_entry_is_read() {
        let root = tempfile::TempDir::new().unwrap();
        let padding = "#".repeat(usize::try_from(LONGEST).unwrap());
        let text = format!(
            "{padding}\n{}",
            entry(&["Type=Application", "Name=Late", "Exec=late"])
        );
        fs::write(root.path().join("late.desktop"), text).unwrap();
        assert_eq!(installed(&[root.path().to_owned()]), []);
    }
}
