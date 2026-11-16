# What the first review did not look at

A review of 16 November 2026, for security, of what
[the review of 15 November](2026-11-15-what-a-client-can-cost.md) listed
as not looked at. Each finding is written so that it can be taken up on
its own: where it is, how it was shown, what to change, and the test to
write first. The roadmap lists them in the order to take them.

## What was asked, and of whom

The areas left out on 15 November:

- the relay beyond what it keeps of an object: what it does when the
  compositor says what it does not expect, and what a program can make
  it do through that;
- the portal's backend and the session's services on D-Bus;
- buffers of the graphics card, as a program's is taken on import;
- the hardware backend, read only, as no test runs it;
- the helpers that check a secret, `omorfos-lock`'s and `omorfos-ask`'s;
- the session's own services: `omorfos-session`, `omorfos-hold`,
  `omorfos-journal`.

The question was the earlier one: what an untrusted Wayland client, a
program on the user's session bus, a program in a sandbox, or a file of
configuration can make happen there, as waiting, allocating without
bound, ending a process, getting more than [ADR 37](../adr/0037-who-may-do-what.md)
promises it, or a secret reaching what it should not. It was read by one
reviewer agent, `review-security`, and each finding was then checked
against the code by the agent that started it, which is how the ranks
below were settled.

How each was shown is said with it, as before: **shown by a test**,
**shown by reading**, or **to be shown**.

## Findings

### 1. A program that makes a sandbox can stop the relay for good

**High. The mechanism shown by a script; the whole effect is to be
shown.**

`Sandboxes::request` in `crates/omorfos-relay/src/sandbox.rs`, at
`wp_security_context_v1.commit`, sets the listening socket it was handed
not to wait, once (`ioctl_fionbio(&*made.listening, true)`), and
`Sandbox::waiting` then accepts from it in a loop until an error, from
the relay's only loop (`crates/omorfos-relay/src/lib.rs:219`). Whether a
socket waits belongs to what both descriptors name, and the program that
made the sandbox holds the other: it takes the flag off again
(`fcntl(F_SETFL)`) and connects once. The relay wakes, accepts that
connection, and waits in the next `accept` for ever. A script in the
reviewer's scratchpad did just that over `SCM_RIGHTS`: after the one
waiting connection was accepted, the next `accept` waited until the
script was killed. Without touching the flag, the program can also take
the waiting connection itself, between the relay's `poll` and its
`accept`, as it holds the listener too.

The relay carries every program's connection, so every window in the
session stops being answered, and the compositor sees nothing go; a relay
that hangs is not noticed (on the roadmap), so nothing recovers. Any
program outside a sandbox can do it. One inside may too: the relay
answers `create_listener` and `commit` for every link and never asks
whether it is one in a sandbox, and the compositor hides the manager's
global from it but cannot refuse a bind it never sees in time. That is to
be shown. Without the relay, Smithay's `SecurityContextListenerSource`
does the same (`listener_source.rs`: set once, accept until it would
wait).

**To change**:

- Rest nothing on the flag, for the reason ADR 90 gives about pipes:
  accept on a thread of its own for each sandbox listened on, no more
  than `SANDBOXES`, and hand what is accepted to the relay's loop
  through a pipe, as `Handing` hands pastes.
- Refuse `create_listener` from a link that came through a sandbox,
  ending it as the compositor would.
- Write Smithay's listener source in `docs/UPSTREAM.md`.

**Test first**, in `crates/omorfos-relay/tests/relay.rs`: a program binds
the manager, sends `create_listener` with a listening socket and a close
descriptor, and `commit`; the test takes the flag off its own copy and
connects once; the relay's turn, on a thread, returns within a second.
Today it never does. And: a link in a sandbox that sends
`create_listener` is ended, and no sandbox is added.

### 2. Any program on the session bus can put up the portal's sealed prompt under any name

**Medium. Shown by reading.**

