# 115. A tooltip stays while pointed at, goes at Escape, and is shown to the keyboard

- Status: accepted
- Date: 2026-11-28

## Context

Finding 6 of the review of 26 November 2026 (#180), after ADR 114: a
tooltip comes after half a second and goes as the pointer leaves what it
is of (ADR 87), as nowhere on it takes the pointer. Someone who reads
through a magnifier, and must bring the pointer to the words, loses it as
they do. None goes at Escape, and the bar cannot be reached by the
keyboard at all, so the date, a title cut short and what a click on
"Awake" does are told only to a pointer (WCAG 1.4.13, 2.1.1).

The keyboard is in a window while the pointer rests on the bar: Escape
goes to the window, not to the panel. And a layer that accepts the
keyboard has it from a click on it until a click elsewhere, so a bar that
accepted it would keep it after every click on a workspace or a window.

## Decision

**A tooltip takes the pointer, and stays while it is on it or on what it
is of; the compositor closes a tooltip at Escape; and a `focus_panel`
action gives the bar the keyboard, on which the ringed item shows its
tooltip.**

- **Staying.** A component's tooltip takes the pointer, as it did not
  (ADR 87): the pointer on it is on what it is of, so it neither goes nor
  comes back as the pointer crosses into it. It hangs against what it is
  of, with nothing between. A click on it takes it away, as a click on
  what it is of does.
- **Escape.** While a component's tooltip is shown, a popup of a layer of
  a program with the panel's role that takes no grab, the compositor
  takes that Escape, press and release, and closes the tooltip with the
  standard `popup_done`: after "About" and a program given every key
  have had it, and only while no layer has the keyboard. Each tooltip is
  told once, so one that stays takes no more Escapes. The panel keeps
  it away until the pointer leaves what it was of, as after a click, so
  the next Escape goes to the window again. The compositor's own tooltip
  over the mark goes the same way.
- **The keyboard.** `focus_panel` gives the keyboard to the bar of the
  screen the pointer is on, as `focus_notifications` gives it to a
  notification: to a layer of namespace `panel` that accepts the
  keyboard. A click on such a layer does not give it the keyboard, so
  clicking the bar leaves the keyboard where it was. On the bar, Tab and
  the arrows move a ring over what can be pressed, and what says which
  radios are off; the ringed item shows its tooltip at once, and Enter
  or Space press it as a click does.
  Escape, or Enter on a workspace or a window, gives the keyboard back:
  the panel says it no longer accepts it, and the compositor takes it
  away. The month, opened by Enter on the clock, takes the keyboard as
  it does now.
- **No default key**: ADR 101's review of the defaults stands. The guide
  says how to bind one.

## Alternatives

- **Escape only while the keyboard is on what the tooltip is of:** Escape
  never leaves the window it was meant for, but a tooltip under a
  pointer at rest is then not to be put away without moving it.
- **The panel told to take the keyboard over IPC**, with an exclusive
  layer while it has it: a new message, where a rule of the compositor's
  about one namespace does.
- **Giving the keyboard to a bar that never asked for it:** layer shell
  says a layer that does not accept the keyboard is never given it.

## Consequences

- A tooltip now covers what is under it for the pointer as well as for
  the eye; it is small, below the bar, and goes at a click or at Escape.
- An Escape is taken from the window only while a tooltip is shown, and
  only once for each.
- A panel of someone else's in namespace `panel` that accepts the keyboard
  is no longer given it by a click; `focus_panel` gives it.
