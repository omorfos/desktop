# 55. The brand, and its pictures drawn as omorfos is built

- Status: accepted
- Date: 2026-10-24
- Amends: [32. The design language](0032-the-design-language.md)

## Context

omorfos had no mark, no icon and no wallpaper of its own. The wallpaper
showed the theme's colour until the user named a picture, and the
desktop entry a login screen shows had no icon.

A brand was made outside this repository: a mark, a name set in a font,
colours, an icon at every size, wallpapers, a picture for the lock and
login screens and a splash, with a guide to them. Five ways were drawn
first:

- **Bloom**, petals over one another;
- **Prism**, a window with the spectrum across it;
- **Meander**, the Greek key in one line;
- **Aperture**, six blades;
- **Knot**, rings through one another.

Prism was chosen, as the window is what a desktop is made of, and light
broken into every colour says what the name says. Within it, a frame of
see-through glass and a pure white light ground were tried and left:
the glass made the mark weak on a busy wallpaper, and white made the
light scheme cold against the navy of the dark one. A regular grid of
dots behind the pictures was tried and left for a scattered one, and a
name with a capital for one in lower case.

It came as pictures already drawn, by a program that was not kept, and
with colours that differ from those of the design language.

## Decision

**The brand is [docs/BRAND.md](../BRAND.md)**, rewritten from the guide
it came with, for what omorfos uses: the mark, the name, the icon, the
colours, the wallpapers and the lock screen's picture, and colours for
foot. The splash is left out, as omorfos shows none, and so are colours
for terminals it does not recommend.

**The design language takes its grounds and accent from it.** The dark
scheme is navy where it was a grey, the light one cream where it was a
paler grey, and the accent cyan where it was blue, with blue among the
accents to choose from. The light scheme has a deeper cyan than the
brand's, which is 1.7 to 1 on cream, where an edge must be 3 to 1. The
rest of the design language stands: its corners, its sizes in points,
Inter for the interface and one accent. The brand's other colours, its
own corners and its sizes in pixels are not taken. The built-in theme
is redrawn in these colours with the rest of stage 4.

**Space Grotesk is for the name and nothing else.** The name is kept as
outlines, so the font is neither installed nor needed.

**The pictures are drawn as omorfos is built**, by `omorfos-brand`, a
program of the workspace, from the rules in the brand, and installed
with the rest. None is kept in the repository as a picture: the rules,
the marks as SVG and the program are. The program draws them the same
every time, dots and all, so that a picture can be drawn again at
another size, a new one added in a few lines, and what was installed
told apart from what was meant. It renders with `resvg`, which omorfos
already draws icons with, and needs nothing else.

**Pictures are under CC BY-SA 4.0** and the code that draws them under
MIT, as the rest of omorfos's code is.

## Consequences

- omorfos has an icon, installed as `omorfos` in `hicolor`, and the
  desktop entry names it.
- Building omorfos draws the pictures, which takes a few seconds and
  needs no tool beyond Rust.
- A wallpaper is shown when the user has named none, from
  `share/backgrounds/omorfos/`: the dark one or the light one, as the
  theme is dark or light, which is worked out as the portal has always
  worked it out and is now the theme's to say. Any picture the user
  names may be such a pair as well.
- The colours of [DESIGN.md](../DESIGN.md) changed; what was measured
  of them was measured again, and holds.
- The pictures drawn here are not the ones that came with the brand to
  the pixel, as the program that drew those was not kept. What they are
  to look like is the brand's rules, and those are kept.
