# 40. One unsafe block

- Status: accepted
- Date: 2026-10-13

## Context

No crate of omorfos used `unsafe`: the workspace forbids it, and security
is the first of the priorities. Everything that needs it, mapping memory
and talking to the GPU, was left to Smithay and the libraries under it.

Giving a program a picture of the screen broke that. The program hands
the compositor a buffer in memory the two share, and the compositor
writes the picture into it. Smithay hands that memory over as a pointer
(`with_buffer_contents_mut`), since it cannot be a slice: the program can
change it at any moment. Rust has no way to write through a pointer that
is not `unsafe`. Until now the compositor had only ever read programs'
buffers, and Smithay does that reading itself.

Ways round were looked for:

- **Write through the file behind the memory**, which needs nothing
  unsafe. Smithay keeps the file to itself, and a buffer cannot be
  matched to its file from outside.
- **Take pictures only in buffers of the GPU**, which the renderer can
  draw straight into. Nothing of ours would be unsafe, but none of it
  could run in CI, which has no GPU, so the path every picture takes
  would be untested; and tools that only know shared memory would not
  work. It moves the unsafe code to where the tests cannot see it.
- **Wait for Smithay** to offer a safe way. Nothing says when, and
  screenshots, sharing the screen and the portals all wait on capture.

## Decision

One crate, `omorfos-shm`, may use `unsafe`, and holds one block of it:
a copy of one row of pixels into a program's buffer.

- **Every other crate still forbids it.** `omorfos-shm` does not take the
  workspace's lints, which a manifest takes whole or not at all. It
  writes them out again with `unsafe_code` denied in place of forbidden,
  and the block is the one place that is allowed, with the reason beside
  it. An unsafe block without its reasoning written down is an error
  there.
- **What the crate offers is safe to call with anything.** It takes the
  buffer, not a pointer, and asks Smithay for the memory itself. Before
  anything is written it checks, in arithmetic that cannot wrap round,
  that the buffer is of the format and size expected and that its last
  row ends inside the memory. That check is a function of numbers alone
  and is tested as one.
- **Why the block is sound** is written on it. In short: the memory is
  valid for its length while Smithay's callback runs; Smithay guards
  against the program making it shorter meanwhile; every row has been
  checked to lie inside it; and no reference to the program's memory is
  ever made, so a program writing at the same moment can spoil only its
  own picture.
- **It is to be taken out again.** The block carries a `TODO` saying so.
  When Smithay offers a safe way to write into a buffer, or draws into
  one itself, the crate becomes a call to that, `unsafe_code` goes back
  to forbidden everywhere, and this record is superseded.

## Consequences

- "No `unsafe` in our crates" becomes "none outside `omorfos-shm`",
  and `AGENTS.md` says so. A second block anywhere, or a second use for
  this crate, needs a decision of its own.
- The crate is small enough to read in a minute, and is the first place
  to look if memory is ever corrupted.
- Asking Smithay for a safe way to do this is on the roadmap.
