# 41. A second place for unsafe: PipeWire's buffers

- Status: accepted, and to be undone as soon as it can be
- Date: 2026-10-14

## Context

Sharing the screen worked, and was not good enough. A screen four
thousand pixels across was read back from the GPU, copied into memory the
portal backend made, copied again out of it, and copied a third time into
memory PipeWire made for whatever read the stream, sixty times a second
and more. The pointer was drawn into every picture, so each movement of
the mouse made a new one. The desktop lagged while anything was shared.

What the desktops that do this well do was looked at: Mutter, KWin and
`xdg-desktop-portal-wlr`. They all have the compositor put each picture
straight into memory that is shared with what reads the stream, and they
pace pictures by the screen and by what the reader takes. The best of
them hand buffers of the GPU over and copy nothing at all.

For that the backend has to hold a buffer of the stream while the
compositor fills it, which is longer than PipeWire's callback lasts, and
it has to make the memory of each buffer itself, as a file the compositor
can be given: PipeWire's own memory for a stream is one block it does not
name. The Rust bindings to PipeWire (`pipewire` 0.10) offer neither
safely. A buffer to be kept is handed out only as a raw pointer
(`dequeue_raw_buffer`, `queue_raw_buffer`), and so is one being made
(`add_buffer`, `remove_buffer`). Rust cannot read or write through a raw
pointer without `unsafe`.

Ways round were looked for:

- **Copy, as before.** Safe, and the lag the user would not accept.
- **Talk to PipeWire some other way**, such as through GStreamer. A far
  larger dependency, with `unsafe` of its own under it, to do less.
- **Wait for the bindings** to offer a safe way. Nothing says when, and
  sharing the screen is what calls, the browser and much else need.

## Decision

A second crate, `omorfos-pipewire`, may use `unsafe`, as `omorfos-shm`
does (ADR 40), and only for this. Everything else still forbids it.

- **It is fenced the same way.** The crate writes the workspace's lints
  out again with `unsafe_code` denied in place of forbidden; each block
  is allowed where it is, with the reason beside it; every block says why
  it is sound, and does one unsafe thing.
- **What it offers is safe to call with anything.** `Removals::furnish`
  gives each buffer a file of its own as PipeWire makes it, and lets the
  file go as PipeWire takes the buffer away. `Held` is a buffer taken
  from a stream: it keeps the stream alive, is given back to it however it
  is let go of, is marked as holding nothing until a picture is put in
  it, and is not touched once PipeWire has taken it away, which PipeWire
  says it does before it frees one. Nothing is handed out that outlives
  what it points at: the file is copied, not lent.
- **Every block carries a `TODO`** saying that it is to go, and naming
  this record.
- **It is to be taken out as soon as it can be.** The wish is plain: no
  `unsafe` in omorfos at all. A safe way to hold a stream's buffer and to
  give one memory, in the bindings themselves, is on the roadmap to be
  offered to them. When there is one, this crate becomes calls to it,
  `unsafe_code` goes back to forbidden, and this record is superseded.

## Consequences

- "None outside `omorfos-shm`" becomes "none outside `omorfos-shm` and
  `omorfos-pipewire`", and `AGENTS.md` says so. A third would need a
  decision of its own, and the bar for one is higher for there being two.
- The compositor writes each picture once, into memory the reader maps,
  and the backend copies nothing. Pictures are taken only while something
  reads the stream, no more often than sixty times a second, and one at a
  time.
- The next step, buffers of the GPU, goes through the same crate: a
  buffer made there can as well be a GPU buffer as a file.
- The crate is small, and is the first place to look if a stream ever
  shows something it should not, or the backend ever crashes.
