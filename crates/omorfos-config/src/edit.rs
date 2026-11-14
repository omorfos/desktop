//! Changing a value in `config.yaml` as a person would, keeping the rest
//! of the file as it is written: its comments, its order and its blank
//! lines (ADR 81).
//!
//! A program that offers a choice, as the welcome card does the wallpaper
//! and the scheme, writes it here, and the compositor and the components
//! take it up as they take up any change to the file. What cannot be
//! changed without guessing at what the person meant, as a value given by
//! an anchor, or a mapping written on one line that a key would have to be
//! added to, is refused, and the file left alone.

use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::Path;

use granit_parser::{Event, Parser, ScalarStyle, Span, StructureStyle};

use crate::config::{GENERAL, parse, read};
use crate::general::General;

/// A value to set: one word, or a mapping of words written on one line,
/// as `{ dark: a, light: b }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value<'a> {
    /// A single value, as `dark`.
    Scalar(&'a str),
    /// A mapping of keys to single values.
    Pairs(&'a [(&'a str, &'a str)]),
}

/// Why a file was left as it was, in English, as the configuration's own
/// diagnostics are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused(pub String);

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Sets each of `changes`, a key's path and its value, in the `config.yaml`
/// of `directory`, making the file if there is none. Nothing is written
/// unless every change can be made, the file read as configuration as it
/// was, and what would be written read as configuration too.
///
/// # Errors
///
/// Returns why the file was left as it was.
pub fn change(directory: &Path, changes: &[(&[&str], Value<'_>)]) -> Result<(), Refused> {
    let path = directory.join(GENERAL);
    let text = read(&path).map_err(|diagnostic| Refused(diagnostic.message))?;
    parse::<General>(&path, &text).map_err(|diagnostic| {
        Refused(format!(
            "it has a mistake to mend first: {}",
            diagnostic.message
        ))
    })?;
    let mut after = text.clone();
    for (key, value) in changes {
        after = set(&after, key, *value)?;
    }
    parse::<General>(&path, &after).map_err(|diagnostic| {
        Refused(format!(
            "the change would not be read: {}",
            diagnostic.message
        ))
    })?;
    if after == text {
        return Ok(());
    }
    written(&path, &after).map_err(|error| Refused(error.to_string()))
}

/// Writes `text` to `path` whole or not at all: to a file beside it, then
/// put in its place, as it was let read and written. Through a link, it is
/// the file linked to that is written, so that the link stays one.
fn written(path: &Path, text: &str) -> std::io::Result<()> {
    let target = fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());
    let folder = target.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(folder)?;
    let beside = folder.join(format!(".{GENERAL}.{}", std::process::id()));
    let kept = fs::metadata(&target).ok().map(|was| was.permissions());
    let mut file = fs::File::create(&beside)?;
    kept.map_or(Ok(()), |permissions| file.set_permissions(permissions))
        .and_then(|()| file.write_all(text.as_bytes()))
        .and_then(|()| file.sync_all())
        .and_then(|()| fs::rename(&beside, &target))
}

/// `text`, a YAML document, with `value` at the key `path`, and the rest
/// as it was.
///
/// # Errors
///
/// Returns why it could not be changed without guessing.
pub fn set(text: &str, path: &[&str], value: Value<'_>) -> Result<String, Refused> {
    let root = tree(text)?;
    // A mapping written a key to a line takes pairs a key at a time, where
    // each is.
    if let (Value::Pairs(pairs), Some(Node::Mapping { flow: false, .. })) =
        (value, root.as_ref().and_then(|root| at(root, path)))
    {
        let mut changed = text.to_owned();
        for (key, word) in pairs {
            let mut below = path.to_vec();
            below.push(key);
            changed = set(&changed, &below, Value::Scalar(word))?;
        }
        return Ok(changed);
    }
    let ((from, to), put) = match &root {
        None => ((text.len(), text.len()), fresh(text, path, value)),
        Some(root) => change_in(text, root, path, value)?,
    };
    let mut changed = text.to_owned();
    changed.replace_range(from..to, &put);
    if holds(&changed, path, value)? {
        Ok(changed)
    } else {
        Err(Refused("the change could not be made safely".to_owned()))
    }
}

/// Whether `text` has `value` at `path`.
fn holds(text: &str, path: &[&str], value: Value<'_>) -> Result<bool, Refused> {
    let root = tree(text)?;
    let word_at = |path: &[&str]| match root.as_ref().and_then(|root| at(root, path)) {
        Some(Node::Scalar { value, .. }) => Some(value.clone()),
        _ => None,
    };
    Ok(match value {
        Value::Scalar(word) => word_at(path).as_deref() == Some(word),
        Value::Pairs(pairs) => pairs.iter().all(|(key, word)| {
            let mut below = path.to_vec();
            below.push(key);
            word_at(&below).as_deref() == Some(*word)
        }),
    })
}

/// What a document's node is, as far as a change needs to know.
#[derive(Debug)]
enum Node {
    /// A single value written plainly or in quotes: where, and what.
    Scalar { span: Span, value: String },
    /// A value left empty, as `key:` with nothing after it.
    Empty,
    /// A mapping, on one line or a key to a line: where it is written,
    /// and its entries.
    Mapping {
        flow: bool,
        span: (usize, usize),
        entries: Vec<Entry>,
    },
    /// What a change is not to touch: a list, a block of lines, or a node
    /// with an anchor, a tag or a key that is not a word; and where it is.
    Other { span: (usize, usize) },
}

/// A key of a mapping and its value.
#[derive(Debug)]
struct Entry {
    key: String,
    key_span: Span,
    value: Node,
}

/// What `node` reads next: a node, or the end of the mapping or list it
/// is in, where that end is, or of everything.
enum Read {
    Node(Node),
    End(usize),
}

impl Read {
    /// The node read, if it was one.
    fn node(self) -> Option<Node> {
        match self {
            Self::Node(node) => Some(node),
            Self::End(_) => None,
        }
    }
}

/// The document's root node, or nothing for a document with none.
fn tree(text: &str) -> Result<Option<Node>, Refused> {
    let mut events = Vec::new();
    for next in Parser::new_from_str(text) {
        let (event, span) = next.map_err(|error| Refused(format!("it cannot be read: {error}")))?;
        let structural = matches!(
            event,
            Event::StreamStart | Event::StreamEnd | Event::DocumentEnd
        );
        if !structural {
            events.push((event, span));
        }
    }
    let documents = events
        .iter()
        .filter(|(event, _)| matches!(event, Event::DocumentStart(..)));
    if documents.count() > 1 {
        return Err(Refused("it has more than one document".to_owned()));
    }
    let mut events = events
        .into_iter()
        .filter(|(event, _)| !matches!(event, Event::DocumentStart(..)));
    Ok(node(&mut events)
        .node()
        .filter(|root| !matches!(root, Node::Empty)))
}

/// The node `events` go on with, and all of it.
fn node<'a>(events: &mut impl Iterator<Item = (Event<'a>, Span)>) -> Read {
    let Some((event, span)) = events.next() else {
        return Read::End(0);
    };
    let whole = (span.start.index(), span.end.index());
    Read::Node(match event {
        Event::Scalar(value, style, anchor, tag) => {
            let plain = matches!(style, ScalarStyle::Plain);
            let quoted = matches!(style, ScalarStyle::SingleQuoted | ScalarStyle::DoubleQuoted);
            if anchor != 0 || tag.is_some() {
                Node::Other { span: whole }
            } else if plain && whole.0 == whole.1 {
                Node::Empty
            } else if plain || quoted {
                Node::Scalar {
                    span,
                    value: value.into_owned(),
                }
            } else {
                Node::Other { span: whole }
            }
        }
        Event::MappingStart(style, anchor, tag) => {
            let (entries, end, words) = mapping(events);
            let span = (whole.0, end);
            if anchor == 0 && tag.is_none() && words {
                Node::Mapping {
                    flow: matches!(style, StructureStyle::Flow),
                    span,
                    entries,
                }
            } else {
                Node::Other { span }
            }
        }
        Event::SequenceStart(..) => {
            // A list ends, as far as what follows it is concerned, where its
            // last item does.
            let mut end = whole.1;
            while let Some(item) = node(events).node() {
                end = last(&item).unwrap_or(end);
            }
            Node::Other {
                span: (whole.0, end),
            }
        }
        Event::MappingEnd | Event::SequenceEnd => return Read::End(whole.1),
        _ => Node::Other { span: whole },
    })
}

