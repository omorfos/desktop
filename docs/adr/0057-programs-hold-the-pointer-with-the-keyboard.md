# 57. A program holds the pointer only while its window has the keyboard

- Status: accepted
- Date: 2026-10-25

## Context

Games turn by the mouse with the pointer locked in place, and remote
desktops and virtual machines keep it within their window: they ask for it
by `pointer-constraints`, and read how far it moved by `relative-pointer`.
The protocol leaves it to the compositor when a constraint takes hold:
"the compositor may require the surface to have keyboard focus", or any
other condition it chooses.

A pointer that is locked cannot be moved off the program that holds it,
and so cannot click anything else. If a program could take hold of it by
the pointer only passing over its window, any window in the background
could, and the user would have no pointer until they found a key that
helps.

## Decision

A program holds the pointer only while its window has the keyboard and
the pointer is over the part of its surface it asked for. A window that
loses the keyboard, by a key that switches windows or a workspace, lets
the pointer go at once, and holds it again when it has the keyboard back
with the pointer still over that part. The keyboard is never held: the
compositor's keys work as ever.

Only a pointer that moves by a distance, as a mouse or a touchpad does,
is held. One that is given a position, as a tablet gives, goes where it
is put. Confined, the pointer goes as far as it is moved within the part,
or along the edge it meets, or stays. Where a program says the pointer
would have moved to while it was locked, the pointer is left there when
it is let go. While a program holds the pointer, an edge of a fullscreen
workspace brings nothing back (ADR 36): it is the program's pointer
against it, not the user asking.

## Consequences

- The user can always get the pointer back with the keyboard, and a
  window they are not using cannot take it.
- A program that asks for the pointer before its window has the keyboard
  waits for it, as a game started in the background does.
- The distances programs are told are timed to the thousandth of a
  second the backend gives the compositor, though the protocol carries
  millionths.