The backend's interfaces, `org.freedesktop.impl.portal.*`, are for
`xdg-desktop-portal` alone, but `answer` in
`crates/xdg-desktop-portal-omorfos/src/bus.rs` never looks at who calls.
A program outside a sandbox calls `Screenshot` or `ScreenCast.Start`
itself with whatever `app` it likes: an installed application's, so the
box names that one, or words of any length and any characters, shown as
they came (`prompt.rs:257`). It is given the genuine sealed prompt, and
on yes a picture or a PipeWire node. ADR 37 says such a program is never
asked about with a prompt, and that the prompt says which application
asks: both are broken. Any peer can also send `Request.Close` or
`Session.Close` on another's paths, which takes back its question or ends
its share. `CreateSession` grows `Sessions` without bound, and nothing is
let go of when the caller leaves the bus. `omorfos-ask`'s polkit agent
already hears polkit's name alone.

**To change**: learn and follow the owner of
`org.freedesktop.portal.Desktop` as `register` in `omorfos-ask`'s
`agent.rs` does polkit's, and refuse every `impl.portal` call from
anyone else with `AccessDenied` (the settings and
`org.freedesktop.ScreenSaver` stay open to all); hold `Sessions` to a
most as `Holds` is, and drop a caller's on `NameOwnerChanged`; hold
`app` to an application's identifier before it is shown.

**Test first**, in `bus.rs`'s tests: a peer that is not the portal calls
`Screenshot` and is refused, and nothing is asked. Today it is.

### 3. Another program can have the program that holds the clipboard let go of by the relay

**Medium. To be shown.**

`take_in` in `crates/omorfos-relay/src/link.rs` ends a link once more
than 256 descriptors wait in what it has read (`DESCRIPTORS`), the
compositor's side too, and reads 8192 bytes at a time. A
`wl_data_source.send` with a short kind is 20 bytes, so 8192 bytes hold
some 400 of them, each with a pipe. A program that once had the keyboard
keeps its offer and calls `receive` thousands of times; Smithay passes
each on to the source without asking about focus. If the program that
copied is not reading for a moment, its socket fills, then the relay's,
and it is let go of with whatever it had unsaved: what ADR 45 exists to
spare, and far sooner than its megabyte. The keeper of the clipboard,
when it holds it, is let go of the same way.

**To change**: towards a program, stop reading once the descriptors
waiting reach the most less what one read can bring, as `take_in`
already stops when the bytes are full, rather than end the link; keep
the ending for what a program sends.

**Test first**, beside
`a_program_that_does_not_read_stops_the_compositor_being_read` in
`link.rs`: the compositor sends 2000 messages of 20 bytes each with a
descriptor, the program reads nothing and then everything, and its link
is still there.

### 4. A program's buffer of the graphics card that is not finished holds up drawing

**Medium. To be shown, with a graphics card.**

Nothing in the compositor calls `Dmabuf::generate_blocker`, and
`dmabuf_imported` answers success without trying. A program commits a
buffer whose implicit fence is still pending, as after a long piece of
work on the card, and the commit is applied at once and the buffer
drawn. Where the card cannot hand a fence on, Smithay waits on it in
`queue_buffer` (`gbm.rs`), on the compositor's thread: up to the kernel's
time for resetting the card, each frame the program does it again. Most
Flatpaks have `--device=dri`, so a program in a sandbox can.

**To change**: as Smithay's `anvil` does, a hook before each commit, in
`new_surface`, that adds a blocker for each buffer of the card attached
(`generate_blocker(Interest::READ)`) and its source to the loop; and
later, `wp_linux_drm_syncobj_v1`.

**Test first**, with the tests that need a card: a program commits a
buffer whose fence a sync file it never signals holds, and another
surface's next frame is still shown within a frame.

### 5. One program can take every place the relay keeps for sandboxes

**Low to medium. Shown by reading.**