/// A mapping's entries, read up to its end: them, where it ends, and
/// whether every key was a word.
fn mapping<'a>(events: &mut impl Iterator<Item = (Event<'a>, Span)>) -> (Vec<Entry>, usize, bool) {
    let mut entries = Vec::new();
    let mut words = true;
    let end = loop {
        let key = match node(events) {
            Read::End(at) => break at,
            Read::Node(key) => key,
        };
        // A key is always followed by its value, if only an empty one.
        let value = node(events).node().unwrap_or(Node::Empty);
        match key {
            Node::Scalar { span, value: key } => entries.push(Entry {
                key,
                key_span: span,
                value,
            }),
            _ => words = false,
        }
    };
    (entries, end, words)
}

/// Where the last of what a node holds ends, if it holds anything.
fn last(node: &Node) -> Option<usize> {
    match node {
        Node::Scalar { span, .. } => Some(span.end.index()),
        Node::Empty => None,
        Node::Mapping {
            flow: true, span, ..
        }
        | Node::Other { span } => Some(span.1),
        Node::Mapping { entries, .. } => entries
            .iter()
            .map(|entry| last(&entry.value).unwrap_or(entry.key_span.end.index()))
            .max(),
    }
}

/// The node at `path` below `node`.
fn at<'a>(node: &'a Node, path: &[&str]) -> Option<&'a Node> {
    let Some((key, rest)) = path.split_first() else {
        return Some(node);
    };
    let Node::Mapping { entries, .. } = node else {
        return None;
    };
    let entry = entries.iter().find(|entry| entry.key == *key)?;
    at(&entry.value, rest)
}

