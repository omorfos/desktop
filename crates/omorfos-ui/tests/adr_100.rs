//! ADR 100's tables, which `tools/scale-survey.sh` works out and holds the
//! decision to, are what `Scale::for_screen` gives: the rule written in
//! awk and the rule built are held to the same rows.

use std::fs;
use std::path::Path;

use omorfos_ui::Scale;

/// The numbers either side of the last "by" in a cell, as "3840 by 2160";
/// nothing where it has none.
fn by(cell: &str) -> Option<(u32, u32)> {
    let words: Vec<&str> = cell
        .split([',', ' '])
        .filter(|word| !word.is_empty())
        .collect();
    let at = words.iter().rposition(|word| *word == "by")?;
    let before = words.get(at.checked_sub(1)?)?.parse().ok()?;
    let after = words.get(at + 1)?.parse().ok()?;
    Some((before, after))
}

#[test]
fn every_screen_in_adr_100_is_given_the_scale_it_says() {
    let adr = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/adr/0100-a-screens-scale-by-how-near-it-is-read.md");
    let adr = fs::read_to_string(adr).unwrap();
    let (mut scale_at, mut screen, mut rows) = (None, (0, 0), 0);
    for line in adr.lines() {
        if !line.starts_with('|') {
            scale_at = None;
            continue;
        }
        let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
        if cells[0] == "Screen" {
            scale_at = cells.iter().position(|cell| *cell == "Scale");
            continue;
        }
        let Some(at) = scale_at.filter(|_| !line.starts_with("|---")) else {
            continue;
        };
        // "the same, in whole centimetres" is the screen of the row above,
        // and a screen that says "nothing" says it is 0 by 0.
        screen = by(cells[0]).unwrap_or(screen);
        let says = by(cells[1]).unwrap_or((0, 0));
        let wanted = Scale::new(cells[at].parse().unwrap());
        assert_eq!(Scale::for_screen(screen, says), wanted, "{line}");
        rows += 1;
    }
    assert_eq!(rows, 46, "the three tables");
}
