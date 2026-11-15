# The launcher

`Super+Space`, or **Run…** in the root menu, opens the launcher: a box on
the screen the pointer is on. Type part of a program's name and the list
narrows to the programs that have it, those that begin with it first. Up
and Down choose among them, as the wheel and the touchpad do, a notch of
the wheel a program; Enter starts the chosen one, and Escape closes the
launcher. A click on a program starts it too, and a click anywhere beside
the launcher closes it. `Ctrl+A`, or `Super+A`, chooses all that is
typed, for what is typed next to take its place, or Backspace to take it
all out, as in the box that asks for a password.

The launcher is as tall as what it lists, up to half the height of the
screen, and the list scrolls past that. Its top stays where it would be
if it stood in the middle of the screen at its tallest, so that the
field typed in does not move as the list grows and shrinks; with nothing
matching, it is the field alone.

The programs are those with a desktop entry installed, in
`~/.local/share/applications` and the system's `applications` directories,
by the names their entries give them in the language the session is in
(`LANGUAGE`, or else `LC_ALL`, `LC_MESSAGES` or `LANG`), or in English
where an entry has none in it. One that needs a terminal is started in
`foot`.

Each program is shown by its icon and its name, with what it says of
itself under the name, and a sum typed is worked out. Everything the
launcher does is on unless `config.yaml` turns it off, each by a key of
its own under `launcher`:

```yaml
launcher:
  icons: false
  descriptions: false
  sums: false
```

`icons` puts each program's icon before its name, as tall as the row's
two lines. Icons are looked for by name in the `hicolor` theme of
`~/.local/share/icons` and the system's `icons` directories, and in
`/usr/share/pixmaps`; PNG and SVG are read.
Icons a program does not bring itself are taken from `Adwaita`.
`descriptions` writes what each program says of itself under its name, and
has that searched too, after the names. `sums` is below. Each takes effect
the next time the launcher is opened.

With `sums` on, as it is unless turned off, a sum typed in the launcher is
worked out as it is typed, and its answer shown above the programs,
chosen: `6*7` shows `= 42`. Enter, or a click on it, puts the answer on
the clipboard and closes the launcher. Numbers are written with a point
for the decimals, as `1.5`; `+`, `-`, `*` or `×`, `/` or `÷`, and `^` for
a power are understood, with brackets, and multiplying before adding, as
by hand. A sum not yet finished is answered as far as it goes, so that the
answer stays as the next sign is typed: `60 + 60 +` shows `= 120`, and a
bracket left open is closed. A number alone is no sum, and nothing typed
is ever run: it is read by the launcher's own code. An answer has at most
15 figures, and a very large or very small one is written with a power of
ten, as `1e20`.

How it looks is in `theme.css`, under `launcher`, `launcher-input`,
`launcher-item` and `launcher-description`; `launcher-input:active` is
what is typed when all of it is chosen. `launcher-backdrop` is the rest
of the screen while it is open: give it a see-through background, such as
`rgba(0, 0, 0, 0.4)`, to dim what is behind. To open the launcher with
another key, bind `show_launcher` in `keys.yaml`.
