# 87. A component's tooltips

- Status: accepted
- Date: 2026-11-15

## Context

The bar and the dock of stage 5 both need tooltips: the dock a program's
name, the bar what does not fit on it. The compositor already draws one,
over the mark that says the screen is captured, as part of its own
picture. A component cannot draw outside its surface, and the panel's
surface is the height of its bar.

## Decision

**A tooltip is a popup of the component's surface, as a popover is, that
takes no grab and lets the pointer through: `Popovers::tip` in
`omorfos-shell`, beside the popover that may be open, with a slot of its
own.**

- **Placed as a popover is,** below what it is of and kept on the screen,
  turned over to hang above a bar along the bottom (the compositor does
  this for every popup, ADR 18's components among them).
- **No grab:** a tooltip is only shown. A grab would end at the next click
  anywhere, and would take the keyboard.
- **An empty input region:** the pointer goes through it to what is under
  it, so a tooltip slid under the pointer does not take the pointer from
  the bar, which would make it go and come back.
- **A slot of its own,** not the popover's: the panel takes a popover being
  open to mean the month is, and Escape and the clock act on that. Opening
  a popover takes the tooltip away.
- **When, by `omorfos_shell::tooltip::Resting`,** apart from Wayland so
  that it is tested alone: after the pointer has rested 500 ms on
  something with a tooltip, as GTK waits; at once on the next while one is
  showing, as GTK and Qt do; and not again on what was clicked until the
  pointer leaves it. None while the month is open. The component's own
  clock (`Panel::turn`'s `now`) is what it goes by, so tests do not wait.
- **Drawn by `omorfos_ui::tooltip`,** which the compositor's tooltip over
  the mark is drawn by too, as the theme's `tooltip` says.

On the panel, what has a tooltip is what the bar cannot show: the date
over the time, a label cut short whole, and over what says the session is
kept awake, what a click does. Workspaces and windows whose names fit have
none.

## Consequences

- A component's tooltip casts no shadow, though `tooltip` has a
  `box-shadow`: its surface is only as large as the box, as a popover's
  is. The compositor's tooltip over the mark has one, as it draws shadows
  itself.
- Half a second is fixed, not a setting, and stays so: the review of the
  defaults left it as it is (ADR 101, 24 November 2026).
- The panel lists every window by its title, a private one's too, and a
  cut title is whole in its tooltip: both are in pictures of the screen
  (ADR 84), as the panel is not told which windows are private.
- That the pointer goes through a tooltip is not tested by what the
  pointer does: none is placed under the pointer in the tests.
