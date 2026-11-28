//! Keeps the files the coverage gate leaves out, a regular expression in
//! `.cargo/config.toml`, the same as those ADR 2 lists, each with its
//! reason: a file left out with none, or one listed that is no longer
//! there or no longer left out, fails here, where nothing said so before.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The regular expression the coverage gate leaves files out by, as cargo
/// reads it from the alias.
fn left_out() -> Regex {
    let config = fs::read_to_string(repository().join(".cargo/config.toml")).unwrap();
    let flag = config
        .lines()
        .find_map(|line| {
            let line = line.trim().trim_end_matches(',').trim_matches('"');
            line.strip_prefix("--ignore-filename-regex=")
        })
        .unwrap();
    // As TOML has it: a backslash written twice is one.
    Regex::new(&flag.replace("\\\\", "\\")).unwrap()
}

/// The files ADR 2's table lists as left out, each with a reason.
fn listed() -> Vec<(String, String)> {
    let adr = repository().join("docs/adr/0002-coverage-policy.md");
    let text = fs::read_to_string(adr).unwrap();
    text.lines()
        .filter_map(|line| line.strip_prefix("| `"))
        .filter_map(|rest| rest.split_once("` | "))
        .map(|(path, reason)| (path.to_owned(), reason.trim_end_matches(" |").to_owned()))
        .collect()
}

/// Every Rust file under `crates/`, as a path from the repository.
fn sources(directory: &Path, found: &mut Vec<String>) {
    for entry in fs::read_dir(directory).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let from = path.strip_prefix(repository()).unwrap();
            found.push(from.to_string_lossy().into_owned());
        }
    }
}

#[test]
fn the_files_the_coverage_gate_leaves_out_are_those_adr_2_gives_a_reason_for() {
    let listed = listed();
    assert!(listed.len() > 3, "only {} listed", listed.len());
    for (path, reason) in &listed {
        assert!(
            repository().join(path).is_file(),
            "{path} is listed but not there"
        );
        assert!(reason.len() > 40, "{path} has no reason worth the name");
    }
    let mut found = Vec::new();
    sources(&repository().join("crates"), &mut found);
    let left_out = left_out();
    let mut out: Vec<&str> = found
        .iter()
        .map(String::as_str)
        .filter(|path| left_out.is_match(path))
        .collect();
    out.sort_unstable();
    let mut paths: Vec<&str> = listed.iter().map(|(path, _)| path.as_str()).collect();
    paths.sort_unstable();
    assert_eq!(out, paths, "left out, against listed with a reason");
}
