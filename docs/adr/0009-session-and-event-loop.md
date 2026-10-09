# 9. The session owns the event loop

- Status: accepted
- Date: 2026-10-09
- Amends: [5. Compositor adapter](0005-compositor-adapter.md)

## Context

ADR 5 gave the event loop to the backend. That kept `Compositor` free of
anything that needs a display, but it had three costs that the hardware
backend of milestone 8 would have made worse.

- The loop itself (accept clients, dispatch, refresh, flush) lived in
  `backend/winit.rs`, the one file excluded from coverage. A second backend
  would have repeated it, untested, a second time.
- The compositor could not be woken. Whatever it waited for besides its
  clients (the configuration directory, IPC peers, programs it started) was
  polled, so the backend woke ten times a second for nothing, and an IPC
  request waited up to a tenth of a second for its answer.
- Smithay's idle-notify, and anything else that needs timers, asks for a
  handle to the loop when the compositor is built. There was none to give.

## Decision

**A `Session` owns the event loop, the Wayland display and the
`Compositor`.** It is ordinary, covered code. A turn of the loop handles
whatever is ready, calls `Compositor::refresh` and flushes clients.
`Session::run` takes turns until the session ends.

**A backend adds to the session; it does not drive it.** Input devices
become sources on the loop (`Session::handle`), which call
`Compositor::input`. Drawing is a closure given to `Session::run`, called
after every turn. The nested backend is now only that.

**The loop's state is the `Compositor`.** Smithay's helpers that register
their own timers need the loop to hand them the type that handles their
protocol, which is the compositor.

**Still no `unsafe`.** The display is moved into the closure of its own
event source, which is the only place that needs the whole of it; a
duplicate of its descriptor wakes the loop. Everything else flushes through
the `DisplayHandle` the compositor already holds.

**The compositor is woken, never polled.** It holds a handle to the loop
and registers what it waits for: the configuration watcher, the IPC
listener, each IPC peer, and each program it starts (through a pidfd).
These sources only wake the loop. What happened is still worked out in
`refresh`, in one place, as before.

**Time stays a parameter.** `refresh` is still told what time it is, and
`Compositor::deadline` says when it next needs calling if nothing else
happens: the end of the quiet period before a reload, or a retry for an IPC
peer whose socket was full. `Session::run` sleeps until then. Tests keep
their own clock, so nothing in the suite waits on a real timer, with the
exception below.

**Wake-ups are tested by running the session for real.** Stepping by hand
cannot show that a sleeping loop wakes. A few tests call `Session::run`
with nothing to prompt it and wait for a saved file, an IPC request or an
exiting program to do so. A watchdog fails them after half a minute rather
than letting them hang.

## Consequences

- An idle session makes no system calls. The IPC socket answers at once;
  the roadmap's note about polling it is closed.
- A backend is smaller and has less to get wrong. The file excluded from
  coverage lost its loop and its client handling.
- `Compositor::new` takes a loop handle, so a compositor cannot be built
  without a loop. Tests that built one directly now build a `Session`.
- A source that only wakes the loop relies on `refresh` reading everything
  that is there. One that left data unread would spin. The peer and
  listener sockets and the watcher are all drained until they would block.
- `rustix` is named as a direct dependency for its `process` feature. It
  was already in the tree through Smithay.
- Reaping a child still calls `try_wait` on each one every turn; the pidfd
  only makes sure there is a turn. With the handful of programs a session
  starts this is not worth more state.
