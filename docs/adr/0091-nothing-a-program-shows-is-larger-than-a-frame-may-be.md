# 91. Nothing a program shows is larger than a frame may be

- Status: accepted
- Date: 2026-11-15

## Context

A window is as large as its program says. A buffer of four pixels can be
said, with `wp_viewporter`, to be two thousand million units across, and
no protocol calls that an error; a part of a window (a subsurface), a
pop-up and a pointer can be said to be as large. A window of a program
written for X11 can be 32767 pixels a side, which is as large as an X
server draws one.

A review
([finding 2](../reviews/2026-11-15-what-a-client-can-cost.md#2-a-window-said-to-be-enormous-ends-the-compositor))
showed what the compositor did with such a size: it believed it. The
frame the desktop kept was that large, and the titlebar painted for it
on the CPU as wide as the frame, 224 GB, which the machine does not
have: the allocation failed and the compositor was ended, the session
with it. Fixing that showed two more places. The overview lays a window
out by the size its program gives, and drew its own label for one two
thousand million units wide; and the renderer the tests draw with
refuses to draw anything stretched that far, so that one such surface
kept the whole screen from being drawn.

Any program can do this, one in a sandbox too, with nothing but what
every program is offered.

## Decision

**A frame is at most 16384 units a side, `Desktop::MOST_SIDE`, and
nothing a program shows is taken to be larger. It is held in three
places, each of which everything behind it goes through, and none of
which rests on another.**

- **The domain keeps no frame larger.** Every size said of a frame from
  outside, as a window opens, as its program resizes it, as it is moved
  and resized, and as it comes back from a compositor before, goes
  through `Desktop::held` in `omorfos-core`. What is painted for a
  frame is as large as the frame is kept, so this is what bounds it, for
  programs written for Wayland and for X11 alike.
- **A canvas has no more than `Canvas::MOST_PIXELS`**, 2 to the power of
  27, more than a screen of 15360 by 8640 has. `Canvas::scaled` in
  `omorfos-ui` gives a single clear pixel in place of one asked to be
  larger, which is drawn on to no effect, and whoever asked goes on.
  So whatever is painted on the CPU, by the compositor or a component,
  for whatever reason a size is wrong, a failed allocation is not how
  it ends.
- **What a program shows becomes what is drawn in one module**,
  `surfaces` in the compositor. A surface said to be larger than
  `MOST_SIDE` either way is left out, and the rest of what its program
  shows is drawn without it (`surfaces::tree`, `surfaces::of`). What a
  window says is the window proper is taken to be no larger
  (`surfaces::proper`), so that nothing of the compositor's own is laid
  out by more. `clippy.toml` refuses the functions of Smithay's that
  these stand in front of anywhere else, so a new place that draws a
  surface, or asks a window how large it is, cannot leave them out.

**16384** is more than twice an 8K screen across, so no window that
fits a screen, or two side by side, is touched; and it is the widest
picture many graphics cards hold, so nothing wider could be drawn in
one piece in any case.

**The program is not sent away.** No protocol says a size is an error,
so a program would be ended for something it was allowed to say; and
every program written for X11 is one client to the compositor,
`Xwayland`, so one of them saying its window is large would end them
all.

**An enormous surface is left out, not cut to its frame.** Cut, it is
still stretched as far as its program said, and that is what a renderer
may refuse; and nothing honest is that large to be cut.

## Alternatives

- **Hold the size where each handler reads it**, as the review first
  proposed: four places in `handlers.rs` and `x11.rs`, and a fifth with
  the next handler. In the domain it is one, and tested without a
  display.
- **Only the canvas.** It stops the failed allocation and leaves a frame
  two thousand million units wide in the desktop, for the next thing
  sized by a frame to find.
- **Draw the surface and let the renderer clip it.** A graphics card
  may well do so. The renderer the tests draw with does not, what a
  card does could not be tried where this was decided, and the
  compositor should not need to know.

## Consequences

- A window cannot be more than 16384 units a side. One that says it is
  has a frame that large, is told so, and is drawn once it draws no
  more than that. A window of a program written for X11 is made that
  size by the window manager, which the compositor is, as it is made
  any size its frame is given, and is drawn again once the X server
  hands it over so.
- A single surface larger than that is not drawn at all, as on a wall of
  screens more than 16384 units across one window stretched over all of
  them would not be.
- Drawing no longer depends on what a renderer does with an enormous
  surface, so it is the same with the software renderer and on a
  graphics card.
- What is not bounded by this, and is on the roadmap: where a surface
  is, which its program also says, and which is added to in whole
  numbers that wrap; a picture of a window by itself, which is as large
  as all its program says it draws; and that every window costs a
  titlebar painted as wide as its frame, so many wide windows cost
  memory in proportion.
