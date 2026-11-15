# 85. One text field, of our own

- Status: accepted
- Date: 2026-11-15

## Context

The launcher's field took typing at its end and Backspace, and `Ctrl+A`;
nothing else. The maintainer found it unnatural: the arrows, `Alt` and
Backspace, Home and End did nothing, and asked whether to reproduce what
people are used to from text editors and browsers, or use something that
has it already, as COSMIC's, GNOME's or KDE's fields.

GNOME's and KDE's fields are GTK's and Qt's: using one makes a component
a GTK or Qt program, with a second toolkit's drawing and theming, not
drawn from `theme.css`, and slower to start. COSMIC's is built on
cosmic-text's `Editor`, which omorfos already draws text with; but that
editor is made for documents of many lines, and every move asks for the
fonts and a laid-out buffer, so that what a key does could only be tested
with fonts in hand, and much of it (lines, pages, indents, scrolling) is
of no use to one line.

## Decision

**One field, `omorfos_ui::Field`, of our own: what is typed, the cursor,
what is chosen and what can be undone, as GTK's fields have them, with
characters and words as Unicode has them, from `unicode-segmentation`.**

- **What it does:** the arrows by a character, `Ctrl` or `Alt` and an
  arrow by a word, and Home and End to an end; Shift with any of them
  choosing; Backspace and Delete by a character, with `Ctrl` or `Alt` by
  a word; `Ctrl+A`; `Ctrl+Z`, and `Ctrl+Shift+Z` or `Ctrl+Y`; `Ctrl+C` and
  `Ctrl+X`; and `Super` as `Ctrl` in each, as ADR 78 has it, which is also
  how the compositor hands `Super` on to a field: so a Mac's `Cmd` and an
  arrow, to an end, is not offered, as it would be told from `Ctrl` and an
  arrow by nothing. `Super` with Left or Right never comes to the field,
  being bound by default to the workspace beside: the maintainer kept that
  on 15 November 2026, and left going by a word to `Ctrl` and `Alt`. A
  word typed is undone at once, as GTK undoes it. It
  knows nothing of keys or drawing: `omorfos_shell::typing` reads the
  keys, and each component draws it.
- **`unicode-segmentation`** is already in the tree through cosmic-text,
  at the same version: what makes an accented letter written as two code
  points one character, and `file.txt` one word.
- **The launcher first.** The lock screen and the boxes that ask for a
  password take Backspace of any reach as one character, as before, and
  nothing else of it: what is typed there is not shown, to move about in.

## Alternatives

- **cosmic-text's `Editor`.** As above: tested only with fonts, and made
  for more than a line.
- **GTK's or Qt's field.** As above: a second toolkit for one field.
- **Each component its own.** What there was; the launcher's was the
  least of them.

## Consequences

- Pasting is not offered yet: nothing lets a component read the
  clipboard, and that is to be one request, of the launcher's role only,
  on the roadmap. Copying and cutting go through the compositor, as the
  launcher's answers to sums do (ADR 82).
- A click does not yet put the cursor where it is, nor a drag choose: the
  field takes a place (`Field::place`), and the launcher is to give it
  one from where the pointer is over what is typed.
- Text wider than the field is not scrolled along to the cursor; the
  launcher takes 64 characters, about as many as it shows.
