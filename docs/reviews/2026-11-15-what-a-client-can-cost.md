# What a client can make the compositor allocate or wait for

A review of 15 November 2026, of the roadmap's item of that name (stage 8,
security). Each finding is written so that it can be taken up on its own:
where it is, how it was shown, what to change, and the test to write
first. The roadmap lists them in the order to take them.

The first two, which let a program stop or end the session, were fixed
the same day, before this was published; each says what was changed,
where that differs from what was first proposed, what the fixing showed
that the review had not, and what it leaves open. The rest are as they
were found.

## What was asked, and of whom

[ADR 37](../adr/0037-who-may-do-what.md) says what can honestly be
promised: a program in a sandbox is truly held to what the compositor
gives it, and a program outside one runs as the user and is not walled
in. So a finding counts for most where **an ordinary Wayland client, one
in a sandbox included**, can bring it about, with no role and nothing but
the protocols every program is offered. What only a peer of the IPC
socket can do, which is the user's own, is listed last and lowest.

What was looked for:

- **waiting**: anything that can stop the compositor's loop, for good or
  for long;
- **allocating**: memory or descriptors a program can make the compositor,
  the relay or the X server keep, without bound;
- **ending**: anything that ends the compositor, as a panic or a failed
  allocation does;
- **more than was meant**: what a program gets that the design says it is
  not to have.

How each finding was shown is said with it: **shown by a test** (written,
run, and taken out again; the test is given, to be put back), **shown by
reading** (the code that does it is named), or **to be shown** (it
follows from what was read, and wants a test before it is believed).

## Findings

### 1. The compositor waits for ever on a pipe a program hands it

**High. Shown by a test. Fixed on the day it was found
([ADR 90](../adr/0090-what-a-program-is-handed-is-written-without-waiting-on-it.md)).**

When the clipboard holds text of the compositor's own, as after "About"
copies, the launcher copies an answer, or `omorfosctl` asks for a copy,
a program pasting was written to at once:
`hand_over` in `crates/omorfos/src/handlers.rs` did
`File::from(to).write_all(text.as_bytes())`, called from
`SelectionHandler::send_selection` there and from
`XwmHandler::send_selection` in `crates/omorfos/src/x11.rs`. Its comment
said the text is shorter than a pipe holds, so the write never waits.
That holds only for an empty pipe. The descriptor is the program's: one
that fills its pipe first and then asks for the text had the compositor
wait in `write` until the pipe was read, which was never. Every screen
froze, and no key reached anything.

Any program with the keyboard could do it, as that is whom the clipboard
is offered to; and one with the `clipboard` role at any time.

The test is
`a_program_that_pastes_through_a_pipe_already_full_does_not_stop_the_compositor`
in `crates/omorfos/tests/integration/clipboard.rs`. As it was first
written, against the write that waited, it hung, and was run under
`timeout`:

```rust
#[test]
fn a_pipe_already_full_does_not_stop_the_compositor() {
    use std::io::Write;
    use std::os::fd::AsFd;
    let mut session = Session::new();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), RED);
    let socket = session.listen();
    let mut peer = omorfos_ipc::Client::connect(&socket).unwrap();
    let text = "from the compositor".to_owned();
    session.ask(&mut peer, &omorfos_ipc::Request::Copy { text });
    session.roundtrip(&mut client);
    let offer = client.state.selection.clone().unwrap();
    let (_reader, mut writer) = std::io::pipe().unwrap();
    // Filled without waiting, then made to wait again, as it is handed over.
    rustix::fs::fcntl_setfl(&writer, rustix::fs::OFlags::NONBLOCK).unwrap();
    while writer.write(&[0; 4096]).is_ok() {}
    rustix::fs::fcntl_setfl(&writer, rustix::fs::OFlags::empty()).unwrap();
    offer.receive("text/plain;charset=utf-8".to_owned(), writer.as_fd());
    session.roundtrip(&mut client);
}
```

**What was changed** is not what was first proposed here, which was to
set `O_NONBLOCK` on the descriptor and write once. That flag is on what
the descriptor names, which the program holds too: it can take the flag
off again between its being set and the write, and the write waits as
before. So nothing rests on the flag. What is handed over goes through
`omorfos_ipc::handing::Handing`: on a thread for each, sixteen at most;
into a pipe only; written into a pipe of our own and moved from there
into the program's with `splice`, told at each move not to wait; and
given up on after half a minute. ADR 90 has the reasons, and what else
was weighed.

