# 62. A change of compositor unseen

- Status: accepted
- Date: 2026-10-31

## Context

Under the relay (ADR 60, ADR 61) a compositor started anew keeps every
program, and since step 3f one is started anew by itself whenever its new
version is installed. Tried on real hardware, the change is plain to see:
the screens go black, come back with the desktop's colour and no wallpaper
or panel, and those come back a moment later. Three things make it so:

1. **The compositor going puts the console back.** Smithay, as it lets the
   graphics card go, sets the screens as they were before the compositor
   took them, which is the console or nothing.
2. **The compositor coming turns every screen off** as it takes the card
   (`DrmDevice::new` told to), and sets each mode anew, which a screen on a
   cable takes a second or two to follow.
3. **The compositor coming draws at once**, before the relay has made
   again what it kept, so its first frames show the session without the
   wallpaper, the panel and the windows.

## Decision

A change of compositor is made unseen in steps, each tried on real
hardware, the first being the one that can be tested without it.

1. **Nothing is shown until what was kept is back.** The relay tells the
   compositor `ready` once every program it kept for it has caught up and
   all of it has been written to the compositor, which is at once when it
   kept none. A compositor under a relay draws nothing until then, so the
   screens go on showing what they did, and its first frame has the
   wallpaper, the panel and the windows in it. One that is not told within
   a second shows the session regardless: the relay is not started anew
   with the compositor, so the one running may be older and never say it,
   and a frame without the panel is better than a screen that stays as it
   was.
2. **The picture before is kept on the screens through the change.** The
   compositor stepping down to be started again leaves the screens as they
   are rather than putting the console back; the one coming takes the
   screens as they are, and sets a mode only where it is not the one they
   are in. Only real hardware can show this, and it is tried there.
3. **The picture before fades into the session.** The compositor coming
   reads what each screen shows from the card, as it finds it, and draws
   it over the session's first frames, fading it out.

With all three, a change of compositor is a moment in which the screens do
not move, and then a fade.

### What is shown meanwhile

Nothing that was not shown already. Until the first frame, the screens
show what the compositor before left on them, or black; a session that was
locked comes back locked (ADR 60), and the compositor before showed the
lock. Input made meanwhile goes to the compositor coming, which places
each window where it was.

## Alternatives

- **Blue and green: the compositor coming started beside the one going**,
  made to catch up with every program out of sight while the one going is
  still used, and handed the screens once it has, so that nothing ever
  stands still. Two things stand in the way. Only one process may set what
  the screens show, so the one coming would draw on the card without
  showing it until the one going hands the screens over, which works.
  But a program speaks to one compositor at a time, through the relay, by
  one stream of numbers and serials: catching a second one up while the
  first is still used would have the relay feed both, keep what each says
  apart, and answer for the second as for the first. That is a relay of
  its own, several times the work of the three steps, for the second or
  so they leave the screens still. It is not ruled out: once the three are
  built and tried, the time a change of compositor stands still is
  measured, and whether blue and green is worth its cost is decided then.
- **Waiting for the panel and the wallpaper to draw anew**, rather than
  for the relay. What they drew is made again by the relay without their
  drawing, and a program that does not draw again for a while would hold
  the session back with it.
- **Waiting longer than a second for an older relay.** Every change of
  compositor under one would stand still for as long.

## Consequences

- The relay says one word more to the compositor, `ready`, which one that
  does not know it passes over, as it does every word it does not know.
- A change of compositor under an older relay waits a second before the
  session is shown, once, until the relay is started anew with the next
  login.
- Steps 2 and 3 touch the hardware backend, which no test runs (ADR 2);
  what they decide is kept out of it, and tested, as the rest is.
