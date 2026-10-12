# 30. Programs written for X11 can draw for the densest screen

- Status: accepted
- Date: 2026-10-13

## Context

X11 has one grid of pixels and no notion of a screen that draws more than
one of them to a unit. ADR 27 left programs written for X11 drawing at one
pixel to a unit, which the compositor stretched to size on a screen with a
scale. They were the right size and blurred, on exactly the screens people
buy to have sharp text.

There are two ways out, and every desktop offers both. Either such
programs go on drawing small and are stretched, or the X server is given
the screen's own pixels and the programs are told how dense they are, and
left to draw larger themselves. The first is always the right size and
always blurred. The second is always sharp, and the right size for
programs that listen.

## Decision

**The X server's pixels are the densest screen's pixels.** Smithay lets a
client have a scale of its own, by which everything it says about sizes and
places is divided, and the X server's is set to the largest scale any
screen has. A window of two hundred of its pixels is a hundred units on a
screen that draws two to a unit, and each pixel the program draws is one
the screen shows. On a screen that is less dense the same window is scaled
down, which loses nothing that could be seen there.

Nothing else in the compositor changes. An X11 window's size and place are
already asked of Smithay in units of layout, and the desktop model never
knew of pixels.

**Programs are told how dense that is, in both places they look.**
Toolkits follow XSETTINGS as it changes: `Xft/DPI`, and for GTK the whole
number it scales by with the rest left to its text. Older programs, and
some large new ones, read `Xft.dpi` once from the resources the X server
keeps. Smithay's window manager publishes the first. For the second the
compositor connects to its own X server as a program would, from a thread
of its own, since the X server may not answer until it has had an answer
from the compositor. What else is kept in the resources is left alone.

**When the scale changes, so does all of that**, and windows that are
there already are sized again, since they are as large as they were and
that is now a different number of pixels.

**It is asked for, with `xwayland.scaling: native`. The default is
`stretched`**, which is one pixel to a unit as before. `native` was the
default as first written, and the first program tried on a real screen,
`xterm` at a scale of 2.5, came up two fifths of its size and unreadable:
its usual font is a bitmap and takes no notice of density. A program that
is blurred can be used and one that is too small cannot, and nothing tells
the compositor which kind a program is. So the default is the one that
never leaves a program unusable, and the sharp one is there for whoever
runs programs that listen.

## Consequences

- Out of the box, programs written for X11 are the right size and blurred
  on a dense screen, as they were.
- With `native` they are sharp, and one that takes no notice of the
  density is small. The setting is for all such programs at once and not
  for one.
- A program reads the resources when it starts. One that is running when
  the scale changes keeps the size it had until it is started again,
  unless its toolkit follows XSETTINGS.
- The pointer over such a window is drawn by the program, at the size the
  environment gives every program, which is in units and not in the X
  server's pixels. GTK is told the right size through XSETTINGS; elsewhere
  the pointer is smaller over these windows than over others.
- `x11rb` becomes something the compositor uses itself, where before it
  only came with Smithay.
