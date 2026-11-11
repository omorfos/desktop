//! What crosses omorfos's own sockets, read as each side reads it: a byte
//! saying which, then the text. A request on the compositor's socket, with
//! the action it may carry, is any program's to send; a reply is what
//! `omorfosctl` reads; the rest is what the holder, the relay and the
//! compositor say to each other. Nothing may panic, and what is read and
//! written again reads as it did.

#![no_main]

use libfuzzer_sys::fuzz_target;
use omorfos_config::Action;
use omorfos_ipc::hold::{Heard, Told, Word};
use omorfos_ipc::relay::Hand;
use omorfos_ipc::seat::{Asked, Seated};
use omorfos_ipc::{Reply, Request, decode};
use serde::Deserialize;

fuzz_target!(|data: &[u8]| {
    let Some((&which, bytes)) = data.split_first() else {
        return;
    };
    let text = String::from_utf8_lossy(bytes);
    match which % 6 {
        0 => {
            if let Ok(Request::Perform { action }) = decode::<Request>(&text) {
                let _ = Action::deserialize(action);
            }
        }
        1 => {
            let _ = decode::<Reply>(&text);
        }
        2 => {
            let told = Told::read(&text);
            assert_eq!(Told::read(&told.written()), told);
        }
        3 => {
            // Cut where it may be, as what carries it is.
            let mut heard = Heard::default();
            for piece in bytes.chunks(7) {
                for word in heard.hear(piece) {
                    let line = word.line();
                    assert_eq!(
                        Word::read(line.strip_suffix('\n').unwrap_or(&line)),
                        Some(word)
                    );
                }
            }
        }
        4 => {
            if let Some(hand) = Hand::read(&text) {
                assert_eq!(Hand::read(&hand.written()), Some(hand));
            }
        }
        _ => {
            for line in text.lines() {
                if let Some(asked) = Asked::read(line) {
                    let again = asked.line();
                    let again = again.as_deref().and_then(|line| line.strip_suffix('\n'));
                    assert_eq!(again.and_then(Asked::read), Some(asked));
                }
                if let Some(seated) = Seated::read(line) {
                    let again = seated.line();
                    assert_eq!(
                        Seated::read(again.strip_suffix('\n').unwrap_or(&again)),
                        Some(seated)
                    );
                }
            }
        }
    }
});
