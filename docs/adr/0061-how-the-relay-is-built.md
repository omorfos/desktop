# 61. How the relay is built

- Status: accepted
- Date: 2026-10-26
- Builds on: [60. The session outlives the compositor](0060-the-session-outlives-the-compositor.md)

## Context

ADR 60 puts a relay, `omorfos-relay`, between programs and the
compositor: it keeps each program's connection and the state of the
protocol on it, and makes that state again for a compositor started
anew, so that a restart closes no window. It left how the relay is built
to the building. Four things have to be settled before any of it is.

Who a program is. The compositor decides by a program's process
whether a window it opens may take the keyboard (ADRs 8 and 36),
names the program it refused something (ADR 37), and says which are
taking pictures of the screen (ADR 43). It asks the socket, by
`SO_PEERCRED`. Behind the relay every socket is the relay's, and every
program would be the relay.

What the relay reads. It is a second reader of what untrusted programs
send, before the compositor's, and ADR 60 has it fuzzed from the first.
It has to know every message of every protocol the compositor offers, to
know which carry descriptors and which make objects, and only those
(the list in `omorfos-ipc`, ADR 60).

How it is started, and what happens when it ends. The holder starts the
compositor; the relay has to be started, given the socket, and joined to
each compositor in turn, and a relay that crashes must not end the
session the holder was made to keep.

How it is built in steps a user can try. Programs are kept only once the
relay can make again everything they made, and a relay that does that
for every protocol is a large piece of work.

## Decision

**The relay tells the compositor who each program is.** The relay and
the compositor are joined by a connection of their own, which only the
holder makes and hands out, one end to each: no program can reach it.
For each program that connects, the relay reads its process, user and
group from its socket, makes a pair of sockets, and hands the
compositor one end with what it read. The compositor takes that end as
the program's connection and what it was told as who the program is,
and asks the socket of no connection that came through the relay. The
relay is as trusted as the holder that started it; what it says is not
checked again.

**The relay reads with a reader of its own**, small and bounded, driven
by the descriptions of each protocol's messages that the Wayland crates
already in the tree carry (`wayland_backend::protocol::Interface`), so
that nothing is added to the tree. It reads every message both ways,
keeps a table of each program's objects and of what each is, and drops
a program that sends what does not parse, names an object it has not
made, or breaks a bound. A global not in the list cannot be bound
through it. A test checks that each protocol in the list has a
description at or above the version listed.

**It is fuzzed with `cargo-fuzz`**, in a crate of its own beside the
workspace, run by a CI job of its own on a nightly toolchain, which
`cargo-fuzz` needs; everything else stays on the stable toolchain the
project is pinned to. What the fuzzer runs is the reader, the table and
the bounds, fed what a program might send.

**The holder holds the relay as it holds the compositor.** It starts
the relay first, hands it the socket, and keeps it running: found ended
as the next compositor is about to be started, it is started again at
once, as the relay holds nothing, such as the seat, that would need time
to be let go; three times a minute, and then given up. For each compositor it starts, it makes a new connection
between the two and hands one end to each, the relay's by the
connection the relay was started with. A compositor started from the
version held, which may be older than the relay, is handed the socket
itself, as before there was one, and the relay is stopped until the
next login. When the relay ends, the programs that came through it lose
their connections, as they would with a compositor that crashed. The
compositor hears its connection to the relay close and steps down, and
the holder starts a new relay and then a compositor joined to it: the
windows are gone either way, and the cost of starting the compositor
again too is a blink, where handing a running compositor a new relay
would need words the holder does not otherwise say. A relay that keeps
failing, three times a minute, is given up, and the compositor is
handed the socket itself for the rest of the login.

The relay's connections, to the holder and to the compositor, carry
records rather than a stream (`SOCK_SEQPACKET`): each hand, its words
and its descriptors, arrives whole and alone, however many come at
once, so that nothing has to find where one ends.

**Components and sandboxes reach the compositor as they do now**, at
first. A component is handed a connection the compositor made, and is
started again with the compositor, as ADR 28 says; a program in a
sandbox connects where its sandbox listens, which the compositor serves.
Both are taken over by the relay in later steps: components kept as
programs are, which ADR 60 asks for, and the sandbox's listener handed to
the relay with what the sandbox is, so that a program in one is still
shown no more than before.

**It is built in three steps, each tried on real hardware.**

1. The relay in the path: started by the holder, reading every message
   both ways and passing it on, telling the compositor who each program
   is. A user sees nothing change but one more process; a restart
   still closes every window.
2. Programs such as `foot` kept: the protocols it uses made again for a
   compositor started anew. A program holding anything the relay
   cannot yet make again is closed when the compositor restarts, as
   before. Each step after this keeps more programs, and none keeps
   fewer.
3. The rest: every protocol in the list, components kept, sandboxes
   listened to by the relay, and the compositor taking up its own new
   version at a quiet moment, as ADR 60 says.

## Alternatives

- **The compositor asks the relay who a program is**, by the connection,
  when it needs to. It would need an answer while handling a request,
  which would wait on another process. Telling it once, as the program
  connects, costs nothing later.
- **`SCM_CREDENTIALS` on each connection the relay makes**, the program's
  process passed as the sender's. The kernel lets a process name another
  only with `CAP_SYS_ADMIN`, which the relay is not to have.
- **wayland-backend's own reader.** It is private to the crate, and its
  server side would have the relay hold the program's objects as a
  compositor does, which is the whole of Smithay's work over again.
- **A relay that passes bytes and descriptors on without reading them.**
  The kernel keeps descriptors with the bytes they came with, so
  passing works; but nothing could then be made again, and the reader is
  what every later step stands on. It is built and fuzzed first.
- **Property tests in place of fuzzing**, which need no other toolchain.
  They are not guided by what the code reaches, and ADR 60 asks for
  fuzzing.
- **`cargo-afl`**, which builds on the stable toolchain. Less used in Rust
  than `cargo-fuzz`, and its tool has to be installed to run it at all.

## Consequences

- Every program's messages are read once more, by the relay. An idle
  desktop sends none.
- The relay sees all that programs send, the clipboard and what is typed
  among it, as the compositor does. It is a process of the user's, as
  the compositor is, started by the holder, and its connection to the
  compositor cannot be reached by programs.
- A program's process is what the relay says it is. A relay that lied
  could let a window take the keyboard it should not; the relay is ours
  and is started only by the holder.
- The compositor still serves the socket itself where there is no relay:
  run nested, run without a holder, or held without a relay that works.
  Both paths are tested.
- The fuzzer's crate is outside the workspace and outside the coverage
  gate (ADR 2), and its CI job is the only one on a nightly toolchain.
- Until step 2, the relay costs a little and gives nothing a user can
  see.
