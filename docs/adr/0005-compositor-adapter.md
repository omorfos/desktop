# 5. Compositor adapter

- Status: accepted
- Date: 2026-10-09

## Context

The `omorfos` crate connects Smithay to the domain model. Smithay code is
event handlers full of branches, and a compositor normally needs a GPU and a
display to run at all. Both make the 100% coverage gate hard to meet
honestly.

## Decision

**The compositor is everything except the event loop and the screen.**
`Compositor` takes input as a small `Input` enum and draws through a
`render` method generic over Smithay's `Renderer`. A backend supplies the
loop, an input source and a renderer. The nested backend uses winit and
OpenGL; the tests use the pixman software renderer and inject `Input`
directly.

**Tests use a real Wayland client.** The harness connects a `wayland-client`
connection to the compositor over a socket pair and steps both sides by hand
on one thread, so every exchange is deterministic. Tests assert on the
protocol events the client receives and on rendered pixels. No protocol
mocks.

**A handler exists only if a test makes a client call it.** This keeps the
protocol surface small and deliberate. The first cut therefore leaves out
the clipboard, interactive move and resize, and popup grabs.

**The desktop model is the source of truth; Smithay's `Space` mirrors it.**
Handlers and input call `Desktop` methods. `Compositor::refresh` then syncs:
it maps the shown tab of each frame into the `Space` in stacking order,
unmaps everything else, sends each toplevel its size and states, and moves
keyboard focus. Doing this in one place after every batch of events is
simpler than updating Smithay piecemeal from each handler.

**Bindings match the key, not the symbol.** A chord is the held modifiers
plus the keysym the key produces without them, so `Super+Shift+r` works on
any layout and Shift is not counted twice. Releases of swallowed presses are
swallowed too.

**Programs are spawned directly** with `WAYLAND_DISPLAY` set, and reaped by
polling `try_wait` during `refresh`. This needs no `unsafe`, no signal
handler and no thread.

**No `unsafe` in the event loop.** The usual Smithay pattern moves the
`Display` into an event source and reaches it with an unsafe call. Instead
the backend keeps the `Display` beside the `Compositor` and registers a
duplicate of its file descriptor only to wake the loop.

**One file is excluded from coverage:** `backend/winit.rs`, which is glue
with no decisions in it. Everything it calls is covered.

## Consequences

- The whole request-to-pixels path runs in CI without a GPU, in about a
  fifth of a second for 51 tests.
- The software renderer is a dependency of the tests only, but its system
  library (pixman) is needed to build and in CI.
- Syncing after every batch does a little redundant work; Smithay only sends
  a configure when something changed, so clients see no extra traffic.
- The winit glue is unverified by tests. It has been started under WSLg but
  not exercised with real applications.
