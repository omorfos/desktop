//! The packages carry what `CREDITS.md` says they do: the text of each
//! licence it names, in each way omorfos is packaged.

use std::fs;
use std::path::Path;

/// A file of the repository, by its path from the top; nothing if it is
/// not there, which the tests then find wanting.
fn read(path: &str) -> String {
    let top = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    fs::read_to_string(top.join(path)).unwrap_or_default()
}

/// The quoted names of the list `key = [ ... ]` in a TOML file, sorted.
fn listed(file: &str, key: &str) -> Vec<String> {
    let text = read(file);
    let after = text.split_once(&format!("{key} = [")).unwrap_or_default().1;
    let list = after.split_once(']').unwrap_or_default().0;
    let mut names: Vec<String> = list
        .split(',')
        .map(|name| name.trim().trim_matches('"').to_owned())
        .filter(|name| !name.is_empty())
        .collect();
    names.sort();
    names
}

#[test]
fn the_crates_licences_are_gathered_as_those_cargo_deny_allows() {
    let accepted = listed("about.toml", "accepted");
    assert!(accepted.contains(&"MIT".to_owned()), "{accepted:?}");
    assert_eq!(accepted, listed("deny.toml", "allow"));
}

#[test]
fn every_package_carries_the_licences_credits_names_and_the_crates() {
    let credits = read("CREDITS.md");
    let packagings = [
        "Makefile",
        "packaging/aur/PKGBUILD",
        "packaging/package.sh",
        "crates/omorfos/Cargo.toml",
    ];
    for licence in [
        "LICENSE-pictures",
        "LICENSE-space-grotesk",
        "LICENSE-lucide",
        "LICENSE-crates",
    ] {
        assert!(credits.contains(licence), "CREDITS.md names {licence}");
        for packaging in packagings {
            assert!(
                read(packaging).contains(licence),
                "{packaging} carries {licence}"
            );
        }
    }
    // Each is a file of the repository but the crates', which is gathered
    // as omorfos is built.
    let files = [
        "LICENSE",
        "LICENSE-pictures",
        "LICENSE-space-grotesk",
        "LICENSE-lucide",
    ];
    for licence in files {
        assert!(!read(licence).is_empty(), "{licence}");
    }
}
