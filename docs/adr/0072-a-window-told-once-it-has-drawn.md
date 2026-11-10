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

## Consequences

- Dragging the edge of a window of a slow program, the frame follows the
  pointer and the program draws as fast as it can, at the size the
  window is each time it is ready, rather than every size in turn.
- Until a window's picture is stretched to its frame, a program behind
  leaves a gap in its frame, or reaches past it, for as long as it takes.
- A test's program has to draw between two changes to be told the
  second, as a real one does.
