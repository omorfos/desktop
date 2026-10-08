//! Keeps the JSON Schemas committed in `schemas/` in step with the code.
//!
//! Regenerate them after changing the configuration types with:
//!
//! ```sh
//! OMORFOS_BLESS=1 cargo test -p omorfos-config --all-features --test schemas
//! ```
#![cfg(feature = "schema")]

use std::path::Path;
use std::{env, fs};

#[test]
fn committed_schemas_match_the_configuration_types() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas");
    let bless = env::var_os("OMORFOS_BLESS").is_some();
    for (name, generated) in omorfos_config::schema::schemas() {
        let path = directory.join(name);
        if bless {
            fs::create_dir_all(&directory).unwrap();
            fs::write(&path, &generated).unwrap();
        }
        let committed = fs::read_to_string(&path).unwrap_or_default();
        assert!(
            committed == generated,
            "{} is out of date; regenerate it with \
             `OMORFOS_BLESS=1 cargo test -p omorfos-config --all-features --test schemas`",
            path.display(),
        );
    }
}
