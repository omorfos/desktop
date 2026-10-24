---
name: ci-failure
description: How to find and fix a failing omorfos pipeline - read the run with gh, say what kind of failure it is, reproduce the runner in a container, fix and watch the next run. Use when told that CI failed, that a pipeline or a pull request's checks are red, or to look at a run's logs.
---

# Fixing a failed pipeline

## 1. Read the logs

`gh` is logged in and can read the runs.

```sh
gh run list --branch main --limit 10
gh run view <run> --json jobs --jq '.jobs[] | [.name, .conclusion] | @tsv'
gh api repos/omorfos/desktop/actions/runs/<run>/jobs --jq '.jobs[] | [.id, .name] | @tsv'
gh api repos/omorfos/desktop/actions/jobs/<job>/logs > <scratchpad>/job.log
grep -n -E "##\[error\]|FAILED|panicked|error(\[E[0-9]+\])?:|TOTAL" <scratchpad>/job.log | head -40
```

`gh run view --log-failed` prints nothing with the `gh` installed here;
the logs of each job, from the API, are whole. Where `gh` cannot be used,
the maintainer downloads the logs and unpacks them as
`~/Downloads/logs_<number>/`, a file for each job.

Note which jobs failed and which did not: both architectures or one, the
tests or the lint. GitHub names the checks by the jobs' names, which are
what branch protection asks for; the workflows in `.github/workflows/`
call them by their ids:

| Check (job id) | What it runs |
|---|---|
| Test and coverage (amd64), (arm64) (`test`) | `cargo coverage --locked`, then the doctests |
| Format and Clippy (`rust`) | `cargo fmt`, clippy with and without all features, `cargo doc` |
| Dependency audit (`dependencies`) | `cargo deny check` |
| Spelling (`typos`) | `typos` |
| Conventional Commits (`commits`) | `committed` over the commits pushed |
| CodeQL (rust), (actions) (`analyze`) | CodeQL's security queries over the code and the workflows, without building; findings go to the security tab |

## 2. Say what kind of failure it is

- **It fails here too.** Then it was committed without the gates having
  been run on that tree. Fix it, and find out how it got past.
- **Every test passes and coverage falls short, in CI only.** What the
  runner has is not what is here, and a branch runs here that does not
  there: its PipeWire is 1.0, which does not say everything 1.4 does.
  Find the lines in the container (below) with a text report, and the
  `^0` under a line that marks what did not run. Make the case reachable
  from a test, or, where no test can make the system say what it does not,
  shape the code so that the branch is not there; and say what the
  difference means for a user, in the ADR it belongs to.
- **Every test of one kind fails in CI and none here.** Something is
  missing on the runner: a package, a directory, a font. Reproduce in the
  container and fix the workflow, and `AGENTS.md` where it lists what is
  needed to build and test.
- **The nightly stress run failed.** It opens or adds to an issue
  labelled `flaky` for the architecture; its run keeps the log of each
  run that failed. Each flake found in it is a failure like those below.
- **One test fails, on one architecture, sometimes.** A race. Read what
  it waited for and did not get. Before making the test more patient, ask
  whether a session on real hardware could lose the same race: if it
  could, the fix belongs in the compositor, and the test is made to bring
  the race about for certain. `docs/TESTING.md` says what the tests may
  and may not assume about time.
- **Text is drawn differently.** CI has DejaVu and nothing else. See
  "Drawn text" in `AGENTS.md`.
- **Pull requests fail with `main`.** A pull request is tested merged with
  the `main` of its day: fix `main`, and the `dependencies` skill says how
  to have Dependabot's tried again.

## 3. Reproduce what the runner has

The runners are Ubuntu 24.04 with only what the workflow installs. The
script beside this file runs a command in that, with `podman`, as an
ordinary user as the runner does, reading the release and the packages
from `.github/workflows/test.yml`:

```sh
.claude/skills/ci-failure/reproduce.sh                      # cargo coverage --locked
.claude/skills/ci-failure/reproduce.sh cargo llvm-cov --locked -p <crate> \
  --text --output-path /cache/report.txt
podman run --rm -v omorfos-ci-24.04:/cache ubuntu:24.04 grep -n -B3 '\^0' /cache/report.txt
```

The first run downloads the packages, Rust and the crates, and takes half
an hour; the volume `omorfos-ci-<release>` keeps all but the packages, so
later runs take minutes. A package that is only recommended by another is
not there: that is how `libEGL` was missing for `Xwayland`.

Only amd64 can be run this way on the maintainer's machine. A fix for
arm64 is checked by reasoning and by the next push; say so.

## 4. Fix, and prove it

- A failure that comes only sometimes gets an issue labelled `flaky`, if
  it has none, with the run and what failed, and what was found as it is
  found. The commit that fixes it says `Fixes #<issue>` in its body.

- Run the test that failed many times over, and the whole test binary a
  few dozen times, as `docs/TESTING.md` says under "Stress"; and what
  failed in the container again.
- Run every gate (the `handover` skill lists them).
- This is a repair to a `main` that is pushed and broken, so it is
  committed on `main` without waiting to be asked, as a `fix:` or `ci:` or
  `test:` commit whose body says what failed, why, and why the fix is in
  the place it is.
- Do not push. Tell the maintainer what failed and why, what was changed,
  and what could not be verified here. Once they have pushed, watch the
  run, in the background, and say how it ended:

  ```sh
  gh run watch <run> --exit-status --interval 30
  ```
