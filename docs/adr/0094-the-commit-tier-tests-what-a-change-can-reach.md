# 94. The commit tier tests what a change can reach

- Status: accepted; the gate on each commit but the last taken out by
  [ADR 96](0096-the-coverage-gate-is-the-last-commits.md)
- Date: 2026-11-16

## Context

The repository holds more programs each month: the compositor, its
components, and programs of their own, as a calculator would be. The
`commit` tier ran every test once before every commit, a minute and a
half, whatever the commit changed: a line in the roadmap or a program
nothing else uses cost as much as a change to the compositor. `commits`,
before a push, ran the coverage gate on every commit not yet pushed, five
minutes each and more, a commit of documents among them.

## Decision

**`commit` runs the tests of each crate a change can reach, and nothing
more; `commits` passes a commit that reaches none as the one before it.**
`tools/affected.sh` says which, from the paths changed:

- A path in a crate's directory reaches that crate, and each crate that
  depends on it, as a dependency of any kind, through any number of
  others, from `cargo metadata` with every feature on, as the gates
  build. A test that runs a program runs one of its own crate, which
  Cargo builds for it (`CARGO_BIN_EXE_*`); the compositor's tests run
  its components in the same process, as libraries it depends on.
- A path outside the crates reaches everything, the lockfile and the
  workspace's manifest among them, unless it is one of a short list read
  by nothing that is built or tested: the decisions, the reviews, the
  roadmap, the guides for those who work on omorfos, the skills and the
  hooks. The tests read files all over the repository: the guide, the
  manual pages, the schemas, the packaging, the workflows, the README.
  So the list is of what is known to be read by nothing, not of what is
  known to be read; one left off it costs a longer run, where one put on
  it wrongly would leave a test unrun.
- The links between documents are a test of every file there is, as one
  moved or gone breaks a link to it, and run whatever changed.
- Where what is reached cannot be worked out, the tier fails; it is
  never taken for nothing.

`affected.test.sh` tests it against the workspace as it is, in every
`commit` run, and that each path on the list is still there.

**One list, asked by every tier, and finer than crates** (added on
18 November 2026). The list was first of what is read by nothing, and
all else outside the crates reached everything: a line changed in the
README, a manual page or a workflow ran every test there is, and `push`
did not ask at all. The maintainer asked that a change be tested by what
reads it. `affected.sh` now has two lists, and `commit`, `commits` and
`push` all ask it:

- **Read by nothing** that is built or tested: the first list, with what
  was missing from it, the guides for agents, the design language, the
  list of standards, and what only a tool every tier runs is set by,
  `clippy.toml` and `rustfmt.toml` among them.
- **Read by less than everything**: each path with what it reaches. What
  is built into the programs reaches the crate it is built into and what
  depends on that, as a crate's own file does: what the schemas promise,
  the credits. The messages of `i18n/` were listed so until they were
  moved into the crate they are built into, and needed no listing. What one test or two read
  reaches those tests alone, written `crate:test`, as the guide and the
  manual pages reach the tests that hold them to the code, and the
  packaging, the README, the licences and the Makefile the test of what
  is packaged. The gates' scripts and the hooks reach `scripts`, which
  is shellcheck and shfmt where they are installed, and the check of
  the scale survey's tables, and the workflows `workflows`, the two
  checks of them.

The built-in theme needed nothing: it is a file of the crate that reads
it, and reaches what draws with it, as it must, a tint changed having
broken tests in the compositor and in each program that draws.

A path on neither list still reaches everything. What keeps a path from
being put on one wrongly is tested, where before it was only watched
for: `affected.test.sh` looks through the crates for what each reads
from outside itself, written out from two folders up or named outright
by a test at the top of the repository, and fails where such a path is
not said to reach the crate that reads it. It finds what is written as
a path; a path made up as the test runs it would not be found.

**A file the second list names is read, though the first has its
folder** (added on 20 November 2026).
[The survey of what scale other systems choose](../reviews/2026-11-19-what-scale-other-systems-choose.md)
and [ADR 100](0100-a-screens-scale-by-how-near-it-is-read.md) are a
review and a decision, and those folders are read by nothing. But their
tables were worked out by
[`tools/scale-survey.sh`](../../tools/scale-survey.sh), which holds both
documents to itself: the two are named on the second list, and reach
`scripts`, where it is run.

**The coverage gate is not narrowed, anywhere.** A change to one crate
can leave a line of another unreached: of one it depends on, whose code
its tests were what ran, as well as of one that depends on it. Showing
that a file of a crate is still covered needs every test that reaches
it, and nearly every crate is reached by the compositor's tests, which
take most of the time. So `handover`, `rest` and `push` run everything
as before, and `commits` still runs the whole gate on each commit that
reaches anything. CI runs everything on every push, and the nightly
stress run runs the suite many times.

## Consequences

- A commit of documents, of the skills or of a program nothing else uses
  goes through `commit` in seconds, and through `commits` with no
  coverage run of its own. One to the compositor, which depends on every
  component, or to a library many use, costs what it did: today the
  compositor's tests are most of the time, and nearly every crate is
  reached through it.
- `commit` is narrower than it was: a test of a crate the change cannot
  reach is not run before the commit. Nothing that is pushed has passed
  less, as `push` runs everything on the last commit, and the per-commit
  gate in `commits` is unchanged for a commit that reaches anything.
- A file a test reads that is put on the list by mistake would leave
  that test unrun before a commit, but not before a push.
- A new kind of thing a test reads needs nothing done: it reaches
  everything until someone puts it on the list, having shown nothing
  reads it.

## Alternatives

- **Narrow coverage to the crates reached too.** Unsound, as above: a
  change to the compositor could leave a function of `omorfos-ipc`
  unreached, and only a run of everything shows it.
- **Keep the coverage of each test binary from run to run, and run again
  only those built anew.** Sound where a binary's build says all it
  depends on, and it would narrow coverage too; but a binary's profile
  can only be told from another's by the build's identity, the files a
  test reads would have to be keyed as well, and the compositor's tests,
  rebuilt by nearly every change, would still be most of each run. Worth
  it when there are more programs than the compositor's tests take time.
- **Have every crate's tests list what they read.** Accurate, but a list
  to keep beside every test; the short list of what nothing reads is
  smaller, and wrong only in the direction that costs time.
