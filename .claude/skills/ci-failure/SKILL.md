---
name: ci-failure
description: How to find and fix a failing omorfos pipeline from the logs the maintainer downloads. Use when told that CI failed, that a pipeline is red, or to look at logs in ~/Downloads.
---

# Fixing a failed pipeline

## 1. Read the logs

The maintainer downloads the logs of a run and unpacks them as
`~/Downloads/logs_<number>/`. Take the newest. There is one file for each
job at the top, and a directory for each job with a file for each step.

```sh
ls -dt ~/Downloads/logs_*/ | head -1
grep -rn -E "##\[error\]|FAILED|panicked|error(\[E[0-9]+\])?:" <directory> | head -40
```

Note which jobs failed and which did not: both architectures or one, the
tests or the lint. The jobs are in `.github/workflows/`:

| Job | What it runs |
|---|---|
| Test and coverage (amd64, arm64) | `cargo coverage --locked`, then the doctests |
| rust | `cargo fmt`, clippy with and without all features, `cargo doc` |
| dependencies | `cargo deny check` |
| typos | `typos` |
| commits | `committed` over the commits pushed |

## 2. Say what kind of failure it is

- **It fails here too.** Then it was committed without the gates having
  been run on that tree. Fix it, and find out how it got past.
- **Every test of one kind fails in CI and none here.** Something is
  missing on the runner: a package, a directory, a font. Reproduce in a
  container (below) and fix the workflow, and `AGENTS.md` where it lists
  what is needed to build and test.
- **One test fails, on one architecture, sometimes.** A race. Read what
  it waited for and did not get. Before making the test more patient, ask
  whether a session on real hardware could lose the same race: if it
  could, the fix belongs in the compositor, and the test is made to bring
  the race about for certain. `docs/TESTING.md` says what the tests may
  and may not assume about time.
- **Text is drawn differently.** CI has DejaVu and nothing else. See
  "Drawn text" in `AGENTS.md`.

## 3. Reproduce what the runner has

The runners are Ubuntu 24.04 with only what the workflow installs. To have
the same here, with `podman`:

```sh
podman run --rm -it -v "$PWD":/src:ro ubuntu:24.04
```

Inside, install exactly the packages named in the failing job of
`.github/workflows/test.yml` (with `--no-install-recommends`, as it does),
and `build-essential pkg-config curl ca-certificates`; do the other steps of
the job; install Rust with rustup; copy `/src` somewhere writable and run
the command that failed. A package that is only recommended by another is
not there: that is how `libEGL` was missing for `Xwayland`.

Only amd64 can be run this way on the maintainer's machine. A fix for
arm64 is checked by reasoning and by the next push; say so.

## 4. Fix, and prove it

- Run the test that failed many times over, and the whole test binary a
  few dozen times, as `docs/TESTING.md` says under "Stress".
- Run every gate (the `handover` skill lists them).
- This is a repair to a `main` that is pushed and broken, so it is
  committed on `main` without waiting to be asked, as a `fix:` or `ci:` or
  `test:` commit whose body says what failed, why, and why the fix is in
  the place it is.
- Do not push. Tell the maintainer what failed and why, what was changed,
  and what could not be verified here.
