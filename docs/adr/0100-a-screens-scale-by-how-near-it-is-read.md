# 100. A screen's scale, by how near it is read

- Status: accepted
- Date: 2026-11-19

## Context

`output.scale: auto` takes how dense a screen is across, over 120 to
the inch, to the nearest quarter from 1 to 4, whatever the screen is
(`Scale::for_screen`). On the maintainer's laptop, 3840 by 2160 at some
280 to the inch, that is 2.5, where 1.75 suits and 2 will do. A dense
monitor on a desk comes out smaller than KDE, GNOME and Windows make
it: a 27-inch 3840 by 2160 at 1.25, where they give 1.7, 1.5 and 1.5.
And a screen of 3840 by 2160 that gives its shape where its size
should be, as 160 by 90 mm, is taken at its word and given 4.

The roadmap asked what the market chooses before a rule was settled,
and
[the survey](../reviews/2026-11-19-what-scale-other-systems-choose.md)
found it out for nine systems, from their code and documents. What
bears on the rule:

- Every system that aims at a density aims a screen read from near
  denser than one on a desk: Windows 110 and 96 to the inch, KDE 125
  and 96, GNOME 135 and 110. omorfos's 120 for both is large on a
  laptop and small on a desk at once.
- The market gives the maintainer's laptop 2 to 2.5. 1.75 is nobody's
  default.
- Whole scales are preferred, by each system in its own way.
- A rule with a line near a common panel gives that panel two
  answers, as what a panel says of its size is rounded, and is not
  always true.
- A television is told by its size, where it is told at all, and what
  televisions run themselves goes by resolution alone.
- A screen that says nothing of its size gets 1 nearly everywhere,
  which at 2160 lines cannot be read from where such a screen is
  watched.

The maintainer asked for a recommendation on 19 November 2026 and
took it: GNOME's two densities with omorfos's quarters, the whole
scale where one is near, and a television or a screen that says
nothing true by its resolution. Writing it down settled the rest,
each part from what the survey found.

## Decision

**A scale is guessed from how near a screen is read, and for a screen
that cannot say, from its resolution.** In this order:

1. **A screen that says nothing true goes by its resolution alone.**
   That is one that gives no size, or gives one of the six shapes
   that get written where a size should be: 16 by 9, 16 by 10, 160 by
   90, 160 by 100, 1600 by 900 and 1600 by 1000 mm, either way up.
   Its scale is the whole number of times 1080 goes into its lines,
   which are the fewer of its pixels across and down: 1 under 2160
   lines, 2 from there, 4 at 4320.
2. **So does a television**, which is any screen that says it is over
   560 mm on its shorter side: 45 inches and more, at 16 by 9. A
   larger television is watched from further, so its size says little
   of how large things should be on it; and a television's size is
   the one least to be believed.
3. **Any other screen goes by how dense it is and how near it is
   read.** How dense is its pixels along the diagonal, over the
   inches it says it is along the diagonal. A screen under 20 inches
   along the diagonal is read from near, as a laptop's own is, and is
   aimed at 135 to the inch; one of 20 inches or more stands on a
   desk, and is aimed at 110. The sum is the one over the other.
4. **The sum is rounded to a quarter up to 2 and to a half beyond
   it, and to 1 or 2 where it is within 0.15 of them, or up to 0.35
   over 2.** So 1 is anything up to 1.15, 2 is anything from 1.85 to
   2.35, and nothing is under 1 or over 4. The lines between one
   scale and the next are at 1.15, 1.375, 1.625, 1.85, 2.35, 2.75,
   3.25 and 3.75. A sum on a line takes the larger scale, but at 1.15
   and 2.35, where it is 1 and 2.
5. **Something is kept back.** No guess leaves fewer than 720 units
   on a screen's shorter side: one that would is brought down a step
   at a time until it does not.

What a screen is taken for is told by the size it says it is and by
nothing else: not its socket, its name or its maker. Each of these is
asked in turn, and the first that is so is taken:

| It says it is | It is taken for | Its scale is from |
|---|---|---|
| nothing, or one of the six shapes | saying nothing true | its lines, over 1080 |
| over 560 mm on its shorter side | a television | its lines, over 1080 |
| under 20 inches along the diagonal | read from near | how dense it is, over 135 |
| anything else, which is 20 inches or more | on a desk, a monitor | how dense it is, over 110 |

