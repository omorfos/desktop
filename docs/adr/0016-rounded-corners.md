# 16. Rounded corners without a shader

- Status: accepted
- Date: 2026-10-09

## Context

A frame's top corners are rounded, because the titlebar is painted by the
compositor and can be cut to any shape. Its bottom corners were square,
because what is there is the window's own picture, which the compositor
only places.

The usual way to round a picture someone else drew is a fragment shader
that discards what lies outside a rounded rectangle. That is what the
roadmap expected, and why this waited for the hardware backend. But a
shader ties drawing to OpenGL. `Compositor::render` is generic over the
renderer precisely so that the tests can draw with a software one and
check pixels; a shader path could not be run in CI, and would be the first
drawing code that is not tested.

## Decision

**The window is drawn in strips, and the border is painted over the
join.** Where a frame's bottom corners are rounded, its window is not
drawn as one rectangle but as a stack of them: one for everything above
the curve, then one for each run of pixel rows that the curve leaves at
the same width. Any renderer can crop a picture to a rectangle, so this
needs nothing the software renderer lacks.

Strips have hard, stepped edges. The border, which the compositor paints,
follows the curve as a smooth quarter ring and lies on top of the steps. A
pixel the curve passes through is left out of the strip and belongs to the
ring alone, so what shows there is the ring blended with whatever is
behind the frame: a properly smooth edge.

**Each strip has a name of its own, kept from frame to frame.** The
renderer works out what changed by following each thing drawn under an
identifier. Every strip of a picture would otherwise share the picture's,
and a frame that had not changed would be redrawn every time. The names
are kept with the frame's decoration and forgotten with it.

**Pop-ups are not cut.** A menu reaches beyond the frame by design.

**The radius is the theme's,** as for the top corners, reduced if the
frame is too small for it. A radius of nothing draws the window whole, as
before.

## Consequences

- Rounded corners are tested like everything else: by pixels, at two
  scales, including that an unchanged frame is not redrawn.
- A window with rounded corners is a handful of things to draw where it
  was one: nine at the default radius, more at a larger scale. Each is a
  cropped copy of the same texture, so the cost is in bookkeeping, not in
  pixels.
- With a border of nothing there is no ring to smooth the steps, and the
  corners are stepped.
- Only frames the compositor decorates are rounded. An application that
  draws its own titlebar rounds its own corners or does not.
- A maximized frame is rounded too, as its top corners already were.
- A window drawn in strips cannot be handed to a hardware plane whole.
  Nothing is handed to planes today; if that changes, rounded windows are
  the ones that stay composited.
