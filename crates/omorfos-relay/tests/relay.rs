//! The relay as the holder, the compositor and programs meet it: what it
//! is handed, who it hands on, and what it passes between them.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::io::{Read, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use omorfos_ipc::relay::{self, Hand, Program, Received, Started};
use omorfos_relay::Relay;
use tempfile::TempDir;

/// The longest a step waits.
const STEP: Duration = Duration::from_millis(20);

/// A relay, and what the holder and the compositor would have of it.
struct Held {
    relay: Relay,
    holder: OwnedFd,
    socket: PathBuf,
    said: Vec<u8>,
    _directory: TempDir,
}

impl Held {
    /// A relay the holder has handed the socket to, and no compositor yet.
    fn new() -> Self {
        let (holder, relays) = relay::pair().unwrap();
        let directory = TempDir::new().unwrap();
        let socket = directory.path().join("wayland-1");
        let listener = UnixListener::bind(&socket).unwrap();
        relay::send(holder.as_fd(), &Hand::Socket, &[listener.as_fd()]).unwrap();
        let mut held = Self {
            relay: Relay::new(relays),
            holder,
            socket,
            said: Vec::new(),
            _directory: directory,
        };
        held.step();
        held
    }

    /// Hands the relay a compositor, and returns the compositor's end.
    fn compositor(&mut self) -> OwnedFd {
        let (relays, compositor) = relay::pair().unwrap();
        relay::send(self.holder.as_fd(), &Hand::Compositor, &[relays.as_fd()]).unwrap();
        self.step();
        compositor
    }

    /// Steps the relay once. Returns whether it goes on.
    fn step(&mut self) -> bool {
        self.relay.step(Some(STEP), &mut self.said)
    }

    /// Steps the relay until `done` says so, or a second has gone.
    fn until(&mut self, mut done: impl FnMut(&mut Self) -> bool) {
        let by = Instant::now() + Duration::from_secs(1);
        while !done(self) {
            assert!(Instant::now() < by, "never came about");
            self.step();
        }
    }

    /// The next record the relay hands `compositor` with a connection: not
    /// the words that it has handed every program it kept, and that they
    /// are ready.
    fn handed(&mut self, compositor: &OwnedFd) -> Received {
        let mut handed = None;
        self.until(|_| {
            handed = relay::receive(compositor.as_fd()).unwrap();
            handed
                .as_ref()
                .is_some_and(|handed| !matches!(handed.hand, Some(Hand::Kept | Hand::Ready)))
        });
        handed.unwrap()
    }

    /// A program that connects, and the compositor's end of its connection,
    /// with who the relay said it is.
    fn connect(&mut self, compositor: &OwnedFd) -> (UnixStream, UnixStream, Program) {
        let program = UnixStream::connect(&self.socket).unwrap();
        let handed = self.handed(compositor);
        let who = if let Some(Hand::Program(who)) = handed.hand {
            Some(who)
        } else {
            None
        };
        let connection = UnixStream::from(handed.fds.into_iter().next().unwrap());
        connection.set_nonblocking(true).unwrap();
        (program, connection, who.unwrap())
    }
}

/// How the relay names the programs these tests are: this process.
fn named() -> String {
    let exe = std::env::current_exe().unwrap();
    let name = exe.file_name().unwrap().to_string_lossy().into_owned();
    format!("{name}, process {}", std::process::id())
}

/// `wl_display.get_registry`, as the object 2.
fn get_registry() -> Vec<u8> {
    let mut bytes = 1_u32.to_ne_bytes().to_vec();
    bytes.extend((12_u32 << 16 | 1).to_ne_bytes());
    bytes.extend(2_u32.to_ne_bytes());
    bytes
}

