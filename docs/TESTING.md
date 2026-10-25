# Testing

How omorfos is tested, and how to write code that the coverage gate will
accept. The rules themselves are in [AGENTS.md](../AGENTS.md) and the
reasons in [ADR 2](adr/0002-coverage-policy.md).

## Where tests go

| What is tested | Where | How |
|---|---|---|
| The model: desktop, menus, keys, placement, decoration | `crates/omorfos-core/src/*.rs`, in `mod tests` | Plain values in, plain values out. No Smithay, no files, no clock. |
| Configuration | `crates/omorfos-config/src/*.rs`, in `mod tests` | Parse YAML or CSS text; assert on the value or on the message. |
| The compositor | `crates/omorfos/tests/integration/*.rs` | A real compositor and a real Wayland client on one thread, drawn in software. |
| The panel, launcher and notifications | Their own crates, and through the compositor in `integration/` | |
| The schemas and the guide | `crates/omorfos-config/tests/` | The schemas in `schemas/` and the examples in `docs/CONFIGURATION.md` are compared with the code. |

Put a behaviour in the model if it can live there: it is then tested
without a compositor, and the compositor's test only has to show that it
is wired up. The compositor only has a Smithay handler if a test makes a
client call it.

Test names are sentences about behaviour:
`a_submenu_closes_when_the_pointer_moves_to_another_row_of_its_parent`.

## Tiers

Running every test takes about a minute and a half under the coverage
gate, and the gates run it twice; one module of tests takes a second.
So the gates come in tiers, each stopping at the first failure, and the
slowest are run while the maintainer tries a change rather than before.
`.claude/skills/handover/gates.sh` runs each.

| Tier | When | What | About |
|---|---|---|---|
| `area <filter>...` | While writing a change, after each step | The tests of what is being changed: a crate (`-p omorfos-core`) or tests of the integration binary by name (`tiles::`) | 1–2 s, and the build |
| `handover` | Before a build is handed over to be tried by hand | `typos`, `cargo fmt`, clippy with all features, every test once under the coverage gate, with those that need a graphics card where there is one, the nightly build | 2 min |
| `rest` | Started as the build is handed over, while it is tried | Clippy without features, the coverage gate as CI runs it, without the tests that need a graphics card, the doctests, `cargo deny`, `zizmor`, `cargo doc` | 2 min |
| `commits [base]` | Before merging more than one commit | Each commit after `base` (`main`) alone, in a worktree of its own: formatting, clippy, the coverage gate | 2 min a commit, more for the first |
| `uncovered` | When the coverage gate has failed | Each line of the last run's regions not reached, by file, line and column | seconds |

A failed coverage gate lists what it did not reach in the tier itself.
`handover` also refuses a line of Rust with the word `DBG` in it, which
is how a line put in only to find something out is marked, as does the
commit hook in `.githooks/` once `git config core.hooksPath .githooks`
has told git where it is.

Nothing is committed until `handover` and `rest` have both passed on
the tree as it is committed, and, for more than one commit, `commits`. CI runs them all again on every push, and
the nightly stress run (see "Flaky tests") runs the suite many times
over.

The failing test comes first, in `area`, and so does what a change is
likeliest to break: its own module, then those beside it. A whole run
finds what the area missed; it is not where a failure should first be
seen.

## The integration harness

`crates/omorfos/tests/integration/common.rs` has a `Session` (the
compositor, a 640 by 480 screen drawn with the software renderer, and its
own configuration directory) and a `Client` (a scriptable Wayland client).

```rust
let mut session = Session::with_config(&[("menu.yaml", MENU)]);
let mut client = session.connect();
session.open(&mut client, "app", (200, 100), RED);   // a window, on screen

session.compositor_mut().input(Input::Motion { x: 300.0, y: 200.0 }, 0);
session.step();

assert_eq!(session.draw().at(510, 218), PANEL);       // a pixel, 0xAARRGGBB
let events = client.take_events();                    // what the client was told
```