`SANDBOXES`, 64, is for the whole session (`take_sandboxes` in
`crates/omorfos-relay/src/lib.rs`). A program that makes 64 and keeps
their close descriptors open has every sandbox made after it let go of,
and no Flatpak started after can connect.

**To change**: a most for each link too, as 4, and the refusal of
finding 1 for a link in a sandbox.

**Test first**: one program makes 64 sandboxes, and a second program's
is still listened on.

### 6. Smaller things

**Low. Shown by reading.**

- **Pinentry's door is kept by the name of a file.** `may_ask` in
  `omorfos-ask`'s `door.rs` lets in any parent whose executable is named
  `gpg-agent`, so a program outside a sandbox can put up the sealed
  prompt for a passphrase by running the real `omorfos-pinentry` under
  such a parent. The module's comment, that this keeps any program of
  the user's from putting the prompt up, says more than is so. Low under
  ADR 37; recorded so that it is not found again.
- **Pinentry's reading waits two seconds a read.** `said()` reads up to
  4000 bytes with two seconds for each `read`, on `omorfos-ask`'s loop:
  a peer let in that sends a byte each 1.9 s holds it for some two hours.
- **Media keys wait on every player.** `omorfos-session`'s `media.rs`
  asks up to 16 MPRIS names, two seconds each, so a player that never
  answers costs a key up to 32 seconds in that one-shot process.
- **Notifications are numbered in a row.** `replaces_id` and
  `CloseNotification` take another program's number; the specification's
  own weakness.
- **A pool of shared memory on a FUSE file** a program serves, outside a
  sandbox, would hold up `omorfos-shm`'s `stat` and `open`. To be shown.

## What was looked at and found sound

So that it is not looked at again for the same thing:

- **`omorfos-lock check`**: fixed PAM services, what is typed cut at a
  NUL, PAM's messages made one line; a helper is a process for each way,
  killed when it stops.
- **`omorfos-ask`'s polkit agent**: polkit's name alone is heard, others
  are given `AccessDenied`; identities come from `/etc/passwd` and
  `/etc/group`; the cookie goes on the helper's standard input.
- **`omorfos-ask`'s pinentry**: its socket is 0600 in the runtime
  directory, what is typed is escaped and the buffers zeroed once sent,
  one question at a time.
- **The portal's `Inhibit` and the screen saver**: 64 for each peer and
  1024 in all, let go only by who took them, and dropped when it leaves
  the bus. One question is asked at a time; a second is refused.
- **`omorfos-notifications`**: pictures sent whole are at most 1024
  pixels a side; tags are cut to 64 characters.
- **`omorfos-session`'s watcher**: logind's signals are matched by who
  sent them, `Unlock` is never heeded, and every call has two seconds.
- **`omorfos-journal`**: control characters are taken out of the name;
  it speaks to journald alone.
- **`omorfos-shm`**: offsets and strides checked, saturating, against
  the mapping and the file's length; written with `pwrite`, so a file
  made shorter cannot raise `SIGBUS`.
- **The relay's objects**: an event to an object gone is kept until
  `delete_id`; a number the compositor made is taken again only once
  gone.
- **The hardware backend**: gamma ramps of length 0 and 1 are guarded,
  the refresh is divided by at least 1, and the session's environment
  goes to `sh` as arguments, never as a script.
- **The holder's socket**: a lock file and `flock`.

## What was not looked at

- `crates/omorfos-relay/src/keep.rs`, most of what the relay does with
  what the compositor says while it catches up, and the bounds on what
  it keeps to send each way; `journal.rs` beyond the earlier review.
- `omorfos-hold`'s `renew.rs`, `policy.rs` and `seating.rs`.
- The lock screen's `screen.rs` and `power.rs`.
- The portal's `feed.rs`, `stream.rs` and `card.rs`.
- `omorfos-session`'s `autostart.rs`, `radios.rs` and `brightness.rs`.
- Whether a buffer of the card the renderer refuses is left out of a
  frame or fails it.
