# The contrast of every part of the built-in theme

Measured for the
[review of 26 November 2026](../2026-11-26-accessibility-design-wcag.md),
from the theme itself and not from the documents: the colours
`Theme::built_in` gives each of the 58 parts in each of its states, in
both schemes, laid over what is behind them as the theme's own test lays
them (`crates/omorfos-config/src/theme.rs`, `meets_contrast`), and
compared as WCAG 2 compares two colours. The figures are of `main` as
the review found it; the launcher as pull request 177 has it, which is
not on `main`, is at the end.

How to read it:

- **Kind**, and the **least** asked of it: text, 4.5 to 1; large text,
  of 18pt or more, 3; a mark, which is anything that says a state
  without words, 3. Four kinds have no least, and are here to be seen: a
  line that says nothing; what only says the pointer is there; an edge
  that is all that bounds a control; and a fill that goes with a mark
  which says the same.
- **Solid** is the part as it is with `reduce_transparency`, or where it
  is not glass at all. Glass is measured three ways more: over the grey
  half way between black and white, which the theme's test calls an
  ordinary ground; over the worst that can be behind it, a white window
  in the dark scheme and a black one in the light; and over the best,
  which is the other.
- ✗ is under the least for its kind. `·` is a part that is solid, where
  what is behind makes no difference.
- **At the worst** is the pair of colours measured there: what is drawn,
  on what it comes to be drawn on.

## The dark scheme, with the accent that is built in

