# The launcher

`Super+Space`, or **Run…** in the root menu, opens the launcher: a box on
the screen the pointer is on. Type part of a program's name and the list
narrows to the programs that have it, those that begin with it first. Up
and Down choose among them, the list following the one chosen. While you
type, no program is lit, and Enter starts the first that matches. Down,
or the pointer over a program, gives the programs the keyboard, as in
Spotlight: the first, or the one pointed at, is lit, the field's cursor
goes, and Left and Right walk the programs too, until something is typed
or Up goes back past the first, which give the field the keyboard again
with nothing lit; the wheel
and the touchpad scroll the list, a notch of the wheel a program, and the
pointer chooses the one it is over, the list staying as it was scrolled.
Enter starts the chosen one, and Escape closes the launcher. A click on a
program starts it too, and a click anywhere beside the launcher closes
it. `Super+A`, as Cmd+A on a Mac, chooses all that is typed (`Ctrl+A`
with `defaults: ordinary` in `keys.yaml`), for what is typed next to take its place, or Backspace to take it
all out, as in the box that asks for a password. What is typed is edited
as in any text field, where the cursor blinks, as it does in every field
of omorfos, for ten seconds after the last key and then stays: Left and
Right move the cursor, by a word with
`Ctrl` or `Alt`, and Home and End to an end; with Shift they choose what
they go over; Backspace and Delete take out a
character, or a word with `Ctrl` or `Alt`; `Ctrl+Z` undoes and
`Ctrl+Shift+Z` does again; a click puts the cursor where it is and a
drag chooses what it goes over; what is too long for the field is shown
from as far back as keeps the cursor in sight; `Ctrl+C` copies what is
chosen and `Ctrl+X` cuts it; `Ctrl+V` pastes in place of what is chosen the first line of
what is on the clipboard, if it is text. `Super` does as `Ctrl` in each,
but with Left or Right, which go to the workspace beside, as they do
everywhere.

The launcher is as tall as what it lists, up to half the height of the
screen, and the list scrolls past that. Its top stays where it would be
if it stood in the middle of the screen at its tallest, so that the
field typed in does not move as the list grows and shrinks; with nothing
matching what is typed, it says so under the field.

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
chosen, with "Enter copies the answer" under it: `6*7` shows `= 42`.
Enter, or a click on it, puts the answer on the clipboard and closes the
launcher. `+`, `-`, `*` or `×`, `/` or `÷`, and `^` for a power are
understood, with brackets, and multiplying before adding, as by hand;
`sqrt` or `√` for a square root, as `sqrt(2)` or `√2`; `sin`, `cos` and
`tan` of an angle in degrees, as on a calculator, so that `sin(30)` is
`0.5`; `ln` and `log`, the logarithms natural and of ten; and `pi` or `π`,
and `e`.

Numbers are written as the locale for numbers writes them: the first of
`LC_ALL`, `LC_NUMERIC` and `LANG` that is set. In English, and wherever
the locale is not known to write a comma, the decimals take a point and
thousands a comma or a space: `1,234.5`. Where it writes a comma, as in
Polish or German, the decimals take a comma and thousands a space or a
point: `1 234,5` or `1.234,5`. What stands between thousands is read only
where three figures follow it, so `2 3` is no number; a point not before
three figures is the decimals there too, as a keypad types it, so `1.5`
is one and a half and `1.500` fifteen hundred. The answer marks its
decimals in the same way, and has nothing between its thousands, so
that it can be pasted anywhere.

`%` makes a number a hundredth, as `50%` is `0.5` and `200*10%` is `20`;
a percentage alone added or taken away is of what it is added to or
taken from, as on a calculator: `200+10%` is `220`.

A sum not yet finished is answered as far as it goes, so that the answer
stays as the next sign is typed: `60 + 60 +` shows `= 120`, `2*3+sq` shows
`= 6`, and a bracket left open is closed. A number or `pi` alone is no
sum, and nothing typed is ever run: it is read by omorfos's own code,
which the calculator is to share (ADR 86). An answer has at most 15
figures, and a very large or very small one is written with a power of
ten, as `1e20`.

## Every program

The `show_programs` action opens the launcher onto every program
installed, as a Mac shows its Apps: a grid of their icons, each with its
name under it, six to a line in alphabetical order, the launcher as wide
as a line of them. It has no key of its own; bind one in `keys.yaml`:

```yaml
bindings:
  - keys: Super+Shift+space
    action: show_programs
```

The arrows walk the grid, and the wheel scrolls it a line at a time past
half the height of the screen. Enter or a click starts the program
chosen. Typing narrows the grid as it narrows the list, and once
something is typed, Left and Right move in it as in any field until Down
gives the grid the keyboard. The grid is the programs' mode, and finds
programs alone: a sum is answered where the launcher opens, `Super+0`.
If the launcher is open already, it is left as it is.

## Modes

The launcher has modes, each with what it finds alone, reached with
Super and a digit while it is open (Ctrl and a digit with `defaults:
ordinary` in `keys.yaml`): `Super+1` the first, and on; `Super+0`, or
Escape in a mode, goes back to what it opens onto, which finds everything.
Their order is `launcher.order` in `config.yaml`:

```yaml
launcher:
  order: [programs, files, actions, clipboard, commands]
```

A mode left out comes after those named, in that order: the list only
orders. A mode not built yet takes no number, and today only `programs`,
the grid of every program, is.

How it looks is in `theme.css`, under `launcher`, `launcher-input`,
`launcher-item`, `launcher-description` and `launcher-tile`, a program in
the grid, as wide as its `min-width`; `launcher-input:active` is
what is typed when all of it is chosen. `launcher-backdrop` is the rest
of the screen while it is open: give it a see-through background, such as
`rgba(0, 0, 0, 0.4)`, to dim what is behind. To open the launcher with
another key, bind `show_launcher` in `keys.yaml`.
