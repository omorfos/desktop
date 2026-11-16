# 95. A thread that cannot be started ends nothing

- Status: accepted
- Date: 2026-11-16

## Context

`std::thread::spawn` panics where the system starts no more threads, as
it does under the user's limit on processes (`RLIMIT_NPROC`), which every
process and thread of the user counts towards. Fifteen places outside the
tests started threads so: the compositor among them, each time the scale
changed under a running X server, which would end the session; the lock
screen, the boxes that ask for passwords, the portal backend, the
notifications daemon, the session's watcher and the clipboard's keeper.
`AGENTS.md` allows no panics in code that is not a test, but the lints
for it do not see one inside the standard library (finding 6 of
`docs/reviews/2026-11-16-the-whole-tree.md`).

## Decision

**Every thread is started by `omorfos_ipc::threads::start`, which names
it and returns the error where it cannot be started, saying so on
standard error; `clippy.toml` refuses `std::thread::spawn` elsewhere.**

- Whoever asked goes on without what the thread would have done: a
  program that cannot start a thread has the system short of processes,
  and to end it, the compositor or the lock screen above all, would leave
  the user worse off than a reply that does not come. What could not be
  done is said once, in the session's log, by the starter, in one place.
- The tests keep `std::thread::spawn`, by a local `#[expect]` on each
  module or test crate that uses it: a test ends with its threads, and
  one that cannot be started fails it, as it should.
- The starter's failure is tested once, in a process of its own whose
  limit on processes is lowered to none; the places that use it add no
  path of their own to be tested.

## Alternatives

- **The error taken where each place takes its others.** The portal
  backend, the notifications daemon and the polkit agent would fail to
  start rather than start with no listener; but each would be a path of
  its own, reached only with the limit lowered, fifteen times over.
  Saying it in one place, and going on, was preferred.
- **Running the work where it was asked for, when no thread can be
  had.** It would wait on what the thread exists not to wait on: a
  program, the X server, or the bus.

## Consequences

- A program short of threads goes on, and what it could not do is in the
  session's log. A portal backend whose listener did not start answers
  nothing, and `xdg-desktop-portal` times out as with one that is not
  there.
- The relay's listener for each sandbox, which takes the error where it
  takes the others, says it there (ADR 90), and is started by the
  standard library's `Builder` as the starter is.
