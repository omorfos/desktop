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

## What was built first, and what it settled

The `private` rule came with pictures of the screen, before the switcher
and the dock that this record is mostly about.

- **It is asked of a window as it is now.** Every other rule is looked at
  once, when a window opens. A browser's title says what page it shows
  only after it has opened, and changes, so a rule on a title that was
  looked at once would hide nothing. Whether a window is private is
  worked out each time the scene is laid out.
- **The whole frame is blank**, titlebar included, since the titlebar
  says what the window is; and with it any window that shares the frame
  as a tab, whose title is on the same titlebar.
- **What stands in its place is kept** from one frame to the next, so
  that a program taking pictures is not told that part of the screen
  changed when it did not. It is the background colour of the `capture`
  part of the theme, so the mark and the blank are one thing to style.
- **It does not fade into a picture.** A window that closes fades out on
  the screen as it last was, from what was drawn of it; for a private
  window that is its contents, so in a picture it is gone at once, and
  only the screen shows it fade.

**A picture of one window** is what the window draws, without the frame
omorfos puts round it, as the protocol means it. It was drawn by itself
in the corner of the screen the window is on, before that screen was
drawn, and cut from what was read back; so it could be no larger than
that screen, and what was see-through in it came out black. It is drawn
now in a picture of its own, as large as the window and clear where
nothing is drawn, and read back from that: a renderer can draw into such
a picture and read it back on real hardware too, which drawing from it
again needs and does not have (see "Made small in one step"). It is
offered see-through (`Argb8888`) as well as solid, and `omorfos-screenshot`
saves it see-through. It is all that the window draws: a program that
draws its own titlebar draws a shadow round what it says is the window,
and the picture was cut to that at first, which left the shadow out;
tried by hand, that looked cropped. A window out of sight can have its picture taken, since what it
last drew is still held; one a rule calls private is a blank of its size.
A window can say it is as large as it likes, and is drawn no larger than
16384 pixels across or down.

**Made small in one step, for now.** The first window drawn small is one
offered in a part of a workspace with nothing in it (ADR 39), and it is
drawn straight at its small size, with the renderer's smoothing, not by
halves. Halving needs the renderer to draw into a picture and then draw
from it, and on real hardware omorfos draws through Smithay's renderer
for several cards, which can draw into a picture of one card but gives
no way to draw from it again. A part is half the screen or a quarter,
so a window is made a third or a sixth of its size there; text may be
rough, and is readable. Halving waits for the switcher, which makes
windows smaller still, and for a way past that renderer: drawing with
the card's own renderer, or reading the picture back and halving it in
memory, each with its cost.

**A window is named by its entry in the list of windows**, which only a
program with the `panel` role is given. So taking a picture of a window
needs `panel` as well as `capture`. The socket for scripts says what each
window is called in that list (`handle`), which is how the screenshot
tool finds the one with focus.

**The switcher came next**, drawn by the compositor as this record has
it, and made small in one step as the offered windows are, not yet by
halves: a window's place is about a sixth of a screen across, and its
title is under it, so that rough text in the picture is not all there is
to know it by. Each window is fitted to its place with its shape kept,
and told to keep drawing while it is shown. The switcher is on the screen
the pointer was on when it opened, and stays there. It closes, without
choosing, when the session is locked.

It is opened by a binding and stays open while the modifiers held with
that binding are, but for Shift, which goes back: letting go of any of
them chooses. Opened by something with no keys held, a menu or a script,
it stays open until Enter, Escape or a click. While it is open the
compositor takes Escape, Enter and the arrows, and the pointer, whose
press chooses the window under it or closes the switcher; every other
key is left to the bindings, which is how Tab goes on to the next
window. No window has the keyboard while it is open: the window with
focus saw Alt go down, and would otherwise see it come up with nothing
between, which a browser takes as Alt pressed alone and shows its menu
bar for. The window chosen, or the one left as it was, has it back
when the switcher closes.

**Icons came after.** Each window's icon is beside its title, and a
private window has it large where its picture would be. It is the one
its program gives (`xdg-toplevel-icon`): by name, looked up in the icon
themes as the shell's components look theirs up, or else as pixels;
failing that, the one its desktop entry names, or the one named as its
`app_id` is.

- **A name is a name.** The protocol lets a program name an icon, and
  the name could be the path of a file, which the compositor would then
  open: one of the program's choosing, that a program in a sandbox could
  not read itself. A name with a `/` in it is passed over for the
  pixels. A desktop entry's `Icon` may still be a path, since the entry
  is installed, not sent.
- **Pixels are drawn by the renderer, not read.** They are in memory the
  program shares, which only `unsafe` code of ours could read (ADR 40);
  the renderer reads it as it reads a window's own. So an icon given as
  pixels is drawn over the switcher, and one found by name is painted
  into it. It is never drawn larger than it was given, which would only
  make it blocky.
- **The compositor reads icon files** now, with the same library the
  shell's components use, which makes it 1.3 MB larger. A drawing is
  made at the size it is wanted at, about two milliseconds each the
  first time; what was found, and what was not, is kept, so the first
  opening of the switcher after a program starts is the only one that
  pays.
