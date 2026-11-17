# 96. The coverage gate is the last commit's

- Status: accepted
- Date: 2026-11-17

## Context

`push` ran the coverage gate, five minutes and more, once on the last
commit and once more on every other commit not yet pushed, each in a
worktree of its own: with twenty commits and more made in a day, a push
of a morning's work took an hour or two. Where the tests that need a
graphics card could run, as they can on a virtual card, it ran the gate
a second time on the last commit, once with them and once without, as
CI has it. And it ran in the working copy, which was not to change
until it ended, so nothing else was done while it ran.

What a commit before the last needs is to build and pass its tests: a
bisect stops at it, and a revert takes it back alone. That a line of it
is left unreached by its own tests says nothing either needs, and the
last commit, which holds it too, is held to the whole gate.

## Decision

**The coverage gate runs on the last commit of what is pushed, once,
without the tests that need a graphics card, as CI runs it.**

- Each other commit not yet pushed passes `commit`: `cargo fmt`, clippy
  with all features, and once, without the coverage gate, the tests of
  each crate it reaches (ADR 94). `commit` writes down the tree it
  passed on, so a commit that passed it as it was made is not checked
  again; one a rebase changed is checked again by `commits`, in a
  worktree of its own, the same way. The push hook takes `commit` for
  each commit but the last.
- The tests that need a graphics card run without the coverage gate,
  where there is a card. What only the card reaches is left out of the
  gate (ADR 2), so the gate would be the same with them.
- `push` checks the last commit out in a worktree of its own, built in
  `target/commits` beside the other commits, so the working copy can go
  on changing while it runs. What passes is the commit, whatever is done
  meanwhile; a commit added meanwhile is not written down.
- Two runs of the coverage gate never run at once: `handover`, `rest`,
  `all`, `commits` and `push` each wait for one already going, through
  `flock`.

## Consequences

- A push costs one coverage run, about five minutes here, however many
  commits it holds, and the commits before the last cost seconds each,
  or nothing where `commit` passed on them.
- A commit before the last that leaves a line of its own unreached, and
  that a later commit covers, is pushed. Nothing on `main` is ever less
  than covered at its tip, which is what CI checks.
- `push` no longer builds the nightly profile: that build is for trying
  a change by hand, which `handover` does.
- `area` and `commit` may run beside a `push`; on a machine of fewer
  cores, tests that wait on a clock may time out under both, and are
  then run again alone, as a flaky test is found and not passed by.
- `gates.sh uncovered` lists what the last coverage run in the working
  copy missed; what a `push` missed is listed in `push.log`.

## Alternatives

- **The coverage gate on every commit, as before.** It holds each commit
  to more than a bisect or a revert needs, at five minutes each.
- **Run the gates in CI only, and have `main` moved on by a merge queue
  when they pass.** It frees the machine altogether, at the cost of a
  failure found ten minutes after a push rather than before. Put on the
  roadmap for after the first release.
