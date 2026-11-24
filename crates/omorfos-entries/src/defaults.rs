//! The applications that open each kind of thing by default, as the
//! `mimeapps.list` files of freedesktop.org's specification say: read only,
//! for the welcome card to show and the root menu to open. Choosing them
//! is for Settings.

use std::path::{Path, PathBuf};

/// The kinds of thing the card names, each by its message, with the types
/// that stand for it, the first that has a default being the one shown.
pub const KINDS: [(&str, &[&str]); 6] = [
    (
        "web",
        &[
            "x-scheme-handler/https",
            "x-scheme-handler/http",
            "text/html",
        ],
    ),
    ("files", &["inode/directory"]),
    ("mail", &["x-scheme-handler/mailto"]),
    ("pictures", &["image/png", "image/jpeg"]),
    ("music", &["audio/mpeg", "audio/flac"]),
    ("video", &["video/mp4", "video/x-matroska"]),
];

/// Where the `mimeapps.list` files are, the one that wins first, as the
/// specification lists them: the desktop's own and then the shared one in
/// each place of configuration, the user's first, then those of the
/// places of data, under `applications`.
#[must_use]
pub fn lists(config: &[PathBuf], data: &[PathBuf]) -> Vec<PathBuf> {
    let named = ["omorfos-mimeapps.list", "mimeapps.list"];
    let configured = config
        .iter()
        .flat_map(|place| named.map(|name| place.join(name)));
    let data = data
        .iter()
        .flat_map(|place| named.map(|name| place.join("applications").join(name)));
    configured.chain(data).collect()
}

/// The desktop entry that opens a thing of `kind` by default, as the first
/// of `lists`, the text of each file, that names one in its `[Default
/// Applications]` says; or none.
#[must_use]
pub fn default_for(types: &[&str], lists: &[String]) -> Option<String> {
    types.iter().find_map(|kind| {
        lists.iter().find_map(|list| {
            let defaults = section(list, "Default Applications");
            let named = defaults.into_iter().find(|(key, _)| key == kind);
            // Of several, the first; an empty value says nothing.
            let first = named.and_then(|(_, value)| value.split(';').next().map(str::trim));
            first.filter(|entry| !entry.is_empty()).map(str::to_owned)
        })
    })
}

/// The keys and values of the section `name` of `text`, in order.
fn section<'a>(text: &'a str, name: &str) -> Vec<(&'a str, &'a str)> {
    let heading = format!("[{name}]");
    let lines = text.lines().map(str::trim);
    let after = lines.skip_while(|line| *line != heading).skip(1);
    let within = after.take_while(|line| !line.starts_with('['));
    within
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim(), value.trim()))
        .collect()
}

/// Where the desktop entry `entry` is, in the first of the places of
/// `data` that has it under `applications`.
#[must_use]
pub fn entry(entry: &str, data: &[PathBuf]) -> Option<PathBuf> {
    // Only a name of a file, never a way out of the directory.
    let plain = !entry.contains('/') && entry.ends_with(".desktop");
    let places = data.iter().filter(|_| plain);
    let found = places.map(|place| place.join("applications").join(entry));
    found.into_iter().find(|path| path.is_file())
}

/// The name and icon an entry's `text` gives, its name as the first of
/// `locales` that it has it in has it.
#[must_use]
pub fn named(text: &str, locales: &[String]) -> Option<(String, Option<String>)> {
    let fields: Vec<(&str, &str)> = crate::fields(text).collect();
    let pick = |key: &str| {
        let best = fields
            .iter()
            .filter_map(|(field, value)| crate::fit(field, key, locales).map(|fit| (fit, *value)))
            .min_by_key(|(fit, _)| *fit);
        best.map(|(_, value)| value.to_owned())
    };
    pick("Name").map(|name| (name, pick("Icon")))
}

