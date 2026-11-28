# 116. A window is moved and resized by the keyboard

- Status: accepted
- Date: 2026-11-28

## Context

Finding 4 of the review of 26 November 2026 (#180): a window is moved
and resized only by dragging, by an edge 7 units thick or a titlebar, and
its menu opens only at a right press on its titlebar. Nothing in `Action`
moves a window, resizes one or opens its menu, so a person who cannot
drag, or uses no pointer, cannot do any of them (WCAG 2.1.1, 2.5.7).

The finding asks for more: the line between tiles, tabs, the picker's
region and a key for the notice. Those are changes of their own, after
this one, each ticked on the finding's table as it comes.

## Decision

**Three actions, `move`, `resize` and `window_menu`, with no default key,
and rows Move and Resize in a window's menu.**

- **`move`** takes the focused window, if its frame could be dragged:
  not maximized, full screen or filling a part of a workspace. The arrows
  then move it 10 units a press, 50 with Shift held; Enter keeps where it
  is, and Escape puts it back where it was. A click anywhere keeps it too,
  and so does the window losing the keyboard, to another window or to the
  lock screen. The keypad's arrows do as the arrows do. Nothing else
  reaches a window meanwhile, no other key and no binding, but for the
  modifiers, which the window is told of still so that none is left held.
- **`resize`** is the same, the arrows changing its size instead: Right
  and Down make it wider and taller, Left and Up narrower and shorter,
  its top left corner staying where it is, and never smaller than a
  drag would let it be.
- **`window_menu`** opens the focused window's menu at its top left
  corner, where a person using the keyboard is looking, rather than at
  the pointer.
- **The window's menu** has Move and Resize, below what it had, while its
  frame could be moved; chosen from the menu, they begin as the actions
  do.
- **No default key**: ADR 101's review of the defaults stands, as it did
  for `show_capturing` and `focus_panel`; the guide says how to bind
  them. GNOME's Alt+F7 and Alt+F8 are the model.

## Alternatives

- **Resize choosing an edge first**, as Mutter does: closer to a drag by
  an edge, at the cost of a step a person has to know of; the corner that
  stays where it is answers most of what resizing is for.
- **Moving a tiled window between parts by the arrows**: that is the
  tiles' own change, with `grow` and `shrink` for the line between them.
- **Default keys on Super+F7 and Super+F8:** zero configuration for the
  keyboard alone, against the review of the defaults.

## Consequences

- While a window is being moved or resized by the keyboard, every key but
  the modifiers is the compositor's until Enter, Escape, a click or the
  window losing the keyboard. Nothing on the screen says the mode is on
  but the window moving; a sign of it is left for when it is asked for.
- A window moved by the keyboard so that most of it is on another screen
  belongs there, as one dragged does.