/// What is to be put where, in a document whose root is `node`, to set
/// `value` at `path`: the range replaced, and what replaces it.
fn change_in(
    text: &str,
    node: &Node,
    path: &[&str],
    value: Value<'_>,
) -> Result<((usize, usize), String), Refused> {
    let Node::Mapping {
        flow: false,
        entries,
        ..
    } = node
    else {
        return Err(Refused(
            "a key cannot be added to what it would go in".to_owned(),
        ));
    };
    let Some((key, rest)) = path.split_first() else {
        return Err(Refused("no key was named".to_owned()));
    };
    let mut matching = entries.iter().filter(|entry| entry.key == *key);
    let (Some(entry), None) = (matching.next(), matching.next()) else {
        if entries.iter().any(|entry| entry.key == *key) {
            return Err(Refused(format!("`{key}` is written more than once")));
        }
        // A key not there yet goes after the mapping's last line, as deep
        // as the keys beside it.
        let indent = entries
            .first()
            .and_then(|entry| entry.key_span.indent)
            .unwrap_or(0);
        let (after, broken) = line_after(text, last(node).unwrap_or(text.len()));
        let lines = lines(path, value, indent);
        let lines = if broken { lines } else { format!("\n{lines}") };
        return Ok(((after, after), lines));
    };
    let indent = entry.key_span.indent.unwrap_or(0);
    match (&entry.value, rest.is_empty()) {
        (Node::Empty, empty) => {
            let colon = colon_after(text, &entry.key_span);
            let put = if empty {
                format!(" {}", written_value(value))
            } else {
                format!(
                    "\n{}",
                    lines(rest, value, indent + 2).trim_end_matches('\n')
                )
            };
            Ok(((colon, colon), put))
        }
        (Node::Mapping { flow: false, .. }, false) => change_in(text, &entry.value, rest, value),
        (Node::Scalar { span, .. }, true) => {
            Ok(((span.start.index(), span.end.index()), written_value(value)))
        }
        (
            Node::Mapping {
                flow: true, span, ..
            },
            true,
        ) if matches!(value, Value::Pairs(_)) => Ok((*span, written_value(value))),
        _ => Err(Refused(format!(
            "`{key}` is written in a way this cannot change"
        ))),
    }
}

