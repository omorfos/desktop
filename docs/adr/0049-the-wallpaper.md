# 49. The wallpaper is a component that reads pictures for itself

- Status: accepted
- Date: 2026-10-22

## Context

Behind the windows there was only the theme's `desktop`, a colour the
compositor draws. A desktop is expected to show a picture there, or a
different colour on each screen, or a directory of photographs in turn
(wish 6).

Three questions come with it: what draws it, what reads the pictures, and
how a picture is made the size of a screen.

## Decision

**A program of its own, `omorfos-wallpaper`, as the panel is (ADR 18).** It
puts a surface on the background layer of each screen through
layer-shell, with the role a panel has, and the compositor starts it with
the session unless `wallpaper.enabled` is off. Reading a picture is
reading a file that may have come from anywhere; done in a program of its
own, a decoder that goes wrong takes down the wallpaper, which is started
again, and not the session. Anything else that speaks layer-shell, such
as `swaybg`, can take its place.

It reads the configuration directory itself and watches it, as the panel
reads the theme, rather than being told over the IPC socket. What it is
to show is `config.yaml`'s, and nothing of the session's state is needed
to show it.

**With nothing to show, it shows nothing.** Until a picture or a colour is
named, it puts no surface up, and the compositor's `desktop` is what is
behind the windows, as before. A screen that is to be one colour is given
a single pixel of it (`wp_single_pixel_buffer_v1`), made the screen's size
by the viewport, rather than millions of the same pixel in shared memory.

**PNG and JPEG, decoded in Rust, with limits.** PNG is read by the decoder
the icons already use. JPEG, which photographs are, needed a decoder:
`zune-jpeg` is in Rust, under MIT, Apache or Zlib, and is what the `image`
crate itself now reads JPEG with. Its default features are left out, so
that it uses no instructions particular to a processor, which it reaches
through `unsafe`: a picture is read once in minutes, and the time that
costs is worth less than the unsafe code it keeps out of reach of a file.
No file over 256 MiB is read, and no picture is decoded that says it is
over 16384 pixels along a side, which a picture of 16K is not. A PNG is
read a row at a time, and only what the screen shows of it is kept: for
`center` and `tile` the part on the screen, for the rest each square of
pixels that still leaves enough to cover or fit on the screen averaged
into one. A 16K picture of 133 million pixels is so read in a second or
two and about 50 MiB, where read whole it would take half a gigabyte. A
JPEG is decoded whole, as `zune-jpeg` does, and so is a PNG that is
interlaced, whose rows come a part at a time; neither may have more than
67 million pixels, which is what a camera of today takes and at four
bytes a pixel what memory allows. The same holds for what is kept. A
photograph is turned the way its EXIF data says the camera was held,
which is a dozen lines and the difference between a portrait standing up
and lying on its side.

Other formats (WebP, AVIF, SVG) are left until someone asks: each is
another decoder of untrusted input.

**Laid in pixels, smoothed both ways.** `fill`, `fit` and `stretch` make the
picture another size; `omorfos-ui`'s `Canvas` gained a way to draw a
picture stretched to any size, smoothed when smaller as the average of
the pixels each covers, and when larger by blending the two nearest along
each side. The shares are counted in whole numbers, in parts of a pixel
fine enough that nothing is rounded. `center` and `tile` draw the picture
as it is, a pixel of it to a pixel of the screen, whatever the screen's
scale: a picture is the size it was made, and it is sharp.

**A directory is looked at again each time its picture changes,** so that
a picture put there joins in without a reload. `shuffled` shows each once
before any is shown again, and not the same twice in a row, from a shuffle
made with SplitMix64 from the clock: chance enough to choose a picture by,
with nothing to depend on.

**What a screen was given is kept, and given again.** Reading a picture of
16K takes over half a second, most of it in undoing the PNG's compression,
which is done in one go from the start and cannot be shared among cores;
a screen at login would show the desktop's colour for that long. So what
each screen was given is written to the cache directory
(`$XDG_CACHE_HOME/omorfos/wallpaper/`), one file for each screen, with
what it was drawn from: the picture's path, its size and when it last
changed, the fit, the colour, and the screen's size and scale. At the next
login a screen for which all of that is still so is given those pixels,
read in milliseconds, and the picture is not read at all. The file is
the screen's pixels as they are, not compressed: compressing them again
would cost a good part of what keeping them saves.

The disk is the cost, at four bytes a pixel: 15 MiB for a screen of
2560 by 1440. A directory shown in turn every few seconds would write that
at each, so while pictures change, a screen's file is written again at
most every half an hour; it is written at once after the configuration
changes. A directory starts at login from the picture its screen's file
was drawn from, if it is still there, so that what was kept is what is
wanted, rather than with the first of a new order that nothing was kept
for.

**The next picture is read before it is due.** Half the time each is shown
for, or ten seconds if that is less, before the next is due, it is chosen
and drawn for each screen that will show it, and kept until it is due, so
that the change happens on time and at once. This is done where everything
else is, on the one thread: the wallpaper is busy for that time, but it
has nothing else to do, and a thread would buy nothing a screen could show
for what it costs to reason about.

**One picture fades into the next, and the compositor does the fading.**
What a screen is to show next is put on a surface of its own above what
it shows, a subsurface, and made from wholly faint to not faint at all
with `wp_alpha_modifier_v1`, which the compositor now offers and its
renderer already heeds. Each step is a number, not a picture: the pixels
go to the compositor once, and blending them is the renderer's work, on
the graphics card where there is one. The steps are taken as the
compositor says it has shown the last, so a fade is as smooth as the
screen, and is finished by the time it should be if the compositor stops
saying so, as it does for a screen that is off. Once it is done, the
picture is shown on the screen's own surface and the one above goes; it
was handed over twice, once for each surface, since each gives its buffer
back on its own.

Drawing each step of the blend in the wallpaper would have handed the
compositor a screenful of pixels at every frame; a fade done by the
compositor on its own, at any change of what a background surface shows,
would fade a wallpaper that is a moving picture at every frame of it.
Only the wallpaper knows that a change is a new picture.

`wallpaper.fade` says how long it takes, a second unless told otherwise,
eased in and out; 0 is at once. The first picture of a session is shown
at once, and so is one drawn because the screen changed size or scale:
that is the screen changing, not the picture.

## Consequences

- A new crate, a new dependency (`zune-jpeg`) and a new program to
  package. A small JPEG made with GIMP is kept in the crate for its tests,
  since no encoder whose licence `deny.toml` allows makes one.
- The colour of a screen is configured in `config.yaml`, beside its
  picture, not in the theme: a theme cannot tell one screen from another.
  Without one, the theme's `desktop` is still the colour.
- A picture is read again whenever it is to be drawn again, at a change of
  the picture, the configuration, the screen's size or its scale, and let
  go of once it is drawn: what is kept between is only the picture handed
  to the compositor.
- Blurring what is behind a glass part, which the wallpaper is most of,
  waits for the look (stage 4).