| What | Kind | Least | Solid | Over grey | Over the worst | Over the best | At the worst |
|---|---|---:|---:|---:|---:|---:|---|
| A title, its window with focus | text | 4.5 | 13.93 | 10.87 | 7.40 | 14.88 | `#e6eaf5` on `#43495f` |
| A title, its window without | text | 4.5 | 7.18 | 5.62 | 3.86 ✗ | 7.54 | `#9aa4c7` on `#3f4558` |
| The tab shown, its frame with focus | text | 4.5 | 13.93 | 10.87 | 7.40 | 14.88 | `#e6eaf5` on `#43495f` |
| A tab not shown, its frame with focus | text | 4.5 | 6.79 | 5.30 | 3.61 ✗ | 7.25 | `#9aa4c7` on `#43495f` |
| The tab shown, its frame without | text | 4.5 | 7.18 | 5.62 | 3.86 ✗ | 7.54 | `#9aa4c7` on `#3f4558` |
| A tab not shown, its frame without | text | 4.5 | 7.18 | 5.62 | 3.86 ✗ | 7.54 | `#9aa4c7` on `#3f4558` |
| The shown tab's underline, its frame with focus | mark | 3.0 | 9.27 | 7.24 | 4.93 | 9.91 | `#22d3ee` on `#43495f` |
| The shown tab's underline, its frame without | mark | 3.0 | 1.62 ✗ | 1.27 ✗ | 1.15 ✗ | 1.70 ✗ | `#2b3a6b` on `#3f4558` |
| The line between tabs | line | – | 1.53 | 1.20 | 1.23 | 1.64 | `#2b3a6b` on `#43495f` |
| A menu's row | text | 4.5 | 13.93 | 10.87 | 7.40 | 14.88 | `#e6eaf5` on `#43495f` |
| A menu's row under the pointer | text | 4.5 | 10.07 | 10.07 | 10.07 | 10.07 | `#e6eaf5` on `#243364` |
| A menu's row the keyboard is on | text | 4.5 | 10.28 | 10.28 | 10.28 | 10.28 | `#0c1226` on `#22d3ee` |
| A menu's keys (dim) | text | 4.5 | 6.79 | 5.30 | 3.61 ✗ | 7.25 | `#9aa4c7` on `#43495f` |
| A menu's keys under the pointer | text | 4.5 | 4.91 | 4.91 | 4.91 | 4.91 | `#9aa4c7` on `#243364` |
| A menu's keys on the keyboard's row | text | 4.5 | 10.28 | 10.28 | 10.28 | 10.28 | `#0c1226` on `#22d3ee` |
| A menu's sign, off | mark | 3.0 | 13.93 | 10.87 | 7.40 | 14.88 | `#e6eaf5` on `#43495f` |
| A menu's sign, on, against its square | mark | 3.0 | 6.05 | 4.78 | 3.52 | 6.55 | `#22d3ee` on `#3c647b` |
| The square of a sign that is on, against the menu | mark | 3.0 | 1.53 ✗ | 1.51 ✗ | 1.40 ✗ | 1.51 ✗ | `#3c647b` on `#43495f` |
| The keyboard's row, its fill against the menu | mark | 3.0 | 9.27 | 7.24 | 4.93 | 9.91 | `#22d3ee` on `#43495f` |
| The pointer's row, its fill against the menu | pointer | – | 1.38 | 1.08 | 1.36 | 1.48 | `#243364` on `#43495f` |
| A menu's separator | line | – | 1.53 | 1.20 | 1.23 | 1.64 | `#2b3a6b` on `#43495f` |
| The notice's words | text | 4.5 | 13.93 | 13.20 | 12.23 | 14.25 | `#e6eaf5` on `#1f2742` |
| The notice's heading (danger) | text | 4.5 | 5.52 | 5.24 | 4.85 | 5.65 | `#f8616d` on `#1f2742` |
| A tooltip | text | 4.5 | 13.93 | 10.87 | 7.40 | 14.88 | `#e6eaf5` on `#43495f` |
| The capture mark's dot on its ground | mark | 3.0 | 6.12 | · | · | · | `#f8616d` on `#0c1226` |
| The bar: a workspace | text | 4.5 | 13.93 | 10.87 | 7.40 | 14.88 | `#e6eaf5` on `#43495f` |
| The bar: the workspace shown | text | 4.5 | 10.07 | 10.07 | 10.07 | 10.07 | `#e6eaf5` on `#243364` |
| The bar: a window | text | 4.5 | 13.93 | 10.87 | 7.40 | 14.88 | `#e6eaf5` on `#43495f` |
| The bar: the window with focus | text | 4.5 | 10.07 | 10.07 | 10.07 | 10.07 | `#e6eaf5` on `#243364` |
| The bar: the time | text | 4.5 | 13.93 | 10.87 | 7.40 | 14.88 | `#e6eaf5` on `#43495f` |
| The bar: the underline of what is shown | mark | 3.0 | 6.71 | 6.71 | 6.71 | 6.71 | `#22d3ee` on `#243364` |
| The bar: the fill of what is shown, against the bar | fill | – | 1.38 | 1.08 | 1.36 | 1.48 | `#243364` on `#43495f` |
| A popover's row | text | 4.5 | 13.93 | 10.87 | 7.40 | 14.88 | `#e6eaf5` on `#43495f` |
| The month: a day | text | 4.5 | 13.93 | 10.87 | 7.40 | 14.88 | `#e6eaf5` on `#43495f` |
| The month: a day of another month (faint) | text | 4.5 | 4.01 ✗ | 3.13 ✗ | 2.13 ✗ | 4.29 ✗ | `#727b9c` on `#43495f` |
| The month: a day of another month, under the pointer | text | 4.5 | 2.90 ✗ | 2.90 ✗ | 2.90 ✗ | 2.90 ✗ | `#727b9c` on `#243364` |
| The month: today | text | 4.5 | 10.28 | 10.28 | 10.28 | 10.28 | `#0c1226` on `#22d3ee` |
| The month: today's fill against the popover | mark | 3.0 | 9.27 | 7.24 | 4.93 | 9.91 | `#22d3ee` on `#43495f` |
| The month: a day's box | line | – | 1.27 | 1.01 | 1.48 | 1.36 | `#222e57` on `#43495f` |
| The month: the ring of the day under the pointer | pointer | – | 9.27 | 7.24 | 4.93 | 9.91 | `#22d3ee` on `#43495f` |
| The launcher: what is typed | text | 4.5 | 10.07 | 10.07 | 10.07 | 10.07 | `#e6eaf5` on `#243364` |
| The launcher: a row | text | 4.5 | 13.93 | 10.87 | 7.40 | 14.88 | `#e6eaf5` on `#43495f` |
| The launcher: the row that is lit | text | 4.5 | 10.28 | 10.28 | 10.28 | 10.28 | `#0c1226` on `#22d3ee` |
| The launcher: what a program says of itself, and the heading (dim) | text | 4.5 | 6.79 | 5.30 | 3.61 ✗ | 7.25 | `#9aa4c7` on `#43495f` |
| The launcher: the same on the row that is lit | text | 4.5 | 10.28 | 10.28 | 10.28 | 10.28 | `#0c1226` on `#22d3ee` |
| The launcher: the lit row's fill against the launcher | mark | 3.0 | 9.27 | 7.24 | 4.93 | 9.91 | `#22d3ee` on `#43495f` |
| The launcher: a program in the grid | text | 4.5 | 13.93 | 10.87 | 7.40 | 14.88 | `#e6eaf5` on `#43495f` |
| A notification's title | text | 4.5 | 13.93 | 10.87 | 7.40 | 14.88 | `#e6eaf5` on `#43495f` |
| A notification's body (dim) | text | 4.5 | 6.79 | 5.30 | 3.61 ✗ | 7.25 | `#9aa4c7` on `#43495f` |
| A notification's edge | line | – | 1.24 | 1.27 | 1.24 | 1.22 | `#51576b` on `#43495f` |
| A notification's edge, the keyboard on it (one unit) | mark | 3.0 | 9.27 | 7.24 | 4.93 | 9.91 | `#22d3ee` on `#43495f` |
| A notification's edge, critical (one unit) | mark | 3.0 | 5.52 | 4.31 | 2.94 ✗ | 5.90 | `#f8616d` on `#43495f` |
| A level: the whole of its bar against the notification | mark | 3.0 | 1.53 ✗ | 1.20 ✗ | 1.23 ✗ | 1.64 ✗ | `#2b3a6b` on `#43495f` |
| A level: how full, against the notification | mark | 3.0 | 9.27 | 7.24 | 4.93 | 9.91 | `#22d3ee` on `#43495f` |
| A level: how full, against the whole | mark | 3.0 | 6.05 | 6.05 | 6.05 | 6.05 | `#22d3ee` on `#2b3a6b` |
| A question's words | text | 4.5 | 13.93 | · | · | · | `#e6eaf5` on `#141c38` |
| A question's detail (dim) | text | 4.5 | 6.79 | · | · | · | `#9aa4c7` on `#141c38` |
| A question's button | text | 4.5 | 14.72 | · | · | · | `#e6eaf5` on `#10172f` |
| A question's button that gives | text | 4.5 | 10.28 | · | · | · | `#0c1226` on `#22d3ee` |
| A question's button: its fill against the box, its only edge | edge | – | 1.06 | · | · | · | `#10172f` on `#141c38` |
| A question's button: the keyboard's ring against the box | mark | 3.0 | 9.27 | · | · | · | `#22d3ee` on `#141c38` |
| A question's field: what is typed | text | 4.5 | 10.07 | · | · | · | `#e6eaf5` on `#243364` |
| A question's field: its edge against the box | mark | 3.0 | 4.46 | · | · | · | `#7482b3` on `#141c38` |
| A question's field: its edge against its own fill | edge | – | 3.22 | · | · | · | `#7482b3` on `#243364` |
| A question's field: its edge with the keyboard, against the box | mark | 3.0 | 9.27 | · | · | · | `#22d3ee` on `#141c38` |
| The switcher's title | text | 4.5 | 13.93 | 10.87 | 7.40 | 14.88 | `#e6eaf5` on `#43495f` |
| The switcher: the edge of the window picked | mark | 3.0 | 9.27 | 7.24 | 4.93 | 9.91 | `#22d3ee` on `#43495f` |
| The overview's title | text | 4.5 | 13.93 | · | · | · | `#e6eaf5` on `#141c38` |
| "About": its heading | text | 4.5 | 13.93 | 10.87 | 7.40 | 14.88 | `#e6eaf5` on `#43495f` |
| "About": what is under it (dim) | text | 4.5 | 6.79 | 5.30 | 3.61 ✗ | 7.25 | `#9aa4c7` on `#43495f` |
| "About": its button | text | 4.5 | 14.72 | 14.72 | 14.72 | 14.72 | `#e6eaf5` on `#10172f` |
| "About": its button once copied | text | 4.5 | 12.32 | 12.32 | 12.32 | 12.32 | `#0c1226` on `#a3e635` |
| "About": its button's fill against the box, its only edge | edge | – | 1.06 | 1.35 | 1.99 | 1.01 | `#10172f` on `#43495f` |