| To | Use |
|---|---|
| Run the compositor's side once | `session.step()` |
| Exchange everything with a client | `session.roundtrip(&mut client)` |
| Open a window, a decorated one, a layer surface | `session.open`, `open_decorated`, `open_layer` |
| Send input | `session.compositor_mut().input(Input::…, time)` |
| Do what a key binding would | `session.compositor_mut().perform(&action)` |
| Look at the screen | `session.draw().at(x, y)`; `draw_on(index)` for another output |
| Change the configuration while running | `session.write_config(name, contents)`, then `eventually` |
| Ask over IPC | `session.listen()`, `session.ask(…)` |
| Prove the event loop wakes by itself | `session.run_until(…)` |

### Time

The session's clock is `session.now`, and it stands still. Nothing takes
time unless the test says so.

- `session.advance(by)` moves the clock and steps.
- `session.settle()` steps, then moves the clock on a second: enough for
  anything the built-in theme fades in or changes the colour of.
- A theme written by a test (`with_config`, `write_config`) has
  `* { transition: none; }` added, so things are there at once. A test of
  a transition says how long it is and uses `advance`, and `half_way` and
  `between` to say what a pixel on its way looks like.

### Waiting

Some effects come late, and not because of the clock: a client's socket
closing, a file having been written, a process having exited. Each waits
for the last copy of a file descriptor to close, and another test that is
starting a program at that moment holds copies of every descriptor for an
instant.

- Use `session.eventually(|session| …)` for these. It steps, moves the
  clock 200 milliseconds and sleeps a millisecond, until the condition
  holds.
- Do not assert straight after the cause. It will pass here and fail on a
  slower machine.
- Write a script a test runs with `runnable`, not `std::fs::write`. A
  file cannot be run while any copy of a descriptor writing it is open,
  so one written by the tests' own process sometimes fails to start with
  "Text file busy".
- Where the clock must not move while waiting (how often something may be
  restarted goes by it), wait with real time only, as `wait` in `x11.rs`
  does.
- Give input once, before waiting, not on every turn of the wait. A
  surface that comes up under the pointer is told it is there without the
  pointer moving again. Every move, click or key is sent to the window it
  lands on, and a test's own client reads nothing while it waits: a few
  hundred turns fill its socket on a slow machine, and the compositor
  disconnects it, taking its windows with it.
- Before making a test more patient, ask whether the compositor could
  lose the same race in a real session. If it could, that is a bug in the
  compositor, and the test should force the race, not wait it out.

### Pixels and text

- Windows are filled with one colour, so a pixel says which window, or
  which part of a frame, is there.
- Text depends on the fonts installed. Assert where ink may and may not
  be, never on exact pixels. CI has DejaVu only; `AGENTS.md` says how to
  have the same.

### Programs written for X11

`x11.rs` starts a real `Xwayland` and talks to it with `x11rb`.

- X servers take the first free number, so tests that start one hold
  `STARTING` to read (`start`, `session`), and a test in which one is
  started again holds it to write (`session_alone`).
- A call to the X server that waits for an answer is made inside
  `meanwhile`, which keeps the compositor running until it returns.

### The graphics card

What depends on the memory of a graphics card is tested where there is
one, in the same files as everything else, through the `gbm` crate Smithay
re-exports: `gpu.rs` makes a buffer on the card and hands it over, and the
software renderer draws it. A test that needs a card is
`#[ignore]`d, with that as the reason, so that where it is not asked for,
as in CI, whose runners have none, it is reported as ignored and not as
passed. Asked for where `/dev/dri/renderD128` cannot be opened, it fails.
The coverage gate is measured in CI, so what only a card reaches is kept
out of decisions, as the hardware backend is.

Run them on a machine with a card, as the user logged in to it, whenever
a change touches buffers of the card, pictures of the screen, or sharing
it:

```sh
cargo test -p omorfos --all-features --locked --test integration -- --include-ignored gpu:: sharing::
```

The `handover` tier asks for them where this user may use a card, and
says so where they may not. What a card reaches is then measured by the
coverage gate here, and not in CI, so 100% here can hide what CI finds
wanting. The `rest` tier measures it as CI does, without them:

```sh
cargo coverage --locked
```

## Stress

After adding a test that starts a process, watches a file or starts an X
server, run it many times, and the whole binary some dozens:

```sh
bin=$(cargo test -p omorfos --all-features --locked --test integration --no-run 2>&1 \
  | grep -o 'target/debug/deps/integration-[0-9a-f]*')
for i in $(seq 1 100); do "$bin" x11:: >/dev/null 2>&1 || echo "failed on run $i"; done
for i in $(seq 1 25); do "$bin" >/dev/null 2>&1 || echo "failed on run $i"; done
```

## Flaky tests

A test that fails only sometimes, or a gate that falls short only
sometimes, is a bug: in the test, or in what it tests if a session could
lose the same race. It is never run again until it passes, by hand or by a
tool that retries.

- **Covered every time, not most times.** A path a test is meant to reach
  is reached because the test waits for something only that path does
  before it goes on, not because threads happened to run in that order.
  Coverage that falls short only sometimes is a test reaching something by
  luck. Where nothing can be waited for, shape the code so that the path
  does not depend on the order.
- **Stressed before it is handed over.** A change to anything that runs on
  a thread of its own, starts a process, uses a socket or watches a file
  is stressed as above, and the report says how many runs passed.
- **Every night**, `.github/workflows/stress.yml` runs the whole suite 30
  times and the coverage gate 3 times, on both architectures. A failure
  opens an issue labelled `flaky` for that architecture, or adds to the one
  that is open, and the logs of each run that failed are kept with it for
  30 days. One seen on a push gets an issue of its own, with the run.
- **Fixed, then closed.** The commit that fixes a flake says `Fixes #N` in
  its body, which closes the issue once it is on `main`.

## Writing code the gate accepts

`cargo coverage` fails under 100% of lines, functions and regions. A
region is any piece of code that can run or not run by itself, so these
count as uncovered if no test goes that way:

| Uncovered | Do instead |
|---|---|
| A `?` whose error no test produces | Make a test produce the error, a real one. |
| The `else` of a `let … else` that is never taken | The same; or, if it cannot happen, see below. |
| The implicit else of an `if let` that always matches | The same. |
| A closure that is never called | Call it from a test, or do without it. |
| A match arm that cannot be reached | Remove it, by making the case not exist. |

**Where it cannot happen, model the data so that it does not exist.** Do
not hide the branch, and do not write a test that fakes it.

- Two options that are always both there or both absent are one option of
  a pair: `a.zip(b)`.
- A value that is always present after a certain point is not an `Option`
  after it: pass it in.
- Combinators (`map`, `and_then`, `filter`, `then_some`, `is_some_and`,
  `unwrap_or_default`) have no branch of their own to cover, where
  `if let` and `match` do. `(!hidden).then_some(&window)` and not
  `Some(&window).filter(|_| !hidden)`, which clippy refuses.
- A wildcard arm is covered by any case that reaches it. An arm for one
  case that never comes is not.

To see what is not covered:

```sh
cargo coverage --locked 2>&1 | awk '$1 ~ /\.rs$/ && ($4 != "100.00%" || $7 != "100.00%" || $10 != "100.00%")'
cargo llvm-cov --workspace --all-features --show-missing-lines \
  --ignore-filename-regex='crates/omorfos/src/backend/(winit|hardware)\.rs'
```

Only the two backends that need a real screen are left out, and they are
kept free of decisions: what they need decided is decided in code that is
covered.

## Lints that shape the code

Clippy's pedantic lints are errors. The ones met most often, and what is
done about them, which is never to allow them:

| Lint | What is done |
|---|---|
| `too_many_lines` (100) | Take a part out into a function that has a name worth reading. |
| `too_many_arguments` (7) | Group what belongs together into a struct or a pair. |
| `struct_excessive_bools` (3) | Group the booleans that are one idea into a struct of their own. |
| `unwrap_used`, `expect_used`, `panic` | Return the error. In test support files: `#![expect(clippy::unwrap_used, reason = "test support code fails the test on any error")]`. |
| `similar_names` | Other names. |
| `missing_docs` | Document it. |

## After changing configuration types

```sh
OMORFOS_BLESS=1 cargo test -p omorfos-config --all-features --test schemas
```

and update the example in `docs/CONFIGURATION.md`, which a test compares
with the defaults.
