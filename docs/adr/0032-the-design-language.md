# 32. The design language

- Status: accepted
- Date: 2026-10-13

## Context

omorfos is to be a whole desktop that looks modern, "in the frosted-glass
manner", and stays simple, clean, accessible and above all secure. A bar,
a dock, a switcher and applications are to be written, by more than one
hand and over a long time. Without rules written down first, each would
choose its own sizes and colours and the whole would look assembled.

The built-in theme so far was chosen a part at a time: a dark palette of
seven colours, `sans-serif` at 11 points, corners of 8 and 6, one
transition of 120 ms.

Frosted glass has a known cost. Text over a blurred picture is as
readable as the picture lets it be, and the first releases of such
designs elsewhere have had to be given more tint afterwards. And the
priorities here put security and correctness above looks.

## Decision

The language is in [docs/DESIGN.md](../DESIGN.md). What was decided, and
against what:

**Glass, with a floor.** Bars, menus and titlebars are tinted, blurred
glass; the content of a window never is. Each kind of surface has a tint
below which the theme does not go, worked out from the contrast its text
must have over the worst thing that can be behind it: 70% where only
full-strength text is written, 90% where dim text is too. The first
figures written down for this, 55% and 82%, were chosen by eye and failed
when measured, which is the argument for measuring. Glass that is not
blurred is not allowed at all. This gives up the most striking pictures,
the nearly clear ones, to keep text readable without a second thought.

**Contrast is a rule, not an aim.** 4.5 to 1 for text and 3 to 1 for the
edges of controls, as WCAG 2 measures them, for every pair of colours in
the palette. The pairs chosen were measured.

**Inter for the interface.** The wish named Inter, Fira and Noto. Inter
was drawn for interfaces at small sizes on screens, has tabular figures
and a tall x-height, is one variable file, is under the Open Font
Licence, and is packaged by every distribution aimed at. Fira Sans has
not had a release in years. Noto's virtue is covering every script, which
fontconfig gives anyway by falling back to it. JetBrains Mono goes with
it for the same reasons. Neither is required: packages recommend them,
and `sans-serif` and `monospace` are what the theme falls back to, so the
tests, which have DejaVu, do not change.

**Dark by default, light as its equal, and the sun to choose if asked.**
Both palettes are given in full and a part is not done until it is right
in both. Dark is the default because the glass is at its best over a dark
tint and because it is what omorfos has been. `auto` is light from
sunrise to sunset, worked out from a latitude and longitude the user
gives. Asking a service where the machine is was rejected: it tells a
third party where the user is and needs the network, for the sake of a
colour. Fixed hours are what there is without a location.

**One accent, which the user chooses.** It means focus and what is picked
out, and nothing else. Red, amber and green keep their meanings on the
titlebar's buttons and for danger, warning and success.

**The window with focus is shown by its titlebar and its shadow, not by
a ring of accent**, once there are shadows. Until then the accent edge
stays.

**Signs from Lucide; pictures from the icon theme.** Drawing a set of
our own is months of work that is not ours to be good at. Lucide is
drawn to one grid and one weight, is under the ISC licence, and only the
signs used are built in, as paths, so there is no dependency at run time
and nothing to load. Adwaita's symbolic icons were the other candidate:
they are heavier in weight and bound to GNOME's look.

**Credit is part of the design.** Whatever is someone else's is named
with its authors and licence in `CREDITS.md` before it is built in, its
licence text is in every package, and the list is shown to the user.

**A pointer of our own, later.** The cursor theme is the user's and
packages recommend Adwaita's. One of our own would not hurt: it is
drawing and no risk, and built in as shapes it would be sharp at every
scale and there when no theme is installed, which is worth having. It is
not worth having before the bar and the dock, so it is put with the
look.

**Movement is short and can be turned off.** Nothing over 300 ms; leaving
quicker than arriving; nothing holds input up.

**Reduced motion, reduced transparency and high contrast are settings
from the first day** of the look being built, and each part says what it
does under each.

**What asks for a secret is drawn above everything and looks like nothing
a program can draw.** This is the one place where the look is a security
property: the rest of the screen is dimmed and blurred, which a program
cannot do.

## Consequences

- Stage 4 has a specification to build: the theme is redrawn to these
  tables, and gains `--surface-raised`, `--border-strong`, `--on-accent`
  and the three colours of meaning.
- The theme language has to grow to say it: `backdrop-filter`,
  `box-shadow`, opacity in a colour of a surface, a transition's
  properties one by one, and a way to follow the scheme (two sets of
  variables). Those are stage 4's.
- The settings under `appearance` are new configuration, and what they
  mean is fixed here before they exist.
- Until blur exists the built-in theme is solid, which is the language's
  own fallback and not a stopgap.
- Packages gain two recommended fonts and a recommended cursor theme.
- `CREDITS.md` is kept, and packaging has to put the licence texts it
  names where the distribution expects them.
- `auto` needs the position of the sun worked out, which is arithmetic
  and no dependency, and a clock to wake by.
- A user's theme is not bound by any of this. The floor on tint binds the
  built-in theme and the settings, not `theme.css`.