/// Where the line holding `at` ends, past its line break, and whether it
/// has one.
fn line_after(text: &str, at: usize) -> (usize, bool) {
    text[at..]
        .find('\n')
        .map_or((text.len(), false), |end| (at + end + 1, true))
}

/// Where a key's colon ends.
fn colon_after(text: &str, key: &Span) -> usize {
    let end = key.end.index();
    text[end..].find(':').map_or(end, |colon| end + colon + 1)
}

/// `path` set to `value` as lines of a file, the first `indent` deep.
fn lines(path: &[&str], value: Value<'_>, indent: usize) -> String {
    let mut lines = String::new();
    for (depth, key) in path.iter().enumerate() {
        let room = " ".repeat(indent + depth * 2);
        if depth + 1 == path.len() {
            let _ = writeln!(lines, "{room}{key}: {}", written_value(value));
        } else {
            let _ = writeln!(lines, "{room}{key}:");
        }
    }
    lines
}

/// A document of nothing but comments, or of nothing, with `path` set.
fn fresh(text: &str, path: &[&str], value: Value<'_>) -> String {
    let lines = lines(path, value, 0);
    if text.is_empty() || text.ends_with('\n') {
        lines
    } else {
        format!("\n{lines}")
    }
}

/// A value as it is written.
fn written_value(value: Value<'_>) -> String {
    match value {
        Value::Scalar(word) => word_written(word),
        Value::Pairs(pairs) => {
            let pairs: Vec<String> = pairs
                .iter()
                .map(|(key, word)| format!("{key}: {}", word_written(word)))
                .collect();
            format!("{{ {} }}", pairs.join(", "))
        }
    }
}