## The light scheme, with the accent that is built in

| What | Kind | Least | Solid | Over grey | Over the worst | Over the best | At the worst |
|---|---|---:|---:|---:|---:|---:|---|
| A title, its window with focus | text | 4.5 | 18.24 | 15.43 | 12.89 | 18.25 | `#0c1226` on `#d9d7cf` |
| A title, its window without | text | 4.5 | 4.85 | 4.51 | 4.14 ✗ | 4.89 | `#5c6888` on `#e4dfca` |
| The tab shown, its frame with focus | text | 4.5 | 18.24 | 15.43 | 12.89 | 18.25 | `#0c1226` on `#d9d7cf` |
| A tab not shown, its frame with focus | text | 4.5 | 5.44 | 4.60 | 3.84 ✗ | 5.44 | `#5c6888` on `#d9d7cf` |
| The tab shown, its frame without | text | 4.5 | 4.85 | 4.51 | 4.14 ✗ | 4.89 | `#5c6888` on `#e4dfca` |
| A tab not shown, its frame without | text | 4.5 | 4.85 | 4.51 | 4.14 ✗ | 4.89 | `#5c6888` on `#e4dfca` |
| The shown tab's underline, its frame with focus | mark | 3.0 | 5.26 | 4.45 | 3.72 | 5.26 | `#0e7490` on `#d9d7cf` |
| The shown tab's underline, its frame without | mark | 3.0 | 1.19 ✗ | 1.11 ✗ | 1.02 ✗ | 1.20 ✗ | `#e4ddc4` on `#e4dfca` |
| The line between tabs | line | – | 1.33 | 1.13 | 1.06 | 1.34 | `#e4ddc4` on `#d9d7cf` |
| A menu's row | text | 4.5 | 18.24 | 15.43 | 12.89 | 18.25 | `#0c1226` on `#d9d7cf` |
| A menu's row under the pointer | text | 4.5 | 18.58 | 18.58 | 18.58 | 18.58 | `#0c1226` on `#ffffff` |
| A menu's row the keyboard is on | text | 4.5 | 5.26 | 5.26 | 5.26 | 5.26 | `#fffdf4` on `#0e7490` |
| A menu's keys (dim) | text | 4.5 | 5.44 | 4.60 | 3.84 ✗ | 5.44 | `#5c6888` on `#d9d7cf` |
| A menu's keys under the pointer | text | 4.5 | 5.54 | 5.54 | 5.54 | 5.54 | `#5c6888` on `#ffffff` |
| A menu's keys on the keyboard's row | text | 4.5 | 5.26 | 5.26 | 5.26 | 5.26 | `#fffdf4` on `#0e7490` |
| A menu's sign, off | mark | 3.0 | 18.24 | 15.43 | 12.89 | 18.25 | `#0c1226` on `#d9d7cf` |
| A menu's sign, on, against its square | mark | 3.0 | 3.95 | 3.41 | 2.92 ✗ | 3.95 | `#0e7490` on `#b0c3c2` |
| The square of a sign that is on, against the menu | mark | 3.0 | 1.33 ✗ | 1.31 ✗ | 1.27 ✗ | 1.33 ✗ | `#b0c3c2` on `#d9d7cf` |
| The keyboard's row, its fill against the menu | mark | 3.0 | 5.26 | 4.45 | 3.72 | 5.26 | `#0e7490` on `#d9d7cf` |
| The pointer's row, its fill against the menu | pointer | – | 1.02 | 1.20 | 1.44 | 1.02 | `#ffffff` on `#d9d7cf` |
| A menu's separator | line | – | 1.33 | 1.13 | 1.06 | 1.34 | `#e4ddc4` on `#d9d7cf` |
| The notice's words | text | 4.5 | 18.24 | 17.17 | 16.28 | 18.24 | `#0c1226` on `#f2f0e7` |
| The notice's heading (danger) | text | 4.5 | 5.38 | 5.07 | 4.80 | 5.38 | `#c4303a` on `#f2f0e7` |
| A tooltip | text | 4.5 | 18.24 | 15.43 | 12.89 | 18.25 | `#0c1226` on `#d9d7cf` |
| The capture mark's dot on its ground | mark | 3.0 | 5.10 | · | · | · | `#c4303a` on `#fcf7e3` |
| The bar: a workspace | text | 4.5 | 18.24 | 15.43 | 12.89 | 18.25 | `#0c1226` on `#d9d7cf` |
| The bar: the workspace shown | text | 4.5 | 18.58 | 18.58 | 18.58 | 18.58 | `#0c1226` on `#ffffff` |
| The bar: a window | text | 4.5 | 18.24 | 15.43 | 12.89 | 18.25 | `#0c1226` on `#d9d7cf` |
| The bar: the window with focus | text | 4.5 | 18.58 | 18.58 | 18.58 | 18.58 | `#0c1226` on `#ffffff` |
| The bar: the time | text | 4.5 | 18.24 | 15.43 | 12.89 | 18.25 | `#0c1226` on `#d9d7cf` |
| The bar: the underline of what is shown | mark | 3.0 | 5.36 | 5.36 | 5.36 | 5.36 | `#0e7490` on `#ffffff` |
| The bar: the fill of what is shown, against the bar | fill | – | 1.02 | 1.20 | 1.44 | 1.02 | `#ffffff` on `#d9d7cf` |
| A popover's row | text | 4.5 | 18.24 | 15.43 | 12.89 | 18.25 | `#0c1226` on `#d9d7cf` |
| The month: a day | text | 4.5 | 18.24 | 15.43 | 12.89 | 18.25 | `#0c1226` on `#d9d7cf` |
| The month: a day of another month (faint) | text | 4.5 | 2.95 ✗ | 2.49 ✗ | 2.08 ✗ | 2.95 ✗ | `#8d95a8` on `#d9d7cf` |
| The month: a day of another month, under the pointer | text | 4.5 | 3.00 ✗ | 3.00 ✗ | 3.00 ✗ | 3.00 ✗ | `#8d95a8` on `#ffffff` |
| The month: today | text | 4.5 | 5.26 | 5.26 | 5.26 | 5.26 | `#fffdf4` on `#0e7490` |
| The month: today's fill against the popover | mark | 3.0 | 5.26 | 4.45 | 3.72 | 5.26 | `#0e7490` on `#d9d7cf` |
| The month: a day's box | line | – | 1.18 | 1.00 | 1.20 | 1.18 | `#efead7` on `#d9d7cf` |
| The month: the ring of the day under the pointer | pointer | – | 5.26 | 4.45 | 3.72 | 5.26 | `#0e7490` on `#d9d7cf` |
| The launcher: what is typed | text | 4.5 | 18.58 | 18.58 | 18.58 | 18.58 | `#0c1226` on `#ffffff` |
| The launcher: a row | text | 4.5 | 18.24 | 15.43 | 12.89 | 18.25 | `#0c1226` on `#d9d7cf` |
| The launcher: the row that is lit | text | 4.5 | 5.26 | 5.26 | 5.26 | 5.26 | `#fffdf4` on `#0e7490` |
| The launcher: what a program says of itself, and the heading (dim) | text | 4.5 | 5.44 | 4.60 | 3.84 ✗ | 5.44 | `#5c6888` on `#d9d7cf` |
| The launcher: the same on the row that is lit | text | 4.5 | 5.26 | 5.26 | 5.26 | 5.26 | `#fffdf4` on `#0e7490` |
| The launcher: the lit row's fill against the launcher | mark | 3.0 | 5.26 | 4.45 | 3.72 | 5.26 | `#0e7490` on `#d9d7cf` |
| The launcher: a program in the grid | text | 4.5 | 18.24 | 15.43 | 12.89 | 18.25 | `#0c1226` on `#d9d7cf` |
| A notification's title | text | 4.5 | 18.24 | 15.43 | 12.89 | 18.25 | `#0c1226` on `#d9d7cf` |
| A notification's body (dim) | text | 4.5 | 5.44 | 4.60 | 3.84 ✗ | 5.44 | `#5c6888` on `#d9d7cf` |
| A notification's edge | line | – | 1.24 | 1.24 | 1.24 | 1.24 | `#c4c2bd` on `#d9d7cf` |
| A notification's edge, the keyboard on it (one unit) | mark | 3.0 | 5.26 | 4.45 | 3.72 | 5.26 | `#0e7490` on `#d9d7cf` |
| A notification's edge, critical (one unit) | mark | 3.0 | 5.38 | 4.55 | 3.80 | 5.38 | `#c4303a` on `#d9d7cf` |
| A level: the whole of its bar against the notification | mark | 3.0 | 1.33 ✗ | 1.13 ✗ | 1.06 ✗ | 1.34 ✗ | `#e4ddc4` on `#d9d7cf` |
| A level: how full, against the notification | mark | 3.0 | 5.26 | 4.45 | 3.72 | 5.26 | `#0e7490` on `#d9d7cf` |
| A level: how full, against the whole | mark | 3.0 | 3.94 | 3.94 | 3.94 | 3.94 | `#0e7490` on `#e4ddc4` |
| A question's words | text | 4.5 | 18.24 | · | · | · | `#0c1226` on `#fffdf4` |
| A question's detail (dim) | text | 4.5 | 5.44 | · | · | · | `#5c6888` on `#fffdf4` |
| A question's button | text | 4.5 | 16.28 | · | · | · | `#0c1226` on `#f6f0da` |
| A question's button that gives | text | 4.5 | 5.26 | · | · | · | `#fffdf4` on `#0e7490` |
| A question's button: its fill against the box, its only edge | edge | – | 1.12 | · | · | · | `#f6f0da` on `#fffdf4` |
| A question's button: the keyboard's ring against the box | mark | 3.0 | 5.26 | · | · | · | `#0e7490` on `#fffdf4` |
| A question's field: what is typed | text | 4.5 | 18.58 | · | · | · | `#0c1226` on `#ffffff` |
| A question's field: its edge against the box | mark | 3.0 | 3.83 | · | · | · | `#7a8099` on `#fffdf4` |
| A question's field: its edge against its own fill | edge | – | 3.91 | · | · | · | `#7a8099` on `#ffffff` |
| A question's field: its edge with the keyboard, against the box | mark | 3.0 | 5.26 | · | · | · | `#0e7490` on `#fffdf4` |
| The switcher's title | text | 4.5 | 18.24 | 15.43 | 12.89 | 18.25 | `#0c1226` on `#d9d7cf` |
| The switcher: the edge of the window picked | mark | 3.0 | 5.26 | 4.45 | 3.72 | 5.26 | `#0e7490` on `#d9d7cf` |
| The overview's title | text | 4.5 | 18.24 | · | · | · | `#0c1226` on `#fffdf4` |
| "About": its heading | text | 4.5 | 18.24 | 15.43 | 12.89 | 18.25 | `#0c1226` on `#d9d7cf` |
| "About": what is under it (dim) | text | 4.5 | 5.44 | 4.60 | 3.84 ✗ | 5.44 | `#5c6888` on `#d9d7cf` |
| "About": its button | text | 4.5 | 16.28 | 16.28 | 16.28 | 16.28 | `#0c1226` on `#f6f0da` |
| "About": its button once copied | text | 4.5 | 4.94 | 4.94 | 4.94 | 4.94 | `#fcf7e3` on `#2f7a35` |
| "About": its button's fill against the box, its only edge | edge | – | 1.12 | 1.06 | 1.26 | 1.12 | `#f6f0da` on `#d9d7cf` |

