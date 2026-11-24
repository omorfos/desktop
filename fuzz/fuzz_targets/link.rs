//! What a program and the compositor send, through the relay's link for the
//! program, read and written as the relay does: over sockets, in pieces of
//! any size, with descriptors handed beside them, the compositor now and
//! then going and the next being joined. A byte says what happens next,
//! and for what is sent, a byte how much of what follows. Nothing either
//! sends may make the link panic, loop or hold more than it is bound to;
//! what it cannot pass on ends it.

#![no_main]

use std::fs::File;
use std::io::{IoSlice, Read};
use std::mem::MaybeUninit;
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;

use libfuzzer_sys::fuzz_target;
use omorfos_ipc::relay::Program;
use omorfos_relay::link::{Link, Way};
use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags, sendmsg};

/// Who the program is, which nothing the link does depends on.
const WHO: Program = Program {
    pid: 1,
    uid: 1000,
    gid: 1000,
    number: None,
};

/// The most descriptors handed with one piece.
const HANDED: usize = 3;

fuzz_target!(|data: &[u8]| {
    let (Ok(handed), Ok((program, ours)), Ok((mut compositor, theirs))) = (
        File::open("/dev/null"),
        UnixStream::pair(),
        UnixStream::pair(),
    ) else {
        return;
    };
    let unwaited = program
        .set_nonblocking(true)
        .and(compositor.set_nonblocking(true));
    let Ok(mut link) = unwaited.and_then(|()| Link::new(ours, WHO, theirs)) else {
        return;
    };
    let mut rest = data;
    while let Some((&what, after)) = rest.split_first() {
        rest = after;
        let joined = link.compositor().is_some();
        // Each is done only when the relay would do it: a way read when
        // the link says it reads, written when it has something to write.
        let done = match what % 8 {
            0 | 1 => {
                let Some((&length, after)) = rest.split_first() else {
                    break;
                };
                let (piece, after) = after.split_at(usize::from(length).min(after.len()));
                rest = after;
                let to = if what % 8 == 0 { &program } else { &compositor };
                send(to, piece, usize::from(what >> 3) % (HANDED + 1), &handed);
                Ok(())
            }
            2 if link.reads(Way::Up) => link.read(Way::Up),
            3 if joined && (link.reads(Way::Down) || !link.again().is_empty()) => {
                link.read(Way::Down)
            }
            4 if link.writes(Way::Up) => link.write(Way::Up),
            5 if link.writes(Way::Down) => link.write(Way::Down),
            // The compositor goes; a program that cannot be kept is let go.
            6 if joined => {
                if link.lose().is_err() {
                    break;
                }
                Ok(())
            }
            7 if !joined => {
                let Ok((next, theirs)) = UnixStream::pair() else {
                    break;
                };
                if next.set_nonblocking(true).is_err() {
                    break;
                }
                compositor = next;
                link.join(theirs)
            }
            _ => Ok(()),
        };
        if done.is_err() {
            break;
        }
        // What the link wrote is taken, so that it is never kept waiting.
        drain(&program);
        drain(&compositor);
    }
});

/// Sends `piece` to `to`, with `count` copies of `handed` beside it, as
/// much as it takes now.
fn send(to: &UnixStream, piece: &[u8], count: usize, handed: &File) {
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(HANDED))];
    let mut control = SendAncillaryBuffer::new(&mut space);
    let fds = [handed.as_fd(); HANDED];
    if count > 0 {
        control.push(SendAncillaryMessage::ScmRights(&fds[..count]));
    }
    // As the relay sends: a side the link has closed is not a signal.
    let flags = SendFlags::NOSIGNAL | SendFlags::DONTWAIT;
    let _ = sendmsg(to, &[IoSlice::new(piece)], &mut control, flags);
}

/// Reads all that has come to `from`, and lets it go, the descriptors
/// beside it with it.
fn drain(mut from: &UnixStream) {
    let mut chunk = [0; 4096];
    while matches!(from.read(&mut chunk), Ok(1..)) {}
}
