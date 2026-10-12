# Agent guide

Read this first. It is the entry point for any AI agent (or human) picking up
work on omorfos. Deeper detail lives in `docs/`.

## What this is

omorfos is a Wayland compositor for Linux (`x86_64`, `aarch64`), written in
Rust and inspired by Fluxbox. It needs `pkg-config` and the development
files for libxkbcommon, pixman, udev, libinput, libseat and GBM to build,
and `dbus-daemon` to run the tests.
It is a fresh implementation, not a port: see
[docs/adr/0001-fresh-implementation-on-smithay.md](docs/adr/0001-fresh-implementation-on-smithay.md).

- Current state and next task: [docs/ROADMAP.md](docs/ROADMAP.md)
- Layering and crate responsibilities: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- The IPC protocol, a compatibility surface: [docs/IPC.md](docs/IPC.md)
- Decisions and their reasons: [docs/adr/](docs/adr/)

## Priorities, in order

1. **Security.** No `unsafe` in our crates (`forbid`). Treat every Wayland
   client, IPC peer and config file as untrusted input.
2. **Correctness.** 100% coverage, enforced in CI.
3. **Performance.** No work when nothing changes; no allocation in hot paths.
4. **Clean architecture.** Dependencies point inwards; the domain knows
   nothing about Smithay, the filesystem or the clock.
5. **User experience.** Sensible with zero configuration; a mistake in the
   config never breaks the session.
6. **Lean.** Every dependency and feature has to justify itself.

## Before every commit

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo coverage
cargo test --workspace --all-features --doc
cargo deny check
```

All five must pass. CI runs the same commands.

## Rules

- **Coverage is 100%** of lines, functions and regions (`cargo coverage`, an
  alias in `.cargo/config.toml`). Do not lower the gate. Code that cannot run
  in CI (real GPU, TTY, seat) is kept free of decisions and listed in
  [docs/adr/0002-coverage-policy.md](docs/adr/0002-coverage-policy.md) before
  it is excluded. Two files are excluded today: the nested winit backend
  and the hardware backend.
- **Test behaviour, not implementation.** Test names read as sentences
  describing the behaviour.
- **No panics in non-test code:** `unwrap`, `expect`, `panic!`, `todo!` and
  `dbg!` are linted. Return errors.
- **Document every public item** (`missing_docs` is on).
- **Never weaken a lint** to make code pass. A local `#[expect(..., reason =
  "...")]` with a real reason is acceptable; a blanket `allow` is not.
- **New dependencies** must pass `cargo deny check` and be justified in the
  commit body. Prefer the standard library.
- **Error paths count.** The region gate treats an untested `?`, an untaken
  `let ... else`, and the implicit else of an `if let` as uncovered. Make
  the failure reachable from a test (a real error, not a mock, where
  possible). Where it truly cannot happen, remove the branch by modelling
  the data so the impossible case does not exist, instead of hiding it.
- **Test the compositor through a real client.** `crates/omorfos/tests/integration`
  runs the compositor and a scriptable Wayland client on one thread and draws
  with the software renderer, so tests can assert on protocol events and on
  pixels. A Smithay handler method only exists if a test makes a client
  call it.
- **Wait for effects that depend on a file descriptor closing.** A client
  disconnecting and a file-change notification both happen when the last
  copy of a descriptor closes. Tests run in parallel in one process, and a
  test that starts a program briefly duplicates every descriptor, so these
  effects can arrive late. Use `Session::eventually` instead of asserting
  straight away, and stress the suite (run the test binary a few hundred
  times) after adding tests that spawn processes or watch files.
- **Drawn text depends on the installed fonts.** CI has DejaVu; your machine
  may resolve `sans-serif` to something else, with different glyph shapes.
  Tests on text must hold for any font: assert on where ink may and may not
  be, not on exact pixels. To reproduce CI, put DejaVu's `.ttf` files in a
  directory, name it in a `<dir>` element of a minimal `fonts.conf`, and run
  the tests with `FONTCONFIG_FILE` pointing at that file.
- **Configuration types validate while deserializing** (`try_from`), so parser
  errors carry a line and column. After changing them, regenerate the JSON
  Schemas: `OMORFOS_BLESS=1 cargo test -p omorfos-config --all-features --test schemas`.
- **Record significant decisions** as a new ADR in `docs/adr/`, and keep
  `docs/ROADMAP.md` current when a task is finished.

## Commits

[Conventional Commits 1.0.0](https://www.conventionalcommits.org/en/v1.0.0/),
checked by `committed` in CI.

- Subject: `type(scope): imperative summary`, lower case, no full stop, at
  most 72 characters. Scope is the crate without the prefix (`core`,
  `config`, `ipc`) or an area (`ci`, `docs`).
- Body: always present. Explain why the change is needed and what it does,
  plus any trade-off taken. Wrap at 100 columns.
- Breaking changes: `!` after the type and a `BREAKING CHANGE:` footer.
- One logical change per commit (atomic commits): each commit builds and
  passes the checks on its own.
- Releases are derived from these messages; see
  [docs/RELEASING.md](docs/RELEASING.md). A new workspace crate must be added
  to `release-please-config.json` so its `Cargo.lock` version is bumped.