## The six accents, and eight of one's own

The eight are not ones anyone is known to use: a grey, each scheme's
surface and its raised surface, white, black, the brand's yellow and
GNOME's blue, to see what an accent of one's own can do (finding 2).

| Scheme | Accent | Its colour | What is written on it | Words on the accent | The accent against a solid surface | The ring against a question's box | A sign on against a sign off | #177: the top hit's outline against the launcher |
|---|---|---|---|---:|---:|---:|---:|---:|
| dark | cyan | `#22d3ee` | `#0c1226` | 10.28 | 9.27 | 9.27 | 1.50 | 1.53 |
| dark | blue | `#5b9cf5` | `#0c1226` | 6.65 | 6.00 | 6.00 | 2.32 | 1.38 |
| dark | teal | `#4fc3b5` | `#0c1226` | 8.68 | 7.83 | 7.83 | 1.78 | 1.47 |
| dark | purple | `#a78bfa` | `#0c1226` | 6.83 | 6.16 | 6.16 | 2.26 | 1.40 |
| dark | pink | `#f27fb0` | `#0c1226` | 7.47 | 6.73 | 6.73 | 2.07 | 1.39 |
| dark | orange | `#f29b5c` | `#0c1226` | 8.51 | 7.68 | 7.68 | 1.81 | 1.44 |
| dark | one's own | `#777777` | `#0c1226` | 4.15 | 3.74 | 3.74 | 3.72 | 1.25 |
| dark | one's own | `#141c38` | `#e6eaf5` | 13.93 | 1.00 | 1.00 | 13.93 | 1.00 |
| dark | one's own | `#fffdf4` | `#0c1226` | 18.24 | 16.45 | 16.45 | 1.18 | 1.88 |
| dark | one's own | `#243364` | `#e6eaf5` | 10.07 | 1.38 | 1.38 | 10.07 | 1.05 |
| dark | one's own | `#ffffff` | `#0c1226` | 18.58 | 16.76 | 16.76 | 1.20 | 1.88 |
| dark | one's own | `#000000` | `#e6eaf5` | 17.45 | 1.25 | 1.25 | 17.45 | 1.07 |
| dark | one's own | `#fde047` | `#0c1226` | 14.10 | 12.71 | 12.71 | 1.10 | 1.67 |
| dark | one's own | `#3584e4` | `#0c1226` | 4.93 | 4.45 | 4.45 | 3.13 | 1.29 |
| light | cyan | `#0e7490` | `#fffdf4` | 5.26 | 5.26 | 5.26 | 3.47 | 1.33 |
| light | blue | `#2563d9` | `#fffdf4` | 5.33 | 5.33 | 5.33 | 3.42 | 1.33 |
| light | teal | `#0f766e` | `#fffdf4` | 5.37 | 5.37 | 5.37 | 3.40 | 1.33 |
| light | purple | `#6d3fd4` | `#fffdf4` | 6.22 | 6.22 | 6.22 | 2.93 | 1.36 |
| light | pink | `#c02a72` | `#fffdf4` | 5.40 | 5.40 | 5.40 | 3.38 | 1.37 |
| light | orange | `#b84a0e` | `#fffdf4` | 5.12 | 5.12 | 5.12 | 3.56 | 1.33 |
| light | one's own | `#777777` | `#fffdf4` | 4.39 | 4.39 | 4.39 | 4.15 | 1.27 |
| light | one's own | `#141c38` | `#fffdf4` | 16.45 | 16.45 | 16.45 | 1.11 | 1.52 |
| light | one's own | `#fffdf4` | `#0c1226` | 18.24 | 1.00 | 1.00 | 18.24 | 1.00 |
| light | one's own | `#243364` | `#fffdf4` | 11.89 | 11.89 | 11.89 | 1.53 | 1.45 |
| light | one's own | `#ffffff` | `#0c1226` | 18.58 | 1.02 | 1.02 | 18.58 | 1.00 |
| light | one's own | `#000000` | `#fffdf4` | 20.61 | 20.61 | 20.61 | 1.13 | 1.61 |
| light | one's own | `#fde047` | `#0c1226` | 14.10 | 1.29 | 1.29 | 14.10 | 1.06 |
| light | one's own | `#3584e4` | `#0c1226` | 4.93 | 3.70 | 3.70 | 4.93 | 1.27 |

