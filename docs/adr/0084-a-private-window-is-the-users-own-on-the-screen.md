# 84. A private window is the user's own on the screen

- Status: accepted
- Date: 2026-11-14

## Context

A rule can say a window is private (ADR 38): it is left blank in any
picture a program takes of the screen, or blurred with `private: blur`,
and on the screen the user sees it as ever. ADR 38 also had it shown as
its icon and title wherever a picture of it would be, and the switcher
(ADR 75) and the overview (ADR 76) drew it as it is in pictures of the
screen: blank or blurred, on the screen too, but with its title.

That was the wrong way round, as the maintainer found using it. On the
screen, the user it belongs to sees the window itself where it stands,
so blanking its small picture in the switcher hides nothing from them
and only makes it harder to find. In a picture of the screen, a screen
shared in a call or recorded, the switcher kept its title, which is often
what says most: the name of the account, the document or the site.

## Decision

**A private window is shown whole in the switcher and the overview on
the screen, title and picture, as any other window is; in a picture of
the screen it is blank, its icon large, or blurred as its rule says, and
without its title.**

- **What counts as the screen** is what counts for windows themselves:
  the scene drawn with the mark (`Extras::mark`), which is the screen
  and a screen mirrored onto another, not a pass for a program's picture.
  The switcher and the overview are told which they are drawn for, and
  so is each window's small picture (`picture_within`).
- **Two paintings, only while needed.** The switcher's rows, or the box
  under its queue and each window's place in the queue, are painted once
  for the screen and, while a window it shows is private, again for
  pictures; what is the same in both is painted once and shared. The
  overview leaves the title of the window picked out of a picture if it
  is private, and draws no blank over it on the screen.
- **No title in a picture, rather than a bar where it was.** A bar as
  wide as the title would say how long it is. The icon stays: it is what
  a blank window was shown by already, and says only which program it
  is, as the window's frame in the blank would.

## Alternatives

- **Keep it, and only take the title out.** Blank on the screen still
  hides from the user what is theirs.
- **Blank on the screen while a picture is being taken.** The switcher
  would change under the user's eyes as a call starts, and the screen
  and the picture would still differ in what is in them.

## Consequences

- The sentence of ADR 38 on where a private window is shown as its icon
  and title, and the bullets of ADRs 75 and 76 that have it shown in the
  switcher and the overview as in pictures, hold for pictures of the
  screen only.
- A window offered in a part of a workspace with nothing in it (ADR 39)
  is still blank on the screen, and its title is written under its blank
  in pictures of the screen as on it: a leak as the switcher's was. Both
  are to follow, and are on the roadmap.
- Tests read the switcher and the overview both on the screen and in a
  picture a program is given.
- Showing on the screen what a picture leaves out made a fault in how
  pictures were drawn a leak: drawn into what the screen is drawn into,
  they made it keep what it had shown two frames before, to be read into
  the next picture. Pictures are drawn by themselves now (ADR 37,
  amended), and a test draws the screen as a graphics card does, into two
  pictures in turn.
