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

/// The programs `make install` installs, as the Makefile lists them: each
/// program, and those built beside another.
fn installed() -> (Vec<String>, Vec<String>) {
    let makefile = read("Makefile").replace("\\\n", " ");
    let listed = |key: &str| -> Vec<String> {
        let line = makefile
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{key} = ")));
        let words = line.unwrap_or_default().split_whitespace();
        words
            .filter(|word| !word.starts_with("$("))
            .map(str::to_owned)
            .collect()
    };
    (listed("PROGRAMS"), listed("INSTALLED"))
}

/// Whether `text` names `path` whole, and not only as the start of a
/// longer name, as `omorfos` starts `omorfos-panel`.
fn names(text: &str, path: &str) -> bool {
    text.match_indices(path).any(|(at, _)| {
        let after = text[at + path.len()..].chars().next();
        !after.is_some_and(|after| after.is_ascii_alphanumeric() || after == '-')
    })
}

#[test]
fn every_package_carries_every_program_the_makefile_installs() {
    let (programs, beside) = installed();
    assert!(
        programs.contains(&"omorfos-hold".to_owned()),
        "{programs:?}"
    );
    assert!(
        beside.contains(&"omorfos-pinentry".to_owned()),
        "{beside:?}"
    );
    let packagings = [
        "packaging/aur/PKGBUILD",
        "packaging/package.sh",
        "crates/omorfos/Cargo.toml",
    ];
    for program in programs.iter().chain(&beside) {
        for packaging in packagings {
            assert!(
                names(&read(packaging), &format!("target/release/{program}")),
                "{packaging} carries {program}"
            );
        }
    }
    // Each is built for a release, by its crate, which is named as it is.
    let release = read(".github/workflows/release.yml");
    for program in &programs {
        assert!(
            names(&release, &format!("-p {program}")),
            "the release builds {program}"
        );
    }
}

#[test]
fn every_program_installed_has_its_page_in_man_and_every_package_carries_them() {
    let (programs, beside) = installed();
    let top = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for program in programs.iter().chain(&beside) {
        let page = top.join(format!("man/{program}.1"));
        assert!(page.is_file(), "man/{program}.1 is missing");
    }
    // Each as it installs them, so that a page only taken away again, as
    // `make uninstall` does, is not taken for one installed.
    let installing = [
        (
            "Makefile",
            "install -Dm644 man/*.1 -t \"$(DESTDIR)$(MAN_DIR)/man1/\"",
        ),
        (
            "Makefile",
            "install -Dm644 man/*.5 -t \"$(DESTDIR)$(MAN_DIR)/man5/\"",
        ),
        (
            "packaging/aur/PKGBUILD",
            "install -Dm644 man/*.1 -t \"$pkgdir/usr/share/man/man1/\"",
        ),
        (
            "packaging/aur/PKGBUILD",
            "install -Dm644 man/*.5 -t \"$pkgdir/usr/share/man/man5/\"",
        ),
        ("packaging/package.sh", "cp man/*.1 man/*.5 "),
        (
            "crates/omorfos/Cargo.toml",
            "[\"../../man/*.1\", \"usr/share/man/man1/\", \"644\"]",
        ),
        (
            "crates/omorfos/Cargo.toml",
            "[\"../../man/*.5\", \"usr/share/man/man5/\", \"644\"]",
        ),
    ];
    for (packaging, line) in installing {
        assert!(read(packaging).contains(line), "{packaging} carries {line}");
    }
}

#[test]
fn every_package_carries_every_page_of_the_guide() {
    let guide = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/guide");
    assert!(guide.join("README.md").is_file(), "the guide begins there");
    let installing = [
        (
            "Makefile",
            "install -Dm644 docs/guide/*.md -t \"$(DESTDIR)$(DOC_DIR)/guide/\"",
        ),
        (
            "packaging/aur/PKGBUILD",
            "install -Dm644 docs/guide/*.md -t \"$pkgdir/usr/share/doc/$pkgname/guide/\"",
        ),
        (
            "packaging/package.sh",
            "cp -r docs/guide \"${stage}/${name}/\"",
        ),
        (
            "crates/omorfos/Cargo.toml",
            "[\"../../docs/guide/*.md\", \"usr/share/doc/omorfos/guide/\", \"644\"]",
        ),
    ];
    for (packaging, line) in installing {
        assert!(read(packaging).contains(line), "{packaging} carries {line}");
    }
}

#[test]
fn no_two_files_the_tarball_copies_side_by_side_have_one_name() {
    // `cp` refuses a second file of a name it has just copied, which
    // failed the tarball when `omorfos-lock` was both a program and the
    // PAM service of the same name.
    let script = read("packaging/package.sh").replace("\\\n", " ");
    let flat = "\"${stage}/${name}/\"";
    let copies = script.lines().filter(|line| {
        let words: Vec<&str> = line.split_whitespace().collect();
        words.first() == Some(&"cp") && words.last() == Some(&flat)
    });
    let mut names: Vec<&str> = Vec::new();
    for copy in copies {
        let words: Vec<&str> = copy.split_whitespace().collect();
        let sources = words[1..words.len() - 1].iter();
        let sources = sources.filter(|word| !word.starts_with('-'));
        names.extend(sources.map(|source| source.rsplit('/').next().unwrap()));
    }
    assert!(names.contains(&"omorfos-lock"), "{names:?}");
    let mut sorted = names.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), names.len(), "a name twice among {names:?}");
}

#[test]
fn a_program_is_named_whole_and_not_by_the_start_of_another() {
    assert!(names(
        "cp target/release/omorfos x",
        "target/release/omorfos"
    ));
    assert!(names(
        "[\"target/release/omorfos\", ",
        "target/release/omorfos"
    ));
    assert!(!names(
        "target/release/omorfos-panel",
        "target/release/omorfos"
    ));
    assert!(names("target/release/omorfos", "target/release/omorfos"));
}
