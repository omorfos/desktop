# 44. Picking what a picture is of

- Status: accepted
- Date: 2026-10-19

## Context

A picture of the screen was of every screen, or of the window with focus:
neither the user, with `Print`, nor an application, through the portal,
could have one screen or a region of one. Snipping tools on other desktops
let the user draw what they want, and an application that asks for a
picture asks with an `interactive` option that says so.

## Decision

**What is pictured is picked over still pictures of the screens.** The
pictures are taken first, as ever; then each screen is shown its own,
still, over all of it and with the keyboard, dimmed, under a crosshair.
A drag draws a region, which is bright and bordered, and letting go
takes it; a click takes that screen; Enter takes every screen; Escape or
another button takes nothing. What is taken is cut from the pictures
already taken, so it is what the screen showed when the user asked,
whatever has moved since, and the picker is in no picture.

- **The dim and the border are surfaces of their own**, a pixel of
  colour each, made as large as they are to be, so that a drag moves
  them and paints nothing again; each screen's picture is painted once.
  They take nothing from the pointer, which is always on the picture.
- **It is a library of its own**, `omorfos-picker`, so that the
  screenshot tool and the portal pick in one way. It waits on an event
  queue of its own on the connection it is given, so a program that has
  more to do, as the portal has while a screen is shared, picks on a
  thread of its own.
- **It is put over the screen as the portal's question is**: a program
  may only if the compositor started it with `may: [panel, overlay]`. One
  that may not is told so, and the user is told by the compositor.
- **`Shift+Print` picks**, and puts what was picked on the clipboard, as
  it put everything before: `omorfos-screenshot --pick --to clipboard`.
  `Print` still takes every screen at once.
- **The look is the theme's `picker` part**: its background is the dim,
  see-through black by default, and its border is round what is drawn,
  the accent.

## Built in steps

1. Screens and regions, for `omorfos-screenshot --pick` and `Shift+Print`.
2. The portal picks the same way after the user has said yes.
3. Windows: the compositor says where each is, the one under the pointer
   is outlined, and a click takes it whole, as `--window` does.

## Alternatives

- **Choices in the question**: buttons for everything, a screen, a window
  or a region, each a step of its own. Clearer to read, and more steps for
  the same thing.
- **Picking over the live screen.** What is taken would be what is there
  when the user lets go, which a video or a menu that closes has changed;
  and the picker would have to stay out of the picture taken after it.

## Consequences

- A region is of one screen: a drag that goes on to another stops at the
  edge of the screen it began on.
- Until step 3, a window is had with `Alt+Print` or by drawing round it.
