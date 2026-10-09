# 12. The pointer's picture

- Status: accepted
- Date: 2026-10-09

## Context

On its own screen the compositor draws the pointer (ADR 10), and at first
it was always one arrow painted in code. Applications could not change it,
so there was no I-beam over text or hand over a link, and nothing showed
that a border could be dragged.

Three parties have a say in what the pointer looks like: the application
under it, the compositor where the pointer is over something of its own,
and the user, who has cursor themes installed and may prefer one.

## Decision

**The compositor decides who has the say, from where the pointer is.**
Over an application's surface it is whatever that application last asked
for. Everywhere else, and during a drag wherever the pointer goes, it is
the compositor's: a resize arrow over a border, a closed hand while a
frame or tab is held, and the ordinary pointer over the desktop, the
titlebar, the menu and the notice. This is one function,
`Compositor::cursor_wanted`, and is tested through pixels.

**Applications are heard both ways they can speak.** They may attach a
picture of their own (`wl_pointer.set_cursor`), which is drawn with its
hotspot at the position, told the scale and sent frame callbacks like any
surface. Or they may name a shape (cursor-shape), which the compositor
draws from its theme; this is what lets an application's pointer match the
rest of the desktop. Either can hide the pointer.

**Named shapes come from Xcursor themes, read by our own lookup.** The
`xcursor` crate parses the files. Finding them is done here instead of by
that crate's theme loader, which reads the environment itself; ours is
given its directories, so it can be tested against a theme made up for the
test. It follows a theme's `Inherits` and ends at `default`, within a
limit so that themes naming each other cannot loop.

**Theme files are untrusted.** They are whatever the user installed. Files
are size-capped, one that does not parse is not used, and a theme's name,
ours or inherited, must be a plain directory name so that it cannot lead
out of the theme directories.

**There is always a pointer.** A shape the theme lacks is shown as the
ordinary pointer; with no theme at all, the painted arrow remains. So a
missing or broken theme costs prettiness, never the pointer.

**The picture is chosen for the scale, not stretched.** A theme holds each
shape at several sizes; the one nearest the configured size times the
scale is used and drawn pixel for pixel.

**The nested backend draws the pointer too,** and hides the host's over
its window, so that what applications ask for is visible there as well and
both backends behave alike.

**Programs the compositor starts are given `XCURSOR_THEME` and
`XCURSOR_SIZE`,** so that those which draw their own pointer use the same
one.

## Consequences

- `theme.yaml` gains `cursor: { theme, size }`.
- `xcursor` is named as a direct dependency. It was already in the tree
  through Smithay's winit backend.
- Animated cursors show their first frame only.
- The pointer is still drawn into the frame, not on a hardware cursor
  plane, so moving it redraws a little of the screen.
- An application's picture that is larger than the scale warrants is drawn
  as large as it says; nothing limits it.
- Tablets are not supported. The cursor-shape protocol covers them, so the
  trait it needs is implemented, empty.
