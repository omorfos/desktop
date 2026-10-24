# Accessibility

omorfos is to be usable by everyone who uses a computer, however they see,
move or read. That is part of how it is designed, not added after:
[docs/DESIGN.md](docs/DESIGN.md) puts legibility before looks, measures
the contrast of every colour it names, never lets colour be the only sign
of something, and asks of every new part whether it can be worked with
the keyboard alone.

omorfos is early, and not all of that is there yet. This says what there
is, what is not there yet, and how to tell us what stands in your way.

## What there is

**Seeing**

- Everything can be drawn larger, by a scale for each screen of 1 to 4 in
  steps of a quarter (`output.scale`, and `scale` under `outputs`).
- The text of the desktop's parts can be set larger in the theme, from
  6pt to 72pt (`font-size` in `theme.css`), as can every colour.
- The pointer can be from 8 to 128 pixels, in any cursor theme installed
  (`cursor.size` and `cursor.theme`).
- The colours of the design language are measured as WCAG 2 does: text at
  least 4.5 to 1 on what it is on, and the edge of what takes input at
  least 3 to 1. The built-in theme is to be redrawn in them; until then a
  theme of your own can set any colour.

**Movement**

- Every change of colour, fade and movement of the desktop's parts is
  turned off with `* { transition: none; }` in `theme.css`, and one
  picture of the wallpaper changes into the next at once with
  `wallpaper.fade: 0`.

**The keyboard**

- Every action of the desktop can be bound to a key, and chains of keys,
  in `config.yaml`. Keys repeat as fast and as soon as you set
  (`keyboard.repeat_rate` and `keyboard.repeat_delay`), or not at all.
- The root menu and a window's menu are steered with the arrows, Enter
  and Escape; windows are switched with Alt and Tab; the launcher is
  typed into.
- The lock screen and the administrator's prompt are used with Tab,
  Shift and Tab, Enter and Escape. On the lock screen, a question that
  would end the session is answered no by Enter unless you move to yes.
- The pictures and windows to share or take a picture of are picked with
  the keyboard as well as the pointer.
- The fields for a password take Backspace held down, which goes on
  taking out as keys repeat, and Ctrl and A, or Super and A, to choose
  all of what was typed; each has a line where the next character goes.
- Languages that are written through an input method can be.

**The pointer**

- A mouse and a touchpad are set apart: speed, acceleration, the way they
  scroll, and tapping to click.

## What is not there yet

These are on the [roadmap](docs/ROADMAP.md), and some are waited on by
others:

- **Screen readers.** Nothing the desktop draws itself is told to one yet:
  the panel, the menus, the launcher, the lock screen and the
  administrator's prompt are pictures to it. Programs that speak to a
  screen reader themselves, as GTK and Qt programs do, may be read by one
  such as Orca, but that has not been tried, and Orca's own keys are not
  yet given to it. This is the largest gap, and it waits on how the
  desktop's parts are to be written.
- **Settings for those who need them**: high contrast, larger text and
  reduced motion as settings of their own, which applications are told
  of, rather than by writing a theme.
- **A magnifier**, **sticky keys** and **slow keys**.
- **An on-screen keyboard.**
- **Everything from the keyboard**: notifications are dismissed with the
  pointer alone so far.
- **Focus shown by more than colour.** The window with focus has its
  border and its titlebar in other colours, and nothing else yet; the
  design language asks for a ring as well, which comes with the
  built-in theme's redrawing.

## Telling us

If something in omorfos stands in your way, open an
[issue](https://github.com/omorfos/desktop/issues/new/choose) and say that
it is about accessibility. Say what you were trying to do, what you use
to do it, such as a screen reader, a magnifier, a switch or only the
keyboard, and what happened. Such an issue is taken as a bug, not a wish:
something omorfos should do and does not.

A change that touches what is drawn is held to the same: see
[CONTRIBUTING.md](CONTRIBUTING.md).
