# 86. One reader of sums, for the launcher and the calculator

- Status: accepted
- Date: 2026-11-15

## Context

The launcher works out a sum as it is typed (stage 6), and stage 7 is to
bring a calculator: the same sums, with a history, a memory, and the
functions and constants the launcher leaves out. The maintainer asked that
the two not each have their own way of reading a sum, so that a sum comes
out the same in each, and that a gap closed in one is closed in both. The
question was whether the launcher should ask the calculator for its
answers, or both should share the code.

The calculator is optional (stage 7): a package of its own, recommended
and never needed, and installable on any desktop. The keyboard's
calculator key opens it where it is installed and the launcher where it
is not, so the launcher has to work out sums without it. And the launcher
works out a sum at every key pressed, where waiting on another program
would be work done on every key for nothing the launcher cannot do itself
(the third priority).

## Decision

**Sums are read and worked out in one crate, `omorfos-sums`, which the
launcher uses and the calculator is to use. Neither works out a sum any
other way.**

- The crate knows nothing of Wayland, the session, the configuration or
  the user's language: it takes what is typed, and how numbers are
  written (a decimal sign and what may stand between thousands), and
  gives the answer as text. So the calculator can take it up anywhere it
  is installed, as it takes nothing of omorfos's session with it.
- What either program needs of a sum that the other does not, as the
  calculator's history and memory, stays in that program. What reading a
  sum needs, as a function, a constant or a way of writing a number, is
  added to the crate, and so to both.
- The launcher does not start, call or depend on the calculator. Once the
  calculator exists, what the launcher may offer of it, as opening a sum
  there to go on with it, is the calculator's work in stage 7.

How numbers are read, decided with it, since both programs read them so:

- **As the locale for numbers writes them**: the first of `LC_ALL`,
  `LC_NUMERIC` and `LANG` that is set, as the C library takes it, not the
  language of the messages, so that someone who reads English and counts
  in Polish is read as they count. Whether a locale writes a comma for
  the decimals is a table of the crate's own, from the Unicode CLDR: the
  C library's `localeconv` would need `unsafe`, is not safe to call while
  another thread sets the locale, and answers only for locales that are
  installed. A locale the table has wrong is a line added to it.
- **What stands between thousands is read only where three figures follow
  it**, and a point not before three figures is the decimals even where a
  comma is, as a keypad types a point: so `2 3` is still no number, and
  `1.5` is one and a half in Polish too. The answer is written with the
  decimals marked as they are read, so that it pastes where it was typed.
- **A percentage alone added or taken away is of what it is added to**, as
  on every desktop calculator: `200+10%` is `220`, where the arithmetic
  alone would make it `200.1`. Anywhere else it is a hundredth.

## Alternatives

- **The launcher asks the calculator.** One program works out every sum,
  but the launcher would work out none where the calculator is not
  installed, and would wait on another process at every key. Rejected.
- **Each its own, kept alike by tests.** The tests would have to hold
  every case twice, and a gap closed in one would stay open in the other
  until someone noticed. Rejected.
- **Left in the launcher, and the calculator built on the launcher's
  crate.** It would take the launcher's Wayland client and everything it
  needs with it, which a program installable on any desktop should not.
  Rejected.

## Consequences

- One more crate in the workspace, with no dependencies, released with
  the rest (`release-please-config.json`).
- The crate is a candidate for stage 9's crates offered to others; until
  then what it promises is what omorfos's own programs need of it.
- A change to how sums are read is a change to both programs at once, and
  is tested once, in the crate.