The same write was in two more programs, which is why it is one type
now and not a change to `hand_over`. `omorfos-clipboard` wrote what it
keeps on a thread for each paste, with a write that waits: nothing
stopped it, but a paste never read held its thread for good, and
nothing counted the threads. `omorfos-screenshot` wrote its picture
where it waits to hear that something else has been copied, so a paste
never read stopped it there, still holding the clipboard. Both hand
over through `Handing` too.

**Still open**, found while this was fixed, and on the roadmap:

- `ask` in `crates/omorfos-clipboard/src/lib.rs` reads what was copied
  on a thread for each kind it is offered in, each held until the
  program that copied closes its pipe, as its comment says, and
  uncounted; and it starts them with `thread::spawn`, which panics
  where a thread cannot be started.
- Reading a paste is written twice, in the launcher (`Pasting`) and in
  the keeper (`ask`), each with its own bounds. One reader beside
  `Handing` would hold both to the same.
- A program can keep all sixteen of a giver's threads busy, asking again
  each half minute through pipes it does not read. While it does, a
  paste of what that giver gives comes out empty: of the compositor's
  own text, or of what the keeper keeps. Nothing else is stopped. Who
  asks is not told to whoever gives, so they cannot be counted by
  program (ADR 90).

### 2. A window said to be enormous ends the compositor

**High. Shown by a test. Fixed on the day it was found
([ADR 91](../adr/0091-nothing-a-program-shows-is-larger-than-a-frame-may-be.md)).**

A window is as large as its program says. With `wp_viewporter`, a buffer
of four pixels is said to be two thousand million units across, and
the frame the desktop kept was that large:
`domain_size(window.geometry().size)` in `crates/omorfos/src/handlers.rs`
went to the desktop unclamped, as did a program written for X11's in
`crates/omorfos/src/x11.rs`.

- **In a frame of ours**, the titlebar was painted on the CPU as wide as
  the frame: `Canvas::scaled` in `crates/omorfos-ui/src/canvas.rs`
  allocated width by height by four bytes, here 224 GB. The allocation
  failed and the process was aborted: `memory allocation of 224000000224
  bytes failed`, `SIGABRT`. A width that the machine does have memory
  for, some tens of millions, was allocated and zeroed instead, which
  took the memory.
- **With no frame of ours**, drawing the screen failed
  (`RenderError::Unsupported`) on every frame for as long as the window
  was there. **This was seen with the software renderer only**, which
  the tests draw with and the shipped compositor does not have: it
  cannot draw a picture stretched more than some sixty-five thousand
  times. As first written, this finding said the hardware backend then
  shows nothing new and the nested one ends. That was inferred, not
  shown: what a graphics card's renderer does with such a surface was
  not tried, as the machine the review was made on has no card it may
  use. It no longer has to be known, as no renderer is handed one now.

The tests are in `crates/omorfos/tests/integration/decorations.rs`:
`a_window_said_to_be_enormous_is_kept_in_a_frame_that_can_be_drawn`,
`a_window_with_no_frame_of_ours_said_to_be_enormous_does_not_stop_the_screen_being_drawn`
and `a_surface_said_to_be_enormous_is_left_out_of_what_its_window_shows`;
and, for a program written for X11,
`a_window_of_a_program_written_for_x11_is_kept_in_a_frame_no_larger_than_one_may_be`
in `x11.rs` there. The first, as it was written against the compositor
as it was, where it ended the process:

```rust
#[test]
fn a_window_said_to_be_enormous_is_held_to_a_size_that_can_be_drawn() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = client.toplevel_uncommitted("big", "big window");
    client.decoration(&window);
    window.surface.commit();
    session.roundtrip(&mut client);
    let enormous = (2_000_000_000, 2_000_000_000);
    client.fill_scaled(&window.surface, (4, 4), enormous, RED);
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    let frame = session.compositor().desktop().frames().next().unwrap();
    assert!(frame.geometry.size.width <= 65_536);
    drop(session.draw());
}
```

To see any of them fail, take the change it is about out and run it
with memory held down, `(ulimit -v 8000000; cargo test …)`, or it may
take the machine's.

**What was changed**, in three places, none of which rests on another:

- **In the domain**, not at the four handlers first named here: every
  size said of a frame goes through `Desktop::held` in
  `crates/omorfos-core/src/desktop.rs`, which keeps it to
  `Desktop::MOST_SIDE`, 16384 units a side, for programs written for
  Wayland and for X11 alike.
