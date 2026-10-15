# 43. What records, on the mark

- Status: accepted
- Date: 2026-10-15

## Context

The mark that says a screen is captured (ADR 37) says nothing of a
microphone or a camera, which a call or a page records from as much as it
shares a screen; and a screen shared through the portal named the portal,
not the program it was shared with. The compositor sees none of this:
programs record sound and pictures from a camera through PipeWire, or open
a camera themselves, and the portal's pictures go to the program they are
shared with through PipeWire as well.

## Decision

**A program of its own, `omorfos-recording`, watches PipeWire's graph and
tells the compositor what records, over a pipe only the two of them
have.** The compositor starts it with the session, as it does the panel,
and starts it again if it stops; nothing turns it off, since the mark is
not to be turned off.

- **What records** is a stream that takes something in, while it runs,
  from what is linked into it: a microphone (`Audio/Source`), the sound
  that plays (anything else of sound, as a speaker's monitor), a camera
  (`Video/Source`), or a screen shared through the portal. The portal's
  screen is a `Video/Source` as a camera is, and is told apart by saying
  it is a screen (`media.role` `Screen`): tried by hand at first, a screen
  shared with Firefox showed as a camera.
- **Who records** is the process PipeWire found the program's connection
  comes from, which the program cannot choose, as it can the name it
  gives itself. The compositor names it by the file it runs, as it does
  what takes pictures of the screen.
- **Through PulseAudio**, as most programs record sound, that process is
  PipeWire's layer that speaks PulseAudio's protocol, which runs the
  PipeWire program itself; tried by hand, Firefox's microphone was said
  to be `pipewire`'s. That layer passes on the process of the program
  it speaks for, and that is believed of a client only if it says it is
  that layer and its process runs the same file as PipeWire's own, which
  no other program can make true.
- **What it says** is a line for each, the kind and the number of the
  process, and a full stop once it has said everything; each time it
  changes. The compositor believes no more than 64 lines, none longer
  than 256 bytes, and what it does not understand it ignores.
- **The mark shows a sign for each kind**, side by side: a dot for the
  screen, a microphone for sound, and a camera. Sound and the camera show
  on every screen. What has stopped stays a moment, as a picture of the
  screen does, so that something brief cannot pass unseen. The tooltip
  says which program does what, and names a program shown a screen
  through the portal with the portal that shares it.

## Alternatives

- **Watch PipeWire in the compositor.** Nothing to start or restart; but
  the compositor would link `libpipewire`, which it does not need for
  anything else, would not start where it is missing, and would take
  PipeWire's messages in itself.
- **Have the portal say it.** The portal speaks to PipeWire already, but
  it can be turned off, and run nested it is not started at all; the
  mark is not to depend on either.

## Consequences

- While the watcher is not running, the mark shows pictures of the screen
  only. The compositor starts it again a few times if it stops.
- A program that opens a camera itself, and not through PipeWire, is not
  seen. Finding that means looking at which processes have the camera
  open, and is left for later.
- Taking in what a program shares is believed of PipeWire as it is: a
  program that records through another, or feeds a stream of its own
  into one, is named as PipeWire sees it.
