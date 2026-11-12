# 75. The switcher on a diagonal

- Status: accepted
- Date: 2026-11-12

## Context

The switcher on `Alt+Tab` showed the windows side by side in rows, each
small in its place over its title, in a box in the middle of the screen
(ADR 38). The roadmap had always meant it to stand them one behind
another on a diagonal, turned through as a queue (wish 5): the same
windows, in the same order, walked by the same keys, drawn another way.

Drawn so, the windows overlap. In rows the compositor painted one canvas
with every window's place in it, and drew each window's picture over it;
with places overlapping, a picture further back would show through the
place of the one in front.

## Decision

**The switcher stands the windows on a diagonal unless
`switcher.style: flat` says rows.** It is a setting of `config.yaml`
rather than of the theme: it changes how the switcher behaves, which
window is where and how it moves, not only how it looks.

- **The queue is the order of use, turned to the one picked.** The one
  picked stands in front, and those after it go back up and to the
  right, going round to those before it: each 85% as large as the one in
  front of it, 30% and 18% of the front's width further across and up,
  and 12% fainter, six at most, the last fading as it goes beyond them.
  The one in front is as wide as fits, but no wider than 720 units of
  layout. Under it is the switcher's box, `switcher`, with the title and
  the icon of the one picked; a window's title is not drawn in its place.
- **A window is its picture, with a border round it**: `switcher-window`,
  its border its `padding` away from the picture, in the window's own
  shape rather than a box of one shape for all, the accent round the one
  picked and nothing round the rest. A box filled
  behind each, as in rows, was tried first and was heavy on a diagonal,
  where being in front already says which is picked.
- **Each window's border is painted once, as large as it is in front**,
  and drawn smaller further back, border and all, rather than painted
  again at each size as the queue turns; and painted again only when it
  changes, which a Tab does to two, the one leaving the front and the
  one coming to it. Each window is drawn whole, its picture under its
  border, before the one behind, so that what is in front covers what is
  behind.
- **Behind the switcher its screen is dimmed and blurred**, by
  `switcher-backdrop`, in rows as on a diagonal, so that the eye goes to
  the switcher: the dim of the theme's colour, 35% black in the built-in
  one, over what is behind blurred by its `backdrop-filter`, as the
  session is under a prompt for a password (`sealed`), and the dim alone
  where blur cannot be drawn or transparency is reduced.
- **A window's picture is never made larger than the window is**, which
  would only blur it: a small one stands in the middle of its place, as
  an icon given as pixels does. This holds in rows too, where a window
  narrower than its place used to be made larger.
- **Going on turns the queue there over 240 ms**, easing in and out, as
  `docs/DESIGN.md` has something moving to where it was sent: the one in
  front grows a little and fades out for the first half, and comes into
  the back for the second, while the rest come forward. A key pressed
  before it has turned takes it on from where it has got to, but no more
  than two windows behind the keys, so that Tab held down does not send
  it spinning. With `appearance.reduce_motion`, it stands at once.
- **A private window is shown as in pictures of the screen**, as the
  switcher is drawn onto it and so into them: blank, in the `capture`
  part's background, with its icon large in the middle; or, if its rule
  says `private: blur`, its picture under glass that blurs it as much as
  in pictures (32 units), blank where glass cannot blur. Blurring every
  private window would show of one meant to be blank its colours and
  shapes.
- **A click goes to the window in front where it is clicked.**
- **It is shown once its keys have been held for 150 ms**, as GNOME's and
  Windows's are, so that a quick `Alt+Tab` goes back to the window before
  without the switcher, its dim and its blur showing for a frame or two.
  It opens on the first Tab all the same, and goes as far as the keys
  take it while it is not shown; opened by other than keys, as from a
  menu, with nothing held to let go, it is shown at once.

How each window stands is the desktop model's (`Switcher::queue`), which
is told how far the queue has still to turn and has no clock; the
compositor keeps the clock.

## Alternatives

- **Replacing the rows.** Less to keep, but rows show more windows at a
  glance, which some prefer, and are what motion reduced to nothing
  looks like anyway.
- **The style in the theme.** A theme says how parts look; which windows
  are shown, and where, is behaviour, and has been the configuration's
  since `appearance`.
- **Perspective, the windows turned in depth.** It would need the
  renderer to draw a picture other than as a rectangle, which Smithay's
  renderers do not; smaller, higher and fainter reads as further away
  without it.
- **Each place painted at its own size at each frame of a turn.** Sharper
  borders at the back, at the cost of painting up to seven places sixty
  times a second.

## Consequences

- `SwitcherLayout`'s items are a type of their own, `SwitcherItem`, with
  how much of each is seen.
- The borders further back are thinner, being drawn smaller; and a
  theme's `box-shadow` on `switcher-window` is not drawn.
- `switcher-window` and `switcher-backdrop` are parts of the theme, new
  with this; `switcher-item` is the rows' alone.
- Blurring the whole screen behind the switcher costs what glass costs,
  which ADR 33 has still to measure on a modest laptop; a theme drops the
  blur with `switcher-backdrop { backdrop-filter: none; }`.
- Pictures in the queue are not rounded at the corners, as windows are
  on the screen (ADR 16): cut into strips at each size, each window's
  picture would be drawn many times over at every frame of a turn.