## The launcher as pull request 177 has it

Of its branch as it stood on 26 November 2026, with 63 parts. What is
the same as on `main` is not said again.

In the dark scheme:

| What | Kind | Least | Solid | Over grey | Over the worst | Over the best | At the worst |
|---|---|---:|---:|---:|---:|---:|---|
| What is typed (20pt) | large text | 3.0 | 13.93 | 10.87 | 7.40 | 14.88 | `#e6eaf5` on `#43495f` |
| "Search", where nothing is typed (dim, 20pt) | large text | 3.0 | 6.79 | 5.30 | 3.61 | 7.25 | `#9aa4c7` on `#43495f` |
| The top hit's words | text | 4.5 | 13.93 | 10.87 | 7.40 | 14.88 | `#e6eaf5` on `#43495f` |
| The top hit's outline, which says what Enter takes | mark | 3.0 | 1.53 ✗ | 1.51 ✗ | 1.40 ✗ | 1.51 ✗ | `#3c647b` on `#43495f` |
| A heading (dim, weight 600) | text | 4.5 | 6.79 | 5.30 | 3.61 ✗ | 7.25 | `#9aa4c7` on `#43495f` |
| A mode | text | 4.5 | 13.93 | 10.87 | 7.40 | 14.88 | `#e6eaf5` on `#43495f` |
| A mode under the pointer | text | 4.5 | 9.08 | 7.18 | 5.29 | 9.83 | `#e6eaf5` on `#3c647b` |
| The mode shown | text | 4.5 | 10.28 | 10.28 | 10.28 | 10.28 | `#0c1226` on `#22d3ee` |
| The mode shown: its fill against the launcher | mark | 3.0 | 9.27 | 7.24 | 4.93 | 9.91 | `#22d3ee` on `#43495f` |
| A mode's key in its chip (dim, 9pt) | text | 4.5 | 5.82 | 4.45 ✗ | 3.08 ✗ | 6.29 | `#9aa4c7` on `#4e5368` |
| The line over the modes | line | – | 1.24 | 1.27 | 1.24 | 1.22 | `#51576b` on `#43495f` |

