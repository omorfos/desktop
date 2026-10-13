# 22. The launcher asks the compositor to start what is picked

- Status: accepted
- Date: 2026-10-12

## Context

Programs could be started from a key binding, the root menu or a terminal.
There was no way to start one by its name without having configured it
first. A launcher is the second of the shell components of ADR 18.

## Decision

**`omorfos-launcher` is a program of its own that lives for one pick.** It
is started when asked for and exits once a program is picked or Escape is
pressed. Nothing of it runs in between, so it costs nothing while it is not
in use and has no state to go stale.

**It offers what the desktop entries say is installed.** It reads the
`.desktop` files in the `applications` directories of `XDG_DATA_HOME` and
`XDG_DATA_DIRS`, the user's own first, so that a user's entry replaces or
hides the system's. They are other programs' files: only so much of each is
read, only so many from a directory, and one that makes no sense is passed
over. The reader is ours; it is a few dozen lines and the format is simple.

**Picking is a value.** `Picker` holds the programs, what has been typed
and which match is chosen, and says what a key does and how it is drawn, as
`Bar` does for the panel. A name matches if it has what was typed, whatever
the case; names that begin with it come first.

**The compositor starts the program.** The launcher sends the `spawn`
action over the IPC socket, as `omorfosctl` would, and leaves. The program
is then the compositor's child, with the session's environment, and
outlives the launcher without anything having to be detached. A program
that needs a terminal is started in the one the built-in bindings use.

**The compositor opens the launcher.** A new action, `show_launcher`, is
bound to `Super+Space` (at first `Super+d`) and listed in the root menu by default. A backend says
where the launcher is, as it does for the panel. Asked while one is open,
the compositor does nothing.

**It takes the keyboard to itself** through layer-shell, on the overlay
layer of the screen the pointer is on. What a key types is worked out with
`xkbcommon` from the layout the compositor sends. The layout is read as
text, not mapped into memory, which is the part of that library that would
have needed `unsafe`; the crate was already in the tree through Smithay.

Its look is three parts in the theme: `launcher`, `launcher-input` and
`launcher-item`, the last `:active` when it is the one that would start.

**A second surface covers the rest of the screen**, under the box, so that
a click beside the launcher is the launcher's to hear of and closes it. It
is one transparent pixel stretched over the screen unless the theme gives
`launcher-backdrop` a colour, to dim what is behind, in which case it is
drawn once at the screen's size. The box stays a small surface of its own,
so that typing redraws the box and not the screen.

**Names alone by default; icons and descriptions are settings.**
`launcher.icons` and `launcher.descriptions` in `config.yaml` each add
something to a row. A description is also searched, after the names, but
only when it is shown: matching on what cannot be seen would look like a
mistake.

**Icons are read with two libraries.** A desktop entry names an icon, which
is found in the `hicolor` theme of the icon directories, in the size that
suits, or among the loose pictures in `/usr/share/pixmaps`. On a real
system five in six of them are SVG, so reading PNG alone would have left
most rows without one. `png` reads the one and `resvg` draws the other, at
the size wanted; both are permissively licensed and `resvg` is built
without its text and raster features. Together they bring some thirty
crates into the launcher, and into nothing else. Icons are other programs'
files: only so much of one is read, a picture is only so large, and one
that makes no sense is no icon. Only the icons of the rows in sight are
looked up, and each once.

## Consequences

- The launcher needs nothing the IPC protocol did not already have.
- It is as large as the theme makes it and does not grow or shrink with
  what matches, which keeps it still while typing.
- A key held down repeats, which under Wayland is for the client to do:
  the compositor says how soon and how often, and the launcher takes the
  key again by its own clock. The time is passed in, as for the
  notifications, so that the tests need not wait.
- A name matches only as a run of letters and not loosely, and only the
  entry's untranslated name and comment are looked at.
- Reading every desktop entry each time it opens takes a moment on a system
  with thousands. If that shows, the list can be kept between runs.
- The panel and the launcher each hand a canvas to the compositor with the
  same twenty lines. A third component will be the time to share them.