/// What a program sends to fill everything between it and the compositor:
/// a registry, the compositor bound, a surface, and that surface committed
/// again and again, which makes nothing, more than a megabyte of it.
fn flood() -> Vec<u8> {
    let message = |object: u32, opcode: u32, arguments: &[u32]| {
        let size = u32::try_from(8 + 4 * arguments.len()).unwrap();
        let mut bytes = object.to_ne_bytes().to_vec();
        bytes.extend((size << 16 | opcode).to_ne_bytes());
        bytes.extend(arguments.iter().flat_map(|argument| argument.to_ne_bytes()));
        bytes
    };
    let mut bytes = get_registry();
    // wl_registry.bind of wl_compositor, as the object 3: its name, then
    // "wl_compositor" as a string, its version and its number.
    let mut bind = vec![1];
    bind.push(14);
    bind.extend(
        b"wl_compositor\0\0\0"
            .chunks(4)
            .map(|word| u32::from_ne_bytes(word.try_into().unwrap())),
    );
    bind.extend([6, 3]);
    bytes.extend(message(2, 0, &bind));
    bytes.extend(message(3, 0, &[4]));
    for _ in 0..200_000 {
        bytes.extend(message(4, 6, &[]));
    }
    bytes
}

/// Reads from `from` until `wanted` bytes have come, stepping `held` while
/// nothing has.
fn read(held: &mut Held, from: &mut UnixStream, wanted: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    held.until(|_| {
        let mut chunk = [0; 64];
        if let Ok(read) = from.read(&mut chunk) {
            bytes.extend_from_slice(&chunk[..read]);
        }
        bytes.len() >= wanted
    });
    bytes
}

#[test]
fn a_program_that_connects_is_handed_to_the_compositor_with_who_it_is() {
    let mut held = Held::new();
    let compositor = held.compositor();
    let (_program, _connection, who) = held.connect(&compositor);
    let expected = Program {
        pid: std::process::id(),
        uid: rustix::process::getuid().as_raw(),
        gid: rustix::process::getgid().as_raw(),
        number: Some(1),
    };
    assert_eq!(who, expected);
    assert_eq!(held.relay.programs(), 1);
}

#[test]
fn what_a_program_and_the_compositor_send_each_other_is_passed_on() {
    let mut held = Held::new();
    let compositor = held.compositor();
    let (mut program, mut connection, _) = held.connect(&compositor);
    program.set_nonblocking(true).unwrap();
    program.write_all(&get_registry()).unwrap();
    assert_eq!(read(&mut held, &mut connection, 12), get_registry());

    // wl_display.delete_id, back.
    let mut deleted = 1_u32.to_ne_bytes().to_vec();
    deleted.extend((12_u32 << 16 | 1).to_ne_bytes());
    deleted.extend(9_u32.to_ne_bytes());
    connection.write_all(&deleted).unwrap();
    assert_eq!(read(&mut held, &mut program, 12), deleted);
}

#[test]
fn no_program_is_let_in_until_there_is_a_compositor() {
    let mut held = Held::new();
    let _waiting = UnixStream::connect(&held.socket).unwrap();
    held.step();
    assert_eq!(held.relay.programs(), 0);
    let compositor = held.compositor();
    held.until(|held| held.relay.programs() == 1);
    assert!(relay::receive(compositor.as_fd()).unwrap().is_some());
}

/// A message to `object`, the `opcode`th of its, of `words`.
fn message(object: u32, opcode: u32, words: &[u32]) -> Vec<u8> {
    let size = u32::try_from(8 + 4 * words.len()).unwrap();
    let mut bytes = object.to_ne_bytes().to_vec();
    bytes.extend((size << 16 | opcode).to_ne_bytes());
    bytes.extend(words.iter().flat_map(|word| word.to_ne_bytes()));
    bytes
}

/// `wl_registry.bind` of the global named `name`, `interface`, as `number`.
fn bind(name: u32, interface: &str, version: u32, number: u32) -> Vec<u8> {
    let mut words = vec![name, u32::try_from(interface.len() + 1).unwrap()];
    let mut text = interface.as_bytes().to_vec();
    text.push(0);
    text.resize(text.len().next_multiple_of(4), 0);
    words.extend(
        text.chunks(4)
            .map(|word| u32::from_ne_bytes(word.try_into().unwrap())),
    );
    words.extend([version, number]);
    message(2, 0, &words)
}

