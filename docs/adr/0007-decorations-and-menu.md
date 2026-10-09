# 7. Decorations and the root menu

- Status: accepted
- Date: 2026-10-09

## Context

Fluxbox's character is in its frames: a titlebar that holds tabs, and a root
menu. On Wayland the compositor has to draw these itself, and has to agree
with each client on who draws the titlebar.

## Decision

**The compositor decorates every window that asks, and only those.** A
client that speaks xdg-decoration is always answered "server side", whatever
it prefers, because frames carry the tabs. A client that never asks is one
that insists on its own titlebar, and is left alone so it does not get two.
A rule can override either way.

**Geometry lives in the core, pixels in the compositor.** `Decoration` and
`Menu` in `omorfos-core` decide where parts are, which part a point is on,
and how a menu reacts to the pointer. The compositor only paints what they
describe. Layout and behaviour are therefore unit-tested without drawing.

**Drawing is done on the CPU into textures.** Titlebars and menu panels are
painted by a small canvas (fills, anti-aliased rounded corners, glyph
blending) and uploaded. The alternative, shaders, would tie drawing to
OpenGL: it could not run under the software renderer, so it could not be
tested in CI, and it would be the second renderer-specific code path. A
titlebar is repainted only when its appearance changes, and its position is
not part of that, so the cost is paid on title, focus and theme changes
only.

**Text uses cosmic-text.** Window titles arrive in any script, which needs
shaping and font fallback. That is a large problem with a maintained
solution; it adds 33 crates, all permissively licensed. A lighter rasteriser
without shaping was rejected as wrong for non-Latin titles.

**The compositor renders its own scene.** Smithay's `Space` draws windows
and then any custom elements on top, which cannot put a lower frame's
titlebar under a higher window. The compositor keeps a list of what is
shown, bottom to top, each with its cached decoration, and builds render
elements from that. Hit testing walks the same list. `Space` is kept only
for output bookkeeping.

**Resizing grabs extend outside the frame.** Borders default to one pixel,
too thin to aim at, so a frame can be grabbed a few pixels beyond its edge
instead of stealing pixels from the window.

**A drag or an open menu owns the pointer.** Windows see the pointer leave
and get no button events until it ends, including the release that ends a
drag, so an application never sees half a click.

**A window's own size is believed only once it has acknowledged ours.**
Otherwise a size still in flight (the maximized size, just after restoring)
would be mistaken for the window resizing itself.

## Consequences

- Only the titlebar's top corners are rounded. Rounding the window's own
  corners needs a shader and is left for the hardware backend.
- A font must be installed; packages depend on or recommend one.
- GTK applications keep their own titlebars and so cannot be tabbed unless
  a rule forces a frame onto them, which gives them two titlebars.
- Each decorated frame holds a titlebar-sized texture, about 200 KiB for a
  full-width titlebar at 1080p.
- There are no titlebar buttons yet; closing is by key binding or the
  application's own controls.
