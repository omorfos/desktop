//! The marks the documentation shows, kept in the repository so that the
//! README can show them, are the ones the brand draws. After changing how
//! they are drawn, write them again:
//!
//! ```sh
//! OMORFOS_BLESS=1 cargo test -p omorfos-brand --test docs
//! ```

use std::path::Path;
use std::{env, fs};

use omorfos_brand::{Scheme, lockup, mark};

#[test]
fn the_marks_in_the_documentation_are_those_the_brand_draws() {
    let docs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/brand");
    let bless = env::var_os("OMORFOS_BLESS").is_some();
    let drawn = [
        ("mark.svg", mark(Scheme::Dark)),
        ("mark-light.svg", mark(Scheme::Light)),
        ("lockup.svg", lockup(Scheme::Dark)),
        ("lockup-light.svg", lockup(Scheme::Light)),
    ];
    for (name, svg) in drawn {
        let path = docs.join(name);
        if bless {
            fs::create_dir_all(&docs).unwrap();
            fs::write(&path, &svg).unwrap();
        }
        let committed = fs::read_to_string(&path).unwrap_or_default();
        assert!(
            committed == svg,
            "{} is not what the brand draws; write it again with \
             `OMORFOS_BLESS=1 cargo test -p omorfos-brand --test docs`",
            path.display()
        );
    }
}
