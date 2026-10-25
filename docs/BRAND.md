# The brand

How omorfos is shown: its mark, its name, its colours and the pictures
that come with it. What the interface looks like is in
[DESIGN.md](DESIGN.md), which takes its grounds and its accent from here.
Why the brand is what it is, and what was tried before it, is in
[ADR 55](adr/0055-the-brand.md).

The pictures are drawn by `omorfos-brand` from the rules below, when
omorfos is built (`make`); none is kept in the repository as a picture,
but for the marks the README shows. A rule
changed here is changed there too, and the two say the same.

## The idea

A window, the oldest picture a desktop has of itself, made a prism:
plain light goes in, and every colour comes out. Everything here is that
one picture: a rounded frame, five bands of colour across it, and on the
larger pictures the same light spilling, blurred, past the frame.

Bright colour, in a quiet layout.

## The mark

[docs/brand/mark.svg](brand/mark.svg), and
[mark-light.svg](brand/mark-light.svg) for light grounds; with the name
beside it, [lockup.svg](brand/lockup.svg) and
[lockup-light.svg](brand/lockup-light.svg). These are drawn by
`omorfos-brand` too, and kept here so that the README can show them; a
test says when they differ from what it draws.

On a square of 300:

| Part | What it is |
|---|---|
| The frame | A rounded square at 48, 204 across, with corners of 44 and a line 18 wide, centred on it: its outside is at 39, and 222 across |
| Inside it | Navy `#0c1226`, or cream `#fcf7e3` for light grounds, up to the frame's inner edge |
| The bands | Five, each 30 wide with 2 between them, turned 45° to the left about the middle and cut to the inside: from the top left, cyan, lime, yellow, orange, pink |
| What they leave | The corners at the top left and the bottom right, in the colour inside. They stay so; the bands do not fill them |

- The bands are not turned another way, recoloured or put in another
  order.
- The frame is paper `#f3f5fb` on dark grounds and navy `#0c1226` on
  light ones, and always solid.
- Round it, at least half the frame's width is kept clear.
- It is not drawn smaller than 16 pixels. Below 24 the gaps between the
  bands close up, which is expected.

### In one colour

For where there can be only one colour, a sign in a tray or something
pressed or printed: the frame, and one rounded bar across the middle at
the same angle, 140 by 30 with round ends, for the bands.
It is installed as the icon `omorfos-symbolic`, in `currentColor`, so
that it takes the colour of what is around it. At 22 pixels or less, where
the bar would be a blot, the frame is drawn alone.

## The name

**omorfos**, from the Greek *όμορφος*, "beautiful". It is written in
lower case always, at the start of a sentence too: never "Omorfos" or
"OMORFOS".

Beside the mark, the name is set in Space Grotesk at a weight of 600,
drawn 0.04 em closer together than the font has it. The two are lined up
on the name's baseline, with a gap of 0.18 of the mark's height, and the
name's capitals would be 0.62 of the mark's outside. The name is drawn as
outlines, so that it looks the same where the font is not installed:
Space Grotesk is used for the name and nothing else, and omorfos does not
install it.

## The icon

The mark filling its square, with 3% clear at each edge and nothing
behind it, drawn at 16, 22, 24, 32, 48, 64, 96, 128, 256, 512 and 1024
pixels and installed as `omorfos` in the `hicolor` icon theme, with the
mark itself as the scalable one.

## Colour

| Name | Value | Used for |
|---|---|---|
| navy | `#0c1226` | The dark ground; inside the mark; text on light grounds |
| navy-2 | `#141c38` | What is raised on the dark ground |
| navy-3 | `#243364` | What is chosen or pressed on it |
| paper | `#f3f5fb` | The frame on dark grounds; the brightest text on them |
| cream | `#fcf7e3` | The light ground, which is warm and not white |
| cream-2 | `#fffdf4` | What is raised on the light ground |

The spectrum, in its order:

| Name | Value |
|---|---|
| cyan | `#22d3ee` |
| lime | `#a3e635` |
| yellow | `#fde047` |
| orange | `#fb923c` |
| pink | `#f472b6` |

All five together are for the mark and the pictures. The interface has
one of them: cyan is its accent on dark grounds, and a deeper cyan,
`#0e7490`, on light ones, where the brand's is too pale to be seen. No
colour of the spectrum is used for text on cream, on which lime and
yellow cannot be read.