- **Where it is allocated**: `Canvas::scaled` makes nothing of more than
  `Canvas::MOST_PIXELS` and gives a single clear pixel past it.
- **Where it is drawn**, which this finding had not seen the whole of.
  With the frame held, the overview still laid the window out by the
  size its program gave, and drew its own label for it two thousand
  million units wide; and the surface itself still reached the
  renderer. Everything a program shows now becomes what is drawn in one
  module, `crates/omorfos/src/surfaces.rs`, which leaves out a surface
  said to be larger than a frame may be, and takes what a window says
  is the window to be no larger. `clippy.toml` refuses the functions of
  Smithay's it stands in front of anywhere else.

What else is sized by a window was looked for, as this finding asked:

- **Pictures of windows in the switcher, the overview and the tiles'
  offers** are the window's own surfaces drawn small, with nothing
  allocated for them; they go through `surfaces` now.
- **The ghost of a window going** keeps what the window was drawn from,
  and makes nothing new.
- **What glass keeps behind a pane** is cut to the screen before it is
  made.
- **Shadows** are one picture for each look, drawn in nine pieces,
  whatever the size of what casts them.

**Still open**, and on the roadmap:

- **A picture of a window by itself** (`portrait_area` in
  `crates/omorfos/src/capture.rs`) is as large as all the window's
  program says it draws, `Window::bbox`, which a part of a window far
  from it makes as large as it likes; and a picture that large is asked
  of the renderer (`Capture::alone`). Read, a renderer that cannot make
  one says so and the picture is not taken; to be shown with a card.
  Only a program that may take pictures, or the user, has one taken.
- **Where a surface is** is its program's to say as well, and is added
  to in whole numbers of 32 bits, in Smithay and in the compositor. A
  release build wraps where they run out, and the tests' build panics.
  Not looked at beyond that nothing found allocates by one.
- **Every window costs a titlebar** painted as wide as its frame, up to
  16384 units now: many wide windows cost memory in proportion, and
  nothing holds a program to a number of windows.

### 3. Any program can take the keyboard with a serial it made up

**Medium. Shown by a test. Fixed the same day (ADR 36, amended).**

What was changed differs a little from what is proposed below. Keeping
the last few serials of each program would honour a click from before
the user went to another program; so only the last key or button pressed
counts, and a token is honoured if that press went to the program that
made it and the token's serial is that press's or one given since,
which takes in the release that toolkits often name. A serial no one
has been given yet is refused. No more than 32 tokens are kept, the
oldest forgotten first, and none longer than a minute. The tests are in
`crates/omorfos/tests/integration/activation.rs`: a serial from before
another program took the keyboard, a serial no one was given, a panel
that was clicked, and forty tokens made in a row. That a token is
forgotten after a minute is not tested, as the time is Smithay's own
clock; the bound by number is.

`token_created` in `crates/omorfos/src/handlers.rs` honours an activation
token "only if the application tied it to something the user did", and
decides that by `data.serial.is_some()`: that a serial was given, not
that it is one the compositor gave. A program that never had a key or a
click sets serial 0, is given a token, and activates its own window,
which is then focused and has the keyboard, and with it the clipboard.
Focus is what a window may not take for itself; this takes it.

The test is the one already there,
`a_window_cannot_raise_itself_without_the_user_having_done_anything` in
`crates/omorfos/tests/integration/activation.rs`, with the one change
that the program claims a serial: `token(&mut session, &mut pushy, true)`.
It fails today: the focus goes to `pushy`.

To change: a token is honoured only if its serial is one the compositor
sent that program for a key or a button pressed, lately. Keep the last
few such serials with the client they went to (`input.rs` sends them),
and have `token_created` look the token's up there, for the client that
made the token; Smithay's `XdgActivationTokenData` has the serial and
the seat, and the client is the token object's. A token made by the
compositor itself (`Compositor::ticket`) is not asked this.

With it: tokens are never forgotten. Smithay keeps each until it is used,
and nothing calls `XdgActivationState::retain_tokens`. Forget those older
than a time, as a minute, by `XdgActivationTokenData::timestamp`, each
turn or as one is made; every browser makes them and uses few.

### 4. Descriptors run out, in the compositor and in the relay

**Medium. To be shown.**