A scale given in `config.yaml` is what it was, and nothing is guessed
for it. 1.75 on the maintainer's laptop is such a setting: the rule's
answer there is 2.

Why these numbers:

- **135 and 110** are GNOME's, which found 135 by trying, to match
  what macOS, Android, iOS and Windows show. Of the densities a
  system uses, 135 is the one nearest to giving the maintainer's
  laptop 2: at the size its panel says, its sum is 2.34 at 135, which
  2 taken up to 0.35 over it makes 2; Windows's 110 gives it 3, and
  KDE's 125 2.5. By the reference pixel 135 is reading from 51 cm, and
  110 from 62.
- **Near or on a desk by size, and not by socket**, as the roadmap
  had it. A second screen of 15 inches beside a laptop is read as the
  laptop is, and so the same panel gets the same scale on either;
  and a machine whose own screen is 27 inches is read as a monitor
  is. A small screen plugged in, as a portable monitor of 13 inches,
  is aimed at 135 as a laptop's own is: it is read from near as well,
  though it is not a laptop's, and one put further off is a setting
  away. At 1920 by 1080 a screen of 19 to 22 inches is 1 either side
  of the line, so the line falls where little turns on it.
- **Halves beyond 2, and 2 up to 0.35 over it,** keep the laptop's
  answer whatever its panel says of itself: 2 for any width from 308
  to 390 mm. Its panel, a BOE NV140QUM-N53, says 309 by 174 mm, 14
  inches and not the 15.6 first taken for it: 316 to the inch, and a
  sum of 2.34, which halves alone make 2.5, the scale that was too
  large. 2 is the sharper, and large enough, and the 14-inch 3840 by
  2400, at 2.40, stays at 2.5. In quarters it would be 2.25 from 336
  mm down. Windows's default has no 225% either.
- **0.15** stops short of the 14-inch 1920 by 1080, whose sum is
  1.17, and which KDE, GNOME and ChromeOS give 1.25.
- **560 mm** is midway, near enough, between the 43-inch screens that
  are put on desks, 535 mm, and the 48-inch, 598 mm. KWin's 500 mm
  takes in the 42-inch.
- **1080 lines** is what a television of 1080 lines has always shown
  a desktop at 1, and what Apple's lays its own out in.
- **720 units** is what Windows keeps back.

## What it gives, and is to be tested by

Worked out by [`tools/scale-survey.sh`](../../tools/scale-survey.sh),
which has the rule above beside the other systems' that the survey's
tables came from. Run with `check`, it holds this document to itself:
every row below, and each figure in the text that was worked out. A
size is as the nominal diagonal makes it, to the nearest millimetre.
"Before" is what `Scale::for_screen` gave the same screen before this
rule was built, by how dense it is across alone. Each row
is to be a test of the rule where it is built, by its name and at the
size it says here.

### Screens under 20 inches

| Screen | Says it is | To the inch | The sum | Scale | Before |
|---|---|---:|---:|---:|---:|
| 13.3 in, 1920 by 1080 | 294 by 166 mm | 166 | 1.23 | 1.25 | 1.5 |
| 14 in, 1920 by 1080 | 310 by 174 mm | 157 | 1.17 | 1.25 | 1.25 |
| 14 in, 1920 by 1200 | 302 by 188 mm | 162 | 1.20 | 1.25 | 1.25 |
| 15.6 in, 1920 by 1080 | 345 by 194 mm | 141 | 1.05 | 1 | 1.25 |
| 16 in, 1920 by 1200 | 345 by 215 mm | 141 | 1.05 | 1 | 1.25 |
| 17.3 in, 1920 by 1080 | 383 by 215 mm | 127 | 0.94 | 1 | 1 |
| 13.5 in, 2256 by 1504 | 285 by 190 mm | 201 | 1.49 | 1.5 | 1.75 |
| 13.3 in, 2560 by 1600 | 286 by 179 mm | 227 | 1.68 | 1.75 | 2 |
| 14 in, 2560 by 1600 | 302 by 188 mm | 216 | 1.60 | 1.5 | 1.75 |
| 16 in, 2560 by 1600 | 345 by 215 mm | 189 | 1.40 | 1.5 | 1.5 |
| 14 in, 2880 by 1800 | 302 by 188 mm | 242 | 1.80 | 1.75 | 2 |
| 16 in, 3200 by 2000 | 345 by 215 mm | 236 | 1.75 | 1.75 | 2 |
| 13.4 in, 3840 by 2400 | 289 by 180 mm | 338 | 2.50 | 2.5 | 2.75 |
| 14 in, 3840 by 2400 | 302 by 188 mm | 323 | 2.40 | 2.5 | 2.75 |
| 15.6 in, 3840 by 2160 | 345 by 194 mm | 283 | 2.09 | 2 | 2.25 |
| the same, in whole centimetres | 340 by 190 mm | 287 | 2.13 | 2 | 2.5 |
| 16 in, 3840 by 2400 | 345 by 215 mm | 283 | 2.10 | 2 | 2.25 |
| 17.3 in, 3840 by 2160 | 383 by 215 mm | 255 | 1.89 | 2 | 2 |

