# 25. What comes onto the screen fades in, and the compositor does it

- Status: accepted
- Date: 2026-10-12

## Context

ADR 24 made colours change over time and left things appearing in a single
frame: a window, the menu, the launcher. Fading a thing in is not a change
of its colours. The whole of it has to be see-through, text, pictures and
all, and for a window that is a picture some other program drew.

There were two places to do it. Each shell component could draw itself
fainter, which would fade the launcher and a notification but not a window,
and would be written three times. Or the compositor could draw whatever it
is about to put on the screen fainter, which is one place and covers
everything, windows included.

## Decision

**The compositor fades in what it puts on the screen.** Every picture it
draws already has an opacity to be drawn at, and it is given one below 1
while the thing is coming in. Components do nothing and know nothing of it.

**How long is the `transition` of the part the thing is.** There is no new
property. A window and its frame take `window`'s, the menu `menu`'s and the
notice `notice`'s. A layer-shell surface takes that of the part it is named
after: the namespace it gives is looked up among the parts, which is how
`panel`, `launcher`, `launcher-backdrop` and `notification` come to fade in
by their own rules. The notifier's surface is renamed from `notifications`
to say which part it is. A surface with any other name, which is another
program's, is there at once: there is no part to ask.

**`Motion` keeps when each thing was first in the picture**, beside the
looks it already keeps, and says how much of it there is to see by the same
clock and easing. Something left out of a picture is forgotten, so a window
comes in again when its workspace is shown again or it is brought back from
being minimized. Every window of a frame counts as in the picture while the
frame is, so showing another tab is not a coming in. A layer surface counts
from when it has drawn something, not from when it asked for a place.

**The built-in theme's 120ms applies**, since it is given to every part.

**Tests under the built-in theme let time pass.** The test session's rule
that nothing takes time (ADR 24) does not reach a session whose theme is
broken, which runs on the built-in one: those tests step once, so that what
is new is seen, and then a second on.

## Consequences

- Windows, the menu, the launcher and the panels arrive softly, with one
  mechanism and nothing added to any component.
- While something is coming in, the part of each screen it covers is drawn
  again every 16ms, and what is under it too, since it cannot be treated as
  hiding anything. That lasts as long as the transition.
- Switching workspace fades its windows in over the desktop. That is the
  whole of a workspace transition for now, and it comes from windows being
  forgotten when they are not shown, not from anything written for it.
- One time serves a part's colours and its coming in. A theme that wants
  them different cannot have it yet; a property of its own would be the
  way.
- A notification that arrives while others are showing is there at once:
  the surface it is drawn on is already on the screen. Fading it is the
  notifier's to do.
- What goes away still goes at once. Fading a window out needs a picture of
  it kept after its program has taken the surface away, which means drawing
  it to a texture as it goes, and the compositor is lent a renderer only
  while a frame is drawn. That is the next decision.