/// The command that opens a thing of `types` by default, as the first of
/// them that has a default in the `mimeapps.list` files of `config` and
/// `data` says, its desktop entry found among `data` and its `Exec` split
/// into the program and its arguments. None if nothing is set for any of
/// them, or the entry it names is not there or runs nothing.
#[must_use]
pub fn command(
    types: &[&str],
    (config, data): (&[PathBuf], &[PathBuf]),
) -> Option<(String, Vec<String>)> {
    let texts: Vec<String> = lists(config, data)
        .iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .collect();
    let named = default_for(types, &texts)?;
    run(&named, data)
}

/// The command the desktop entry `named` runs, found among `data`: its
/// `Exec`, split into the program and its arguments. None if it is not
/// there or runs nothing.
fn run(named: &str, data: &[PathBuf]) -> Option<(String, Vec<String>)> {
    let text = entry(named, data).and_then(|path| std::fs::read_to_string(path).ok())?;
    let mut fields = crate::fields(&text);
    let exec = fields.find_map(|(field, value)| (field == "Exec").then_some(value))?;
    let mut words = crate::arguments(exec).into_iter();
    words.next().map(|program| (program, words.collect()))
}

/// The desktop entries a `xdg-terminals.list` names, in its order, as
/// `xdg-terminal-exec` reads one: an entry to a line, any action after a
/// `:` left aside, a `+` before one taken off, and lines that are empty, a
/// comment after `#`, or one left out with a `-` passed over.
#[must_use]
pub fn terminals(list: &str) -> Vec<&str> {
    let lines = list.lines().map(str::trim);
    let named = lines.filter(|line| !line.is_empty() && !line.starts_with(['#', '-']));
    let named = named.map(|line| line.trim_start_matches('+'));
    named
        .map(|line| line.split_once(':').map_or(line, |(entry, _)| entry).trim())
        .collect()
}

/// The terminal to open, as `xdg-terminal-exec` chooses it: the first
/// desktop entry the `xdg-terminals.list` files of `config` name, in each
/// place omorfos's own, `omorfos-xdg-terminals.list`, before the one every
/// desktop reads, that is among `data`, run as its `Exec` says; or, if
/// none is named or none named is there, `fallback`, the program and its
/// arguments.
#[must_use]
pub fn terminal(
    (config, data): (&[PathBuf], &[PathBuf]),
    (program, arguments): (&str, &[String]),
) -> (String, Vec<String>) {
    let named = ["omorfos-xdg-terminals.list", "xdg-terminals.list"];
    let lists = config
        .iter()
        .flat_map(|place| named.map(|name| place.join(name)));
    let texts: Vec<String> = lists
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .collect();
    let mut entries = texts.iter().flat_map(|text| terminals(text));
    let found = entries.find_map(|entry| run(entry, data));
    found.unwrap_or_else(|| (program.to_owned(), arguments.to_vec()))
}

