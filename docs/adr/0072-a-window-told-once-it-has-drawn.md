# 72. A window told something new once it has drawn the last

- Status: accepted
- Date: 2026-11-10

## Context

"Movement" has a window that changes size do so smoothly: its frame
easing to where it goes, and its last picture stretched to the frame
until its program has drawn the new size, then faded into it. Each of
those needs to know when the program has caught up. Meanwhile a program
was told each size its window passed through as soon as the window had
it: dragging an edge told it one at every move of the pointer, whether
or not it had drawn the last, and a program slow to draw fell further
behind with each, drawing sizes long gone. Only the line between windows
that share a workspace waited for them (ADR 39).

## Decision

A program written for Wayland is told nothing new about its window until
it has drawn what it was told last. What the window is to be is worked
out as before, and its frame is drawn so at once; telling the program
waits, and it is told how the window is then, the latest, once it has
drawn. The sizes in between are never asked.

- **Drawn** is the program having acknowledged the last it was told and
  committed since, or what it shows being the size it was last asked
  already. The second is for a program asked to stay as large as it is,
  as a window is when it is first placed, which need not draw again and
  may not.
- **Its states wait with its size**, focus among them. They go together:
  told it is no longer maximized before it has drawn itself maximized, a
  program is told so with the size it is to be, not with the size of
  being maximized. Focus waits as long as the program takes to draw,
  which is a frame or two for one that is not stuck, and a program that
  is stuck draws nothing it could show either way.
- **A program that never draws is never told again**, and its frame goes
  on being drawn as the window is to be, over its last picture. Nothing
  is gained by telling it more.
- **A program written for X11 is told at once**, as before. X11 has a way
  to ask a program to say when it has drawn (`_NET_WM_SYNC_REQUEST`),
  which the window manager in Smithay does not offer yet.

The line between windows that share a workspace goes on waiting for
them as well, as ADR 39 has it: there the line itself is held, so that
it and the windows either side of it move together.

The rest of a window changing size smoothly comes after, in this order:
its last picture stretched to the frame until the program has drawn the
new size, then faded into it; tiling, maximizing and filling the room of
a window that closed easing over 240 ms; and the line between windows
easing when it moves by itself.

### Stretched until it has drawn

Its last picture is stretched to fill the frame, across and down apart,
while its program is behind: until the program has drawn since it was
told the size the frame has. Once it has, the picture it had fades into
the new one over the time the theme gives a window to come in,
`window`'s `transition`, 180 ms in the built-in theme, drawn over it
where it is in the windows' order and cut to the frame's corners as it
was. The picture is not moved: it is at once where the frame is, which
is what "Movement never holds input up" asks, and nothing is drawn
where there is nothing.

- **Behind, not merely another size.** A program that has drawn since it
  was told, and chose another size, as a terminal may to fit whole lines
  or a window that cannot be as small as its frame, is drawn as it
  draws: stretching it for good would blur it.
- **Where windows change over a time.** With `window { transition:
  none; }` a picture is drawn as it is, short of the frame or past it,
  until the program draws, as before: the theme says windows are to
  change at once, and a stretched picture fading is changing over a
  time. Reduced motion leaves it, the fade being a change of opacity,
  which it keeps, at most 120 ms (ADR 71).
- **Not faded while held.** A frame held by hand, by its edge or by the
  line between it and another, has its program draw at each step, a
  little behind: each picture fading into the next would leave the one
  before over it for as long as the fade, which over a window that can
  be seen through, as a terminal's often can, shows as one that cannot,
  and over any as what it showed a step ago. So a picture stretched
  while its frame was held is the program's own once it has drawn, as
  it is on the way to where a frame was sent, until the program has
  caught up after it is let go.
- **The pointer is not stretched.** For as long as the picture is, the
  pointer is in the window where it would be in the picture
  unstretched: the program is given where it is in what it drew, and a
  frame or two later it has drawn the size the frame has.

### Eased to where it was sent

A frame sent somewhere is drawn easing there over 240 ms, ease-in-out,
as `docs/DESIGN.md` has a window moving to where it was sent: maximized
or restored, tiled, arranged, put beside others, changed over with
another, or given the room of one that closed or left. Its program is
told at once how the window is to be, as before, and only that; the
frame is drawn on its way there, its decoration laid out over where it
is, and the picture stretched to fill it, whether the program is behind
or has drawn already. Sent elsewhere on its way, it sets out from where
it has got to.

- **Sent, not put.** The desktop counts each time it sends frames
  somewhere, and a frame whose place changes while the count does is
  sent. One dragged by hand, by its edge or by the line between it and
  another, one whose program makes it another size, and one carried
  with its workspace as the workspaces slide, is where it is put at
  once: what the hand moves follows it. The count is in the desktop,
  where what sends a frame is, rather than in each of the many places
  in the compositor that ask it to; the time is not.
- **Input does not wait.** What the pointer is on is what is drawn
  under it, the titlebar's buttons and the window alike, at each step
  of the way: nothing is held until the frame gets there. In the window
  itself it is where it would be in the picture unstretched, as it is
  while the picture is stretched for a program behind.
- **Fullscreen is not eased.** A window going fullscreen or coming back
  is there at once: it is the program's own wish, often a film's, and
  the panels go from under it at once.
- **No fade on the way.** A program that draws the new size while its
  frame is on its way is stretched to the frame from that picture,
  without the fade: the picture it had would be left behind where it
  was, while the frame went on. Once there, a picture stretched only
  for the way is the program's own, and does not fade either.
- **Where windows change over a time**, and motion is not reduced, as
  for the stretched picture: otherwise it is there at once.
- **The line goes with them.** The line between two parts that moves by
  itself eases as the frames either side of it do: put back in the
  middle, which sends them, or cutting more of the screen once a part
  has closed, as the line across half of it becomes the one across all
  of it. It is the line it was if it is the same line of the same
  output lying the same way, and otherwise the nearest that lay the
  same way; one that was not there is there at once. It is taken hold
  of where it is going, as the desktop has it.

The titlebar is painted again at each width it passes through, for the
240 ms; the shadow is laid out again, not painted.

## Consequences

- Dragging the edge of a window of a slow program, the frame follows the
  pointer and the program draws as fast as it can, at the size the
  window is each time it is ready, rather than every size in turn.
- A program behind no longer leaves a gap in its frame, or reaches past
  it: its last picture fills it, stretched, until it draws.
- A test's program has to draw between two changes to be told the
  second, as a real one does.
- A window maximized, tiled or given the room of one that closed goes
  there rather than jumping, and its program is still asked one size;
  the line between two goes with them.
