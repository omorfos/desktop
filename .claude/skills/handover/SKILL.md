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
- Write the failing test first, and after each step run the tests of what
  is being changed, which take a second or two, not the whole suite:

  ```sh
  .claude/skills/handover/gates.sh area -p omorfos-core tiles::
  ```

- After changing configuration types, regenerate the schemas and
  `schemas/promised.txt`:
  `OMORFOS_BLESS=1 cargo test -p omorfos-config --all-features --test schemas`.
  A line gone from `promised.txt` is a breaking change: its commit is
  marked with `!` (ADR 73), or CI fails it.
- Update `docs/CONFIGURATION.md` for anything a user can set or do. Its
  example and its copy of the built-in theme are checked by tests against
  the defaults.
- Tick the item in `docs/ROADMAP.md`, and add what the work showed is
  still missing.
- A library or program that is now needed, to build, to test or to run,
  is recorded everywhere at once: "What it needs" in `README.md`, the
  packages installed in `.github/workflows/`, `depends` or `recommends`
  in the `[package.metadata.deb]` of `crates/omorfos/Cargo.toml` (shared
  libraries are found by `$auto`; what is loaded or run at run time is
  not), and `depends`, `makedepends` or `optdepends` in
  `packaging/aur/PKGBUILD`.
- A decision someone would later ask "why" about gets an ADR, numbered
  after the last in `docs/adr/`, or a paragraph added to the ADR it
  belongs to.
- A test that passes the first time it runs is shown to fail with the
  fault it is about put in for one run (`docs/TESTING.md`, "A test that
  passes at once"); `docs/TESTING.md` lists what else catches tests out.
- Some changes touch the same places every time; each place is checked
  by a test or the build, but one at a time:
  - **A part of the theme**: the `Part` enum in
    `crates/omorfos-config/src/style.rs`, its doc comment saying which
    properties are used and which states it has, `Part::ALL` and its
    count, its name in `Part::name` and in the list of names in the test
    there; its rule in `crates/omorfos-config/src/default.css`, and the
    same in the copy of the built-in theme in `docs/CONFIGURATION.md`;
    the list of parts in the guide; and the `backdrop-filter` or
    `box-shadow` row of the guide if it uses them.
  - **An action**: the `Action` enum in
    `crates/omorfos-config/src/action.rs`, its doc comment (the schemas'
    words), the parse test there; its arm in `Compositor::perform`; a row
    in the guide's table of actions; the schemas blessed, which adds a
    line for `keys.yaml` and `menu.yaml` to `promised.txt`; and no
    default key unless the review of the defaults says so.
  - **A key of `config.yaml`**: the type in
    `crates/omorfos-config/src/general.rs` with `deny_unknown_fields`, a
    parse test of its default, each value and a wrong one, its export in
    `lib.rs`; the schemas blessed; the example `config.yaml` at the top of
    the guide, which is checked against the defaults, and a section of the
    guide.

## 3. Gates

In tiers, as `docs/TESTING.md` says under "Tiers". Before handing over,
the `handover` tier, on the tree as it will be committed: `typos`,
`cargo fmt`, clippy with all features, every test once under the
coverage gate, with the tests that need a graphics card where this user
may use one, and the nightly build.

```sh
.claude/skills/handover/gates.sh handover
```

The `rest` tier, clippy without features, the coverage gate without the
tests that need a graphics card, the doctests, `cargo deny`, `zizmor` and
`cargo doc`, is started in
the background as the build is handed over (step 5), and has to have
passed before anything is committed. Never two runs of the gates at
once: they share the machine, and the X11 tests time out.

The tools are in `~/.cargo/bin`. Each tier keeps its whole output in
`target/gates/<tier>.log`: read that, not a filtered copy, when a step
fails. When the coverage gate fails, the tier lists each line it did not
reach, with the file, the line and the column; `gates.sh uncovered` lists
them again from the last run. Mark a line put in only to find something
out with the word `DBG`, as `eprintln!("DBG ...")`: the `handover` tier
and the commit hook refuse a change that still has one. The hook in `.claude/settings.json`
formats a Rust file each time it is edited or written as a file, but not
one changed by a command, such as a script that rewrites it: the format
gate still counts. If the change touches anything that runs on a thread
of its own, starts a process, uses a socket or watches a file, stress the
tests that reach it as `docs/TESTING.md` says, and say in the report how
many runs passed. A test that fails only sometimes is a bug ("Flaky tests"
there), never one to run again until it passes.
If the change touches buffers of the graphics card, pictures of the screen
or sharing it, run the tests that need a card, which CI skips, as
`docs/TESTING.md` says, and say in the report whether they ran.

## 4. Build for trying by hand

The `handover` tier builds it, with the nightly profile, and says the
size of `target/nightly/omorfos`.

## 5. Report, and stop there

Leave the work uncommitted, start the `rest` tier in the background, and
tell the maintainer:

- what to try, step by step, and what they should see;
- what they might not expect: a default that changed, something that
  only real hardware can show, what was left out and put on the roadmap;
- the install command, and the size in bytes of `target/nightly/omorfos`,
  so that a stale install shows:

  ```sh
  sudo make install PROFILE=nightly
  ```

- what is next in the order;
- that the `rest` tier is running, and, once it ends, how.

Say what was not verified. Code in the hardware backend is never run by
the tests; say so when a change depends on it.

## 6. On "commit and merge"

- The `rest` tier has to have passed. If anything changed since the gates
  ran, run `gates.sh all` again, as the last thing before `git commit`.
- One logical change per commit, each of which builds and passes alone. If
  a change has to be split, split it by whole files. Never by staging
  hunks and stashing the rest: that broke `main` once. With more than one
  commit, run `gates.sh commits` before merging: it checks out each in a
  worktree of its own and runs formatting, clippy and the coverage gate
  on it alone.
- Where one file holds parts of two commits, give the first commit a
  version of the whole file that has only its part: `main`'s version with
  that part made, or, where the work went in steps, the file as it was
  after the first step, kept aside then for the purpose (copy the files a
  step touched before going on to the next). Commit it, put the final
  version back, and commit the rest. Before merging, the last commit's
  tree is the tree the gates passed on: `git rev-parse 'HEAD^{tree}'`.
- The commit hook is in `.githooks/`. If `git config core.hooksPath` does
  not say `.githooks`, set it, and say so.
- The message follows `AGENTS.md`: Conventional Commits, a body that says
  why and what and the trade-off, wrapped at 100 columns. Check it:
  `committed --config committed.toml main..HEAD`.
- Merge into `main` by fast-forward only, and delete the branch.
- Never push. The maintainer pushes. Say how far `main` is ahead of
  `origin/main`.
