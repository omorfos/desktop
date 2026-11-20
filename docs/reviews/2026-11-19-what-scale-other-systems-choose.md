# What scale other systems choose for a screen

A survey of 19 November 2026, for the first step of the roadmap's "A
screen once plugged in remembered, and a scale that suits it chosen for
it": what the market chooses, found out before omorfos settles a rule
of its own. It settles nothing. It says what each system does and how
that was found out, what each would give the panels there are, where
the maintainer's laptop falls among them, and what that leaves to
decide.

## What was asked, and how it was answered

`output.scale: auto` divides how dense a screen is by 120 to the inch
and takes the nearest quarter, whatever kind of screen it is
(`Scale::for_screen`, in
[`crates/omorfos-ui/src/scale.rs`](../../crates/omorfos-ui/src/scale.rs)).
On the maintainer's laptop, 3840 by 2160 at some 280 to the inch, that
is 2.5, where 1.75 suits and 2 will do. The [roadmap](../ROADMAP.md)
asks what other systems give by default to laptops by size and
resolution, to monitors on a desk and to televisions, with that laptop
as one point.

Nine systems for computers and three for televisions were looked at.
Each thing said of them is marked by how it is known:

- **read**: the rule was read in the system's own code, at the commit
  linked under [Sources](#sources), from its mirror on GitHub;
- **documented**: its maker says so in its own documents;
- **reported**: users or the press say so of machines they have. This
  is the least sure, and is used only where nothing better was found.

Every rule read or documented was then written again as a small
program, and each of those held to what its own project holds the rule
to: the 13 screens of mutter's test, with fractions and without, to
the tolerance that test allows; niri's 17; every case of cosmic-comp's
`test_scale`; the 46 LCD panels of Chromium's list of known
Chromebook displays; Microsoft's worked example; and omorfos's own
tests. All agree. No test of the rule was found in KWin, sway or
Hyprland, so theirs are held by reading alone. The tables below are
what those programs answer, **worked out** and not seen: nothing was
run on the systems themselves, and no screen was looked at. The
program is [`tools/scale-survey.sh`](../../tools/scale-survey.sh).
Run with `check`, it holds this document to itself: every row of
every table, and each figure in the text that was worked out.

A panel in the tables is as wide and tall in millimetres as its
nominal diagonal makes it, to the nearest. A real one says a few
millimetres more or less, or whole centimetres, and that alone moves
some answers (finding 5).

## What a scale is aimed at

Every rule here is the same sum. A scale is how dense the screen is,
over how dense a screen its maker thinks needs none: how many units of
layout are to be left to the inch. The rules differ in that number,
and in whether it changes with the kind of screen.

Behind the number is how far the screen is read from. CSS defines its
pixel as an angle, that of 1/96 inch seen from 28 inches, an arm's
length: 0.26 mm at 71 cm, and 1.3 mm at 3.5 m. A unit read from nearer
is smaller on the glass and looks the same. By it, *d* units to the
inch stand for reading from 2688/*d* inches. Windows's rule is this
sum outright, with a distance for each kind of device.

| System | A laptop's own screen | A monitor | A television | Known |
|---|---|---|---|---|
| Windows 10 and 11 | 110, read from 24.5 in | 96, from 28 in | 32, from 84 in | documented |
| KDE Plasma | 125 | 96 | 30.5, on a screen over 500 mm tall | read |
| GNOME, and niri after it | 135, under 20 in along the diagonal | 110, from 20 in | 110, as a monitor | read |
| ChromeOS | steps that leave 112 to 144 | 96 | 40 from 50 in, behind a flag that is off | read |
| macOS | 127 to 133 | 109 on Apple's own; 2 on any 4K monitor | | reported |
| COSMIC | by 144, up to the next half | by 120, up to the next half; 2 from 2000 lines | 2 over 1200 lines, on a screen 19 in tall or more | read |
| sway | 2 over 192, on 1200 lines or more; else 1 | the same | the same | read |
| Hyprland | 1.5 over 140, 2 over 200 | the same | the same | read |
| omorfos today | 120 | 120 | 120 | read |

By the reference pixel, KDE's 125 is reading from 55 cm and its 96
from 71; GNOME's 135 from 51 cm and its 110 from 62; Apple's 127 from
54 cm; Windows's 110 from the 62 cm it names. omorfos's 120 is 57 cm
for every screen there is.

## The rules, one by one

### Windows 10 and 11

**Documented**, in Microsoft's guidance of 2021 to makers of screens
with rounded corners. Windows works out an *optimal zoom* from how dense
the screen is and how far it is read from, 96 to the inch and 28
inches being the zoom of one. The page gives the formula as a picture;
its worked example, 2560 by 1600 at 12.4 inches, 243.46 to the inch,
read from 24.5 inches, a zoom of 2.2, fits density/96 × distance/28,
which is the reference pixel's sum. The distances are 16.3 inches for
a small tablet, 20 for a tablet, 24.5 for a laptop, 28 for a desktop
and 84 for a television. How Windows tells which a screen is, the page
does not say.

The default is then the zoom brought to a step: 100% under 1.2, 125%
under 1.43, 150% under 1.78, 200% under 2.32, and 250% from there. It
never lands on 175% or 225%. And it is never so much as to leave fewer
than 720 lines: 100% under 900 lines, 125% under 1080, 150% under
1440, 200% under 1800, 250% from there. The page stops at 250%.

Held against Microsoft's table of what Windows 8.1 gave twelve common
displays, each taken for the kind its size suggests, the rule gives
the same for eleven:

| Screen | To the inch | Level | Left to the inch | The rule above |
|---|---:|---:|---:|---|
| 10.6 in, 1920 by 1080 | 208 | 150% | 139 | the same, from a zoom of 1.54 as a tablet |
| 10.6 in, 1366 by 768 | 148 | 100% | 148 | the same, from a zoom of 1.10 as a tablet |
| 11.6 in, 1920 by 1200 | 195 | 150% | 130 | the same, from a zoom of 1.78 as a laptop |
| 11.6 in, 1366 by 768 | 135 | 100% | 135 | the same, from a zoom of 1.23 as a laptop |
| 13.3 in, 1920 by 1200 | 170 | 150% | 113 | the same, from a zoom of 1.55 as a laptop |
| 13.3 in, 2560 by 1440 | 221 | 200% | 110 | the same, from a zoom of 2.01 as a laptop |
| 13.3 in, 1366 by 768 | 118 | 100% | 118 | the same, from a zoom of 1.07 as a laptop |
| 15.4 in, 1920 by 1080 | 143 | 125% | 114 | the same, from a zoom of 1.30 as a laptop |
| 15.6 in, 3200 by 1800 | 235 | 200% | 118 | the same, from a zoom of 2.15 as a laptop |
| 17 in, 1920 by 1080 | 130 | 125% | 104 | 100%, from a zoom of 1.18 as a laptop |
| 23 in, 3840 by 2160 | 192 | 200% | 96 | the same, from a zoom of 2.00 as a monitor |
| 24 in, 2560 by 1440 | 122 | 125% | 98 | the same, from a zoom of 1.28 as a monitor |

**Reported**, of Windows 10 and 11: 200% on the 13.5-inch 2256 by
1504 of a Framework 13, as the rule says; 250% on a 15.6-inch 4K
laptop, as it says; 300% on 13- and 14-inch 4K laptops, which is over
the page's most, though a panel of 2160 lines still has 720 left at
300%; and on televisions 300% at 4K and 150% at 1080 lines, whatever
their size, each of which leaves 1280 by 720.

Before Windows 8.1 it went by density alone, **documented**: 100%
under 116 to the inch, 125% from 116 to 138, 150% over that, and never
less than 1024 by 768 left.

### macOS

No rule is published. A Mac has a default "looks like" size for each
panel, and what is known of them is **reported**:

| Screen | Panel | To the inch | Looks like | Scale | Left to the inch |
|---|---|---:|---|---:|---:|
| MacBook Air, 13.6 in | 2560 by 1664 | 225 | 1470 by 956 | 1.74 | 129 |
| MacBook Air, 15.3 in | 2880 by 1864 | 224 | 1710 by 1107 | 1.68 | 133 |
| MacBook Pro, 13.3 in | 2560 by 1600 | 227 | 1440 by 900 | 1.78 | 128 |
| MacBook Pro, 14.2 in | 3024 by 1964 | 254 | 1512 by 982 | 2 | 127 |
| MacBook Pro, 16 in, of 2019 | 3072 by 1920 | 226 | 1792 by 1120 | 1.71 | 132 |
| MacBook Pro, 16.2 in | 3456 by 2234 | 254 | 1728 by 1117 | 2 | 127 |
| iMac and Studio Display, 27 in | 5120 by 2880 | 218 | 2560 by 1440 | 2 | 109 |
| a 4K monitor, 23.8 in | 3840 by 2160 | 185 | 1920 by 1080 | 2 | 93 |
| a 4K monitor, 27 in | 3840 by 2160 | 163 | 1920 by 1080 | 2 | 82 |
| a 4K monitor, 31.5 in | 3840 by 2160 | 140 | 1920 by 1080 | 2 | 70 |

Apple's own laptops leave 127 to 133 to the inch and its own desktops
109. Where the panel was not made for it, as the Air's, the default is
a fraction all the same. A 4K monitor of another maker gets 2 whatever
its size, which its owners find large at 27 inches and over.

### KDE Plasma

**Read**, in KWin's `chooseScale`. A screen on an eDP, LVDS or DSI
socket is the machine's own: 125 to the inch where the machine has a
lid switch, 130 without one where the screen is over 90 mm on its
shorter side (a tablet), 135 otherwise (a phone). Any other screen is
96, or 30.5 if it is over 500 mm tall, which is taken for a
television. The scale is the lesser of the two axes', never over 3,
and never so much that fewer than 800 units are left either way (720
on a tablet, 360 on a phone). It is rounded to a twentieth; under 1.2
it is 1; within 0.06 of a whole number it is that number. A screen
under 3 mm either way is taken to be lying and gets 1.

### GNOME

**Read**, in mutter 51's `calculate_scale`. 135 to the inch on a
screen under 20 inches along the diagonal, 110 from 20 inches: the
code says 135 was found by trying, as what best matches macOS,
Android, iOS and Windows and the scales GNOME's own users knew to be
good, and that larger screens are sat further from. The scale wanted
is then moved to the nearest that the panel divides by: a fraction
with a denominator of 4 or less, between 1 and 4, that leaves a whole
number of units each way and as many of them as 600 by 600. 3840 by
2160 divides by 1.5, 1.67, 2 and 2.5, and not by 1.75 or 2.25; 2256 by
1504 by 1.33 and 2 and nothing between.

Fractions are chosen by default in the code read, and mutter's `NEWS`
has them no longer an experiment as of GNOME 50. Where only whole
scales are allowed, as was the default before, the scale wanted is
lowered by an eighth and the nearest whole one taken. By the same
`NEWS` the default was reworked for GNOME 44, and in 49 it was made
to lean less towards the smaller scale and to pick its fractions
better. A screen that says nothing of its size gets 1, as does one
that says 1600 by 900, 1600 by 1000, 160 by 90, 160 by 100, 16 by 9 or
16 by 10 mm, which is a shape and not a size.

### niri

**Read**. It says it follows mutter's rule as it was in GNOME 46: 135
and 110, either side of 20 inches, to the nearest quarter between 1
and 4 that leaves 800 by 480 units, whether the panel divides by it or
not. A screen that says nothing gets 1.

### COSMIC

**Read**, in cosmic-comp's `calculate_scale`. Only 1, 1.5 and 2 are
chosen. A laptop's screen, told by its eDP socket, takes its density
over 144; any other takes it over 120; each is rounded to a hundredth
and then *up* to the next half. Then the resolution decides more than
the density does: a screen of 1200 lines or fewer is 1, one of 1600 or
fewer is at most 1.5, and one of 2000 or more is 2 whatever its size.
A screen 19 inches tall or more is a television: 1 up to 1200 lines, 2
over. One that says nothing is 1 up to 1600 lines and 2 over.

By reading: what the code calls the diagonal is the product of the
sides, not the root of their squares summed. "Under 20", which is to
tell a laptop, is then true only of a phone, and "from 40", which is
to tell a television, of nearly everything else: a laptop is in effect
told by its socket alone, and a television by its height alone. Its
own tests pass as it is written, and on the panels here it makes no
difference.

### sway

**Read**, in `compute_default_scale`. 2 where the screen has 1200
lines or more and is over 192 to the inch both ways; otherwise 1. A
screen that says nothing, or one of the six shapes mutter knows, gets
1.

### Hyprland

**Read**, in `getDefaultScale`. 2 over 200 to the inch along the
diagonal, 1.5 over 140, otherwise 1; then moved to the nearest scale,
in steps of 1/120, that leaves a whole number of units each way (a 16
inch 2560 by 1600 gets 1.6, not 1.5). By reading: a screen that says
no size has its pixels divided by nothing, comes out denser than
anything, and gets 2.

### ChromeOS

**Read**, in Chromium's display manager. A Chromebook's own screen
goes by steps of its density: 1.25 from 150 to the inch, 1.6 from 180,
1.78 from 220, 2 from 230, 2.4 from 270 and 2.67 from 310, with three
panels named and given a scale of their own. Chromium keeps the list
of the panels Chromebooks have had, and what each gets:

| Panel | To the inch | Scale | Looks like | Left to the inch |
|---|---:|---:|---|---:|
| 11.6 in, 1920 by 1080 | 190 | 1.6 | 1200 by 675 | 119 |
| 12.3 in, 2400 by 1600 | 235 | 2 | 1200 by 800 | 117 |
| 13.3 in, 1920 by 1080 | 166 | 1.25 | 1536 by 864 | 133 |
| 13.5 in, 2256 by 1504 | 201 | 1.6 | 1410 by 940 | 126 |
| 14 in, 1920 by 1080 | 157 | 1.25 | 1536 by 864 | 126 |
| 14 in, 2560 by 1600 | 216 | 1.6 | 1600 by 1000 | 135 |
| 15.6 in, 1920 by 1080 | 141 | 1 | 1920 by 1080 | 141 |
| 13.3 in, 3840 by 2160 | 331 | 2.67 | 1440 by 810 | 124 |
| 14 in, 3840 by 2160 | 315 | 2.67 | 1440 by 810 | 118 |
| 15.6 in, 3840 by 2160 | 282 | 2.4 | 1600 by 900 | 118 |

Any other screen is driven at a scale of 1, and the first time it is
plugged in it is zoomed towards 96 to the inch, to the nearest of the
nine zooms offered for its width. The code says 96 is the density most
used on external screens by ChromeOS's own count of what its users
choose. Behind a flag that is off, a screen of 50 inches or more would
go by 40 to the inch instead. That is what its makers are trying and
not what ChromeOS chooses: as it is, a television gets what any other
screen that is plugged in does, and the tables below have it so. A
screen that says it is 40 mm wide or less, or 30 mm tall or less, or
50 by 40, 160 by 90 or 160 by 100 mm, is not believed, and is not
scaled.

### The systems televisions run

**Documented**. None of the three asks how large the television is.
Android TV gives every television 960 by 540 units: a third more
pixels than units at 720 lines, twice at 1080, four times at 2160.
Fire TV is the same. Apple's guidelines lay tvOS out in points, 1920
across by the sum of their grids and margins, drawn at one pixel to
the point or at two. Their units are not a desktop's, as what they
draw is made large for a room; what they show is that a television is
scaled by its resolution alone, as a larger one is watched from
further.

## What they give the panels there are

**Worked out.** For a laptop, KDE is with a lid switch and ChromeOS is
a Chromebook's own screen; for the rest, ChromeOS is the zoom a screen
gets when first plugged in. GNOME is with fractions, as since
GNOME 50. Windows is by the documented rule, with its zoom beside it
to show how near a line each falls; † is where the zoom is as high as
where 300% is reported, and the page stops at 250%. macOS has no
column, as it has no rule: its table is above.

### Laptops

| Panel | To the inch | omorfos | KDE | GNOME | niri | COSMIC | sway | Hyprland | ChromeOS | Windows | Windows's zoom |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 13.3 in, 1920 by 1080 | 166 | 1.5 | 1.3 | 1.25 | 1.25 | 1 | 1 | 1.5 | 1.25 | 1.5 | 1.51 |
| 14 in, 1920 by 1080 | 157 | 1.25 | 1.25 | 1.25 | 1.25 | 1 | 1 | 1.5 | 1.25 | 1.5 | 1.43 |
| 14 in, 1920 by 1200 | 162 | 1.25 | 1.3 | 1.25 | 1.25 | 1 | 1 | 1.5 | 1.25 | 1.5 | 1.47 |
| 15.6 in, 1920 by 1080 | 141 | 1.25 | 1 | 1 | 1 | 1 | 1 | 1.5 | 1 | 1.25 | 1.29 |
| 16 in, 1920 by 1200 | 141 | 1.25 | 1 | 1 | 1 | 1 | 1 | 1.5 | 1 | 1.25 | 1.29 |
| 17.3 in, 1920 by 1080 | 127 | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 1.16 |
| 13.5 in, 2256 by 1504 | 201 | 1.75 | 1.6 | 1.33 | 1.5 | 1.5 | 2 | 2 | 1.6 | 2 | 1.83 |
| 13.3 in, 2560 by 1600 | 227 | 2 | 1.8 | 1.67 | 1.75 | 1.5 | 2 | 2 | 1.78 | 2 | 2.07 |
| 14 in, 2560 by 1600 | 216 | 1.75 | 1.7 | 1.67 | 1.5 | 1.5 | 2 | 2 | 1.6 | 2 | 1.96 |
| 16 in, 2560 by 1600 | 189 | 1.5 | 1.5 | 1.33 | 1.5 | 1.5 | 1 | 1.6 | 1.6 | 1.5 | 1.72 |
| 14 in, 2880 by 1800 | 242 | 2 | 2 | 1.67 | 1.75 | 2 | 2 | 2 | 2 | 2 | 2.21 |
| 16 in, 3200 by 2000 | 236 | 2 | 1.9 | 1.67 | 1.75 | 2 | 2 | 2 | 2 | 2 | 2.15 |
| 13.4 in, 3840 by 2400 | 338 | 2.75 | 2.7 | 2.5 | 2.5 | 2 | 2 | 2 | 2.67 | 2.5† | 3.08 |
| 14 in, 3840 by 2400 | 323 | 2.75 | 2.6 | 2.5 | 2.5 | 2 | 2 | 2 | 2.67 | 2.5† | 2.95 |
| 15.6 in, 3840 by 2160 | 283 | 2.25 | 2.25 | 2 | 2 | 2 | 2 | 2 | 2.4 | 2.5 | 2.58 |
| the same, saying 340 by 190 mm | 287 | 2.5 | 2.3 | 2 | 2.25 | 2 | 2 | 2 | 2.4 | 2.5 | 2.62 |
| 16 in, 3840 by 2400 | 283 | 2.25 | 2.25 | 2 | 2 | 2 | 2 | 2 | 2.4 | 2.5 | 2.58 |
| 17.3 in, 3840 by 2160 | 255 | 2 | 2 | 2 | 2 | 2 | 2 | 2 | 2 | 2.5 | 2.32 |

### Monitors on a desk

| Panel | To the inch | omorfos | KDE | GNOME | niri | COSMIC | sway | Hyprland | ChromeOS | Windows | Windows's zoom |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 21.5 in, 1920 by 1080 | 102 | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 1.1 | 1 | 1.07 |
| 23.8 in, 1920 by 1080 | 93 | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 0.96 |
| 27 in, 1920 by 1080 | 82 | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 0.85 |
| 23.8 in, 2560 by 1440 | 123 | 1 | 1.3 | 1 | 1 | 1.5 | 1 | 1 | 1.3 | 1.25 | 1.29 |
| 27 in, 2560 by 1440 | 109 | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 1.15 | 1 | 1.13 |
| 31.5 in, 2560 by 1440 | 93 | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 0.97 |
| 34 in, 3440 by 1440 | 110 | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 1.15 | 1 | 1.14 |
| 49 in, 5120 by 1440 | 109 | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 1.25 | 1 | 1.13 |
| 23.8 in, 3840 by 2160 | 185 | 1.5 | 2 | 1.67 | 1.75 | 2 | 1 | 1.5 | 2 | 2 | 1.93 |
| 27 in, 3840 by 2160 | 163 | 1.25 | 1.7 | 1.5 | 1.5 | 2 | 1 | 1.5 | 1.6 | 1.5 | 1.70 |
| 31.5 in, 3840 by 2160 | 140 | 1.25 | 1.45 | 1.25 | 1.25 | 2 | 1 | 1 | 1.4 | 1.5 | 1.46 |
| 42.5 in, 3840 by 2160 | 104 | 1 | 2.7 | 1 | 1 | 2 | 1 | 1 | 1.1 | 1 | 1.08 |
| 27 in, 5120 by 2880 | 218 | 1.75 | 2.25 | 2 | 2 | 2 | 2 | 2 | 2.25 | 2 | 2.27 |
| 31.5 in, 6016 by 3384 | 219 | 1.75 | 2.3 | 2 | 2 | 2 | 2 | 2 | 2.25 | 2 | 2.28 |

### Televisions, and screens that say nothing true

Windows is as a television here, read from 84 inches; it is reported
at 300% on a 4K television of any size.

| Panel | To the inch | omorfos | KDE | GNOME | niri | COSMIC | sway | Hyprland | ChromeOS | Windows | Windows's zoom |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 32 in, 1920 by 1080 | 69 | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 1.5 | 2.15 |
| 43 in, 1920 by 1080 | 51 | 1 | 1.35 | 1 | 1 | 1 | 1 | 1 | 1 | 1.5 | 1.60 |
| 43 in, 3840 by 2160 | 102 | 1 | 2.7 | 1 | 1 | 2 | 1 | 1 | 1.1 | 2.5† | 3.20 |
| 55 in, 3840 by 2160 | 80 | 1 | 2.65 | 1 | 1 | 2 | 1 | 1 | 1 | 2.5 | 2.50 |
| 65 in, 3840 by 2160 | 68 | 1 | 2.2 | 1 | 1 | 2 | 1 | 1 | 1 | 2 | 2.12 |
| 77 in, 3840 by 2160 | 57 | 1 | 1.9 | 1 | 1 | 2 | 1 | 1 | 1 | 2 | 1.79 |
| a television saying 1600 by 900 mm, 3840 by 2160 | 61 | 1 | 2 | 1 | 1 | 2 | 1 | 1 | 1 | 2 | 1.91 |
| a screen saying no size, 3840 by 2160 |  | 1 | 1 | 1 | 1 | 2 | 1 | 2 | 1 |  |  |
| a screen saying no size, 1920 by 1080 |  | 1 | 1 | 1 | 1 | 1 | 1 | 2 | 1 |  |  |

Behind its flag, ChromeOS would give the 55-inch television 2, the
65-inch 1.78 and the 77-inch 1.33.

## The laptop the roadmap names

3840 by 2160 at some 280 to the inch is, by its density, the 15.6-inch
4K panel: 345 by 194 mm, and 283 to the inch. By that size today's
rule gives 2.25. That it gives 2.5 on the maintainer's says the panel
gives its width as 342 mm or less, as one that speaks in whole
centimetres says 340: this has not been read from the machine.

| Who | Scale | Left to the inch | Units across | As if read from |
|---|---:|---:|---:|---|
| what suits the maintainer | 1.75 | 162 | 2194 | 42 cm |
| what will do | 2 | 141 | 1920 | 48 cm |
| GNOME, COSMIC, sway, Hyprland | 2 | 141 | 1920 | 48 cm |
| niri (2.25 if the panel says 340 mm) | 2 | 141 | 1920 | 48 cm |
| KDE (2.3 if it says 340 mm) | 2.25 | 126 | 1707 | 54 cm |
| omorfos today (2.5 if it says 340 mm) | 2.25 | 126 | 1707 | 54 cm |
| ChromeOS, which had this panel | 2.4 | 118 | 1600 | 58 cm |
| Windows | 2.5 | 113 | 1536 | 60 cm |

Apple makes no such panel. Its nearest, 254 to the inch at 16.2
inches, is at 2.

The market gives this laptop 2 to 2.5. What will do, 2, is the low end
of that, and five of the eight that have a rule choose it. What suits,
1.75, is under every default found: it leaves 162 to the inch, where
Android aims a phone at 160, and of the systems that aim at a density
none aims a laptop above 135.

## What it found

### 1. Every system that aims at a density aims a laptop denser than a monitor, and omorfos does not

**Read and documented.** Windows by 14% (110 and 96), GNOME by 23% (135
and 110), KDE by 30% (125 and 96); Apple, **reported**, by some 17 to
22% (127 to 133, and 109). omorfos's 120 for both is between each
pair but Windows's, which it is above.

**Worked out.** So omorfos is large on a laptop and small on a desk at
once. Of the 18 rows of laptops above it chooses more than GNOME on 14
and never less, and more than KDE on 10 and less on one. Of the 14
monitors it never chooses more than KDE, GNOME or Windows, and less
than them on 7, 4 and 6: a 27-inch 4K monitor gets 1.25 where they
give 1.7, 1.5 and 1.5.

### 2. On the named laptop the market says 2 to 2.5, and 1.75 is nobody's default

**Worked out**, in the table above. In quarters, a density between 133
and 151 gives it 2, and one between 151 and 174 gives 1.75. GNOME's
135 is the one density a system uses that gives 2 here. At 150, three
common laptops come out smaller than any system gives them: the
14-inch 2880 by 1800 at 1.5, and the 13.5-inch 2256 by 1504 and the
16-inch 2560 by 1600 at 1.25, but for sway, which gives the last 1.

### 3. Whole scales are preferred, each in its own way

**Read.** KWin takes the whole number within 0.06 of it. COSMIC knows
only whole and half, and gives every panel of 2000 lines or more 2.
sway knows only whole. GNOME and Hyprland choose only what the panel
divides by. **Documented**: Windows's default has no 175% or 225%.
**Reported**: Apple makes its better panels for 2.

1.75 and 2.25 do not divide 3840 by 2160, which they leave 2194.29 and
1706.67 units across, nor does 1.75 divide 2880 by 1800. omorfos and
niri choose such scales; KWin does too, in twentieths.

### 4. Under 1.2 a monitor is left alone, or nearly

**Read and documented.** KWin gives 1 under 1.2, and Windows's first
line is at a zoom of 1.2. ChromeOS alone goes finer, and zooms such a
monitor by a tenth or so. **Worked out**: a rule in quarters aimed at
96 with no such floor gives the 27-inch 2560 by 1440 and the 34-inch
3440 by 1440 1.25, which is more than any system gives them. Aimed at
110, as GNOME is, they are 1 without a floor.

### 5. A rule with a line near a common panel gives that panel two answers

**Worked out.** The 15.6-inch 4K panel is 2.25 by omorfos if it says
343 mm or more and 2.5 if it says less; by niri it is 2 if it says
341 mm or more and 2.25 if less. On ChromeOS a 27-inch 4K monitor
falls midway between zooms of 1.6 and 1.8. On Windows the 14-inch 1920
by 1080 sits at a zoom of 1.43 and the 17.3-inch 4K at 2.32, each on a
line. What a panel says is rounded, to the millimetre or the
centimetre, and is not always true.

### 6. A television is told by its size, and is given what a room needs

**Read and documented.** KWin takes a screen over 500 mm tall for one,
COSMIC one 19 inches (483 mm) tall or more, ChromeOS, behind its flag,
one of 50 inches along the diagonal; Windows does not say how. None
goes by which socket it is on, HDMI or another. What a 4K television
is left with is 1280 by 720 on Windows as **reported**, some 1450 by
815 on KDE at 55 inches, and 1920 by 1080 on COSMIC. GNOME, niri,
sway, Hyprland, ChromeOS and omorfos give a television 1 or little
more, as a monitor: 3840 by 2160 units, to be read from a sofa.

**Worked out.** KWin's line takes in the 42.5-inch 4K monitor, which
is 529 mm tall and gets 2.7 on a desk. Its 30.5 to the inch is also
the density at which a 4K screen that says it is 1600 by 900 mm, the
shape and not the size, comes out at 2.

### 7. A screen that says nothing gets 1 nearly everywhere

**Read.** KWin, GNOME, niri, sway and ChromeOS give it 1. COSMIC goes
by resolution: 1 up to 1600 lines, 2 over. Hyprland gives 2, it seems
by accident. What is not believed differs: six shapes by mutter and
sway; two of them, 50 by 40 mm and anything smaller than a phone by
ChromeOS; anything under 3 mm by KWin; and by niri, COSMIC, Hyprland
and omorfos only a size of nothing.

### 8. Each keeps something back for the screen to be usable

**Read and documented.** Windows leaves at least 720 lines. KWin
leaves at least 800 units each way and never goes over 3. mutter
leaves 600 by 600, niri 800 by 480. omorfos has 1 and 4 as its only
bounds.

### 9. A laptop is told from a tablet by its lid, or not at all

**Read.** KWin asks whether the machine has a lid switch. mutter goes
by size alone, under 20 inches or not. COSMIC goes by the eDP socket.
**Documented**: Windows has both kinds, and reads a tablet from
nearer. omorfos already knows a screen on an eDP, LVDS or DSI socket
as the machine's own.

## What it leaves to decide

For the roadmap's "A better guess", in the order the findings give:

- **Whether each kind of screen has a density of its own, and which.**
  For a laptop, 110 (Windows), 125 (KDE), 127 to 133 (Apple) or 135
  (GNOME) have a system behind them, and give the named laptop 2.5,
  2.25, 2.25 and 2. Anything that gives it 1.75 has none. For a
  monitor, 96 (Windows, KDE, ChromeOS) or 110 (GNOME, Apple's own).
- **Whether 1.75 is the rule's to give, or the maintainer's to set.**
  A later step keeps what is set for a screen. A rule that gives 2
  there, and a setting of 1.75 kept for that panel, is within what
  the market does. A rule that gives 1.75 there, as one aimed at 160
  does, is under KDE, GNOME, ChromeOS and Windows at once on 15 of
  the 18 rows of laptops; the other three are the panels 1920 across
  of 15.6 inches and more, which they leave at 1 as well.
- **How near a whole scale is near.** In quarters the named laptop
  rounds to 2 from 135 by itself. Whether 1.75 and 2.25 are ever
  chosen, as on the 14-inch 2880 by 1800 that all but GNOME and niri
  give 2, or only what the panel divides by.
- **Where a monitor stops being scaled**: a floor as KWin's and
  Windows's 1.2, or a density of 110 that needs none.
- **What tells a television, and what it is left with**: a height, as
  KWin and COSMIC, which catches the largest monitors; and something
  between 1280 by 720 and 1920 by 1080 units at 4K.
- **What a screen that says nothing gets**: 1, as most, or 2 from some
  resolution up, as COSMIC, which is what makes a 4K television that
  says nothing usable.
- **What is kept back**: the fewest units a guess may leave.
- **What the rule is tested by.** The rows of these tables, each at
  its nominal size and at the size in whole centimetres, would hold it
  to the market and show where it has a line on a panel.

## What was not looked at, and what is not sure

- Nothing was run. A port that agrees with its project's tests can
  still be wrong where they do not look, and KWin's, sway's and
  Hyprland's had none to agree with.
- Windows over 250%, and Windows on a television, are from users'
  accounts, as are all of Apple's defaults. Microsoft's formulas are
  pictures and were not seen: the sum is taken from the worked
  example. Its table for Windows 8.1 was read as a search engine gave
  it, as the page would not open from here; so was the text of the CSS
  specification, from where others quote it.
- What the maintainer's laptop says its size is has not been read
  from it.
- How Windows tells a laptop from a tablet, a desktop or a
  television; and whether anything in what a screen says of itself
  tells a television, beyond its size: no EDID was read.
- Samsung's and LG's televisions; phones and tablets, but in passing;
  desktops that draw with X11; Weston, labwc and others that were not
  opened.
- Projectors, which have no size to say.
- What suits weak eyes is a setting and not a default, and is not
  here.

## Sources

Code, each at the commit its link names, which is the one its mirror
on GitHub had on 19 November 2026, or for sway on 21 September:

- KWin:
  [`chooseScale`](https://github.com/KDE/kwin/blob/c24a5f4e85683de94108fc3ca6ff48ea726f5037/src/outputconfigurationstore.cpp#L886)
  in `src/outputconfigurationstore.cpp`; `isInternal` in
  `src/backends/drm/drm_connector.cpp`.
- mutter 51.1:
  [`calculate_scale`](https://github.com/GNOME/mutter/blob/9bc8cd8427a5aa9bf8976acaef4426ce09a08537/src/backends/meta-monitor.c#L2455)
  and `for_each_scale` in `src/backends/meta-monitor.c`;
  `meta_monitor_has_aspect_as_size` in
  `src/backends/meta-monitor-manager.c`; the cases in
  `src/tests/monitor-scale-tests.c`; `NEWS`, for 44, 49 and 50.
- niri:
  [`src/utils/scale.rs`](https://github.com/YaLTeR/niri/blob/a855fd519f7341d378577da6a7b2763baeb70f79/src/utils/scale.rs).
- cosmic-comp:
  [`calculate_scale`](https://github.com/pop-os/cosmic-comp/blob/abb6ebc49ba58daa14163872954cdef3f830a04a/src/backend/kms/device.rs#L1319)
  and its tests in `src/backend/kms/device.rs`.
- sway:
  [`compute_default_scale`](https://github.com/swaywm/sway/blob/1652c54b73f67df17b7b4ab0b0f7048204aa8104/sway/config/output.c#L408)
  in `sway/config/output.c`.
- Hyprland 0.57.0:
  [`getDefaultScale`](https://github.com/hyprwm/Hyprland/blob/49b566a9780413a2498b7e8b925337ddb41a41a7/src/output/Monitor.cpp#L1296)
  in `src/output/Monitor.cpp`.
- Chromium:
  [`display_change_observer.cc`](https://github.com/chromium/chromium/blob/25ba7ca183aafe2247743fbc2c0f4102994d6d01/ui/display/manager/display_change_observer.cc)
  and `managed_display_info.cc` in `ui/display/manager/`; the steps of
  zoom and the list of known panels in
  [`ui/display/types/display_constants.h`](https://github.com/chromium/chromium/blob/25ba7ca183aafe2247743fbc2c0f4102994d6d01/ui/display/types/display_constants.h);
  `IsDisplaySizeValid` in `ui/display/util/display_util.cc`.

Documents:

- Microsoft,
  [Guidance for rounded display bezels](https://learn.microsoft.com/en-us/windows-hardware/design/component-guidelines/guidance-for-rounded-display-bezels),
  "Calculating the Windows Default Scale Factor and Effective
  Resolution", last updated 2 December 2021.
- Microsoft,
  [DPI-related APIs and registry settings](https://technet.microsoft.com/library/dn528846%28v=win.10%29),
  for Windows 8.1: "Table 1 Windows 8.1 Scaling Levels".
- Microsoft,
  [DPI](https://learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-8.1-and-8/ff716252%28v=win.10%29),
  "Windows Automatic DPI Configuration", for Windows 7 and 8.
- Microsoft,
  [High DPI and Windows](https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/high-dpi-and-windows?view=windows-11):
  the distance a screen is seen from is in the default since 8.1.
- W3C, CSS Values and Units,
  [the reference pixel](https://www.w3.org/TR/css3-values/#reference-pixel),
  read as quoted on
  [www-style](https://lists.w3.org/Archives/Public/www-style/2010Jun/0693.html)
  and in
  [the text of CSS 2.1](https://fantasai.inkedblade.net/style/specs/css2.1/px-unit);
  the sum of 2688 is
  [Simon Sapin's](https://lists.w3.org/Archives/Public/www-style/2013Jan/0538.html).
- Android,
  [Build TV layouts](https://developer.android.com/training/tv/playback/compose/layouts)
  and
  [Layouts](https://developer.android.com/design/ui/tv/guides/styles/layouts)
  in its guide to designing for television; Amazon,
  [Display and Layout (Fire TV)](https://developer.amazon.com/docs/fire-tv/display-and-layout.html).
- Apple, Human Interface Guidelines,
  [Layout](https://developer.apple.com/design/human-interface-guidelines/layout.md)
  and
  [Images](https://developer.apple.com/design/human-interface-guidelines/images.md).
- Mozilla Hacks,
  [CSS Length Explained](https://hacks.mozilla.org/2013/09/css-length-explained/),
  for the reference pixel at the 160 to the inch of a phone.

Reports:

- Of Macs: 9to5Mac,
  [the MacBook Pro's resolutions](https://9to5mac.com/2021/10/19/new-macbook-pro-screen-resolution-options/),
  for the 14.2- and 16.2-inch at 2 and the 16-inch of 2019; MacRumors'
  forums for the
  [13.6-inch Air](https://forums.macrumors.com/threads/default-looks-like-screen-resolution-for-m2-air.2360072),
  the
  [15.3-inch Air](https://forums.macrumors.com/threads/macbook-air-15-display-resolution.2468969),
  the
  [14.2-inch Pro](https://forums.macrumors.com/threads/macbook-pro-m1-14-resolution-and-scaling.2352122)
  and
  [a 4K monitor](https://forums.macrumors.com/threads/4k-monitor-defaults-to-1080p.2172456);
  How-To Geek for the
  [13.3-inch Pro](https://www.howtogeek.com/305776/how-to-make-text-and-other-items-bigger-or-smaller-on-your-macs-retina-display/);
  AppleInsider's forum for
  [the 27-inch at 5120 by 2880](https://forums.appleinsider.com/discussion/comment/3399430/).
- Of Windows: Framework's forum for
  [the 13.5-inch at 200%](https://community.frame.work/t/question-about-screen-resolution/8783);
  HP's for
  [13-inch 4K at 300%](https://h30434.www3.hp.com/t5/Notebook-Video-Display-and-Touch/Default-scale-Spectre-x360-13-4K-screen/td-p/7743706);
  Thurrott for
  [14-inch 4K at 300%](https://www.thurrott.com/?p=273946); Tom's
  Guide's forum for
  [a 15.6-inch 4K at 250%](https://forums.tomsguide.com/threads/dell-xps-9560-font-scaling.428493/post-1830949);
  Eleven Forum for
  [televisions](https://www.elevenforum.com/t/difficulty-with-screen-dimensions.6431/latest).