#[test]
fn when_the_compositor_goes_a_program_that_can_be_kept_waits_for_the_next() {
    let mut held = Held::new();
    let compositor = held.compositor();
    let (mut program, mut connection, _) = held.connect(&compositor);
    program.write_all(&get_registry()).unwrap();
    read(&mut held, &mut connection, 12);
    drop((compositor, connection));
    held.step();
    held.step();
    assert_eq!(held.relay.programs(), 1, "kept");
    program.set_nonblocking(true).unwrap();
    let mut nothing = [0; 8];
    assert!(
        program.read(&mut nothing).is_err(),
        "not closed, and told nothing"
    );
}

#[test]
fn when_the_compositor_goes_a_program_holding_what_cannot_be_made_again_is_let_go_and_said_to_be() {
    let mut held = Held::new();
    let compositor = held.compositor();
    let (mut program, mut connection, _) = held.connect(&compositor);
    // Touch, which no compositor offers yet.
    let mut sent = get_registry();
    sent.extend(bind(1, "wl_seat", 7, 3));
    sent.extend(message(3, 2, &[4]));
    program.write_all(&sent).unwrap();
    read(&mut held, &mut connection, sent.len());
    drop((compositor, connection));
    held.until(|held| held.relay.programs() == 0);
    let mut rest = Vec::new();
    assert_eq!(program.read_to_end(&mut rest).unwrap(), 0, "closed");
    let said = String::from_utf8_lossy(&held.said);
    assert!(
        said.contains(&format!(
            "{} was let go, as it held wl_touch, which cannot be made again",
            named()
        )),
        "{said}"
    );
}

#[test]
fn a_compositor_started_anew_is_handed_the_programs_kept_and_their_registries_first() {
    let mut held = Held::new();
    let compositor = held.compositor();
    let (mut program, mut connection, who) = held.connect(&compositor);
    program.write_all(&get_registry()).unwrap();
    read(&mut held, &mut connection, 12);
    drop((compositor, connection));
    held.step();
    let next = held.compositor();
    let handed = held.handed(&next);
    assert_eq!(
        handed.hand,
        Some(Hand::Program(who)),
        "under the same number"
    );
    let mut again = UnixStream::from(handed.fds.into_iter().next().unwrap());
    again.set_nonblocking(true).unwrap();
    // The relay's sync takes the next number the compositor would take.
    let mut expected = get_registry();
    expected.extend(message(1, 0, &[3]));
    assert_eq!(
        read(&mut held, &mut again, expected.len()),
        expected,
        "then the relay's sync"
    );
}

#[test]
fn a_program_that_sends_what_cannot_be_read_is_let_go_and_said_to_be() {
    let mut held = Held::new();
    let compositor = held.compositor();
    let (mut program, _connection, _) = held.connect(&compositor);
    // A request to an object it has not made.
    let mut stray = 9_u32.to_ne_bytes().to_vec();
    stray.extend((8_u32 << 16).to_ne_bytes());
    program.write_all(&stray).unwrap();
    held.until(|held| held.relay.programs() == 0);
    let said = String::from_utf8_lossy(&held.said);
    assert!(
        said.contains(&format!(
            "{} was let go, for a request to no object",
            named()
        )),
        "{said}"
    );
}

#[test]
fn a_program_that_closes_is_let_go_quietly() {
    let mut held = Held::new();
    let compositor = held.compositor();
    let (program, _connection, _) = held.connect(&compositor);
    drop(program);
    held.until(|held| held.relay.programs() == 0);
    assert_eq!(held.said, b"");
}

