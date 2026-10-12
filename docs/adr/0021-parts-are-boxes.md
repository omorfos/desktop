# 21. Every part is a box, and values are written as CSS writes them

- Status: accepted
- Date: 2026-10-12

## Context

ADR 20 made themes stylesheets, with ten properties and colours in
hexadecimal only. The first theme written against it asked for
`rgba(...)` as a panel's background and was refused. The properties were
also too few to do what themes are usually for: a panel that floats clear
of the screen's edge, rounded buttons with room between them.

"All of CSS" is not on offer. Most of it lays out a document, and there is
no document here: what is on the screen is decided by the components. What
can be had is the part of CSS that says how a box looks.

## Decision

**Every part is a box**, with a margin, a border on each side, padding and
its content, from the outside in. `Style` says so: `border` is a line for
each side, and `padding` and `margin` a size for each. A height is measured
to the outside of the border. `min-width` and `max-width` hold a box to a
narrowest and a widest.

**One painter draws every box.** `Canvas::draw_box` fills a box's
background and draws its borders, rounding its corners as far as the style
says and smoothing them. What is under a rounded corner is left alone, so a
box can sit on another. The titlebar's tabs, the menu and its rows, the
notice, the panel and everything on it are drawn by it, where each painter
used to fill its own rectangles. `omorfos_ui::inset` gives the room left
inside a box's edges.

**The panel is laid out from the boxes.** Its margin makes its surface
larger than the bar and leaves the rest transparent, so the bar floats and
the compositor keeps windows clear of bar and margin alike. Each button's
margin is room on the bar that is not part of the button. What was the
panel's own, the padding of ten and the widest window button of 240, is now
in the built-in stylesheet. The workspace and window buttons are two parts,
`workspace` and `task`, since they are held to different widths and a theme
will want to tell them apart.

**Values are written as CSS writes them.** Colours may be `rgb()` and
`hsl()` with or without opacity, in the old comma form or the new, or any of
the names CSS has. `opacity` fades everything a part draws. `border` gives
width, kind and colour at once, for every side or one. `padding` and
`margin` take one to four sizes. A variable may be part of a value, not only
the whole of it: `border: 1px solid var(--accent)`. Fonts have a weight and
may slant.

**It is still strict.** What is not understood is still an error with a
place, not something passed over.

## Consequences

- With no `theme.css` the desktop looks as it did, to within a pixel in two
  places: the notice is two pixels taller and a tab's title a pixel further
  in, because padding is now inside the border, as in CSS.
- `panel-button` is gone after one commit of existing; a theme that named
  it names `workspace` and `task`.
- Not every part honours every property. The menu's geometry belongs to the
  window-management model, which knows nothing of themes, so a menu row's
  margin does nothing; a window's border is one width all round because its
  geometry is. The guide lists what is honoured where.
- A box does not clip what is inside it. A square button in the corner of a
  rounded panel covers the corner, as it would in a browser without
  `overflow: hidden`; the theme gives the panel padding or the button a
  margin.
- Where borders of different widths meet at a rounded corner, the corner
  takes the thicker one. Getting that exactly as browsers do is a great
  deal of geometry for something themes rarely do.
- Filling in variables is bounded twice over, by how many and by how long
  the value may grow, since one variable may now hold several.
- Shadows, gradients, background images and transitions remain to be done,
  each with drawing of its own.
