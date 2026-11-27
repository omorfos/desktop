# 112. A time limit can be turned off, set, or put off by being there

- Status: accepted
- Date: 2026-11-27
- Takes up: [ADR 88](0088-restart-and-shut-down-ask-first.md), for how long
  the question before leaving waits, and
  [ADR 101](0101-super-drives-the-desktop-and-the-ordinary-way-is-a-setting.md),
  which kept that minute fixed

## Context

A review of the design for accessibility
(`docs/reviews/2026-11-26-accessibility-design-wcag.md`, #180) found that
times a person meets cannot be changed (finding 7). WCAG 2.2.1, at level
A, asks that a time limit can be turned off, set over a range of ten
times, or put off when it is about to run out.

The one that matters is the question before restarting, shutting down
or logging out: left alone a minute, it goes ahead. A press that slipped
raises it, and someone who has not found the box, read it and reached
Escape within the minute loses what was not saved. ADR 88 gave the
reason for the minute, that whoever walked away from a machine being
turned off meant it to go off, and ADR 101 kept it fixed.

The others: the question whether to keep the screens as the display key
has them puts them back after 15 seconds, and has no buttons; a
notification goes after its time, under the pointer as much as not, and
a sender's time beats even `notifications.timeout: 0`, "until it is
dismissed"; two presses count as one within 400 ms, fixed; and the lock
screen empties its field after 30 seconds, held by keys and buttons but
not by the pointer.

## Decision

**Each time limit is turned off, set, or put off by being there, as the
maintainer decided on 27 November 2026.** Being there is the compositor's
own input, a key, a button or the pointer moving, where it tells programs
waiting for the user to be away that the user is not: no program can hold
a count open by what it draws.

- **The question before leaving** waits `power.wait` seconds, 60 unless
  set, from 0 to 3600; and any key, or the pointer moving, starts the
  count again. With 0 there is no count, and it waits until it is
  answered, saying nothing of seconds. Whoever walks away from a machine
  being turned off still has it go off, as ADR 88 wanted; whoever is still
  at it, finding the box, is never cut short.
- **The question whether to keep the screens** has buttons, Put back and
  Keep, answered as ADR 111 has every box that asks answered, the
  keyboard starting on Put back. Being there starts its 15 seconds again,
  but never past two minutes after it was asked: it is how the screens
  come back after a way of showing them that shows nothing, and a hand on
  a mouse in front of a dark screen is there too. **Escape puts them back
  at once,** from wherever the keyboard is, however soon or late. No
  setting: a setting that turned it off could leave a dark screen dark.
- **A notification** stays while the pointer is on it, and goes its time
  after the pointer leaves. The user's `notifications.timeout` is the
  shortest a notification is shown, a sender's own time making one last
  longer and not shorter, and `0`, until it is dismissed, beats any
  sender's time. A critical one stays until it is dismissed unless its
  sender set a time, and then for at least the user's. One that shows a
  level, as the volume and the brightness do, keeps its sender's time: it
  says what the key just pressed did, and is not there to be read.
- **Two presses count as one** within `mouse.double_click` milliseconds,
  400 unless set, from 200 to 2000.
- **The lock screen** counts the pointer moving as being there, as it
  counts keys and buttons, once it is showing its field; moving the
  pointer over the time alone does not wake it. Its 30 seconds stay
  fixed: emptying the field is for when someone walks away mid-password.

Left as they are: the volume and the brightness shown for a second and a
half, a tooltip's half second and a full screen's bar.

## Alternatives

- **No count before leaving,** as the portal's question has none: the
  simplest, and nothing is ever done for the user; but "Shut Down" and
  walking away would no longer turn the machine off, which is why ADR 88
  has the count.
- **The count ending in Cancel,** as the lock screen's own question does
  after thirty seconds: safe, and the same loss.
- **The screens' count put off without end:** a dark screen with a mouse
  being moved in front of it might never come back by itself.
- **The screens' count fixed,** buttons added alone: allowed, the limit
  being essential, but the one person reading slowly is cut short for the
  sake of the dark screen, which two minutes still brings back.

## Consequences

- Two settings, `power.wait` and `mouse.double_click`, each a line added
  to what the schemas promise, not a breaking change.
- A notification a sender meant to last a second lasts the user's
  timeout, five seconds unless set; but a level, which is what a second
  is for, does not.
- A history of notifications, to read one again that went, is a feature
  of its own and on the roadmap, apart.