/// The places of configuration, as the specification has them: the user's
/// first, from `XDG_CONFIG_HOME` or `HOME`, then those of
/// `XDG_CONFIG_DIRS`, or `/etc/xdg`.
#[must_use]
pub fn configuration(
    config_home: Option<&Path>,
    home: Option<&Path>,
    config_dirs: Option<&str>,
) -> Vec<PathBuf> {
    let own = config_home
        .map(Path::to_owned)
        .or_else(|| home.map(|home| home.join(".config")));
    let shared = config_dirs
        .filter(|dirs| !dirs.is_empty())
        .unwrap_or("/etc/xdg");
    let shared = shared
        .split(':')
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from);
    own.into_iter().chain(shared).collect()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn the_default_is_the_first_list_that_names_one_for_the_first_type_that_has_one() {
        let mine = "[Added Associations]\ntext/html=nope.desktop\n\n[Default Applications]\nimage/png=\nx-scheme-handler/mailto=thunderbird.desktop;geary.desktop;\n";
        let system = "[Default Applications]\ntext/html=firefox.desktop\nimage/jpeg = loupe.desktop\ninode/directory=nautilus.desktop\n[Removed Associations]\nimage/png=eog.desktop\n";
        let lists = [mine.to_owned(), system.to_owned()];
        let web = default_for(KINDS[0].1, &lists);
        assert_eq!(web.as_deref(), Some("firefox.desktop"), "by text/html");
        let mail = default_for(KINDS[2].1, &lists);
        assert_eq!(mail.as_deref(), Some("thunderbird.desktop"), "the first");
        let pictures = default_for(KINDS[3].1, &lists);
        assert_eq!(
            pictures.as_deref(),
            Some("loupe.desktop"),
            "png says nothing"
        );
        assert_eq!(default_for(KINDS[4].1, &lists), None);
        assert_eq!(default_for(KINDS[1].1, &[]), None);
    }

    #[test]
    fn the_lists_are_looked_for_as_the_specification_orders_them() {
        let config = [PathBuf::from("/home/u/.config"), PathBuf::from("/etc/xdg")];
        let data = [PathBuf::from("/usr/share")];
        let found: Vec<String> = lists(&config, &data)
            .iter()
            .map(|path| path.display().to_string())
            .collect();
        assert_eq!(
            found,
            [
                "/home/u/.config/omorfos-mimeapps.list",
                "/home/u/.config/mimeapps.list",
                "/etc/xdg/omorfos-mimeapps.list",
                "/etc/xdg/mimeapps.list",
                "/usr/share/applications/omorfos-mimeapps.list",
                "/usr/share/applications/mimeapps.list",
            ]
        );
        let home = Path::new("/home/u");
        assert_eq!(
            configuration(None, Some(home), None),
            [PathBuf::from("/home/u/.config"), PathBuf::from("/etc/xdg")]
        );
        assert_eq!(
            configuration(Some(Path::new("/c")), None, Some("/a::/b")),
            [
                PathBuf::from("/c"),
                PathBuf::from("/a"),
                PathBuf::from("/b")
            ]
        );
        assert_eq!(
            configuration(None, None, Some("")),
            [PathBuf::from("/etc/xdg")]
        );
    }

    #[test]
    fn a_list_of_terminals_names_one_to_a_line_leaving_aside_comments_actions_and_those_left_out() {
        let list = "# mine\n\n  kitty.desktop  \n-foot.desktop\n+alacritty.desktop:new-window\n";
        assert_eq!(terminals(list), ["kitty.desktop", "alacritty.desktop"]);
        assert_eq!(terminals(""), Vec::<&str>::new());
    }

    /// A place of data with an entry for each of `entries` that runs the
    /// program of its name, with `--here`.
    fn installed(entries: &[&str]) -> tempfile::TempDir {
        let data = tempfile::TempDir::new().unwrap();
        let applications = data.path().join("applications");
        fs::create_dir(&applications).unwrap();
        for name in entries {
            let entry = format!("[Desktop Entry]\nName={name}\nExec={name} --here\n");
            fs::write(applications.join(format!("{name}.desktop")), entry).unwrap();
        }
        data
    }

    #[test]
    fn the_terminal_is_the_first_named_that_is_there_omorfoss_own_list_first_in_each_place() {
        let (mine, system) = (
            tempfile::TempDir::new().unwrap(),
            tempfile::TempDir::new().unwrap(),
        );
        let data = installed(&["kitty", "alacritty", "wezterm"]);
        let config = [mine.path().to_owned(), system.path().to_owned()];
        let data = [data.path().to_owned()];
        let foot = ("foot", &[][..]);
        let chosen = || terminal((&config, &data), foot);
        let place = |dir: &tempfile::TempDir, name: &str, list: &str| {
            fs::write(dir.path().join(name), list).unwrap();
        };

        // Nothing named: the fallback.
        assert_eq!(chosen(), ("foot".to_owned(), Vec::new()));
        // The system's list names one.
        place(&system, "xdg-terminals.list", "wezterm.desktop\n");
        assert_eq!(chosen().0, "wezterm");
        // One not there is passed over for the next that is.
        place(&mine, "xdg-terminals.list", "gone.desktop\nkitty.desktop\n");
        assert_eq!(chosen(), ("kitty".to_owned(), vec!["--here".to_owned()]));
        // omorfos's own before the one every desktop reads.
        place(&mine, "omorfos-xdg-terminals.list", "alacritty.desktop\n");
        assert_eq!(chosen().0, "alacritty");
        // And with none there of all those named, the fallback.
        let nothing = [tempfile::TempDir::new().unwrap().path().to_owned()];
        assert_eq!(terminal((&config, &nothing), foot).0, "foot");
    }

    #[test]
    fn the_command_of_a_default_is_the_exec_of_its_entry_without_what_files_would_fill() {
        let config = tempfile::TempDir::new().unwrap();
        let data = tempfile::TempDir::new().unwrap();
        let applications = data.path().join("applications");
        fs::create_dir_all(&applications).unwrap();
        let list = "[Default Applications]\nx-scheme-handler/https=browser.desktop\n\
                    text/plain=gone.desktop\ninode/directory=bare.desktop\n\
                    image/png=empty.desktop\n";
        fs::write(config.path().join("mimeapps.list"), list).unwrap();
        let browser = "[Desktop Entry]\nName=Browser\nExec=firefox --new-window %u\n";
        fs::write(applications.join("browser.desktop"), browser).unwrap();
        fs::write(
            applications.join("bare.desktop"),
            "[Desktop Entry]\nName=Bare\n",
        )
        .unwrap();
        fs::write(
            applications.join("empty.desktop"),
            "[Desktop Entry]\nExec=%u\n",
        )
        .unwrap();
        let places = ([config.path().to_owned()], [data.path().to_owned()]);
        let command = |types: &[&str]| command(types, (&places.0, &places.1));
        assert_eq!(
            command(KINDS[0].1),
            Some(("firefox".to_owned(), vec!["--new-window".to_owned()]))
        );
        assert_eq!(command(&["text/plain"]), None, "an entry that is not there");
        assert_eq!(command(&["inode/directory"]), None, "an entry without Exec");
        assert_eq!(command(&["image/png"]), None, "nothing left to run");
        assert_eq!(command(&["audio/flac"]), None, "nothing set");
    }

    #[test]
    fn an_entry_is_found_by_its_name_alone_and_read_for_its_name_and_icon() {
        let first = tempfile::TempDir::new().unwrap();
        let second = tempfile::TempDir::new().unwrap();
        let there = second.path().join("applications");
        fs::create_dir_all(&there).unwrap();
        fs::write(there.join("loupe.desktop"), "").unwrap();
        let data = [first.path().to_owned(), second.path().to_owned()];
        assert_eq!(
            entry("loupe.desktop", &data),
            Some(there.join("loupe.desktop"))
        );
        assert_eq!(entry("missing.desktop", &data), None);
        assert_eq!(entry("../applications/loupe.desktop", &data), None);
        assert_eq!(entry("loupe", &data), None, "not an entry");

        let text = "[Desktop Entry]\nName=Image Viewer\nName[pl]=Przeglądarka obrazów\nIcon=org.gnome.Loupe\n";
        let english = named(text, &[]).unwrap();
        assert_eq!(
            english,
            (
                "Image Viewer".to_owned(),
                Some("org.gnome.Loupe".to_owned())
            )
        );
        let polish = named(text, &["pl".to_owned()]).unwrap();
        assert_eq!(polish.0, "Przeglądarka obrazów");
        assert_eq!(named("[Desktop Entry]\nIcon=x\n", &[]), None, "nameless");
        let iconless = named("[Desktop Entry]\nName=X\n", &[]).unwrap();
        assert_eq!(iconless.1, None);
    }
}
