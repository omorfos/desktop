//! What a program and the compositor send, read as the relay reads it: a
//! byte saying which of the two sent a message, then the message, again
//! and again, the compositor now and then ending and another being joined.
//! Nothing it is given may make the relay panic, loop or grow without
//! bound; a message it cannot read ends the connection.

#![no_main]

use libfuzzer_sys::fuzz_target;
use omorfos_relay::keep::Keeping;
use omorfos_relay::objects::Objects;
use omorfos_relay::wire::{self, HEADER};

fuzz_target!(|data: &[u8]| {
    let mut objects = Objects::default();
    let mut keeping = Keeping::default();
    let mut rest = data;
    while let Some((&from, bytes)) = rest.split_first() {
        // The compositor ends, and the next is joined.
        if from % 3 == 2 {
            if keeping.lost(&objects).is_err() {
                break;
            }
            keeping.join();
            rest = bytes;
            continue;
        }
        let Ok(Some(header)) = wire::header(bytes) else {
            break;
        };
        let (message, after) = bytes.split_at(header.size);
        let mut message = message.to_vec();
        let read = if from % 3 == 0 {
            sent(&mut objects, &mut keeping, &mut message)
        } else {
            told(&mut objects, &mut keeping, &mut message)
        };
        if read.is_none() {
            break;
        }
        keeping.to_compositor.clear();
        keeping.to_program.clear();
        rest = after;
    }
});

/// The program sends `message`, read and kept as a link does.
fn sent(objects: &mut Objects, keeping: &mut Keeping, message: &mut [u8]) -> Option<()> {
    let header = wire::header(message).ok()??;
    let read = objects.request(message, header).ok()?;
    let described = &read.interface.requests[usize::from(read.opcode)];
    keeping
        .journal
        .request(header, message, &[], &read, described);
    keeping.request(&read, message, described);
    Some(())
}

/// The compositor sends `message`, sifted and read as a link does.
fn told(objects: &mut Objects, keeping: &mut Keeping, message: &mut [u8]) -> Option<()> {
    let header = wire::header(message).ok()??;
    let Some(header) = keeping.before(objects, header, message).ok()? else {
        return Some(());
    };
    let read = objects.event(message, header).ok()?;
    let body = message.get(HEADER..).unwrap_or_default().to_vec();
    if keeping.journal.event(header, &body, &read) {
        keeping.after(&read, message);
    }
    Some(())
}