#[test]
fn a_program_that_cannot_be_handed_to_the_compositor_is_said_not_to_be_let_in() {
    let mut held = Held::new();
    let compositor = held.compositor();
    // The compositor takes nothing more, but has not gone.
    rustix::net::shutdown(&compositor, rustix::net::Shutdown::Read).unwrap();
    let _program = UnixStream::connect(&held.socket).unwrap();
    held.until(|held| !held.said.is_empty());
    let said = String::from_utf8_lossy(&held.said);
    assert!(said.contains("a program could not be let in"), "{said}");
    assert_eq!(held.relay.programs(), 0);
}

#[test]
fn a_program_gone_while_the_compositor_does_not_read_is_let_go() {
    let mut held = Held::new();
    let compositor = held.compositor();
    let (mut program, _connection, _) = held.connect(&compositor);
    // More than everything between can hold, while the compositor's end
    // takes none of it.
    let syncs = flood();
    program.set_nonblocking(true).unwrap();
    // Until nothing more goes for a few steps: everything between is full.
    let mut sent = 0;
    let mut still = 0;
    while still < 5 {
        let more = program.write(&syncs[sent..]).unwrap_or_default();
        sent += more;
        still = if more == 0 { still + 1 } else { 0 };
        held.step();
    }
    assert!(sent < syncs.len(), "it all went");
    drop(program);
    held.until(|held| held.relay.programs() == 0);
}

#[test]
fn what_waits_to_be_written_is_written_once_there_is_room() {
    let mut held = Held::new();
    let compositor = held.compositor();
    let (mut program, mut connection, _) = held.connect(&compositor);
    program.set_nonblocking(true).unwrap();
    let flood = flood();
    // First everything between is filled, while the compositor's end takes
    // nothing; then it takes all it has each time, and the rest goes on.
    let mut sent = 0;
    let mut still = 0;
    while still < 5 {
        let more = program.write(&flood[sent..]).unwrap_or_default();
        sent += more;
        still = if more == 0 { still + 1 } else { 0 };
        held.step();
    }
    let mut read = 0;
    held.until(|held| {
        let mut chunk = vec![0; 65536];
        while let Ok(more @ 1..) = connection.read(&mut chunk) {
            read += more;
        }
        sent += program.write(&flood[sent..]).unwrap_or_default();
        held.step();
        read == flood.len()
    });
}

#[test]
fn a_hand_the_relay_does_not_know_or_without_what_it_says_is_passed_over() {
    let mut held = Held::new();
    relay::send(held.holder.as_fd(), &Hand::Compositor, &[]).unwrap();
    let anyone = Hand::Program(Program {
        pid: 1,
        uid: 0,
        gid: 0,
        number: None,
    });
    relay::send(held.holder.as_fd(), &anyone, &[]).unwrap();
    assert!(held.step());
    let _waiting = UnixStream::connect(&held.socket).unwrap();
    held.step();
    assert_eq!(held.relay.programs(), 0, "no compositor");
}

#[test]
fn a_compositor_handed_before_the_socket_waits_for_it() {
    let (holder, relays) = relay::pair().unwrap();
    let mut relay = Relay::new(relays);
    let (towards, _compositor) = relay::pair().unwrap();
    relay::send(holder.as_fd(), &Hand::Compositor, &[towards.as_fd()]).unwrap();
    let mut said = Vec::new();
    assert!(relay.step(Some(STEP), &mut said));
    assert!(relay.step(Some(STEP), &mut said), "with no socket to serve");
    assert_eq!(relay.programs(), 0);
}

#[test]
fn the_relay_ends_when_the_holder_has_gone() {
    let mut held = Held::new();
    let (holder, _) = relay::pair().unwrap();
    drop(std::mem::replace(&mut held.holder, holder));
    assert!(!held.step());
}

