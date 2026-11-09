# 66. A pointer of our own, drawn here and installed as a theme

- Status: accepted
- Date: 2026-11-09
- Builds on: [65. A scheme by the time of day](0065-a-scheme-by-the-time-of-day.md)

## Context

The pointer came from a cursor theme, `default` unless another was
named: whatever the system had set up, or, where no theme had a shape,
an arrow painted by the compositor. Themes are pictures at a few fixed
sizes, so at a size between them, or a scale that is not whole, the
pointer was blurred or the wrong size, and a system with no theme had a
plain arrow for every shape. DESIGN.md had a pointer of omorfos's own
drawn as shapes, in the brand's colours, waiting for stage 4.

Programs show the pointer in two ways. Those that name a shape, through
the cursor shape protocol, as GTK 4, Qt 6, Firefox and Chromium do, have
it drawn by the compositor. Those written for X11, and older ones written
for Wayland, read a cursor theme from disk themselves, by the name they
are told in `XCURSOR_THEME`, in the X server's resources, in the settings
toolkits follow, or through the settings portal.

## Decision

**`cursor.theme: omorfos`, the default, is omorfos's own pointer.** Its
36 shapes are written as SVG on a grid of 24 in `omorfos-brand`, which
already drew the brand's pictures with resvg, and the compositor draws
the shape it needs at the size it needs, so that it is sharp at any size
and any scale. It is navy rimmed with cream in the dark scheme, navy-3
and not the ground's navy, which at its size is black, and cream rimmed
with navy in the light one, and is drawn again when the scheme
changes. A theme that is named is read from disk as before, and a shape
it has no picture for, nor its ordinary arrow, is drawn as omorfos's own,
which replaces the painted arrow.

**The same shapes are installed as two cursor themes**, `omorfos-dark`
and `omorfos-light`, drawn by `omorfos-brand` as omorfos is built, at
the sizes Adwaita's are, 24 to 96, with the older names X11 programs ask
for as links. Programs that read a theme are told the one for the
scheme, and told again as it changes, as they are of any other change.
So the pointer looks the same over every program, if at a size between
those on disk a little less sharp over those that read it.

## Alternatives

- **Drawn by the compositor only.** Less to build and to install, but
  the pointer would change look between windows, and over every X11
  program would be whatever theme the system had.
- **Only as an installed theme.** One path for every program, but at
  sizes between those drawn, and for scales such as 1.25, the
  compositor's pointer would be no sharper than any theme's, which is
  what the pointer of our own was for.
- **One theme in one scheme.** Simpler, but a navy pointer on the light
  scheme's cream, or a cream one on navy, is the pointer that does not
  belong; and the brand has both.
- **The theme written at login at the exact size.** It would be sharp
  over every program, but it means writing files into the user's home as
  the session starts, and pointing every program's search path at them;
  a sandboxed program would not see them anyway.

## Consequences

- The packages install about 5 MB more, before compression, most of it
  flat colour, and no longer recommend the Adwaita cursors, which are
  wanted only to be named.
- D-Bus and systemd, which start programs too, are told the session's
  environment again whenever it changes, and when the compositor is
  started anew under the holder, which they were not before: a program
  they started still had the theme of the login.
- A program that read the theme as it started keeps it: with `scheme:
  auto`, its pointer may stay in the morning's scheme after sunset, as
  the lock screen's colours do (ADR 65), until it is started again.
- Shapes are as cheap to change as an SVG string, and are tried on real
  hardware, not by the tests, which only hold each to being drawn, with
  its hot spot on it, and in the colours of its scheme.
- The ring of waiting does not turn yet: the compositor draws one picture
  of a shape, and the themes on disk hold one each.
