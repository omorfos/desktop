# Screens

With more than one screen, they are placed side by side from left to
right in the order they were plugged in, a laptop's own first. To say
otherwise, list them under `outputs`:

```yaml
outputs:
  - { name: HDMI-A-1, scale: 1.25, mode: 2560x1440@144 }
  - { name: eDP-1, place: below, offset: -30% }
```

The screens listed stand in that order, each to the right of the one
listed before it unless its `place` says `left`, `above` or `below`: side
by side their tops are in line, and one above or below another is in the
middle of it. `align` says otherwise: `start` lines up the tops, or the
left sides, `middle` the middles and `end` the bottoms, or the right
sides. `offset` then moves it along the edge the two share, in units of
layout, such as `-300`, or as a share of its own length along that edge,
such as `-30%`: less than nothing to the left, or up. The first listed
has none before it, and you are told if it is given a `place`, an
`align` or an `offset`.

To have the screens stand one way at a desk and another away from it,
list each way under `arrangements`, as `outputs` is listed: the first
that names just the screens plugged in, in whatever order, is used in
place of `outputs`, and is followed again as soon as a screen is plugged
in or taken out.

```yaml
arrangements:
  - [{ name: eDP-1, scale: 2 }]
  - [{ name: HDMI-A-1, primary: true }, { name: eDP-1, place: below }]
```

One listed screen may be `primary: true`: the notice of what is wrong is
shown on it, and the windows of a screen that is unplugged go to it.
Without one, the first screen plugged in is.

A screen may instead show what another shows, as a projector does a
laptop's, with `mirror` and the other's name:

```yaml
arrangements:
  - [{ name: eDP-1 }, { name: HDMI-A-1, mirror: eDP-1 }]
```

It shows the other's picture as large as fits on it, in the middle, with
black either side where the two are not of a shape. It stands nowhere of
its own: the pointer and windows do not go onto it, and programs are not
told of it, so it has no panel or wallpaper of its own; what was on it
goes to the other. It may have a `mode` and a `rotate`, but not a
`place`, `align`, `offset`, `scale` or `primary`, and a screen it mirrors
cannot mirror another. While the one it mirrors is not plugged in, it
stands as any screen does. `omorfosctl screens` lists it after those that
stand, where the one it mirrors stands.

A screen is turned off with `enabled: false`, as a laptop's own is under
its lid at a desk:

```yaml
arrangements:
  - [{ name: eDP-1, enabled: false }, { name: HDMI-A-1 }]
```

It shows nothing and stands nowhere, and programs are not told of it, as
one unplugged; what was on it goes to the others. It takes no `place`,
`align`, `offset`, `scale`, `primary` or `mirror`, and no screen may
mirror it. If every screen plugged in would be off, the first is left on
and you are told, so that a mistake never leaves you with no screen at
all. `omorfosctl screens` lists it last, as off.

The display key (`XF86Display`, the `next_screens` action) has the
screens stand another way for now, without changing this file: first as
the configuration says, then all showing what the one that comes first
shows, then that one alone, then each of the others alone, and round
again. Each screen keeps its `mode` and `rotate`. In the middle of each
screen that is on, a box says which way they are in, such as "Mirrored:
every screen shows what eDP-1 shows", and asks whether to keep them so:
Enter keeps them, Escape puts them back as they were, and so does
waiting 15 seconds, since it is the key you press when a screen has gone
dark. Pressing the key again goes on to the next way. What is kept is for
the screens plugged in when it was; plugging one in or taking one out
has them stand as this file says again, and so does logging in.

A screen turned on its stand is said with `rotate`, as `xrandr --rotate`
says it: `left`, with the top of the picture on the left as the screen
was made, `right`, or `inverted`; `normal` is as it was made. It then
stands as tall as it was wide, and what it shows, and pictures of it,
are the right way up. Inside another desktop, in a window, `rotate` is
not used. Those that are connected but not listed stand to the right of
all the rest, and each listed may have a `scale` of its own in
place of `output.scale`, and a `mode`: the size it is driven at in
pixels, and how many times a second it is drawn if that matters. Without
a rate, the fastest the screen offers at that size; without a mode, what
the screen says suits it best. A mode the screen does not offer is not
used, and what it is driven at instead is said among the problems, as a
mistake in this file is, until the mode is put right. A change of mode
takes effect as soon as the file is saved. `omorfosctl screens` lists
the modes each screen offers, written as `mode` takes them. A screen's name is the `output` shown by
`omorfosctl state`. One that is listed but not plugged in is skipped.

`output.scale` is how large everything is drawn, on every screen that
`outputs` does not give a scale of its own. With `auto`, omorfos
chooses for each screen from how dense it is, so that a laptop with a sharp panel
is as readable as a desktop monitor. If the choice does not suit you, give
a number: `1` is one pixel to a pixel, `1.5` half as large again, `2`
twice the size.