#[test]
fn a_program_whose_connection_closes_a_moment_before_the_compositor_has_gone_is_kept() {
    let mut held = Held::new();
    let compositor = held.compositor();
    let (_program, connection, _) = held.connect(&compositor);
    // A compositor ending closes its connections one at a time, and another
    // process may hold a copy of the last for a moment.
    drop(connection);
    let closing = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        drop(compositor);
    });
    held.step();
    held.step();
    closing.join().unwrap();
    assert_eq!(
        held.relay.programs(),
        1,
        "kept: {}",
        String::from_utf8_lossy(&held.said)
    );
}

#[test]
fn a_program_the_compositor_lets_go_while_it_goes_on_is_let_go() {
    let mut held = Held::new();
    let compositor = held.compositor();
    let (mut program, connection, _) = held.connect(&compositor);
    // Its connection closed, with no error said and the compositor there.
    drop(connection);
    held.until(|held| held.relay.programs() == 0);
    let mut rest = Vec::new();
    assert_eq!(program.read_to_end(&mut rest).unwrap(), 0, "closed");
    let said = String::from_utf8_lossy(&held.said);
    assert!(
        said.contains(&format!(
            "{} was let go, as the compositor closed its connection",
            named()
        )),
        "{said}"
    );
    drop(compositor);
}

#[test]
fn a_program_kept_for_a_compositor_that_has_gone_already_is_let_go_and_said_to_be() {
    let mut held = Held::new();
    let compositor = held.compositor();
    let (_program, connection, _) = held.connect(&compositor);
    drop((compositor, connection));
    held.until(|held| held.relay.programs() == 1);
    held.step();
    // Handed one whose end is closed, the program cannot be handed to it.
    let (relays, gone) = relay::pair().unwrap();
    drop(gone);
    relay::send(held.holder.as_fd(), &Hand::Compositor, &[relays.as_fd()]).unwrap();
    held.until(|held| held.relay.programs() == 0);
    let said = String::from_utf8_lossy(&held.said);
    assert!(
        said.contains(&format!(
            "{} could not be kept for the next compositor",
            named()
        )),
        "{said}"
    );
}

#[test]
fn a_compositor_is_told_once_every_program_kept_has_been_handed_to_it() {
    let mut held = Held::new();
    let compositor = held.compositor();
    let first = relay::receive(compositor.as_fd()).unwrap().unwrap();
    assert_eq!(first.hand, Some(Hand::Kept), "none, at first");
    let ready = relay::receive(compositor.as_fd()).unwrap().unwrap();
    assert_eq!(ready.hand, Some(Hand::Ready), "and none to wait for");
    let (_program, connection, who) = held.connect(&compositor);
    drop((compositor, connection));
    held.step();
    let next = held.compositor();
    let handed = relay::receive(next.as_fd()).unwrap().unwrap();
    assert_eq!(handed.hand, Some(Hand::Program(who)));
    let then = relay::receive(next.as_fd()).unwrap().unwrap();
    assert_eq!(then.hand, Some(Hand::Kept), "then that it is all");
    // Not ready while what the program had is still being made again, which
    // waits on this compositor answering.
    held.step();
    assert!(relay::receive(next.as_fd()).unwrap().is_none());
}

#[test]
fn a_part_the_compositor_started_is_kept_as_programs_are_with_its_roles() {
    let mut held = Held::new();
    let compositor = held.compositor();
    let (relays, mut part) = UnixStream::pair().unwrap();
    let started = Started {
        pid: std::process::id(),
        roles: 16,
    };
    relay::send(
        compositor.as_fd(),
        &Hand::Started(started),
        &[relays.as_fd()],
    )
    .unwrap();
    drop(relays);
    let handed = held.handed(&compositor);
    let Some(Hand::Part(who, roles)) = handed.hand else {
        panic!("{:?}", handed.hand);
    };
    assert_eq!((who.pid, roles), (started.pid, 16));
    assert!(who.number.is_some(), "numbered as programs are");
    // What it says is passed on as a program's is.
    let mut connection = UnixStream::from(handed.fds.into_iter().next().unwrap());
    connection.set_nonblocking(true).unwrap();
    part.write_all(&get_registry()).unwrap();
    assert_eq!(read(&mut held, &mut connection, 12), get_registry());

    // Kept for the next compositor, which is told its roles again.
    drop((compositor, connection));
    held.step();
    let next = held.compositor();
    let again = held.handed(&next);
    assert_eq!(again.hand, Some(Hand::Part(who, 16)));
}

