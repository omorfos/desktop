//! Desktop entries: the small files an application installs to say what
//! it is called and how it is started, read by the launcher to offer it
//! and by the session to start it at login.
//!
//! They are other people's files and are read as such: only so much of
//! each, and whatever does not make sense is passed over. See the Desktop
//! Entry Specification.

use std::fs::File;
use std::io::Read;
use std::path::Path;

/// The most of a desktop entry that is read, in bytes. Real ones are a few
/// kilobytes, most of it translations.
pub const LONGEST: u64 = 256 * 1024;

/// Reads the beginning of a file, if it can be read and is text.
#[must_use]
pub fn read(path: &Path) -> Option<String> {
    let mut text = String::new();
    let file = File::open(path).ok();
    let read = file.map(|file| file.take(LONGEST).read_to_string(&mut text));
    read.and_then(Result::ok).map(|_| text)
}

/// The keys of the main group of an entry and their values, in order,
/// each trimmed. What comes before the group, and the groups after it,
/// such as the actions an application offers, are not part of it.
pub fn fields(entry: &str) -> impl Iterator<Item = (&str, &str)> {
    let mut inside = false;
    entry.lines().map(str::trim).filter_map(move |line| {
        if line.starts_with('[') {
            inside = line == "[Desktop Entry]";
        }
        let field = line.split_once('=').filter(|_| inside);
        field.map(|(key, value)| (key.trim(), value.trim()))
    })
}

/// The items of a list such as `OnlyShowIn`, written apart by `;`.
pub fn list(value: &str) -> impl Iterator<Item = &str> {
    value
        .split(';')
        .map(str::trim)
        .filter(|item| !item.is_empty())
}

/// Splits the `Exec` line of a desktop entry into a program and its
/// arguments, as the specification has it: words apart from one another by
/// spaces, a word in double quotes kept whole with `\` before a character
/// that is meant as itself. A word in single quotes is kept whole too, with
/// nothing special inside them, as `GLib`, and with it the desktops, take
/// what entries in the wild are written with. The places where files to
/// open would go (`%f`, `%U` and the like) are left out, since there are
/// none.
#[must_use]
pub fn arguments(exec: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word: Option<String> = None;
    let (mut quoted, mut single) = (false, false);
    let mut characters = exec.chars();
    while let Some(character) = characters.next() {
        match character {
            '\'' if !quoted => {
                single = !single;
                // Even nothing between quotes is a word.
                word.get_or_insert_default();
            }
            other if single => word.get_or_insert_default().push(other),
            '"' => {
                quoted = !quoted;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_main_group_of_an_entry_counts() {
        let text = "# a comment\n\n[Desktop Entry]\nType = Application\nName = Editor \n\
                    Exec = edit\n\n[Desktop Action new]\nName=New Window\nExec=edit --new\n";
        let read: Vec<_> = fields(text).collect();
        assert_eq!(
            read,
            [
                ("Type", "Application"),
                ("Name", "Editor"),
                ("Exec", "edit")
            ]
        );
        let stray = "Name=Stray\n[Other]\nName=Other\n";
        assert_eq!(fields(stray).count(), 0, "nothing outside the group counts");
    }

    #[test]
    fn a_list_is_its_items_apart_by_semicolons() {
        let items: Vec<_> = list("GNOME; KDE;;Unity;").collect();
        assert_eq!(items, ["GNOME", "KDE", "Unity"]);
        assert_eq!(list("").count(), 0);
    }

    #[test]
    fn a_file_is_read_only_so_far_and_only_if_it_is_text() {
        let directory = tempfile::TempDir::new().unwrap();
        let entry = directory.path().join("a.desktop");
        std::fs::write(&entry, "[Desktop Entry]\n").unwrap();
        assert_eq!(read(&entry).as_deref(), Some("[Desktop Entry]\n"));
        let long = directory.path().join("long.desktop");
        let size = usize::try_from(LONGEST).unwrap() + 10;
        std::fs::write(&long, "x".repeat(size)).unwrap();
        assert_eq!(read(&long).map(|text| text.len() as u64), Some(LONGEST));
        let binary = directory.path().join("binary.desktop");
        std::fs::write(&binary, [0xff, 0xfe]).unwrap();
        assert_eq!(read(&binary), None);
        assert_eq!(read(&directory.path().join("nowhere")), None);
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
    fn a_word_in_single_quotes_is_kept_whole_as_glib_keeps_it() {
        // As the system's `im-launch.desktop` says it: the specification has
        // double quotes only, and GLib, and with it the desktops, take
        // single quotes too.
        assert_eq!(
            arguments("sh -c 'IM_CONFIG_CHECK_ENV=1 im-launch true'"),
            ["sh", "-c", "IM_CONFIG_CHECK_ENV=1 im-launch true"]
        );
        // Nothing is special inside them, a backslash or a double quote.
        assert_eq!(arguments(r#"say 'a "b" \c'"#), ["say", r#"a "b" \c"#]);
        // A single quote inside double quotes is itself.
        assert_eq!(arguments(r#"say "it's""#), ["say", "it's"]);
        assert_eq!(arguments("say ''"), ["say"]);
    }

    #[test]
    fn places_for_files_to_open_are_left_out_of_a_command() {
        assert_eq!(arguments("view %f"), ["view"]);
        assert_eq!(arguments("view %U --all"), ["view", "--all"]);
        assert_eq!(arguments("view --file=%f"), ["view", "--file="]);
        assert_eq!(arguments("discount 50%% off"), ["discount", "50%", "off"]);
        assert_eq!(arguments("odd %"), ["odd"]);
    }
}
