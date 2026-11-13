//! Keeps the manual pages in `man/` made from what each program says of
//! itself, as `--help` says it, so that the two cannot disagree.
//! `omorfosctl`'s is kept by its own tests, as it has no library to be
//! reached through.
//!
//! Make them again after changing what a program says of itself with:
//!
//! ```sh
//! OMORFOS_BLESS=1 cargo test --workspace --all-features manual_page
//! ```

use std::path::Path;
use std::{env, fs};

use omorfos_ipc::help::About;
use omorfos_ipc::manual;

/// The repository's `man/`.
fn pages() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../man")
}

/// What each program reached from here says of itself.
const ABOUTS: &[About] = &[
    omorfos::cli::ABOUT,
    omorfos_hold::ABOUT,
    omorfos_relay::ABOUT,
    omorfos_panel::ABOUT,
    omorfos_launcher::ABOUT,
    omorfos_notifications::ABOUT,
    omorfos_screenshot::ABOUT,
    omorfos_clipboard::ABOUT,
    omorfos_recording::ABOUT,
    omorfos_session::ABOUT,
    omorfos_ask::ABOUT,
    omorfos_ask::PINENTRY,
    omorfos_wallpaper::ABOUT,
    omorfos_lock::ABOUT,
    xdg_desktop_portal_omorfos::ABOUT,
];

#[test]
fn every_manual_page_is_made_from_what_its_program_says() {
    let bless = env::var_os("OMORFOS_BLESS").is_some();
    for about in ABOUTS {
        let path = pages().join(format!("{}.1", about.name));
        let kept = manual::kept(&path, &manual::page(about), bless).unwrap();
        assert!(
            kept,
            "{} is out of date; make it again with \
             `OMORFOS_BLESS=1 cargo test --workspace --all-features manual_page`",
            path.display(),
        );
    }
}

#[test]
fn every_page_a_manual_page_points_to_is_there() {
    let mut pointed = 0;
    for entry in fs::read_dir(pages()).unwrap() {
        let path = entry.unwrap().path();
        let page = fs::read_to_string(&path).unwrap();
        // Each is written `\fBname\fR(section)`.
        for named in page.split("\\fB").skip(1) {
            let Some((name, rest)) = named.split_once("\\fR(") else {
                continue;
            };
            let section = rest.split(')').next().unwrap_or_default();
            let name = name.replace("\\-", "-");
            let file = pages().join(format!("{name}.{section}"));
            assert!(
                file.is_file(),
                "{} points to {name}({section})",
                path.display()
            );
            pointed += 1;
        }
    }
    assert!(pointed > 100, "{pointed}");
}