#[test]
fn a_part_handed_without_a_connection_or_with_one_that_is_not_is_not_let_in() {
    let mut held = Held::new();
    let compositor = held.compositor();
    let started = Hand::Started(Started { pid: 1, roles: 16 });
    relay::send(compositor.as_fd(), &started, &[]).unwrap();
    let file = std::fs::File::open("/dev/null").unwrap();
    relay::send(compositor.as_fd(), &started, &[file.as_fd()]).unwrap();
    held.until(|held| !held.said.is_empty());
    let said = String::from_utf8(held.said.clone()).unwrap();
    assert!(
        said.starts_with("omorfos-relay: a part of the session could not be let in:"),
        "{said}"
    );
    assert_eq!(held.relay.programs(), 0);
}

/// `wl_registry.global`, the global named `name` offered as `interface`.
fn global(name: u32, interface: &str, version: u32) -> Vec<u8> {
    let mut bound = bind(name, interface, version, 0);
    // A bind is the global as offered, and the number it is bound as.
    bound.truncate(bound.len() - 4);
    let size = u32::try_from(bound.len()).unwrap();
    bound[4..8].copy_from_slice(&(size << 16).to_ne_bytes());
    bound
}

#[test]
fn where_a_programs_windows_were_is_handed_back_with_it_and_the_session_first() {
    let mut held = Held::new();
    let compositor = held.compositor();
    let (mut program, mut connection, who) = held.connect(&compositor);
    program.set_nonblocking(true).unwrap();
    program.write_all(&get_registry()).unwrap();
    read(&mut held, &mut connection, 12);
    let mut offered = global(1, "xdg_wm_base", 6);
    offered.extend(global(2, "wl_compositor", 6));
    connection.write_all(&offered).unwrap();
    read(&mut held, &mut program, offered.len());
    // A window: the shell and compositor bound as 3 and 4, its surface 5,
    // made a window's, 6, and a toplevel, 7.
    let mut made = bind(1, "xdg_wm_base", 6, 3);
    made.extend(bind(2, "wl_compositor", 6, 4));
    made.extend(message(4, 0, &[5]));
    made.extend(message(3, 2, &[6, 5]));
    made.extend(message(6, 1, &[7]));
    program.write_all(&made).unwrap();
    read(&mut held, &mut connection, made.len());

    let number = who.number.unwrap();
    let placed = |toplevel: u32, place: &str| Hand::Placed {
        number,
        toplevel,
        place: place.to_owned(),
    };
    let desk = Hand::Desk("the session".to_owned());
    for hand in [
        placed(7, "before"),
        placed(7, "here"),
        placed(5, "no window"),
        desk.clone(),
    ] {
        relay::send(compositor.as_fd(), &hand, &[]).unwrap();
    }
    // Of another program than any there is, it is kept by none.
    let other = Hand::Placed {
        number: number + 1,
        toplevel: 7,
        place: "nowhere".to_owned(),
    };
    relay::send(compositor.as_fd(), &other, &[]).unwrap();
    held.step();
    drop((compositor, connection));
    held.step();

    let next = held.compositor();
    let mut hands = Vec::new();
    held.until(|_| {
        let received = relay::receive(next.as_fd()).unwrap();
        hands.extend(received.map(|received| received.hand));
        hands.last() == Some(&Some(Hand::Kept))
    });
    assert_eq!(
        hands,
        [
            Some(desk),
            Some(Hand::Program(who)),
            Some(placed(7, "here")),
            Some(Hand::Kept)
        ]
    );
}
