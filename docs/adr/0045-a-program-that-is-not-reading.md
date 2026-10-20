# 45. A program that is not reading

- Status: accepted
- Date: 2026-10-20

## Context

The compositor sends each program what concerns it: where the pointer is
over its windows, keys, what to draw at. What it sends goes into the
program's socket, and the kernel keeps it there until the program reads.
When the socket is full, the Wayland library the compositor is built on
keeps the rest itself, but only up to 4096 bytes. Past that it
disconnects the program, and its windows go with it.

A program does not read while it is busy: loading a large file, stopped
in a debugger, or swapped out on a machine short of memory. Moving the
mouse over its window meanwhile sends it a move, of 28 bytes, for each
turn of the compositor, and the kernel counts each by what it costs it,
several hundred bytes, not by its size. A few hundred moves fill the
socket, and a few dozen more fill the 4096 bytes. So a program that was
busy for well under a second while the mouse moved over it could lose
everything not yet saved. The tests showed it first: a test's own
program, not read while the test waited, was disconnected on a slow
machine, and a window the test depended on was gone.

## Decision

**The compositor keeps up to a mebibyte for each program that is not
reading**, set once as the display is made, before any program connects:
`KEPT_FOR_A_PROGRAM` in `session.rs`. That is some 37,000 moves of the
pointer, half a minute or more of a mouse moved without a stop; what is kept is
only taken as it is needed, so a program that reads costs nothing more.
A program that never reads is still disconnected once it has been sent
that much, so that one program cannot make the compositor keep ever more.

- **The limit is set with `set_default_max_buffer_size`**, which
  `wayland-server` offers only with its `libwayland_1_23` feature. The
  feature is named for the C library, but with the backend written in Rust,
  which the compositor uses, it links nothing and asks for no version of
  libwayland. Were the compositor ever moved to the C backend, it would
  then need libwayland 1.23 or later.
- **A program disconnected is let go of the next time the compositor
  hears from any program**, as the library does it. In a session that is
  at once; a test that waits for it has another program speak.

## Alternatives

- **Leave the limit where it was.** It is the default of this library and
  of libwayland alike, but a program losing its windows for being busy
  for a moment is the worst a desktop can do to its user.
- **No limit.** One program that never read would make the compositor
  keep everything sent to it until memory ran out. Every program is
  untrusted.
- **Send a move only when the pointer has moved.** It would save only the
  moves of a mouse pushed against the edge of the screen: a mouse moving
  over a busy window moves.
- **Fold the moves a program has not read into the last one.** It would
  keep least of all, but the library does not say how much is waiting for
  a program, so the compositor cannot know when to.

## Consequences

- A program that stops reading for a while keeps its windows, and hears
  everything it was sent once it reads again.
- A program that never reads costs the compositor up to a mebibyte before
  it is disconnected, where it cost four kibibytes. Each program that
  connects can hold that much; a program that wants to cost more has to
  connect again and again, as it could already.
