# 26. What goes from the screen fades out, drawn as it last was

- Status: accepted
- Date: 2026-10-12

## Context

ADR 25 fades in what comes onto the screen and left what goes to go in a
single frame. Fading out is harder for one reason: by the time the
compositor knows a window has gone, the window's picture has gone with it.
A program that closes a window destroys the surface, and one that exits
takes everything. Nothing is left to draw fainter.

So a picture has to be kept. The usual way is to draw the window into a
texture of the compositor's own at the moment it goes. That needs a
renderer at that moment, and the compositor is lent one only while a frame
is being drawn, by backends that are kept free of decisions because they
cannot be tested (ADR 2). Lending it at other times would put new logic in
exactly those files.

## Decision

**What is drawn is noted as it is drawn.** While a frame is drawn, each
window and each shell panel has the pieces it is drawn as written down:
where each was, which part of its buffer, and the texture the renderer drew
it from, or the colour if it was one colour. This replaces what was noted
the frame before. Holding a texture keeps it alive after the program has
let go of the buffer it came from.

**When the thing goes, its pieces are drawn in its place**, as a `Ghost`,
fainter each frame, over the `transition` of the part it was: `window`, or
the part a shell panel is named after. A window keeps the frame it had,
which is the compositor's own picture and needed no noting. The menu and
the notice are the compositor's own too and are simply kept. What has gone
is drawn over what is left at the same level of the stack: a closed window
over the other windows and under the panel, a panel among panels.

**"Gone" is not being drawn any more**, whatever the reason: closed,
minimized, on a workspace that was left. So one workspace fades into the
next. Showing another tab of a frame is not a going, as it is not a coming
in.

**Noting costs nothing where there is no fade.** It is skipped for a part
whose `transition` takes no time. What was noted of anything neither on
the screen nor fading is let go of in the same pass, so a hidden window's
textures are not held.

**The textures are kept without knowing what they are.** The compositor is
not generic over the renderer; drawing is. What is noted is kept as
`dyn Any` and read back as the renderer's own texture type when the next
frame is drawn. If it was noted with some other renderer, nothing is drawn.

**A piece that cannot be drawn is left out, not an error.** The software
renderer draws a shared-memory buffer straight from the program's memory,
which is gone once the program is. The frame is drawn without that piece.
On a GPU the texture is the compositor's copy, or holds the buffer, and
outlives the program.

**Locking the session drops everything that is fading**, so that nothing of
the session is drawn after it is unlocked that was not there before.

**Buffers of a single pixel are accepted** (`wp_single_pixel_buffer_v1`).
They are the one kind of content that is not a texture, and what is noted
has to say so; supporting them made that case one that occurs and is
tested, and they are what a toolkit uses for a plain background.

## Consequences

- Closing a window, the launcher or a notification, leaving a workspace
  and dismissing the menu are all softened, by one mechanism in the
  compositor. Components still know nothing of it.
- With a theme that fades, every frame allocates a short list for each
  window and shell panel on the screen. It is small beside what drawing
  them already allocates, and a theme with `transition: none` does not pay
  it.
- In tests the window itself fades out only while its program is still
  there, since the tests draw with the software renderer. That a program
  can vanish without the frame being lost is tested; that its picture then
  fades is only seen on a GPU.
- A texture of a window that has gone is held for as long as its fade
  lasts, which a theme can make ten seconds.
- One notification leaving while others stay still goes at once, as one
  arriving does: that is the notifier redrawing a surface that stays.
