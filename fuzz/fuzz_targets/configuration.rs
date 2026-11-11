//! The configuration's files, read as the compositor reads them: a byte
//! saying which file, then what is in it, the others left out; loaded, and
//! loaded again over what that made, as a reload is. Nothing in a file may
//! make loading panic, loop or grow without bound: what is wrong is said,
//! and what was there before is kept.

#![no_main]

use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;

use jiff::Timestamp;
use jiff::tz::TimeZone;
use libfuzzer_sys::fuzz_target;
use omorfos_config::{Config, FILES};

/// A directory of this process's own, so that jobs run side by side do
/// not read each other's files.
static PLACE: LazyLock<PathBuf> = LazyLock::new(|| {
    let place = std::env::temp_dir().join(format!("omorfos-fuzz-{}", std::process::id()));
    fs::create_dir_all(&place).expect("a directory to load from");
    place
});

/// What is loaded over at first, made once: it is the built-in theme, whose
/// stylesheet takes longer to read than most files given.
static BUILT_IN: LazyLock<Config> = LazyLock::new(Config::default);

fuzz_target!(|data: &[u8]| {
    let Some((&which, text)) = data.split_first() else {
        return;
    };
    for file in FILES {
        let _ = fs::remove_file(PLACE.join(file));
    }
    let file = FILES[usize::from(which) % FILES.len()];
    fs::write(PLACE.join(file), text).expect("the file to load");
    // A moment of its own, so that a run is the same run again whatever
    // the time of day.
    let now = Timestamp::from_second(1_800_000_000)
        .expect("a moment")
        .to_zoned(TimeZone::UTC);
    let (loaded, _) = Config::load_at(&PLACE, &BUILT_IN, &now);
    let _ = Config::load_at(&PLACE, &loaded, &now);
});