### Screens of 20 inches and more

| Screen | Says it is | To the inch | The sum | Scale | Before |
|---|---|---:|---:|---:|---:|
| 21.5 in, 1920 by 1080 | 476 by 268 mm | 102 | 0.93 | 1 | 1 |
| 23.8 in, 1920 by 1080 | 527 by 296 mm | 93 | 0.84 | 1 | 1 |
| 27 in, 1920 by 1080 | 598 by 336 mm | 82 | 0.74 | 1 | 1 |
| 23.8 in, 2560 by 1440 | 527 by 296 mm | 123 | 1.12 | 1 | 1 |
| 27 in, 2560 by 1440 | 598 by 336 mm | 109 | 0.99 | 1 | 1 |
| 31.5 in, 2560 by 1440 | 697 by 392 mm | 93 | 0.85 | 1 | 1 |
| 34 in, 3440 by 1440 | 797 by 333 mm | 110 | 1.00 | 1 | 1 |
| 49 in, 5120 by 1440 | 1198 by 337 mm | 109 | 0.99 | 1 | 1 |
| 23.8 in, 3840 by 2160 | 527 by 296 mm | 185 | 1.68 | 1.75 | 1.5 |
| 27 in, 3840 by 2160 | 598 by 336 mm | 163 | 1.48 | 1.5 | 1.25 |
| 31.5 in, 3840 by 2160 | 697 by 392 mm | 140 | 1.27 | 1.25 | 1.25 |
| 42.5 in, 3840 by 2160 | 941 by 529 mm | 104 | 0.94 | 1 | 1 |
| 27 in, 5120 by 2880 | 598 by 336 mm | 218 | 1.98 | 2 | 1.75 |
| 31.5 in, 6016 by 3384 | 697 by 392 mm | 219 | 1.99 | 2 | 1.75 |

### Televisions, and screens that say nothing true

| Screen | Says it is | Taken for | Scale | Before |
|---|---|---|---:|---:|
| 32 in, 1920 by 1080 | 708 by 398 mm | a monitor | 1 | 1 |
| 42 in, 3840 by 2160 | 930 by 523 mm | a monitor | 1 | 1 |
| 43 in, 1920 by 1080 | 952 by 535 mm | a monitor | 1 | 1 |
| 43 in, 3840 by 2160 | 952 by 535 mm | a monitor | 1 | 1 |
| 48 in, 3840 by 2160 | 1063 by 598 mm | a television | 2 | 1 |
| 55 in, 3840 by 2160 | 1218 by 685 mm | a television | 2 | 1 |
| 65 in, 3840 by 2160 | 1439 by 809 mm | a television | 2 | 1 |
| 77 in, 3840 by 2160 | 1705 by 959 mm | a television | 2 | 1 |
| 85 in, 7680 by 4320 | 1882 by 1058 mm | a television | 4 | 1 |
| a television saying a shape, 3840 by 2160 | 1600 by 900 mm | saying nothing true | 2 | 1 |
| a screen saying nothing, 3840 by 2160 | nothing | saying nothing true | 2 | 1 |
| a screen saying nothing, 1920 by 1080 | nothing | saying nothing true | 1 | 1 |
| a projector saying no size, 1920 by 1200 | nothing | saying nothing true | 1 | 1 |
| a screen saying 16 by 9 mm, 2560 by 1440 | 16 by 9 mm | saying nothing true | 1 | 4 |

### The lines, and what is kept back

The lines are where the decision has them, and a test stands to
either side of each and not on it: no screen's sum falls on one, and
what a number on a line comes to depends on how it is held. A sum of
1.14 is 1, and one of 1.16 is 1.25. 1.37 is 1.25 and 1.38 is 1.5;
1.62 is 1.5 and 1.63 is 1.75; 1.84 is 1.75 and 1.86 is 2; 2.35 is 2
and 2.36 is 2.5; 2.74 is 2.5 and 2.76 is 3; 3.24 is 3 and 3.26 is
3.5; 3.74 is 3.5 and 3.76 is 4, as is anything over.

