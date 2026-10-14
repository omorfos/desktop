---
name: handover
description: The routine for a change to omorfos from start to merge - branch, implement, run every gate, build for the maintainer to try on real hardware, and commit and merge only on their word. Use when starting a roadmap item or any feature or fix, when about to say a change is ready, and when told "commit and merge".
---

# Handing a change over

The rules are in `AGENTS.md` and how to test is in `docs/TESTING.md`. This
is the order things are done in, and what the maintainer expects to be told.

## 1. Start

- Take the next open item from `docs/ROADMAP.md` unless told otherwise.
- Branch from `main`: `feat/<what>`, `fix/<what>` or `docs/<what>`.
- A repair to a `main` that is already pushed and broken (CI failing) is
  the one thing done straight on `main` and committed without being asked.
  Say so afterwards.

## 2. Implement

- The model first (`omorfos-core`, `omorfos-config`), with its tests, then
  the compositor, then a test through a real client.
- After changing configuration types, regenerate the schemas:
  `OMORFOS_BLESS=1 cargo test -p omorfos-config --all-features --test schemas`.
- Update `docs/CONFIGURATION.md` for anything a user can set or do. Its
  example and its copy of the built-in theme are checked by tests against
  the defaults.
- Tick the item in `docs/ROADMAP.md`, and add what the work showed is
  still missing.
- A decision someone would later ask "why" about gets an ADR, numbered
  after the last in `docs/adr/`, or a paragraph added to the ADR it
  belongs to.

## 3. Gates

All of them, on the tree as it will be committed:

```sh
typos
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo coverage --locked
cargo test --workspace --all-features --doc --locked
cargo deny check
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
```

The tools are in `~/.cargo/bin`. If tests were added that start processes,
watch files or start an X server, stress them as `docs/TESTING.md` says.

## 4. Build for trying by hand

```sh
cargo build --profile nightly --locked -p omorfos -p omorfosctl -p omorfos-panel \
  -p omorfos-launcher -p omorfos-notifications -p omorfos-screenshot -p xdg-desktop-portal-omorfos
ls -l target/nightly/omorfos
```

## 5. Report, and stop there

Leave the work uncommitted. Tell the maintainer:

- what to try, step by step, and what they should see;
- what they might not expect: a default that changed, something that
  only real hardware can show, what was left out and put on the roadmap;
- the install command, and the size in bytes of `target/nightly/omorfos`,
  so that a stale install shows:

  ```sh
  sudo install -m755 target/nightly/{omorfos,omorfosctl,omorfos-panel,omorfos-launcher,omorfos-notifications,omorfos-screenshot,xdg-desktop-portal-omorfos} /usr/local/bin/
  ```

- what is next in the order.

Say what was not verified. Code in the hardware backend is never run by
the tests; say so when a change depends on it.

## 6. On "commit and merge"

- If anything changed since the gates ran, run at least `cargo fmt --all
  --check`, clippy and the whole test suite again, as the last thing before
  `git commit`.
- One logical change per commit, each of which builds and passes alone. If
  a change has to be split, split it by whole files. Never by staging
  hunks and stashing the rest: that broke `main` once.
- The message follows `AGENTS.md`: Conventional Commits, a body that says
  why and what and the trade-off, wrapped at 100 columns. Check it:
  `committed --config committed.toml main..HEAD`.
- Merge into `main` by fast-forward only, and delete the branch.
- Never push. The maintainer pushes. Say how far `main` is ahead of
  `origin/main`.
