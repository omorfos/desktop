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
takes it; the window under the pointer is shown so until a drag begins,
and a click takes it, or the screen where there is none; Enter takes
every screen; Escape or another button takes nothing. What is taken is cut from the pictures
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
- **The portal picks after the question**, once the user has said yes,
  over the pictures taken as the question went. The question comes
  first, so that the application asking is named before anything is
  shown; picking then says what was given. The picker waits on the
  portal's connection on a thread of its own, so the portal reads that
  connection only when there is something to read: otherwise it would
  wait with the picker, and answer nothing else, until the compositor
  next said something. An application that
  withdraws its request while the user picks is told no at once, and
  the picker stays until the user is done with it, so that nothing
  vanishes under the pointer; what is picked then is not kept.
- **The compositor says where windows are at its socket for scripts**,
  asked with `"placed"` as the pictures are taken: each window that can
  be seen, top first, with the part of it on each screen in that
  screen's pixels. It is a request of its own, and not part of the
  state, which would otherwise be sent to every panel each time a
  window moved. A program that cannot ask picks screens and regions.
- **A window clicked is taken whole and anew**, by its handle, as
  `--window` takes one: the parts that others covered are in it, and so
  it is the window as it is when it is clicked, not as it was when the
  key was pressed. A window that has gone meanwhile is a failure.
- **The screen to share is picked the same way**, where there is more
  than one, after the user has said yes: only a whole screen can be,
  the one under the pointer shown bright and bordered, and a click or
  Enter shares it. A window or a region would change size while it is
  shared, which the stream cannot yet follow.
- **The picker says what to do**, in a line at the bottom of each screen,
  out of the way of what is along the top, in the look of `tooltip`, in
  words its caller gives, so that sharing can say "share". It is no
  wider than the screen.
- **The look is the theme's `picker` part**: its background is the dim,
  see-through black by default, and its border is round what is drawn,
  the accent.

## Built in steps

1. Screens and regions, for `omorfos-screenshot --pick` and `Shift+Print`.
2. The portal picks the same way after the user has said yes.
3. Windows: the compositor says where each is, the one under the pointer
   is outlined, and a click takes it whole, as `--window` does.
4. Sharing a window, when the application can take one: the same picker,
   offering screens and windows but no region, the window under the
   pointer or else its screen. It is shared by the window's own capture
   source, so that it is shared whatever covers it and at its size as
   that changes; a region would have to be cut from every picture, and
   is left for later.

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
- What the picker outlines is what the window draws, not its frame,
  since a picture of a window is without the frame.