A 7-inch 1280 by 800, whose sum is 1.6, stays at 1, as 1.25 would
leave it 640 units. A 6.8-inch 1920 by 1080, whose sum is 2.39, gets
1.5, which leaves 1280 by 720. A screen of 3840 by 2160 that says it
is 10 mm square gets 3.

## Alternatives

- **One density for every screen**, as now. Large on a laptop and
  small on a desk at once, as the survey's first finding has it.
- **Windows's densities, or KDE's.** Aimed at Windows's 110 the
  maintainer's laptop is 2.5, which is what was too large, and at
  KDE's 125 it is on the line between 2 and 2.5. Aimed at their 96 a
  monitor needs a floor, as both have, or the 27-inch 2560 by 1440 is
  scaled.
- **A density that gives that laptop 1.75**, which is one from 153 to
  173. Aimed at 160, the rule is under KDE, GNOME, ChromeOS and
  Windows at once on 15 of the 18 rows of screens under 20 inches; at
  150 on 12; at 140 on 6; at 135 on one.
- **Quarters all the way**, with the whole scale where within 0.15.
  What was first recommended. It leaves that laptop 1% from the line
  to 2.25 if its panel says 340 mm, and over it from 336 mm down.
- **Only the scales a panel divides by**, as GNOME and Hyprland. It
  rules out 1.75 on nearly every panel, which is what the maintainer
  runs. The pull to 1 and to 2, and halves beyond, are most of what
  it would give.
- **A television by its density**, as KDE and Windows. It rests on
  the size a television says, and gives a larger one less: by KDE's
  rule 2.65 at 55 inches and 1.9 at 77.
- **1 for a screen that says nothing**, as most give. At 2160 lines
  that is what cannot be used.

## Consequences

- **Most screens under 20 inches come out a step smaller than today,
  and the densest monitors a step larger**: 13 of the 18 rows above
  are smaller and none larger; of the 14 monitors, the 23.8- and
  27-inch 3840 by 2160 and the two denser still are larger, and none
  smaller. Until a guess is kept, as the roadmap has it kept, whoever
  has `auto` sees the change the first time omorfos starts with this:
  on most laptops, everything a step smaller overnight. So it is to
  be said where it is released, in the commit that builds it, which
  the release's notes are made from; and the guide is to say what
  `auto` does now and how to have the old size back, which is the
  number it was for `output.scale`, or for that screen in `outputs`.
- **All 32 rows are within what the survey's eight other rules give,
  and all but one within KDE, GNOME, ChromeOS and Windows.** The one
  is the 14-inch 2560 by 1600, at 1.5 where those four give 1.6 to 2.
- **Lines remain.** Five of the 18 rows are within 2% of one: the
  14-inch 1920 by 1080, at 1.17 by the line at 1.15; the 14- and
  16-inch 2560 by 1600, at 1.60 and 1.40 by those at 1.625 and 1.375;
  the 17.3-inch 3840 by 2160, at 1.89 by that at 1.85; and the 14-inch
  3840 by 2400, at 2.40 by that at 2.35. A panel
  says the same of itself each time, so a machine keeps its answer;
  two machines of one model whose panels say different sizes may not
  get the same.
- **A television of 43 inches or less is taken for a monitor.** At
  3840 by 2160 it gets 1, which is right on a desk and cannot be read
  from a sofa. A 48-inch on a desk gets 2, which is large there. Each
  is one setting away, and that setting is kept once screens are.
- **On a television things are smaller than Windows and KDE make
  them**: 1920 by 1080 units at 2160 lines, where Windows is reported
  to leave 1280 by 720 and KDE some 1450 by 815 at 55 inches.
- **None of this has been seen on a screen.** The answers are worked
  out, and the other systems' are ports of their rules, held to their
  own tests where they have any. What the maintainer's laptop says of
  its size is still to be read from it; the rule gives 2 for anything
  a panel of 15.6 inches could say.
- **To build it**, `Scale::for_screen` is given a screen's size both
  ways, in pixels and in millimetres, where it has only how far it is
  across; the hardware backend can give it all four. No key of
  `config.yaml` changes. What `docs/guide/screens.md` says `auto`
  does changes with it.