## The pictures

Each in dark and in light.

1. **The ground** is navy or cream, lifted towards the top left by a
   wide, soft ellipse of navy-2 or cream-2 at 90%.
2. **Dots** are scattered over it, few and soft: most small, a few up to
   22 pixels across on a screen 3840 wide, each between 4% and 10%
   opaque. There are about 460 on a picture of 3840 by 2160, and as many
   for its area on others. They are to be felt more than seen; a regular
   grid of them was tried and is not used. A grain too fine to see, at
   5%, keeps the lift from stepping from one shade to the next.
3. **The spectrum**, made something of its own on each, in its order
   and always all five.
4. **The mark**, on top and solid, on Prism and the lock screen only.

On Prism, the first wallpaper and the one shown when nothing is named,
the spectrum is **the spill**: the five bands again, wider than the
mark's, lying under the mark along its angle and blurred a long way, at
about half opacity.

| Picture | The mark | The spill |
|---|---|---|
| Wallpaper | Right of the middle, across 56% to 86% of the width of a screen of 16 by 9, so that one of 16 by 10 or 21 by 9, cut to fit, keeps it whole | Under the mark |
| The lock screen | Small, in the bottom right, about 18% of the height, so that the middle is clear for the field | Quieter |

The lock screen's picture is not drawn yet: it comes with the lock screen
having a picture of its own.

The other wallpapers have no mark, and the spectrum is:

| Wallpaper | The spectrum |
|---|---|
| Spill | The bands alone, 7.5% of the width each, across the screen on the mark's angle a little right of the middle, blurred a long way at 55%; and again, 4% each and less blurred, at 45%, so that each colour is seen |
| Horizon | The rings of an ellipse whose middle is far under the bottom edge, cyan outermost, each 2.4% of the height: an arc whose top is two thirds of the way down, softly edged at 60% over a wide glow of itself at 70%. The top two thirds are left to the ground |
| Facets | The bands, 6% of the width each, right of the middle on the mark's angle, each starting and ending somewhere of its own; cut into lengths of 4% to 12% of the width, each cut along one diagonal or the other into two triangles, each at an opacity of its own and fainter towards the ends; over a glow of the bands at 35% |
| Ground | A line 10 pixels high on a screen 3840 wide along the bottom edge, the colours running into each other from left to right, glowing a little above it at 45% |

The wallpapers are installed in `share/backgrounds/omorfos/`, where the
wallpaper looks for them, each as a pair, as `spill-dark.png` and
`spill-light.png`, so that the directory, named as the picture, shows them
in turn as the theme is.

## The terminal

omorfos recommends foot, and the brand has colours for it. The spectrum
is in the bright colours, with each normal one a little deeper, so that
bold reads as a step. Red is a coral, as the spectrum has none and its
pink is magenta; blue is a sky blue, kept well apart from cyan, which
`ls` and `git` both use a great deal; bright black, for comments, is just
under 5 to 1 on the ground; and the cursor is orange, which no colour of
the terminal's is, so that it is always found.

| Colour | Normal | Bright |
|---|---|---|
| 0, 8 black | `#141c38` | `#7482b3` |
| 1, 9 red | `#f8616d` | `#ff8a93` |
| 2, 10 green | `#84cc16` | `#a3e635` |
| 3, 11 yellow | `#facc15` | `#fde047` |
| 4, 12 blue | `#60a5fa` | `#93c5fd` |
| 5, 13 magenta | `#f472b6` | `#f9a8d4` |
| 6, 14 cyan | `#22d3ee` | `#67e8f9` |
| 7, 15 white | `#b8c0de` | `#f3f5fb` |

The background is navy, the text `#e6eaf5`, the cursor orange with navy
under it, and what is chosen navy-3. These are for the dark scheme; the
light one is still to be made, with the same colours made darker until
each is 4.5 to 1 on cream.

## Licence

The mark, the name as drawn and the pictures are under
[CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/): they may
be shared and changed, with credit and under the same licence. That
licence gives no right to the name: something made from them is not to
be passed off as omorfos. The program that draws them is under the MIT
licence, as all of omorfos's code is. [CREDITS.md](../CREDITS.md) lists
the font the name is drawn from.
