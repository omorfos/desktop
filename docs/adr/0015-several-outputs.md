# 15. Several outputs

- Status: accepted
- Date: 2026-10-09
- Amends: [10. Hardware backend](0010-hardware-backend.md),
  [11. Scaling](0011-scaling.md)

## Context

The compositor was built around one output: one size, one scale, one set
of panels, one lock surface. A laptop on a desk has two screens, and they
come and go while the session runs.

The window-management model was already written for several: it places a
new window on the output under the pointer, maximizes onto the output a
frame is mostly on, and moves frames off an output that disappears. What
knew only one was the adapter around it.

## Decision

**One space, and each output shows a rectangle of it.** Windows, the
pointer, the menu and the notice are positioned in a single space in units
of layout, as before. An output has a position in it, a size in pixels and
a scale. Drawing an output means drawing the space from that output's
corner at that output's scale; nothing else about drawing changed.

**Outputs stand side by side, left to right, in the order they appeared,**
with their tops aligned. The hardware backend takes a laptop's own panel
first. There is no setting for the arrangement yet.

*Amended:* the screens listed under `outputs` stand in that order, each
next to the one listed before it, as its `place` says: to the right, the
default, or left, with the tops in line; or above or below, the middles
in line. Those not listed stand to the right of all the rest. A place
next to the one before was chosen over coordinates, which leave gaps and
overlaps to be made by mistake and have to be worked out again when a
screen's size changes. A listed screen may be lined up at the start, the
middle or the end of the edge it shares with the one before it (`align`),
and moved along it (`offset`), in units or as a share of its own length,
which stays right when a screen's mode changes.

*Amended:* a listed screen may be turned (`rotate`, as `xrandr` says it).
It is drawn turned, by the output's transform, into what is drawn as the
screen is made, and stands in the space as large as it is the right way
up. Pictures of it are drawn the right way up into memory of their own,
as pictures of a window by itself are, rather than read back from what
the screen shows, which is turned; the protocol's transform on a picture
would have every program reading them turn them itself.

*Amended:* each way the screens may stand can be listed under
`arrangements`, and the first naming just the screens plugged in is used
in place of `outputs`, chosen again as one comes or goes. They are
written in the configuration, as kanshi has them, rather than remembered
from what the user last did, so that nothing changes behind the user's
back and an arrangement can be read and shared.

*Amended:* a listed screen may `mirror` another. It is then taken out of
the space and out of what programs are told of, as an unplugged one is,
so that nothing of the desktop's is put on it, and it is still driven:
drawing it is drawing what the other shows, from the same elements, each
made larger or smaller by Smithay's `RescaleRenderElement` to fit and
moved to the middle, with black bars over whatever of the space lies
beyond the other screen. A scale for the mirror was not the way: scales
are quarters from one to four, which neither fits a picture closely nor
makes one smaller. Drawing the other once more into a picture of its own
and laying that on the mirror would fit exactly, but it draws twice and
needs a renderer that can draw into what it can read. The mirror's own
output is given the other's scale, since Smithay sizes much of what it
draws by the scale of the output it is drawn on, and only then is the
whole made to fit. A mirror counts
among the screens a locked session must have been hidden on before the
locker is told it is locked.

**An output is named by Smithay's `Output` in every call about it.**
`add_output` returns one, and resizing, scaling, drawing and frame timing
take it. What the compositor needs to draw comes from the output itself,
so drawing an output that has just been unplugged is harmless and no call
can fail for lack of one.

**Each output has its own scale** (ADR 11), suggested by its own density
unless the configuration names one for all. A surface is told the scale of
the output most of its window is on. A frame's titlebar is painted for
that output too, and stretched on the other if the frame straddles two of
different scales.

**The pointer is always on an output.** It crosses where two meet and
slides along an edge that leads nowhere. If its output goes, it moves to
the nearest point of the one it would otherwise be on.

**Panels belong to an output:** the one they ask for, or the one the
pointer is on. Each output has its own reserved edges. Panels on an output
that goes are closed.

**Locking covers every output.** The locker gives a surface for each, and
an output without one is black. The locker is told the session is locked
only once every output has drawn a frame without the session in it. The
keyboard goes to the lock surface on the pointer's output, or failing that
to any.

**A window is told a frame was shown by the output most of it is on,** so
it draws at that screen's pace and not at the sum of two.

**The hardware backend drives every connected screen** of the seat's
graphics card and looks again when udev reports a change, and when the
session comes back from another virtual terminal. A screen that cannot be
set up is reported and left dark; the others and the session carry on.

**A session with no output keeps running.** Windows keep their places and
come back when a screen does.

## Consequences

- At first the arrangement could not be changed and there was one scale
  setting for all outputs. `outputs` in `config.yaml` now lists screens by
  name: they stand in the order listed, ahead of any not listed, and each
  may have its own scale. An output's identity to the window-management
  model is still the order it was added in, so rearranging moves screens
  and their windows with them without disturbing which workspaces are
  whose.
- Screens are side by side only: no mirroring, and none above another. A
  list keeps the arrangement free of gaps the pointer could not cross,
  which positions given by hand would not.
- One graphics card. A screen on a second card is not seen.
- A screen is driven at the mode it prefers. Its refresh rate is still
  reported to applications as 60 Hz.
- The notice of problems is on the first output only.
- A window is told to enter outputs by Smithay as it overlaps them; the
  scale it is told is the compositor's choice of one of them.
- The hardware side has no tests, as before. The compositor's side is
  tested with two software screens.
