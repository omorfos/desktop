# 39. Windows sharing a workspace of their own

- Status: accepted
- Date: 2026-10-13

## Context

A fullscreen window has a workspace to itself (ADR 36). The next thing
wanted is two, three or four windows sharing such a workspace, as macOS
has them side by side: filling the screen between them, with a line
between that is dragged to give one more room and the other less. A
window dragged to an edge of the screen should make one, and the
titlebar's fullscreen button, with the pointer resting on it, should
offer the arrangements.

omorfos is not a tiling window manager and is not to become one. On a
desktop, windows are where the user puts them, as in Fluxbox. This is
about the other kind of workspace.

## Decision

### A workspace made for windows is tiled

What ADR 36 called a solo becomes the simplest case of a **tiled
workspace**: one made for up to four windows, which fill the screen
between them. It keeps everything a solo has: no panels and no
titlebars, its place in the row after the desktop it came from, no
number, the edges that bring things back, and it goes when its last
window does.

Desktops do not change. Nothing on a desktop is ever tiled.

### The arrangements

```
 one        two          two         three: one beside two
┌─────┐   ┌──┬──┐      ┌─────┐      ┌──┬──┐   ┌──┬──┐
│     │   │  │  │      ├─────┤      │  ├──┤   ├──┤  │
└─────┘   └──┴──┘      └─────┘      └──┴──┘   └──┴──┘

 three: one over or under two        four
┌─────┐   ┌──┬──┐                   ┌──┬──┐
├──┬──┤   ├──┴──┤                   ├──┼──┤
└──┴──┘   └─────┘                   └──┴──┘
```

An arrangement is a split of the screen in two, left and right or top
and bottom, each side of which is one window or is split once more the
other way. That gives exactly the eight above and nothing deeper, and
four windows at most.

Each split has a line that can be dragged. In the arrangement of four
the two columns are split separately, so the left pair can be divided
differently from the right pair.

### The lines

- A line is drawn as a hairline, and is taken hold of within a few
  pixels either side, where the pointer shows that it can be.
- Dragging it gives one side more and the other less, as it is dragged.
  A double click puts it back in the middle.
- No window is made smaller than it says it can be, and none smaller
  than a sixth of the screen: the line stops.

### What a window is told

