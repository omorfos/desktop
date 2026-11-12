# 74. Night light

- Status: accepted
- Date: 2026-11-12

## Context

A screen as white as daylight late in the evening is hard on the eyes,
and the light of it is said to keep people awake. Most desktops make the
screen warmer after dark, by taking away some of its blue. omorfos knows
already when it is dark where the user is: `scheme: auto` (ADR 65) works
out sunset and sunrise from `appearance.location`, or goes by the hours
of `appearance.day`.

There are two ways to make a screen warmer. Each screen has a gamma
table, through which the card sends every pixel on its way out; setting
it tints everything, after the compositor has drawn, at no cost to
drawing. Or the compositor tints what it draws, as a last step, which
costs a pass over every screen at every frame.

## Decision

**Night light is the compositor's own, through each screen's gamma
table.** `night_light` in `config.yaml`, off unless set, makes the
screens as warm as a light of `temperature` kelvin, 4000 unless set,
from 1900 to 6500, 6500 being daylight and the screens as they are.

- **Dark is as `scheme: auto` has it**, by `appearance.location` or
  `appearance.day`, whatever the scheme is set to: one place says when
  it is night. The screens turn warm over the 20 minutes after dusk and
  back over the 20 before dawn, a step each 30 seconds, worked out from
  how much of the twenty minutes behind and ahead is dark, a minute at a
  time, so that no rise or set has to be found exactly.
- **The tint is a black body's colour beside daylight's**, by Tanner
  Helland's fit to it, which is a formula and free to use; redshift's
  table, which most tools copy, is GPL's.
- **The `night_light` action switches it by hand until it next turns**:
  off for the rest of an evening, or on now, as warm as it says, where it
  is off or by day. It lasts until the schedule turns the screens warm or
  back, so that an evening it was switched off is followed by one it is
  on.
- **Each change fades in over two seconds**, easing in and out, evenly
  in mireds (the reciprocal of the temperature, which the eye sees
  change evenly): one by hand, the configuration's, and each of the
  schedule's steps, so that none jumps at the user. The first a
  compositor is asked for is there at once, as one started again by
  night finds the screens warm already, and fading in from daylight
  would flash them white.
- **The gamma table is set again whenever it may have been lost**: when
  a screen's mode changes, when it is turned back on, and when the card
  is taken back from another virtual terminal. A screen whose table
  cannot be set says so once in the session's log, not at each step.
- **Pictures of the screen and a screen shared keep their colours**, as
  the table is after them.

## Alternatives

- **Tinting as the compositor draws.** It would work on a screen whose
  table cannot be set, and on an HDR screen, but costs at every frame
  what the table does for nothing, and would tint pictures of the screen
  unless they were drawn twice.
- **`wlr-gamma-control`, for `gammastep` or `wlsunset` to do it.** Lean,
  but not there with nothing set up, which a desktop should be; and a
  program setting every screen's gamma is one more thing a sandbox has to
  be kept from. It can be offered beside this one if it is asked for.
- **A schedule of its own.** Its own times would disagree with when the
  desktop turns dark, for no gain.

## Consequences

- The compositor depends on `jiff` itself, already in the tree, for the
  moment the schedule is asked at.
- What sets the table is in the hardware backend, which no test runs;
  the schedule, the tint and the table it makes are tested.
- An HDR screen (stage 8) needs its gamma table for its own curve; there
  night light is to tint as the compositor draws instead.
