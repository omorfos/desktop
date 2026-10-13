# 38. Pictures of windows

- Status: accepted
- Date: 2026-10-13

## Context

Three things are to show a window small: the switcher on `Alt+Tab`, the
dock when the pointer is on a program's icon, and an overview of a
workspace. The compositor has every window's picture already; it is what
it draws the screen from. The dock is a program of its own (ADR 18) and
has none.

A window's picture is what is in the window: a letter being written, a
bank's page. Handing pictures to another program is the `capture`
privilege (ADR 37), and with it comes the mark that says the screen is
being captured. A dock that set that mark off each time the pointer
crossed it would teach the user to ignore the mark.

## Decision

**The pictures never leave the compositor. Whoever wants one shown says
where, and the compositor draws it there.**

### The switcher and the overview are the compositor's

They are drawn by the compositor, as the menu is. They are worked with
the keyboard while a key is held, which only the compositor knows; they
have to work when every component has stopped; and they show every
window there is. Themed like every other part.

### A component is lent a picture, not given it

The dock decides everything about its preview: when it opens, the name
above it, the row of several windows of one program, what a click does.
For the picture itself it leaves a rectangle empty and asks the
compositor: *draw window so-and-so here, on my surface*. The compositor
draws that window, small, over that rectangle, for as long as it is
asked to, and stops when the surface goes or the rectangle is taken
back.

- The component never has the pixels. It cannot keep them, send them or
  read them, so this is not capture, needs no more than the `panel`
  role that already lets it list windows, and sets off no mark.
- The picture is the window as it is now, not as it was when the
  pointer arrived: it changes as the window does.
- Any program with the `panel` role may ask, so another dock can have
  previews too.
- A program that wants the pixels themselves, to record or to send,
  asks for capture, and is treated as ADR 37 says.

### Small, and well made small

A large window drawn at a tenth of its size in one step shimmers and
loses its text. A picture is made small by halves, as the blur makes its
pictures (ADR 33), and the small one is kept.

- It is made again only when the window changes **and** the picture is
  being shown. Nothing is kept or made for a window nobody is looking at
  small.
- A window whose picture is being shown is told to keep drawing, as if
  it were on the screen, but no more often than the picture is shown
  changing. So a film in a window on another workspace moves in its
  preview, and stops costing anything when the preview closes.
- A window that is minimized or on another workspace has the last
  picture it drew, until it draws another.

### Windows that are not to be pictured

A rule can say a window is private:

```yaml
- match: { app_id: org.keepassxc.KeePassXC }
  private: true
```

A private window is shown as its icon and title wherever a picture
would be, in the switcher, in previews and in the overview, and is left
blank in any capture of the screen. A program can say the same of its
own window where a protocol for it exists.

## Alternatives

- **Hand the dock the pictures**, through the capture protocol. The dock
  could then do anything with them, cleverly or wrongly. It makes the
  dock a program that sees everything on the screen, which is the
  largest privilege there is, for the sake of a tooltip.
- **The compositor draws the dock's whole preview**, name and frame and
  all. Then part of the dock is in the compositor, themed and laid out
  in two places, and another dock gets ours or none.
- **No live pictures: an icon and a title.** Honest and nearly free, and
  what a private window gets. Less than what was asked for.

## Consequences

- The compositor gains one small thing to be asked: a window's picture
  in a rectangle of a surface. It is asked by the route components
  already speak, and it is checked like any request: the window exists,
  the rectangle is inside the asker's own surface, the asker has the
  role.
- A component cannot draw over the picture, since the picture is laid
  over its surface. A close button on a preview goes beside it.
- The switcher, the overview and the previews share one way of making a
  window small, with the tests the blur's pictures have: what is under
  the rectangle is the window's colour, it changes when the window does,
  and nothing is made when nothing is shown.
- Rules gain `private`, which capture honours when capture is built.
- The cost while a preview is open is one small picture remade when its
  window changes. Closed, it is nothing.
