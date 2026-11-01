# 63. The holder keeps the seat

- Status: accepted
- Date: 2026-11-01
- Builds on: [60. The session outlives the compositor](0060-the-session-outlives-the-compositor.md),
  [62. A change of compositor unseen](0062-a-change-of-compositor-unseen.md)

## Context

A compositor takes the seat, its screens and input devices, through
libseat, which on most systems asks logind. logind lets one process at a
time control a session, and gives it the devices it opens: the graphics
card as its master, the keyboard and mice to read. When the compositor
ends, logind takes them back and puts the virtual terminal back into text,
and the next compositor takes control anew.

Tried on real hardware (ADR 62), that moment between the two is seen:
with no one in control, the kernel's console draws itself on the screens,
for as long as the next compositor takes to draw, about a fifth of a
second. Keeping the card open, as step 2 of ADR 62 does, keeps the
picture but does not keep the console off.

## Decision

**`omorfos-hold` takes the seat once, and keeps it while compositors come
and go.** It opens it as it starts, before the first compositor, through
Smithay's libseat session: Smithay owns the descriptors libseat gives out,
which the `libseat` crate leaves to its caller, and our crates have no
`unsafe` to close them with. The holder then controls the session for as
long as it runs, so logind never puts the virtual terminal back into text
between two compositors.

**Each compositor takes the seat from the holder.** With what it hands
over, the holder tells it `seat`, the name of the seat, and hands it one
descriptor more, last, after the socket and any card (ADR 62), so that a
compositor that does not know it takes nothing else for it: the
compositor's end of a connection of their own for the seat. Over it, the compositor asks, and the holder
answers:

- `open PATH`: the holder opens the device through libseat and answers
  `opened ID` with its descriptor, or `refused ERRNO`;
- `close ID`: the holder closes it;
- `switch N`: the holder asks for the virtual terminal `N`.

The holder says, of its own accord, `paused` as the user leaves for
another virtual terminal, having let logind have the devices, and
`active` as they come back; the first thing it says is which the seat is,
and the compositor opens nothing until it is active. The compositor
implements Smithay's `Session` over this connection, so that the graphics
card and libinput take their devices from it as they take them from
libseat now; what the holder says while the compositor waits for an
answer is kept, and heard after.

**As a compositor ends, the holder closes every device it opened for it.**
logind takes master from the card and stops the input devices, as it did
when the compositor ended, but the virtual terminal stays as it is, the
session being the holder's. The holder keeps the first card it opened
for the compositor open, as the compositor handed it over with `showing`
(ADR 62), and so whether the compositor stepped down or failed: what it
drew stays on the screens, and the kernel does not bring its console back
as it does once a card is closed by everyone. The next compositor opens
the card anew, as master, and takes the screens as they are. Neither
taking master away nor closing a card that is still open elsewhere puts
the console back, in the kernel (Linux 6.12) or its Intel driver: only
the virtual terminal going back into text, or every file of the card
closing, does.

**While no compositor runs**, the holder answers logind itself: a pause is
let happen at once, and the next compositor is told the seat is paused.

**Where the holder does not keep the seat, the compositor takes its own**,
as before: the holder could not take one, as where it is run to be tried
by hand; or it starts the version of the compositor held from before
(ADR 60), which may be older than this, and takes the seat again only
with the next login, as it does the relay (ADR 61); or it is an older
holder, which says nothing of a seat.

**Taking up its own new version, the holder lets the seat go first**: what
libseat holds cannot be handed to another program. The new version takes
the seat again before it starts the next compositor; the console shows
for that moment, once, as before. Installing puts a new file in place of
the old whether the holder changed or not, so the holder takes up the file
installed only where what is in it is not what it runs: an installation
that did not change the holder leaves it, and the seat, as they are.

## Alternatives

- **The next compositor taking control from the one going**, by force,
  which logind allows and which would not put the virtual terminal back
  into text. libseat does not ask for it, and the compositor going would
  have to stay until the next had, while two compositors serve the same
  programs.
- **The holder opening the card itself**, to be its master between two
  compositors. Only logind may make a card's master another than the
  process that first opened it, and the holder is not logind.
- **One card opened for every compositor in turn**, handed on rather than
  opened anew. What each compositor made on it, which only closing the
  card frees, would be kept for as long as the session lasts.

## Consequences

- The holder links Smithay, with only its libseat session, which adds no
  crate to those already built.
- What the holder and compositor say to each other about the seat is a
  surface between versions, as the rest of what they say is (`docs/IPC.md`):
  lines are added, never changed, and either passes over one it does not
  know.
- The holder is what logind knows as the session's controller; a holder
  that fails ends the session, as it did already. It opens for a
  compositor only what libseat would have opened for it, and only on the
  seat it was given.
- Whether the console stays off between two compositors, and the picture
  on, can only be seen on real hardware, and is tried there; the protocol,
  and what the holder does with a compositor's devices, is tested, with a
  seat of the tests' own, and the holder run as a program takes libseat's
  `noop` seat, which controls nothing, in the tests, never the one of the
  session they run in.
