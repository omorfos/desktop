//! A record of every look of the built-in theme, kept while the theme's
//! syntax changes under it (ADR 110): it may gain a line for a part added,
//! and never changes one. Written again with
//! `OMORFOS_BLESS=1 cargo test -p omorfos-config --test looks`.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::fmt::Write as _;

use omorfos_config::{Accent, Color, Part, Scheme, State, States, Theme};

/// FNV-1a over `text`: a hash whose value does not change from one build
/// to the next, as the standard library's may.
fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The states of `of`, one bit of `combination` each, in their order.
fn combination(of: &[State], combination: usize) -> States {
    of.iter()
        .enumerate()
        .fold(States::NONE, |states, (bit, state)| {
            states.when(*state, combination >> bit & 1 == 1)
        })
}

/// Every look of the built-in theme, a line for each part in each scheme
/// with each accent: its looks in every state, and in every state of the
/// part it is in, its fonts, and how it arrives, hashed.
fn looks_of_the_built_in_theme() -> String {
    let accents = [
        Accent::Cyan,
        Accent::Blue,
        Accent::Teal,
        Accent::Purple,
        Accent::Pink,
        Accent::Orange,
        Accent::Own(Color::rgb(0x12, 0x34, 0x56)),
    ];
    // The states there were when the record was made: what is pressed
    // (ADR 113) is drawn so part by part, and what was is kept the same.
    let kept: Vec<State> = State::ALL
        .into_iter()
        .filter(|state| *state != State::Pressed)
        .collect();
    let every = 1 << kept.len();
    let holders = 1 << Part::HOLDER_STATES.len();
    let mut lines = String::new();
    for scheme in [Scheme::Dark, Scheme::Light] {
        for accent in accents {
            let theme = Theme::built_in(scheme, accent);
            for part in Part::ALL {
                let held: Vec<States> = if part.holder().is_some() {
                    (0..holders)
                        .map(|n| combination(&Part::HOLDER_STATES, n))
                        .collect()
                } else {
                    vec![States::NONE]
                };
                let mut text = format!("{:?}", theme.starting(part));
                for n in 0..every {
                    let states = combination(&kept, n);
                    for holder in &held {
                        let style = theme.style_in(part, states, *holder);
                        write!(text, "{style:?}").unwrap();
                    }
                    write!(text, "{:?}", theme.font(part, states)).unwrap();
                }
                let hash = fnv(&text);
                writeln!(lines, "{scheme:?}-{accent:?} {part:?} {hash:016x}").unwrap();
            }
        }
    }
    lines
}

#[test]
fn the_built_in_looks_are_as_they_were() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/looks.golden");
    let now = looks_of_the_built_in_theme();
    if std::env::var_os("OMORFOS_BLESS").is_some() {
        std::fs::write(path, &now).unwrap();
    }
    let kept = std::fs::read_to_string(path).unwrap();
    let changed: Vec<&str> = now
        .lines()
        .filter(|line| !kept.lines().any(|was| was == *line))
        .collect();
    assert_eq!(changed, [] as [&str; 0], "looks changed, or parts not kept");
}