In the light:

| What | Kind | Least | Solid | Over grey | Over the worst | Over the best | At the worst |
|---|---|---:|---:|---:|---:|---:|---|
| What is typed (20pt) | large text | 3.0 | 18.24 | 15.43 | 12.89 | 18.25 | `#0c1226` on `#d9d7cf` |
| "Search", where nothing is typed (dim, 20pt) | large text | 3.0 | 5.44 | 4.60 | 3.84 | 5.44 | `#5c6888` on `#d9d7cf` |
| The top hit's words | text | 4.5 | 18.24 | 15.43 | 12.89 | 18.25 | `#0c1226` on `#d9d7cf` |
| The top hit's outline, which says what Enter takes | mark | 3.0 | 1.33 ✗ | 1.31 ✗ | 1.27 ✗ | 1.33 ✗ | `#b0c3c2` on `#d9d7cf` |
| A heading (dim, weight 600) | text | 4.5 | 5.44 | 4.60 | 3.84 ✗ | 5.44 | `#5c6888` on `#d9d7cf` |
| A mode | text | 4.5 | 18.24 | 15.43 | 12.89 | 18.25 | `#0c1226` on `#d9d7cf` |
| A mode under the pointer | text | 4.5 | 13.69 | 11.82 | 10.11 | 13.69 | `#0c1226` on `#b0c3c2` |
| The mode shown | text | 4.5 | 5.26 | 5.26 | 5.26 | 5.26 | `#fffdf4` on `#0e7490` |
| The mode shown: its fill against the launcher | mark | 3.0 | 5.26 | 4.45 | 3.72 | 5.26 | `#0e7490` on `#d9d7cf` |
| A mode's key in its chip (dim, 9pt) | text | 4.5 | 5.44 | 4.60 | 3.84 ✗ | 5.44 | `#5c6888` on `#d9d7cf` |
| The line over the modes | line | – | 1.24 | 1.24 | 1.24 | 1.24 | `#c4c2bd` on `#d9d7cf` |
