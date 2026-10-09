# 11. Scaling

- Status: accepted
- Date: 2026-10-09

## Context

Screens differ in how many pixels they pack into a centimetre. Drawn one
pixel to a pixel, a titlebar that is comfortable on a desktop monitor is a
couple of millimetres tall on a laptop with a dense panel. The first run on
real hardware was on such a laptop.

Dense screens rarely want a whole-number scale: twice the size is often
too big and once too small. Wayland has a protocol for telling an
application the exact fraction, and one for an application to say what
size its buffer stands for.

## Decision

**Everything is laid out in units, and the output has a scale that turns
them into pixels.** The domain model, placement, hit testing, the pointer
and the configuration never see pixels. Only two places do: drawing, and
the size the backend reports for its screen. At a scale of one a unit is a
pixel, so nothing changes for existing setups.

**A scale is a whole number of units of 1/120, kept to quarters between 1 and 4.**
That unit is what the fractional-scale protocol speaks, so the compositor
and an application rounding the same length agree. Quarters are steps
every toolkit that scales at all can match. `Scale` does the arithmetic in
integers, rounding positions, not lengths, so that things that touch in
units still touch in pixels.

**The scale is automatic unless configured.** `output.scale` in
`config.yaml` is `auto` or a number. For `auto` the backend suggests one:
the hardware backend from the screen's width in pixels and millimetres,
aiming for the size things have at 120 dots per inch, and the nested
backend from the host desktop. A screen that does not say how big it is
gets one. The setting is under `output` so that settings for particular
screens have somewhere to go when there are several.

**The compositor's own drawing is painted at the scale, not stretched.** A
`Canvas` is described in units and has as many pixels as the scale gives
it, so titlebars, the menu, the notice and the pointer are sharp at any
scale. They are repainted when the scale changes, like any other change to
their appearance.

**Applications are told the scale three ways,** because they differ in
what they understand: the exact fraction (fractional-scale, with
viewporter so they can act on it), the next whole number up
(`wl_surface.preferred_buffer_scale`), and the output they are on
(`wl_surface.enter`, for those that read the output's scale). Every
surface is told from one place, each time the scene is brought up to
date; each of the three only sends when its answer changed.

## Consequences

- An application that understands none of this is stretched, and looks
  soft. One that only does whole scales draws at the next one up and is
  shrunk, which looks better than stretching.
- At a fractional scale two things that touch in units can end up a pixel
  apart or overlapping on screen, where one is positioned by rounding its
  corner and another by rounding its size. Frame borders against the
  titlebar are the visible case. Not yet addressed.
- The automatic choice is a guess from what the screen reports, and
  screens misreport. That is why it can be overridden.
- Pointer motion from a mouse is in units, so the pointer covers the same
  share of the screen for the same movement at any scale.
- One scale for the one output. With several screens each will need its
  own, and windows that straddle two will need a rule.