/// A word as it is written: as it is where nothing could take it for
/// something else, and in quotes otherwise.
fn word_written(word: &str) -> String {
    let lower = word.to_lowercase();
    let plain = !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_alphanumeric() || "._/~+-".contains(c))
        && !word.starts_with(['-', '.'])
        && word != "~"
        && word.parse::<f64>().is_err()
        && !["true", "false", "null", "yes", "no", "on", "off"].contains(&lower.as_str());
    if plain {
        word.to_owned()
    } else {
        format!("\"{}\"", word.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAIR: &[(&str, &str)] = &[("dark", "/a/x-dark.png"), ("light", "/a/x-light.png")];

    fn scheme(text: &str, word: &str) -> Result<String, Refused> {
        set(text, &["appearance", "scheme"], Value::Scalar(word))
    }

    fn picture(text: &str) -> Result<String, Refused> {
        set(text, &["wallpaper", "picture"], Value::Pairs(PAIR))
    }

    fn refused(result: Result<String, Refused>) -> String {
        result.err().map(|why| why.to_string()).unwrap_or_default()
    }

    #[test]
    fn a_value_written_is_changed_and_nothing_else() {
        let text = "# mine\nappearance:\n  scheme: dark   # night owl\n  accent: teal\n\nkeys: x\n";
        assert_eq!(
            scheme(text, "light"),
            Ok(
                "# mine\nappearance:\n  scheme: light   # night owl\n  accent: teal\n\nkeys: x\n"
                    .to_owned()
            )
        );
        let quoted = "appearance:\n  scheme: 'dark'\n";
        assert_eq!(
            scheme(quoted, "auto"),
            Ok("appearance:\n  scheme: auto\n".to_owned())
        );
    }

    #[test]
    fn a_key_not_there_goes_after_the_last_line_beside_it_as_deep() {
        let text =
            "appearance:\n    accent: teal # mine\n\n# the wallpaper\nwallpaper:\n    fit: fill\n";
        assert_eq!(
            scheme(text, "light"),
            Ok("appearance:\n    accent: teal # mine\n    scheme: light\n\n# the wallpaper\nwallpaper:\n    fit: fill\n"
                .to_owned())
        );
        let nested = "appearance:\n  day:\n    from: '07:00'\n# end\n";
        assert_eq!(
            scheme(nested, "auto"),
            Ok("appearance:\n  day:\n    from: '07:00'\n  scheme: auto\n# end\n".to_owned())
        );
    }

    #[test]
    fn a_key_not_there_goes_after_a_last_value_left_empty_on_one_line_or_listed() {
        assert_eq!(
            scheme("appearance:\n  accent:\nkeys: x\n", "dark"),
            Ok("appearance:\n  accent:\n  scheme: dark\nkeys: x\n".to_owned())
        );
        assert_eq!(
            scheme("appearance:\n  day: { from: '07:00' }\n", "dark"),
            Ok("appearance:\n  day: { from: '07:00' }\n  scheme: dark\n".to_owned())
        );
        assert_eq!(
            scheme("autostart:\n  - foot\n  - [a, b]\n", "dark"),
            Ok("autostart:\n  - foot\n  - [a, b]\nappearance:\n  scheme: dark\n".to_owned())
        );
    }

    #[test]
    fn a_word_that_cannot_be_written_as_one_value_is_refused() {
        // Its line break would be read as a space.
        let said = refused(scheme("appearance:\n  scheme: dark\n", "dark\n   light"));
        assert_eq!(said, "the change could not be made safely");
        let said = refused(scheme("appearance:\n  scheme: dark\n", "dark\nlight"));
        assert!(said.starts_with("it cannot be read"), "{said}");
        assert!(
            !holds(
                "appearance:\n  scheme: dark\n",
                &["appearance"],
                Value::Scalar("x")
            )
            .unwrap()
        );
    }

    #[test]
    fn a_section_not_there_is_added_at_the_end() {
        assert_eq!(
            scheme("keys: x\n", "dark"),
            Ok("keys: x\nappearance:\n  scheme: dark\n".to_owned())
        );
        assert_eq!(
            scheme("keys: x", "dark"),
            Ok("keys: x\nappearance:\n  scheme: dark\n".to_owned()),
            "a file not ended by a line break gets one"
        );
    }

    #[test]
    fn a_file_of_nothing_or_only_comments_is_written_to() {
        assert_eq!(
            scheme("", "dark"),
            Ok("appearance:\n  scheme: dark\n".to_owned())
        );
        assert_eq!(
            scheme("# nothing yet\n", "dark"),
            Ok("# nothing yet\nappearance:\n  scheme: dark\n".to_owned())
        );
        assert_eq!(
            scheme("# nothing yet", "dark"),
            Ok("# nothing yet\nappearance:\n  scheme: dark\n".to_owned())
        );
    }

    #[test]
    fn a_key_left_empty_is_given_its_value() {
        assert_eq!(
            scheme("appearance:\nkeys: x\n", "dark"),
            Ok("appearance:\n  scheme: dark\nkeys: x\n".to_owned())
        );
        assert_eq!(
            scheme("appearance:\n  scheme:\n", "dark"),
            Ok("appearance:\n  scheme: dark\n".to_owned())
        );
    }

    #[test]
    fn a_mapping_is_written_on_one_line_or_set_a_key_at_a_time() {
        assert_eq!(
            picture("wallpaper:\n  picture: ~/hills.jpg\n  fit: fill\n"),
            Ok("wallpaper:\n  picture: { dark: /a/x-dark.png, light: /a/x-light.png }\n  fit: fill\n"
                .to_owned())
        );
        assert_eq!(
            picture("wallpaper: { fit: fill }\n").map_err(|_| ()),
            Err(()),
            "a key is not added to a mapping on one line"
        );
        assert_eq!(
            picture("wallpaper:\n  picture: { dark: ~/n.jpg, light: ~/d.jpg }\n"),
            Ok(
                "wallpaper:\n  picture: { dark: /a/x-dark.png, light: /a/x-light.png }\n"
                    .to_owned()
            )
        );
        assert_eq!(
            picture("wallpaper:\n  picture:\n    dark: ~/n.jpg # night\n    light: ~/d.jpg\n"),
            Ok("wallpaper:\n  picture:\n    dark: /a/x-dark.png # night\n    light: /a/x-light.png\n"
                .to_owned())
        );
        assert_eq!(
            picture("wallpaper:\n  fit: fill\n"),
            Ok("wallpaper:\n  fit: fill\n  picture: { dark: /a/x-dark.png, light: /a/x-light.png }\n"
                .to_owned())
        );
    }

    #[test]
    fn a_word_is_quoted_where_it_would_be_read_as_something_else() {
        for (word, written) in [
            ("dark", "dark"),
            ("~/Pictures/a b.png", "\"~/Pictures/a b.png\""),
            ("~/x.png", "~/x.png"),
            ("~", "\"~\""),
            ("yes", "\"yes\""),
            ("Off", "\"Off\""),
            ("1.5", "\"1.5\""),
            ("-x", "\"-x\""),
            (".hidden", "\".hidden\""),
            ("", "\"\""),
            ("a\"b\\c", "\"a\\\"b\\\\c\""),
            ("#203040", "\"#203040\""),
        ] {
            assert_eq!(word_written(word), written, "{word}");
            let text = scheme("appearance:\n  scheme: dark\n", word);
            assert_eq!(
                text,
                Ok(format!("appearance:\n  scheme: {written}\n")),
                "{word}"
            );
        }
    }

    #[test]
    fn what_cannot_be_changed_without_guessing_is_refused() {
        for (text, why) in [
            ("appearance: { accent: teal }\n", "cannot change"),
            ("appearance: &a\n  scheme: dark\n", "cannot change"),
            ("appearance:\n  scheme: &s dark\n", "cannot change"),
            (
                "base: &b dark\nappearance:\n  scheme: *b\n",
                "cannot change",
            ),
            ("appearance:\n  scheme: !!str dark\n", "cannot change"),
            ("appearance:\n  scheme: |\n    dark\n", "cannot change"),
            ("appearance:\n  scheme: [dark]\n", "cannot change"),
            ("appearance:\n  scheme:\n    a: b\n", "cannot change"),
            ("appearance: dark\n", "cannot change"),
            ("appearance:\n  - scheme\n", "cannot change"),
            (
                "appearance:\n  scheme: dark\nappearance:\n  accent: teal\n",
                "more than once",
            ),
            ("a: 1\n---\nb: 2\n", "more than one document"),
            ("- a\n- b\n", "cannot be added"),
            ("{ a: 1 }\n", "cannot be added"),
            ("appearance:\n  scheme: dark\n  [x]: y\n", "cannot change"),
            ("appearance: [\n", "cannot be read"),
        ] {
            let said = refused(scheme(text, "light"));
            assert!(said.contains(why), "{text:?}: {said}");
        }
        assert_eq!(
            refused(set("a: 1\n", &[], Value::Scalar("x"))),
            "no key was named"
        );
        for pairs in [
            "wallpaper:\n  picture: [a]\n",
            "wallpaper:\n  picture:\n    dark: &a ~/n.jpg\n    light: ~/d.jpg\n",
        ] {
            assert!(refused(picture(pairs)).contains("cannot change"), "{pairs}");
        }
        let one = "appearance:\n  scheme: { a: b }\n";
        assert!(refused(scheme(one, "dark")).contains("cannot change"));
    }

    #[test]
    fn a_change_that_would_not_read_as_set_is_refused() {
        // A key written in quotes is the same key, but not one this finds
        // by its span the way a person wrote it; the change is checked.
        let text = "appearance:\n  \"scheme\": dark\n";
        assert_eq!(
            scheme(text, "light"),
            Ok("appearance:\n  \"scheme\": light\n".to_owned())
        );
    }

    fn folder() -> tempfile::TempDir {
        tempfile::TempDir::new().unwrap()
    }

    #[test]
    fn the_file_is_made_changed_or_left_alone_as_a_whole() {
        let there = folder();
        let file = there.path().join("config.yaml");
        let scheme: &[&str] = &["appearance", "scheme"];
        change(there.path(), &[(scheme, Value::Scalar("light"))]).unwrap();
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "appearance:\n  scheme: light\n"
        );

        let picture: &[&str] = &["wallpaper", "picture"];
        let both = [
            (scheme, Value::Scalar("dark")),
            (picture, Value::Pairs(PAIR)),
        ];
        change(there.path(), &both).unwrap();
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "appearance:\n  scheme: dark\nwallpaper:\n  picture: { dark: /a/x-dark.png, light: /a/x-light.png }\n"
        );
        // Set as it is, it is not written again.
        let before = fs::metadata(&file).unwrap().modified().unwrap();
        change(there.path(), &both).unwrap();
        assert_eq!(fs::metadata(&file).unwrap().modified().unwrap(), before);
        let left: Vec<_> = fs::read_dir(there.path()).unwrap().collect();
        assert_eq!(left.len(), 1, "nothing is left beside it");
    }

    #[test]
    fn a_file_with_a_mistake_or_a_change_it_would_not_take_is_left_alone() {
        let there = folder();
        let file = there.path().join("config.yaml");
        let mistaken = "appearance:\n  scheme: purple\n";
        fs::write(&file, mistaken).unwrap();
        let scheme: &[&str] = &["appearance", "scheme"];
        let said = change(there.path(), &[(scheme, Value::Scalar("dark"))]).unwrap_err();
        assert!(
            said.0.starts_with("it has a mistake to mend first"),
            "{said}"
        );
        assert_eq!(fs::read_to_string(&file).unwrap(), mistaken);

        fs::write(&file, "appearance:\n  scheme: dark\n").unwrap();
        let said = change(there.path(), &[(scheme, Value::Scalar("purple"))]).unwrap_err();
        assert!(said.0.starts_with("the change would not be read"), "{said}");
        let accent: &[&str] = &["appearance", "accent"];
        let refused_by_set = "appearance: { scheme: dark }\n";
        fs::write(&file, refused_by_set).unwrap();
        assert!(change(there.path(), &[(scheme, Value::Scalar("light"))]).is_err());
        assert_eq!(fs::read_to_string(&file).unwrap(), refused_by_set);
        fs::write(&file, "appearance:\n  scheme: dark\n").unwrap();
        let half = [
            (accent, Value::Scalar("teal")),
            (scheme, Value::Pairs(PAIR)),
        ];
        assert!(change(there.path(), &half).is_err());
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "appearance:\n  scheme: dark\n",
            "not half done"
        );
    }

    #[test]
    fn a_file_that_cannot_be_read_or_written_is_said_to_be() {
        use std::os::unix::fs::PermissionsExt as _;
        let there = folder();
        fs::create_dir(there.path().join("config.yaml")).unwrap();
        let scheme: &[&str] = &["appearance", "scheme"];
        assert!(change(there.path(), &[(scheme, Value::Scalar("dark"))]).is_err());

        let shut = folder();
        fs::set_permissions(shut.path(), fs::Permissions::from_mode(0o500)).unwrap();
        let said = change(shut.path(), &[(scheme, Value::Scalar("dark"))]);
        fs::set_permissions(shut.path(), fs::Permissions::from_mode(0o700)).unwrap();
        assert!(
            said.is_err(),
            "a folder that cannot be written to is said to be"
        );

        let blocking = there.path().join("blocking");
        fs::write(&blocking, "").unwrap();
        let said = written(&blocking.join("config.yaml"), "x");
        assert!(
            said.is_err(),
            "a file where its folder would have to be is not made one"
        );
    }

    #[test]
    fn a_link_stays_a_link_and_the_file_it_names_is_changed_as_it_was_let() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};
        let there = folder();
        let mine = there.path().join("dotfiles");
        fs::create_dir(&mine).unwrap();
        let real = mine.join("config.yaml");
        fs::write(&real, "appearance:\n  scheme: dark\n").unwrap();
        fs::set_permissions(&real, fs::Permissions::from_mode(0o600)).unwrap();
        let config = there.path().join("omorfos");
        fs::create_dir(&config).unwrap();
        symlink(&real, config.join("config.yaml")).unwrap();

        let scheme: &[&str] = &["appearance", "scheme"];
        change(&config, &[(scheme, Value::Scalar("light"))]).unwrap();
        assert!(
            fs::symlink_metadata(config.join("config.yaml"))
                .unwrap()
                .is_symlink()
        );
        assert_eq!(
            fs::read_to_string(&real).unwrap(),
            "appearance:\n  scheme: light\n"
        );
        assert_eq!(
            fs::metadata(&real).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