Its size, and that it is tiled and against which edges
(`xdg_toplevel`'s tiled states), so that it draws itself square and
without a shadow. It is not told it is fullscreen, unless it is alone.

### Finding things again

Resting the pointer against the top of the screen brings back the panels
along the top, as now, and the titlebar of **every** window, each across
the top of its own tile. The second button on a tile's titlebar takes
that window back to the desktop.

A titlebar in the bottom row is half a screen from the edge that brought
it back, and has to be reached. With one fullscreen window what was
brought back goes the moment the pointer leaves it (ADR 36); here that
would take the titlebar away as the pointer set out for it. So on every
workspace made for windows, one window or several:

- **What was brought back stays while the pointer is moving.** It goes
  when the pointer has been still for a second somewhere that is not
  one of the things brought back, or a button is pressed in a window, or
  a key is pressed. Crossing the screen to a titlebar does not lose it,
  and stopping on it keeps it for as long as the pointer is there.
- **A tile has an edge of its own.** Resting the pointer on the line
  above a tile brings back that tile's titlebar alone, without going to
  the top of the screen for it. The line is still taken hold of and
  dragged to make room: resting on it and pressing on it are different
  things.

Both are tried by hand before they are fixed: a second, and a third of a
second on the line, are guesses to be felt.

The window with focus is marked by a hairline of the accent round its
tile, where there is more than one.

### Making one

- **From the titlebar's second button.** A click makes the window
  fullscreen, as now. Resting the pointer on the button brings up a
  small picture of each arrangement, and a click on a part of one makes
  a new workspace in that arrangement with the window in that part. The
  same choices are in the titlebar's menu, in words, for the keyboard.
- **By dragging.** A window dragged until the pointer is against an edge
  of the screen is shown, while still held, the half it would take: the
  left or right half at the sides, the top or bottom half at the top and
  bottom, a quarter at a corner. Let go there, it is put in a new
  workspace in that arrangement. Dragged back from the edge, nothing
  happens.
- **By a key.** `tile: left` and the like, for the window with focus.

### Filling the other parts

A workspace made this way begins with one window and one to three parts
with nothing in them. A part with nothing in it is filled:

- by the next window that opens while the workspace is shown, so that
  the launcher, used there, puts a program beside the one already there;
- by choosing one of the windows of the desktop it came from, which are
  listed in the part itself, small, to be clicked (ADR 38 has the
  compositor draw these);
- or it is left, and shows the wallpaper.

When every part is full, a window that opens goes to the desktop, as it
does from a fullscreen window now. A dialog opens over the window it
belongs to, as now.

### Changing one

- A window dragged by its titlebar onto another's part changes places
  with it.
- A window that leaves, by its button, by closing, or by being sent to a
  desktop, gives its room to its neighbour: four become three, three
  two, two one, and the last leaving ends the workspace.
- A window in a tile that asks to be fullscreen, a film say, is given
  the whole workspace until it leaves fullscreen, with the others out of
  sight behind it, and then is back in its part.
- `focus: left` and the like move the focus from tile to tile, and
  `swap: left` moves the window.

### What the IPC says

A workspace made for windows has `windows`, in the order of its parts,
in place of the one `window` it has now, and its arrangement.

## In what order

1. The model: arrangements, parts, the sizes of splits, windows told
   they are tiled; two windows side by side made from the menu and by a
   key. A window leaving gives its room up.
2. The lines: drawn, and dragged. What was brought back at an edge
   staying while the pointer moves, for every workspace made for
   windows.
3. Three and four, changing places, focus by direction, and a tile's
   titlebar brought back at the line above it.
4. A part with nothing in it filled by the next window to open.
5. The pictures of arrangements at the titlebar's button.
6. Dragging to an edge, with the half shown before letting go.
7. Choosing a window for an empty part, when the compositor can draw
   windows small (ADR 38).

### What was built first

Two halves, made by the titlebar's menu and by `tile`, with the other
half filled by the next window to open there, which was to be the fourth
step and was needed at once: without it there is no way to have a second
window. Each window is told it is tiled on every side. The window that
leaves gives its half to the other, which is then alone and fullscreen.

What is brought back at an edge staying while the pointer moves, which
was to come with the lines, came with this too: with one window above
another, the lower one's titlebar could not be reached without it. And
a window that leaves a half without closing goes back to the desktop it
came from, where the user follows it if they were looking at it; at
first it was left floating over the window it had shared with.

The line came next. It is drawn a hair wide in the colour of a window's
border, since the theme has no part for it yet; it is taken hold of
within four pixels either side; and it stops a sixth of the screen from
either edge. What a window says is the least it can be is not asked yet,
so a window can be given less room than it will draw in, and then draws
over the line.

Three and four came next, with focus and changing places by direction
(`Super+Ctrl` and an arrow, and with `Shift`). How a third part is made
was settled here: `tile` on a window that fills a part has it take a
half of that part, where the part is a whole side and the half is the
other way than the workspace is cut. So one word does for all of it:
"this window takes that half of where it is, and the rest is room for
another". Asked for a half it cannot take, the window goes to that half
instead, changing places with what was there, which is what the word
would be taken to mean. A fullscreen window asked the same takes a half
of the workspace it has, and no new one is made.

Changing places by dragging a titlebar onto another part, and a tile's
titlebar brought back at the line above it, are not built: the first
needs a titlebar that can be dragged where it is laid over a window, and
the second has been less wanted since what is brought back stays while
the pointer moves.

A window that is open already joins a workspace made for windows by
`join_workspace` and the workspace's `id`, which the menu of its titlebar
offers by name under "Send to workspace" for each that has room: a part
with nothing in it, or one window that has the whole, which then keeps
the left half. The user is taken there with it, unlike a window sent to
a desktop, since it was asked to be beside something.

## Alternatives

- **Tile on desktops too**, as snapping does on other desktops: a window
  dragged to an edge takes half of the desktop it is on. Then there are
  two kinds of half a screen that behave differently, one with a panel
  and floating windows over it and one without. One kind is enough, and
  dragging to an edge is given to the kind that was asked for.
- **Any number of windows, split as deep as wanted.** That is a tiling
  window manager, and a good one is a project of its own.
- **In the arrangement of four, one cross that moves as one.** Simpler
  to draw and to drag. It cannot give three windows a third each of
  anything, and the tree above has to be kept for the arrangements of
  three anyway.
- **The titlebars always shown on tiles.** Easier to find. It gives up
  the room that the workspace was made to give.

## Consequences

- The roadmap's "windows snapped to a half or a quarter of a desktop" is
  withdrawn in favour of this.
- The model's solo becomes a tiling of one, and what was built for it
  (ADR 36) carries over: where it stands in the row, where its windows
  go back to, panels hidden and brought back.
- The compositor gains lines to draw and to drag, a picture of
  arrangements to show at a button, and a shape to show under a window
  being dragged.
- A window told it is tiled that will not be the size it is told, as
  some terminals will not, leaves a strip of its part showing the
  wallpaper. That is left as it is.
- IPC: `window` on a workspace becomes `windows`. Nothing has been
  released against it.
