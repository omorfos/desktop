//! What a program and the compositor send, read as the relay reads it: a
//! byte saying which of the two sent a message, then the message, again
//! and again. Nothing it is given may make the relay panic, loop or grow
//! without bound; a message it cannot read ends the connection.

#![no_main]

use libfuzzer_sys::fuzz_target;
use omorfos_relay::objects::Objects;
use omorfos_relay::wire;

fuzz_target!(|data: &[u8]| {
    let mut objects = Objects::default();
    let mut rest = data;
    while let Some((&from, bytes)) = rest.split_first() {
        let Ok(Some(header)) = wire::header(bytes) else {
            break;
        };
        let (message, after) = bytes.split_at(header.size);
        let read = if from & 1 == 0 {
            objects.request(message, header)
        } else {
            objects.event(message, header)
        };
        if read.is_err() {
            break;
        }
        rest = after;
    }
});
