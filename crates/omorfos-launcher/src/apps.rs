//! The programs there are to start: what the desktop entries installed on
//! the system say.
//!
//! A desktop entry is a small file an application installs to say what it
//! is called and how it is started (see [`omorfos_entries`]). Only so many
//! of them are read.

use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;

use omorfos_entries::{arguments, fields, fit, read};

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
    /// Reads a desktop entry, its name and what it says of itself in the
    /// first of `locales` (see [`omorfos_entries::locales`]) it has them
    /// in, or untranslated. Returns nothing for one that is not a program
    /// to offer: it is of another type, is hidden, or lacks a name or a
    /// command.
    #[must_use]
    pub fn parse(entry: &str, locales: &[String]) -> Option<Self> {
        let (mut name, mut exec) = (None, None);
        let (mut description, mut icon) = (None, "");
        let (mut application, mut shown, mut terminal) = (false, true, false);
        for (key, value) in fields(entry) {
            if let Some(fit) = fit(key, "Name", locales) {
                better(&mut name, fit, value);
                continue;
            }
            if let Some(fit) = fit(key, "Comment", locales) {
                better(&mut description, fit, value);
                continue;
            }
            match key {
                "Type" => application = value == "Application",
                "Exec" => exec = Some(value),
                "Icon" => icon = value,
                "NoDisplay" | "Hidden" => shown &= value != "true",
                "Terminal" => terminal = value == "true",
                _ => {}
            }
        }
        let command = exec.map(arguments).filter(|command| !command.is_empty());
        let name = name.map(|(_, name)| name);
        let offered = name.zip(command).filter(|_| application && shown);
        let description = description.map_or("", |(_, description)| description);
        offered.map(|(name, command)| Self {
            name: name.to_owned(),
            command,
            terminal,
            description: description.to_owned(),
            icon: icon.to_owned(),
        })
    }
}

/// Keeps `value` in `best` if it is said and fits as well as what is
/// there or better, the later of two that fit as well counting, as a key
/// written twice does.
fn better<'a>(best: &mut Option<(usize, &'a str)>, fit: usize, value: &'a str) {
    if !value.is_empty() && best.is_none_or(|(kept, _)| fit <= kept) {
        *best = Some((fit, value));
    }
}

/// Where desktop entries are installed, the user's own first: the
/// `applications` directory of each place programs install what they
/// share (see [`omorfos_icons::data_directories`]).
#[must_use]
pub fn directories(
    data_home: Option<&OsStr>,
    home: Option<&OsStr>,
    data_dirs: Option<&OsStr>,
) -> Vec<PathBuf> {
    let data = omorfos_icons::data_directories(data_home, home, data_dirs).into_iter();
    data.map(|directory| directory.join("applications"))
        .collect()
}

