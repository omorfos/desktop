# 80. Actions locked in a lockfile

- Status: accepted
- Date: 2026-11-14

## Context

The workflows named each action by a commit, with its version in a
comment beside it, so that a tag moved by whoever holds the action's
repository could not change what runs here. That held for the actions
named, but not for those an action uses in turn: `zizmor-action` runs
CodeQL's, and `attest-build-provenance` runs `attest`, each at whatever
its tag says on the day. GitHub made workflow lockfiles generally
available on 28 October 2026: `.github/workflows/actions.lock` holds the
commit of every action a workflow uses, and of every one those use, and
the runner refuses one that is not there, or not where the lockfile says
it came from.

## Decision

**Every workflow is locked by `.github/workflows/actions.lock`**, made by
GitHub's `gh actions-lock` extension, and the workflows name actions by
their versions, as `actions/checkout@v7.0.1`.

- The lockfile is never written by hand: a changed `uses:` is locked
  with `gh actions-lock`, and Dependabot locks its own updates in the
  same pull request.
- CI checks it in a job of its own, "Actions locked", with `gh
  actions-lock --verify`, the extension pinned to the version installed
  here; the `rest` tier runs `--verify-local`, which needs no network.
- zizmor counts an action in the lockfile as pinned from 1.32.0, which
  CI and the `rest` tier run.
- When it was made, every commit locked was the one the workflows named
  before, so nothing that runs changed with it.

## Alternatives

- **Commits in the workflows, as before, and a lockfile too.** The
  extension writes versions in place of commits as it locks; keeping both
  would be two records of one thing for Dependabot to keep in step, and
  the lockfile is the one the runner checks.
- **Commits in the workflows alone.** What the actions use in turn would
  stay unlocked.

## Consequences

- An action's own container image is still not locked: `cargo-deny-action`
  builds on an image named by a tag, as it did before, which only its
  maintainers can pin by digest.
- A lockfile that moves a version such as `v4` back to an older commit is
  not caught by the check; the workflows name whole versions, so none
  moves without a change to a workflow, which is reviewed.
- Branch protection asks for "Actions locked" only once the maintainer
  adds it; until then a pull request can be merged with it failing.
