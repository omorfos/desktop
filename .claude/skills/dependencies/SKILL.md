---
name: dependencies
description: How Dependabot's pull requests to omorfos are tried, merged or closed - main green first, each rebased and checked for a second version of a crate, merged by rebase only when the maintainer says so. Use when asked about Dependabot, dependency updates or their pull requests, or when Smithay is to be updated.
---

# Updating dependencies

Dependabot opens a pull request a week for the crates
(`build(deps): ...`) and for the actions (`ci(deps): ...`), as
`.github/dependabot.yml` says. Minor and patch updates of crates come
together in one.

```sh
gh pr list --author app/dependabot
gh pr checks <number>
gh pr diff <number>
```

## 1. Main first

A pull request is tested merged with the `main` of the day it was opened
or last rebased. If `main` is red, every pull request is too, whatever it
changes: fix `main` first (the `ci-failure` skill), and once that is
pushed and green, ask for each to be tried again on top of it:

```sh
gh pr comment <number> --body "@dependabot rebase"
```

Dependabot takes a few minutes to do it. Wait until each is no longer
behind `main` and its checks have finished, in the background, before
reading what they say.

## 2. Look at each

- **A second version of a crate.** `cargo deny` fails on one
  (`multiple-versions = "deny"`), with the two versions and what asks for
  each. An update of a crate that is ours only because another of ours
  brings it in, as Smithay brings `x11rb` and `xkbcommon`, is closed with a
  comment saying so, and ignored in `.github/dependabot.yml` until that
  other crate moves. Otherwise the update waits for the crate that holds
  the old version back.
- **A listing in `skip` that no longer matches.** `cargo deny` warns of
  it: take it out of `deny.toml` in a commit of its own.
- **A major version.** Read its changelog for what it changed that we
  use. Try it here, on a branch with the pull request's changes, through
  every gate (the `handover` skill lists them); a change of code it needs
  is a commit of its own after Dependabot's.
- **An action.** The workflows pin actions by commit, with the version in
  a comment; Dependabot changes both. Nothing to try here but that its
  checks pass.

## 3. Merge, when told to

Only when the maintainer has said to merge. One whose checks are all
green is merged by rebase, which keeps `main` in a line and Dependabot's
own message, which `committed` has already passed:

```sh
gh pr merge <number> --rebase --delete-branch
```

Then `main` here is behind GitHub's. Commits of ours not pushed yet are
put on top of it, and `cargo deny check` is run again, since the lockfile
may have changed under them:

```sh
git fetch origin && git rebase origin/main
```

Never rebase what has been pushed. Say what was merged, what was closed
and why, and how far `main` is ahead of `origin/main`.

## Smithay

An update of Smithay is the one that most changes what is built: most of
the tree comes through it. When it comes, `cargo deny` says which of our
own crates it has left behind, `x11rb` and `xkbcommon` among them: update
those in the same change, take them out of the `ignore` in
`.github/dependabot.yml`, and take out of `deny.toml`'s `skip` what is no
longer there. Its changes of interface go through the compositor's tests
like any other change, and are tried by the maintainer on real hardware
before they are merged.

One that offers a protocol at another version, or a protocol more, fails
the test of what the compositor offers, on purpose: the relay knows only
what `omorfos-ipc`'s list says (ADR 60). Bring the list up to date in the
same change, the version and the note on how a program's state in it is
made again, rather than hold the update back: a fix in Smithay is not to
wait on the relay. The compositor cannot hold the version back itself
until Smithay lets it be chosen, as `docs/UPSTREAM.md` says.

An update of Smithay, or of the PipeWire bindings, may also mend what
omorfos works around. Run the checks of what it is in, in
`docs/UPSTREAM.md`: for each that shows a fault gone, take the workaround
out in a commit of its own, after Dependabot's, tick the roadmap item it
names, and take the entry out of the list.
