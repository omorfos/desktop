# 70. Signs and pictures in menus, and what is on in the accent

- Status: accepted
- Date: 2026-11-10

## Context

ADR 32 chose Lucide for the signs of the interface, shown at 16 with
lines 1.5 wide, and the icon theme for programs' pictures; none of
Lucide was built in. The menus were words alone. And a row that turned
something on and off said the opposite of how things were, so that its
words changed under the pointer: "Maximize" and "Restore", "Shade" and
"Unshade", "Keep above" and "Stop keeping above", "Show on every
workspace" and "Show on this workspace only". Read quickly, a row says
what is so as much as what it will do.

## Decision

Every row of a menu has room for a sign or a picture before its label,
16 across, whether or not it has one, so that the labels of a menu stand
in one line. It is in the middle of its row, and as far from its label
as the row's start is from it. The built-in theme makes that 8, and rows
32 tall, where they were 28, so that the sign has 8 on every side: 6, in
rows of 28, was tried first and was tight, and 8 at the sides alone left
it closer to the top and bottom than to its label. What
the theme gives `menu-sign` is drawn round the sign, as far out as its
padding, and moves nothing. What stands there is worked out from what
the row does, by one function for the root menu and a window's:

- **A sign** for most actions: a cross to close, a line to minimize,
  four corners for fullscreen, the halves of the screen to tile, a plus
  for a new workspace, a magnifying glass for the launcher, two arrows
  round to read the configuration again, an "i" for "About", a padlock
  to lock, an arrow out of a door to quit. An action that has none,
  such as moving focus, has nothing there.
- **A sign in the accent** for one that turns something on and off,
  while it is on: arrows outwards to maximize, a window with its top
  closed to shade, a pin to keep above, an eye in a frame for every
  workspace, a cup of coffee to keep the session awake. Its words are the
  same either way, and so is its sign; while it is on, the sign is in the
  accent on a faint square of it, a fifth of the accent over the menu
  (`menu-sign:active`, `--accent-tint`), as libadwaita's and Material's
  toggles are. It is seen by its shape as well as its colour, and is
  quieter than the accent all over the square, which was tried first and
  stood out more than anything else in the menu. A switch at the end of
  the row says it as plainly and was left for the room it takes; a bar
  of the accent at the row's edge, for being easily missed on glass. A
  pair of signs, one for each way, a pin and a pin struck through, was
  weighed and left: it is read as what is so as readily as what choosing
  the row will do, which is the fault of the words that changed. A tick
  in the sign's place was too, and left for a sign that says what the
  row does whichever way it is.
- **The program's own icon** for one that starts a program: the icon its
  desktop entry names, found by the name of the program's file, as
  `foot` finds `foot.desktop`, or else the theme's icon of that name, as
  a window's is found (ADR 54). It is in full colour; a sign is in the
  colour `menu-sign` gives, the label's in the built-in theme, hovered or
  not, and the accent while it is on.
- **What `menu.yaml` names**, as an `icon` of an item, the theme's icon
  by its name or a picture by its path, in place of the above. A program
  whose desktop entry is not named as its file is (Ghostty's,
  `com.mitchellh.ghostty`) or that is started through another
  (`flatpak run`) needs it. A submenu has nothing unless it is given
  one, but for the two of a window's menu, which have signs of their own.

The signs are Lucide's drawings, those used and no others, copied as
Lucide gives them, from lucide-static 1.61.0, into
`crates/omorfos-icons/signs`, and drawn with resvg, which the icons of
programs are drawn with already. Their lines are drawn 2.25 wide on
Lucide's grid of 24 in place of its 2, which is 1.5 at 16. Each is drawn
once for each size and colour it is wanted in, and kept. Which sign
stands for what is the model's, `omorfos_core::Sign`, named for what it
means; how one is drawn is `omorfos-icons`'s. A version of Lucide is
taken up by copying the files again, from a release at least two weeks
old.

The tick in "About"'s button stays drawn as a stroke (ADR 69): it is
drawn as it goes, a share of its length at a time, which a drawing in a
file is not. The menus have no tick of their own.

Menus are 240 wide, where they were 220, for "Show on every workspace"
after its sign. A submenu lies 4 over the edge of the menu it opens
from, its first row level with the row that opens it, so that the two are
seen to belong together; where both are, the pointer is on the submenu,
which is over the other.

## Consequences

- Each package carries `LICENSE-lucide`, Lucide's ISC licence and the
  MIT licence of Feather, from which some of the signs come.
- A window's menu no longer says "Restore", "Unshade", "Stop keeping
  above" or "Show on this workspace only".
- Being able to arrange the root menu in `menu.yaml` may go, once there
  is a menu of omorfos's own on the bar (stage 5); `icon` would go with
  it.
- A program started from the menu is known by its file's name only, not
  by the desktop entry whose `Exec` starts it.
