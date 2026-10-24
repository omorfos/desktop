//! The `omorfos-brand` executable, run as the Makefile runs it. Drawing
//! everything takes a while, so only what it does without drawing is
//! tried here; what it draws is tried with the library.

use std::process::Command;

#[test]
fn asked_nothing_it_says_how_it_is_used() {
    let ran = Command::new(env!("CARGO_BIN_EXE_omorfos-brand"))
        .output()
        .unwrap();
    assert_eq!(ran.status.code(), Some(2));
    assert_eq!(ran.stderr, b"usage: omorfos-brand OUT\n");
}

#[test]
fn where_nothing_can_be_written_it_says_so() {
    let ran = Command::new(env!("CARGO_BIN_EXE_omorfos-brand"))
        .arg("/dev/null/brand")
        .output()
        .unwrap();
    assert_eq!(ran.status.code(), Some(1));
    assert!(
        String::from_utf8(ran.stderr)
            .unwrap()
            .starts_with("omorfos-brand: ")
    );
}