/// The programs installed in `directories`, by name, each named in the
/// first of `locales` its entry has a name in (see [`App::parse`]). Where
/// two directories have an entry of the same file name, the one in the
/// earlier directory is the one that counts, hidden or not, which is how a
/// user's own entry replaces or removes the system's.
#[must_use]
pub fn installed(directories: &[PathBuf], locales: &[String]) -> Vec<App> {
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
            apps.extend(read(&path).and_then(|entry| App::parse(&entry, locales)));
        }
    }
    apps.sort_by_key(|app| app.name.to_lowercase());
    apps
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use omorfos_entries::{LONGEST, locales};

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
        assert_eq!(App::parse(&text, &[]), Some(expected));
    }

    #[test]
    fn a_program_is_called_and_described_in_the_users_language_where_its_entry_has_it() {
        let text = entry(&[
            "Type=Application",
            "Name[pl]=Przegl\u{105}darka",
            "Name=Web Browser",
            "Name[pl_PL]=",
            "Comment=Browse the web",
            "Comment[de]=Im Netz",
            "Comment[pl]=Przegl\u{105}daj",
            "Exec=browser",
        ]);
        let polish = locales(&["pl_PL.UTF-8".to_owned()]);
        let browser = App::parse(&text, &polish).unwrap();
        assert_eq!(
            (browser.name.as_str(), browser.description.as_str()),
            ("Przegl\u{105}darka", "Przegl\u{105}daj"),
            "a translation left empty is passed over, and the key itself comes last"
        );
        let german = App::parse(&text, &locales(&["de_DE.UTF-8".to_owned()])).unwrap();
        assert_eq!(
            (german.name.as_str(), german.description.as_str()),
            ("Web Browser", "Im Netz")
        );
    }

    #[test]
    fn installed_programs_are_listed_by_their_names_in_the_users_language() {
        let root = tempfile::TempDir::new().unwrap();
        let write = |file: &str, lines: &[&str]| {
            fs::write(root.path().join(file), entry(lines)).unwrap();
        };
        write(
            "files.desktop",
            &[
                "Type=Application",
                "Name=Files",
                "Name[pl]=Pliki",
                "Exec=files",
            ],
        );
        write(
            "notes.desktop",
            &[
                "Type=Application",
                "Name=Notes",
                "Name[pl]=Notatki",
                "Exec=notes",
            ],
        );
        let polish = locales(&["pl".to_owned()]);
        let apps = installed(&[root.path().to_owned()], &polish);
        let names: Vec<&str> = apps.iter().map(|app| app.name.as_str()).collect();
        assert_eq!(names, ["Notatki", "Pliki"]);
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
        let notes = App::parse(&text, &[]).unwrap();
        assert_eq!(notes.description, "Write things down");
        assert_eq!(notes.icon, "");
    }

    #[test]
    fn only_the_main_group_of_an_entry_counts() {
        let text = "# a comment\n\n[Desktop Entry]\nType = Application\nName = Editor \n\
                    Exec = edit\n\n[Desktop Action new]\nName=New Window\nExec=edit --new\n";
        assert_eq!(App::parse(text, &[]), Some(app("Editor", &["edit"])));
        let stray = "Name=Stray\nExec=stray\nType=Application\n[Other]\nName=Other\n";
        assert_eq!(
            App::parse(stray, &[]),
            None,
            "nothing before the group counts"
        );
    }

    #[test]
    fn what_is_not_a_program_to_offer_is_passed_over() {
        let offered = ["Type=Application", "Name=Tool", "Exec=tool"];
        assert!(App::parse(&entry(&offered), &[]).is_some());
        let with = |extra: &str| {
            let mut lines = offered.to_vec();
            lines.push(extra);
            App::parse(&entry(&lines), &[])
        };
        assert_eq!(with("NoDisplay=true"), None);
        assert_eq!(with("Hidden=true"), None);
        assert!(with("NoDisplay=false").is_some());
        assert_eq!(
            App::parse(&entry(&["Type=Link", "Name=Site", "Exec=x"]), &[]),
            None
        );
        assert_eq!(App::parse(&entry(&["Name=Tool", "Exec=tool"]), &[]), None);
        assert_eq!(
            App::parse(&entry(&["Type=Application", "Exec=tool"]), &[]),
            None
        );
        assert_eq!(
            App::parse(&entry(&["Type=Application", "Name=Tool"]), &[]),
            None
        );
        let empty = ["Type=Application", "Name=", "Exec=tool"];
        assert_eq!(App::parse(&entry(&empty), &[]), None);
        let nothing = ["Type=Application", "Name=Tool", "Exec=%f"];
        assert_eq!(App::parse(&entry(&nothing), &[]), None);
        assert_eq!(App::parse("", &[]), None);
        assert_eq!(App::parse("not an entry at all", &[]), None);
    }

    #[test]
    fn a_program_may_need_a_terminal() {
        let text = entry(&["Type=Application", "Name=Top", "Exec=top", "Terminal=true"]);
        assert!(App::parse(&text, &[]).unwrap().terminal);
        let text = entry(&["Type=Application", "Name=Top", "Exec=top", "Terminal=false"]);
        assert!(!App::parse(&text, &[]).unwrap().terminal);
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
        let apps = installed(&[own, missing, system], &[]);

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
        assert_eq!(installed(&[root.path().to_owned()], &[]), []);
    }
}
