# The wallpaper

`omorfos-wallpaper` shows a picture or a colour behind everything on each
screen. Until you name one, it shows Prism, the wallpaper that comes with
omorfos, in `share/backgrounds/omorfos/`: the dark one while the theme is
dark and the light one while it is light, changing as the theme does.
Without those installed, it shows nothing, and the theme's `desktop` is
what is behind the windows.

Four more come with it, each dark and light: Spill, Horizon, Facets and
Ground. Name one as `picture`, as
`/usr/share/backgrounds/omorfos/horizon-dark.png` with its `-light` pair
(below), or the directory itself, to have all five in turn.

```yaml
wallpaper:
  picture: ~/Pictures/hills.jpg
  fit: fill
```

`picture` is a PNG or a JPEG, written as a path from the root or from your
home directory, as `~/`. A photograph is turned the way the camera says it
was held. It may also be a directory of pictures, which are shown in turn,
each for `every` seconds (5 to a week), by their names or, with `order:
shuffled`, in any order, a different one each time through. A picture put
in the directory, or changed in it, is shown at once, and one taken out
while it is shown is followed at once by the next; those in directories
within it are not shown. A single picture named is read again at once when
it changes.

A picture may also be one for a dark desktop and one for a light one,
each a file or a directory, and the wallpaper shows the one that goes with
the theme, changing over when the theme does:

```yaml
wallpaper:
  picture:
    dark: ~/Pictures/night.jpg
    light: ~/Pictures/day.jpg
```

In a directory, two pictures named alike but for `-dark` and `-light` at
the end, as `hills-dark.png` and `hills-light.jpg`, are a pair, and are
shown in turn as one picture, each as the theme is. One without its other
half is shown whatever the theme.

The desktop is dark while the theme writes its menus in a colour brighter
than the one it writes them on, as the built-in theme does; this is what
applications are told too, so that they follow it.

`fit` says how a picture is laid on the screen:

- `fill`: as large as it must be to cover the screen, its shape kept, and
  what does not fit cut off equally on two sides.
- `fit`: as large as fits on the screen, its shape kept.
- `stretch`: made the screen's size and shape.
- `center`: as it is, a pixel of it to a pixel of the screen, in the
  middle.
- `tile`: as it is, again and again from the top left corner.

`color` is the colour wherever the picture does not cover the screen, or
of the whole screen without one; without it, the background of the
theme's `desktop`. Write it in quotes, as `'#203040'`, since YAML reads
what follows a bare `#` as a comment; a `color:` left with nothing after it
is an error that says so.

A screen can show something else, named as `omorfosctl state` names it.
One listed with a `picture` or a `color` shows only its own; one listed
with only a `fit` shows the rest as every screen does:

```yaml
wallpaper:
  picture: ~/Pictures/hills.jpg
  screens:
    - { name: HDMI-A-1, picture: ~/Pictures/sea.png, fit: fit }
    - { name: DP-1, color: '#203040' }
```

It follows the configuration as you save it. A picture that cannot be
read, or is larger than 16384 pixels along a side, is not shown: the
colour is, and why is written to the session's log. A PNG is read a row
at a time and made no larger than the screen needs as it is, so a picture
of 16K takes under a second and little memory. A JPEG, or a PNG that is
interlaced, is read whole first, and may have 67 million pixels at most.

What each screen was given is kept in `~/.cache/omorfos/wallpaper/`, one
file for each screen, as large as the screen's pixels at four bytes each.
When you log in, a screen whose picture has not changed, laid the same way
on a screen of the same size, is shown that at once, without the picture
being read again. A directory starts from the picture kept, if it is still
there. While a directory's pictures are shown in turn, a screen's is kept
again at most every half an hour, so that pictures changing every few
seconds are not each written to the disk. The next picture of a directory
is read half the time it is shown for, or ten seconds, before it is due,
so that it changes the moment it is. Deleting the directory is safe: what
is in it is made again.

A change of picture, colour or fit fades from the old to the new over
`fade` milliseconds, eased in and out: 1000 unless you say otherwise, up
to 10000, and 0 for at once. So does going from the dark picture of a
pair to the light one as the theme changes. The fade begins once the new
picture is ready, so a large one that takes a while to read still fades
in for all of that time. The first picture of a session is shown at
once, as is one drawn again because a screen changed size or scale.

To use another program for it, such as `swaybg`, turn it off and start
that, which needs to be let put a surface behind the windows as a panel
does:

```yaml
wallpaper:
  enabled: false
autostart:
  - { run: [swaybg, -i, /usr/share/backgrounds/hills.png], may: [panel] }
```

Like `autostart`, turning it off or on takes effect the next time you log
in.
