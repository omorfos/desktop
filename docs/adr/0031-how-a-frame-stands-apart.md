# 31. Shaded, kept above, on every workspace: how a frame stands apart

- Status: accepted
- Date: 2026-10-13

## Context

A menu on a window's titlebar is where people look for what else can be
done with it, and three of the things asked for there were things the
desktop model had no notion of: rolling a window up into its titlebar,
keeping it above the others, and showing it on every workspace. All three
are old habits of Fluxbox, which is where this began.

Each could have been done in the compositor, as drawing. Each is a fact
about where frames are and in what order, which is what the model is for
(ADR 1).

## Decision

**A frame has a standing**: shaded, above, sticky, each on or off, beside
maximized, fullscreen and minimized. The model owns it and the frames it
hands out say it.

**Shaded is a frame as tall as its titlebar.** Its geometry is what it
was, with the height of the titlebar, so everything that goes by a frame's
geometry (drawing its border, hitting it with the pointer, what is behind
it) is right without knowing. What it would be when let down is kept. A
frame with no titlebar cannot be shaded, and one that is fullscreen is not
while it is. The compositor leaves the window out: it is not drawn, cannot
be pointed at, and is not told to be any other size, since a window told
it has no height would redraw itself as nothing.

**Above is a level of the stack.** Frames are handed out in levels:
ordinary, kept above, fullscreen. Within a level they are in the order
they were raised. So raising an ordinary frame cannot put it over one that
is kept above, and nothing has to remember to check.

**Sticky is carried.** A frame is on one workspace, as ever. When its
output shows another, the sticky frames of the one it was showing are
moved to the one it shows now, on top, and one of them that had focus
keeps it. Everything that goes by which workspace a frame is on stays
true. Sending a sticky frame to a workspace makes it not sticky: it cannot
be both on one and on all.

**Each is an action**, like maximize: `shade`, `keep_above`, `stick`,
which turn it on or off for the window with focus, and can be bound to
keys.

**The titlebar's menu is the root menu's machinery with other entries.**
A right click on a titlebar gives that window focus and opens a menu of
actions, which are carried out as their keys would carry them out, on the
window with focus. Each entry says what choosing it will do, so one that
is on reads as turning it off.

## Consequences

- Four things can be done from a window's titlebar that could not be done
  at all, and three of them from the keyboard.
- Rules cannot yet open a window shaded, above or sticky, and the IPC
  state does not say which windows are. Both are additions.
- A shaded window goes on drawing itself, unseen. It is not told it is
  hidden, because nothing in the protocols says "rolled up".
- A sticky frame is on the workspaces of its own output only, which is
  what each output having its own workspaces means (ADR 17).
- The menu acts on the window with focus, not on a window it remembers.
  Should focus move while it is open, which nothing but another program
  asking can do, what is chosen is done to the wrong window.
