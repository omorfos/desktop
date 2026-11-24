//! ADR 3's rule that every program of the session but the compositor
//! starts from the copies the compositor keeps of what last loaded: none
//! opens the configuration without them. The launcher, the screenshot tool
//! and the welcome card once did, each by a line that read as right alone
//! (finding 4 of the review of 24 November 2026), so every program's source
//! is read for one.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

/// What `source`, a file of Rust, opens the configuration with outside its
/// tests and does not go on to start from the copies: each statement with
/// `Store::open` in it and no `starting_from`, from there to its end.
/// `Store::open_in_session`, which finds the copies itself, is not one.
fn opened_without(source: &str) -> Vec<&str> {
    // A file's own tests are its last module, and open what they like.
    let tests = source.rfind("mod tests {").unwrap_or(source.len());
    let opened = source[..tests].split("Store::open(").skip(1);
    let statements = opened.map(|after| after.split(';').next().unwrap_or_default());
    statements
        .filter(|statement| !statement.contains(".starting_from("))
        .collect()
}

/// Every file of Rust in `directory` and the directories in it.
fn sources(directory: &Path) -> Vec<PathBuf> {
    let entries = fs::read_dir(directory).into_iter().flatten().flatten();
    let mut found = Vec::new();
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            found.extend(sources(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
    found
}

/// The files of every program's own source under `crates`, the directory
/// the crates are in: not the compositor's, which keeps the copies, nor
/// this crate's, which is what they are kept by.
fn programs(crates: &Path) -> Vec<PathBuf> {
    let keepers = ["omorfos", "omorfos-config"];
    let entries = fs::read_dir(crates).into_iter().flatten().flatten();
    let others = entries.filter(|entry| !keepers.iter().any(|keeper| entry.file_name() == *keeper));
    others
        .flat_map(|entry| sources(&entry.path().join("src")))
        .collect()
}

/// The crate that `file`, of those [`programs`] found under `crates`, is
/// in: the first part of its path there.
fn crate_of<'a>(crates: &Path, file: &'a Path) -> Option<&'a OsStr> {
    file.strip_prefix(crates).ok()?.iter().next()
}

#[test]
fn what_opens_the_configuration_without_the_copies_is_found() {
    let none = [""; 0];
    assert_eq!(
        opened_without("let store = Store::open(config);"),
        ["config)"]
    );
    // Going on to start from them, on the same line or the next.
    let on = "let store = Store::open(config).starting_from(kept);";
    assert_eq!(opened_without(on), none);
    let next = "let store = Store::open(config)\n    .starting_from(kept);";
    assert_eq!(opened_without(next), none);
    let handed = "Desk::new(session, Store::open(&config).starting_from(kept.as_deref()));";
    assert_eq!(opened_without(handed), none);
    // But not in a statement of its own, which reads as right and may be
    // left out.
    let later = "let store = Store::open(config);\nlet store = store.starting_from(kept);";
    assert_eq!(opened_without(later), ["config)"]);
    // The way in that finds them itself.
    let found = "let store = Store::open_in_session(config, runtime);";
    assert_eq!(opened_without(found), none);
    // A file's tests open what they like.
    let tested = "fn start() {}\n\n#[cfg(test)]\nmod tests {\n    Store::open(directory);\n}\n";
    assert_eq!(opened_without(tested), none);
}

#[test]
fn every_program_but_the_compositor_opens_the_configuration_from_the_kept_copies() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut opening = 0;
    for file in programs(&crates) {
        let source = fs::read_to_string(&file).unwrap();
        opening += usize::from(source.contains("Store::open"));
        assert_eq!(
            opened_without(&source),
            [""; 0],
            "{} opens the configuration without the copies the compositor keeps: open it with \
             `Store::open_in_session`, or go on to `.starting_from(kept)` in the same statement",
            file.display()
        );
    }
    // The programs that open it were read, wherever this is run from.
    assert!(opening >= 5, "only {opening} files that open it found");
}

#[test]
fn the_compositor_and_what_keeps_the_copies_are_not_held_to_starting_from_them() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let read = programs(&crates);
    let of = |name: &str| {
        let named = |file: &&PathBuf| crate_of(&crates, file).is_some_and(|part| part == name);
        read.iter().filter(named).count()
    };
    assert_eq!(of("omorfos"), 0, "the compositor keeps them");
    assert_eq!(of("omorfos-config"), 0, "by this crate");
    assert!(of("omorfos-launcher") > 0, "the launcher is read");
}
