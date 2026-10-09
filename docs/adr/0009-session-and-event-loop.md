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

**Polling is kept for now.** The session still wakes ten times a second so
that `refresh` can look at the configuration directory, the IPC socket and
the programs it started. Waking for them instead is the next step, and is
what the loop now makes possible.

## Consequences

- A backend is smaller and has less to get wrong. The file excluded from
  coverage lost its loop and its client handling.
- Tests that built a `Compositor` and a display by hand now build a
  `Session`, so they exercise the loop a backend uses.
