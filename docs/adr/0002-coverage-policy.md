# 2. Coverage policy

- Status: accepted
- Date: 2026-10-09

## Context

The project targets 100% test coverage. Part of a compositor talks to a real
GPU, TTY and seat, none of which exist on CI runners, so a literal 100% of
every line cannot be measured honestly.

## Decision

CI fails unless `cargo llvm-cov` reports 100% of lines, functions and
regions across the workspace.

Code that cannot execute in CI is kept out of that measurement only when all
of the following hold:

1. It sits behind a trait whose other implementation (headless) is covered.
2. It contains no policy: only calls into the operating system or Smithay.
3. The file is listed in the table below, with its reason, in the same
   commit that adds the exclusion.

Exclusions are made by file path in the coverage command (the `coverage`
alias in `.cargo/config.toml`), never by scattering attributes through the
code, so the full list is visible in one place.

## Excluded files

| File | Reason |
|---|---|
| `crates/omorfos/src/backend/winit.rs` | Opens a window on a host display and draws with OpenGL; CI runners have neither. It translates winit events into `Input` and draws with `Compositor::render` after each turn of the `Session`, all of which are covered through the software renderer. |
| `crates/omorfos/src/backend/hardware.rs` | Takes the seat, opens the graphics card and the input devices, and sets a mode on a real screen; CI runners have none of these. It translates libinput events into `Input` with the same function as the nested backend, and draws with `Compositor::render`. What it needs decided (which backend to use, where the pointer stops, which keys switch virtual terminal) is decided in covered code. See ADR 10. |

## Consequences

- Hardware glue must stay thin, which also keeps it easy to review.
- Doctests are not instrumented on stable Rust. They run in CI but do not
  count towards coverage, so behaviour must also be covered by unit or
  integration tests.
- Branch coverage needs nightly and is not gated yet.