Nothing raises the limit on open descriptors: there is no `setrlimit` in
any crate. Under a login it is 1024, of 524288 allowed. A pool of shared
memory is a descriptor the compositor keeps for as long as the pool
lasts (Smithay's `Pool`), and one the relay keeps beside it, to hand the
next compositor (`crates/omorfos-relay/src/journal.rs`, the `fds` of each
`Message`); so are a buffer of the graphics card's, each of its planes, a
keymap, and a pipe being pasted through. One program that makes a
thousand pools, each a few bytes, has taken them all, and an honest
session with many windows comes nearer to it than it should: the relay
holds every program's at once.

What then happens is to be shown by a test that runs the compositor, and
the relay, in a process whose limit was lowered: new programs cannot be
taken (`accept` fails), programs cannot be started (no pipe, no socket
pair), descriptors a program sends are lost on the way (`MSG_CTRUNC`,
which the relay takes for a broken program), and a listening socket that
cannot be accepted from stays readable, which a loop that waits on it
spins on.

To change:

- `omorfos`, `omorfos-relay` and `omorfos-hold` raise their soft limit to
  the hard one as they start (`rustix::process::setrlimit`, `Nofile`), as
  other compositors do. What they start then has it too, as lowering it
  again between `fork` and `exec` needs `pre_exec`, which is `unsafe`;
  say so in the ADR this is recorded in, with what it costs: a program
  that uses `select` on more than 1024 descriptors of its own.
- The relay holds a program to a most number of descriptors kept, and
  ends one past it, as it does one past `objects::MOST` objects.
- Failing to accept is neither the end of the session nor a loop that
  spins: say it once, and wait.

### 5. What is added to an object is kept without end

**Medium. Shown by reading.**

The relay keeps, to make a program's state again, every request that
adds to an object: `Rule::Adding` in `crates/omorfos-relay/src/journal.rs`
pushes onto `Kept::adding`, for `wl_region.add` and `subtract`, `offer`
on the three kinds of clipboard source, `zwp_linux_buffer_params_v1.add`,
`xdg_toplevel_icon_v1.add_buffer` and
`ext_image_copy_capture_frame_v1.damage_buffer`. None is counted. One
region added to a million times is a million messages kept, and Smithay
keeps as many rectangles for it in the compositor
(`RegionAttributes::rects`), and a string for each kind a source offers.
A program is held to `objects::MOST` objects, and to nothing in what each
may hold.

To change: the relay counts what it keeps of an object and ends a program
past a most, as 4096, with the error it ends one with past `MOST`; the
fuzzer's target `messages` then holds it. For a compositor with no relay,
nested or after a relay is given up, the same is to be asked of Smithay:
write it in `docs/UPSTREAM.md`.

### 6. Each kind a program offers costs the X server an atom, and the compositor a wait

**Medium. Shown by reading.**

What a program written for Wayland copies is offered to programs written
for X11 in every kind the program named: `offer_to_x11` in
`crates/omorfos/src/x11.rs` hands them all to Smithay. When a program
written for X11 asks which kinds there are (`TARGETS`), Smithay asks the
X server for an atom for each, one at a time, waiting for each answer
(`conn.intern_atom(false, …).reply_unchecked()` in Smithay's
`xwayland/xwm/mod.rs`). The X server keeps an atom for as long as it
runs. So a program that offers ten thousand kinds of its own naming has
the compositor wait ten thousand times, each time a program written for
X11 looks at the clipboard, and has the X server keep ten thousand names
for good; done again with new names, without end. The roadmap already
has the compositor and the X server each waiting on the other as a
freeze seen in the tests.

To change: `offer_to_x11` hands on at most a few dozen kinds, each no
longer than a kind is (255 bytes) and of the characters a kind is
written in, the rest left out for programs written for X11 only. Write
the waiting in `docs/UPSTREAM.md`, to be asked of Smithay with the other
waits on the X server.

### 7. What was refused is remembered without end, by a name the program chose

**Low to medium. Shown by reading.**

A program refused something it has no role for is told of to the user,
once for each thing said: `Refused::say` in `crates/omorfos/src/roles.rs`
keeps every message it has said in `told`, and the message names the
program by what `/proc/<pid>/comm` says, which a program sets as it likes
(`prctl(PR_SET_NAME)`). A program that changes its name and asks again,
for a picture of the screen for one, which every program is offered and
refused, adds a message each time: kept for good, looked through one by
one at each refusal, and each put before the user as something that
happened, in words the program chose fifteen characters of.

To change: `told` is held to a most, as 64, past which nothing more is
told until the compositor is started again, or the oldest is let go; a
program is told of once for each thing asked, by its connection and not
by its name; and the name is shown without control characters.

### 8. Every notification of idleness is set again at every key and every move

**Low to medium. Shown by reading.**

`ext_idle_notifier_v1` is offered to every program, one in a sandbox
too. Each notification a program asks for is a timer in the compositor's
loop, and Smithay sets every one again whenever the user does anything
(`notify_activity_for_wl_seat` in Smithay's `wayland/idle_notify/mod.rs`):
with as many as a program may have objects, each move of the pointer
costs a quarter of a million timers taken out and put back.

To change: a program is held to a few notifications, as 16, and ended or
left unanswered past them; and whether a program in a sandbox is to be
told when the user is away at all is to be decided, as the lock screen
and `omorfos-session` are what it is for.

### 9. Smaller things

**Low. Shown by reading.**

- **The icons of applications looked up are never forgotten.**
  `WindowIcons::entries` in `crates/omorfos/src/icons.rs` keeps an entry
  for every application identifier it was asked about, valid or not, each
  as long as a program made it. Keep those that could name a desktop
  entry only, and no more than a most.
- **Titles and identifiers are as long as they came.** Four kilobytes from
  a program written for Wayland, eight from one written for X11 (Smithay
  reads 2048 words of a property), copied each turn and shaped again at
  each change. A title of more than a Wayland message holds, told to a
  panel through `ext_foreign_toplevel_list_v1`, is to be shown by a
  test: a panel built on libwayland is expected to be sent away by it.
  Hold a title to 1024 characters and an identifier to 255 where they
  enter: `names` in `handlers.rs`, and `title` and `class` in `x11.rs`.
- **A peer of the IPC socket that never stops is read from for ever.**
  `Peer::read_lines` in `crates/omorfos/src/ipc.rs` reads until the socket
  has no more, gathering every line, within one turn. A peer is the
  user's own program, so this is no wall broken (ADR 37); read a most
  number of lines a turn from each, and leave the rest for the next.
- **A notification names a file to be read as its picture.** Any program
  on the user's bus gives a path (`image-path`, or `app_icon`), which
  `omorfos-notifications` reads and decodes. To be shown: whether an SVG
  so named can have the decoder read other files it refers to, and how
  large a picture is decoded.

## What was looked at and found sound

So that it is not looked at again for the same thing:

- **A program that does not read** is kept no more than 1 MiB for and then
  sent away (`KEPT_FOR_A_PROGRAM`, ADR 45).
- **The IPC socket** takes the user's own programs only (`SO_PEERCRED`),
  32 of them, a message of at most 64 KiB, a megabyte owed to each, and
  4096 bytes to copy.
- **What only a role allows** is refused without it, and what a sandbox
  may not be shown is not shown to it: layer surfaces, the lock, the list
  of windows, the clipboard at any time, input methods and virtual
  keyboards, the making of a sandbox.
- **Other people's files** are read only so far: desktop entries, the
  indexes of icon themes, pointers, the configuration.
- **Notifications** are held to eight of programs' at a time, 400
  characters, and a picture of sides that make sense.
- **The relay's reader** is fuzzed every night, and a program is held to
  `objects::MOST` objects.
- **The pointer held and every key given** to a program are let go by a
  key that is never the program's (ADR 57, ADR 78).

## What was not looked at

- The relay beyond what it keeps of an object: what it does when the
  compositor it speaks to says what it does not expect.
- The portal and the services on D-Bus in any depth, beyond that the
  portal's backend is spoken to by `xdg-desktop-portal` and not by
  programs.
- Buffers of the graphics card: what a program's is taken for on import.
- The hardware backend, which no test runs, and what a graphics card's
  renderer does with what it is handed: the review was made on a machine
  with no card it may use, so whatever is said here of drawing was seen
  with the software renderer.
- The numbers a program gives for where something is, as apart from how
  large: the offset of a part of a window, a pop-up's place, where a
  window says it begins in what it shows. They are added to in whole
  numbers of 32 bits, in Smithay and in the compositor, which a release
  build wraps and the tests' build panics on.
- The helpers that check a secret (`omorfos-lock`'s, `omorfos-ask`'s), and
  the session's own services.
