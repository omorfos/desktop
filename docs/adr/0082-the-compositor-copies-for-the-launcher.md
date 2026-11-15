# 82. The compositor copies for the launcher

- Status: accepted
- Date: 2026-11-15

## Context

The launcher works out a sum as it is typed, and Enter copies the
answer. What is on a Wayland clipboard is given, each time it is pasted,
by the program that put it there, and the launcher ends as soon as
Enter is pressed. A program copying has to stay to give what it copied:
`omorfos-screenshot` does, holding its picture until something else is
copied. The launcher, started with the `overlay` role and nothing more,
may not use `ext-data-control` at all.

The compositor already puts text of its own on the clipboard: "About"
copies what the computer is, given by the compositor for as long as
nothing else is copied, and told to programs written for X11 too.

## Decision

**The launcher asks the compositor to copy, over its socket:
`{"copy": {"text": "..."}}`, and ends. The compositor puts the text on
the clipboard as its own, as "About" does.**

- The text is at most 4096 bytes, `LONGEST_COPY`, and a request for
  more is refused. A program pasting is handed it in one write, which a
  pipe takes whole however small it is made, so the compositor is never
  kept waiting by one.
- It is a request of the socket, not an action: nothing asked for a key
  or an entry in a menu that copies fixed text, and one would be a
  promise of `keys.yaml` and `menu.yaml` to keep.

## Alternatives

- **The launcher holding the clipboard**, as `omorfos-screenshot` does.
  It would need the `clipboard` role, and to stay after it is closed,
  out of sight; the compositor opens no second launcher while the first
  is still running, so the next `Super+Space` would do nothing until
  something else was copied.
- **An action, `copy: text`**, performed through `perform` as starting a
  program is. As above, a key and a menu entry nobody asked for.

## Consequences

- Any program that can reach the compositor's socket can set the
  clipboard. Such a program can already start any other, `wl-copy`
  among them, so nothing is given that it did not have.
- What the launcher copies does not need `omorfos-clipboard` to keep it
  after the launcher ends: the compositor gives it until something else
  is copied.
