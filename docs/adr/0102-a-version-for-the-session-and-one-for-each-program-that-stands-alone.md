# 102. A version for the session, and one for each program that stands alone

- Status: accepted
- Date: 2026-11-21
- Takes up: [ADR 6](0006-release-process.md), with
  [ADR 98](0098-which-programs-stand-alone.md)

## Context

Every crate has one version, set once in `[workspace.package]`, and
release-please makes one release of them all, tagged `vX.Y.Z`, with one
changelog (ADR 6). Nothing has been released yet.

ADR 98 settled which programs are to run where omorfos is not installed,
each "packaged by itself". Someone who runs the lock screen on Sway gains
nothing from a new version made by a fix to the compositor, which is not
in what they installed. A changelog that is mostly about a compositor
they do not run does not tell them what changed in the program they do.

Not every program can go its own way. The compositor, `omorfos-hold`,
`omorfos-relay`, `omorfos-session`, `omorfosctl` and the portal's backend
speak to one another in ways no one else does, and are replaced together
in a running session (ADR 59, ADR 60). Versioned apart, they would need
a table of which of each works with which. The libraries are published
nowhere, and no one outside this repository depends on them, so a
version of their own would be a number nobody reads.

Every push runs every test under the coverage gate, on two
architectures, and every release builds every program. Once programs
are released one at a time, most of that work is spent on what did not
change.

## Decision

**Three kinds of crate, versioned three ways.**

- **The session**: the compositor and what keeps its session (`omorfos`,
  `omorfos-hold`, `omorfos-relay`, `omorfos-session`, `omorfosctl`), the
  portal's backend, and the programs of the shell that ADR 98 calls
  omorfos's own (the bar, notifications, the welcome card,
  `omorfos-ask`, `omorfos-screenshot`, `omorfos-recording`). One
  version, as now: the workspace's, tagged `vX.Y.Z`, with
  `CHANGELOG.md` at the root.
- **A program that stands alone** has a version of its own from the
  first release in which it is packaged by itself, and not before: until
  then it is part of the session, and released with it. Its first
  version of its own is the session's at the time it leaves, so that no
  number goes backwards.
- **The libraries** have no version anyone reads. Each program builds
  them in (ADR 98), so a change to a library is released as a change to
  each program that reaches it.

**Versions are chosen as they are now.** The table in
[RELEASING.md](../RELEASING.md) holds for each program alone, by the
commits that reach it. What a program promises is what ADR 73 lists for
it: its name, its own file of settings and its schema, its theme's
parts. A change that breaks a promise of one program is breaking for
that program, and for no other.

**Tags and releases, one for each program.** A program that stands
alone is tagged `<program>-vX.Y.Z`, as `omorfos-lock-v0.4.0`, and has a
GitHub release of its own that carries only its packages: a `.deb` and a
tarball for each architecture, named after it, and an AUR package of its
own name, where the AUR is turned on. Its changelog is `CHANGELOG.md` in
its crate. The session keeps `vX.Y.Z` and its packages as ADR 6 has
them. The session's package depends on each program that stands alone
which it starts, at the least version it needs, rather than carrying
it.

**Each package carries the licences of what is in it.** omorfos's MIT
licence asks for its notice with every copy, and what each program is
built from has licences of its own. A program's package carries
`LICENSE`; `LICENSE-crates`, gathered by `cargo about` from that
program's own tree of crates rather than the workspace's; and the
licence of each picture, font or set of signs from `LICENSES/` that it
ships or draws with, and no other. `CREDITS.md` says which program
carries which, as it says what each is used for.

**Its version, licence and credits are said where it already speaks.**
Each program's `About`, which its `--help`, `--version` and manual page
are made from (ADR 77), says its licence and, where it carries any, what
it is made with that is someone else's: the manual page has a section
for them, made as the rest of it is, so that nothing is written twice.
A program without a window of its own, as the lock screen, the
wallpaper, the clipboard keeper and `omorfos-pinentry` are, says them
nowhere else: there is nowhere else to say them. An application with a
window of its own, of stage 7, has an About window as programs on every
desktop do, built once in `omorfos-ui` and filled from the same
`About`, with its name, version, licence and credits, so that what it
says and what its manual page says cannot differ.

**release-please keeps one pull request for each.** Each program that
stands alone is a package of its own in
`.github/release-please-config.json`, with its component in its tag, and
`separate-pull-requests` is set, so that each is released when it is
ready rather than all at once. Its Cargo workspace plugin bumps a
program when a library it depends on changes, which commits under the
library's directory alone would not do. Commits are scoped by crate, as
they are already, so that each changelog says what it is about.

**A release builds what it releases.** The release workflow builds and
packages, on each architecture, only the programs whose releases were
made in that run, as release-please says which they are. A release of
the session builds the session. A release of the lock screen builds the
lock screen.

**CI builds and tests what a change reaches.** `tools/affected.sh`
already says what a change can reach, and the tiers that run before a
commit ask it (ADR 94). CI asks it too:

- A change that reaches nothing that is built, as one to the documents
  alone, is not built: its spelling and links are checked, and nothing
  more.
- A change that reaches only programs that stand alone, and nothing
  that depends on them, is built, linted and tested for those programs
  alone, and the coverage gate is measured over them. Nothing else can
  reach their lines, so nothing is lost.
- Any other change runs everything, as now. ADR 94's reason holds
  there: nearly every crate is reached by the compositor's tests, and
  whether a file is still covered can only be shown by every test that
  reaches it.
- Pushes to `main` and the nightly stress run always run everything.

## Alternatives

- **One version for everything, for good.** Simplest, and what ADR 6
  chose while there was one thing to release. It makes the programs that
  stand alone change version for changes they do not contain, and
  undoes most of ADR 98's point.
- **A version for every crate.** Thirty numbers, of which the session's
  parts would need a table of compatibility and the libraries' would be
  read by nobody.
- **Narrowing the coverage gate on every pull request.** Most pull
  requests would be faster. One that left a line of another crate
  unreached would show it only on `main`, after it is merged, which is
  what ADR 94 and ADR 96 set out to prevent. It is narrowed only where
  nothing else can reach what changed.

## Consequences

- Nothing changes until the first program is packaged by itself, the
  roadmap's item of stage 9 for the lock screen, the wallpaper and
  `omorfos-clipboard`. With it come: the program's entry in
  `.github/release-please-config.json` and the manifest, its changelog,
  the release workflow's building by what was released, its own
  packaging and AUR package, CI asking `affected.sh`, and
  [RELEASING.md](../RELEASING.md) saying all of it.
- `affected.sh` has to know which crates are programs that stand alone
  and that nothing depends on. That is read from `cargo metadata`,
  which it already reads, with the list of programs in this ADR's
  terms kept in one place.
- A pull request that reaches only such a program waits minutes where
  it waited for the whole suite.
- `About` gains the program's licence and credits, and the manual
  pages a section for them; the packaging gathers `LICENSE-crates` for
  each program by itself, and `CREDITS.md` says which program carries
  which of `LICENSES/`. The About window comes with the first
  application of stage 7.
- Each program that stands alone has a README of its own (as the
  roadmap's item says), since it may be found and installed without
  the rest: what it is, where it has been tried, how to install and
  set it, and what it does without where omorfos is not. Its options
  are in its manual page, made from its `About`, and the README points
  there rather than copying them.
- Someone reading the tags reads two kinds: `v0.5.0` for the session,
  `omorfos-lock-v0.3.0` for a program on its own.
