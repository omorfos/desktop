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
   took them, which is the console or nothing; and what it drew goes with
   the card: each picture Smithay lets go of is removed from the card,
   and so is every one left as the card is closed, which turns off what
   shows it.
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
   compositor stepping down to be started again hands the holder its
   card, open, with the word `showing`, gives the card up, and never lets
   go of what it drew with: the process ends straight after, and nothing
   is put back or removed as it does. The holder keeps the card open, and
   hands it to the next compositor, after the socket, telling it
   `showing`, as it hands it to its own new version where it takes one up
   in between. The next takes the screens as they are, rather than
   turning every one off first, which it does still when it is handed no
   card, as at login; drives each with what drives it already, where it
   may; and sets a mode only where the screen is not in the one it is to
   be in. Once each screen shows what it draws, or nothing, it closes the
   card handed to it, and with it what the one before drew, which no
   screen shows any longer; and as the user leaves for another virtual
   terminal, which takes the screens. A screen that cannot be drawn on as
   it was left, which the card may refuse, has every screen turned off
   and set anew, as at login, and the card let go, rather than the
   picture before left on it for good. Only real hardware can show this,
   and it is tried there.

   Tried on a laptop's own screen (Intel, Linux 6.12, systemd 257), the
   screen is set in no mode anew and the picture is kept, but not
   through the whole change: as the compositor before ends, logind,
   having no one in control of the session for a moment, takes the card
   from it and puts the virtual terminal back into text, and the
   kernel's console draws itself on the screen at once, for as long as
   the next takes to draw, about a fifth of a second. A card kept open
   does not keep the console off: only someone in control of the
   session throughout does, which the holder is to be (see "No one in
   control of the session" below).
3. **The picture before fades into the session.** The compositor coming
   reads what each screen shows from the card, as it finds it, and draws
   it over the session's first frames, fading it out.

With all three, and the session never out of anyone's control, a change of
compositor is a moment in which the screens do not move, and then a fade.

### No one in control of the session

logind lets one process at a time control a session, and between one
compositor letting go and the next taking it, the session has none: logind
takes the cards from the one going and puts the virtual terminal back into
text, and the console shows. Kept, the session would never be without one:
`omorfos-hold`, which outlives every compositor, takes control of it once,
and hands each compositor the cards and input devices it opens for it, as
logind does now, taking them back as one ends and as the user leaves for
another virtual terminal. Smithay takes its seat through a trait the
compositor may implement over its connection to the holder. It is a step
of its own, after the third, with a decision of its own.

### What is shown meanwhile

Nothing that was not shown already. Until the first frame, the screens
show what the compositor before left on them, black, or the console of the
virtual terminal the session is on, which shows what was typed there before
the session started; a session that was
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
- **Closing what the compositor drew without taking it off the screens**,
  as Linux 6.8's `DRM_IOCTL_MODE_CLOSEFB` does, which was made for a hand
  over such as this one. Neither `drm` nor `drm-ffi` offers it, and called
  directly it needs `unsafe`, which our crates forbid. The card kept open
  by the holder does the same with what there is, on any kernel.
- **Leaving the screens as they are, and no more.** Not putting the
  console back is not enough: the card closes as the compositor ends, and
  what it drew is removed with it.

## Consequences

- The relay says one word more to the compositor, `ready`, which one that
  does not know it passes over, as it does every word it does not know.
- The compositor says one word more to the holder, `showing`, with its
  card; and the holder tells the next one more line, with the card after
  the socket. A holder that does not know the word reads past it and the
  card it came with closes; a compositor that does not know the line
  takes the socket and drops the card. Either way the screens go dark
  through the change, as before.
- The holder, and then the next compositor, hold the card of the one
  before, which is no longer the card's master: it cannot set what the
  screens show, and is not handed to anything they start. With it, they
  could read what the one before drew, which was the session they hold.
- What Smithay puts back on the screens as the session ends is what they
  showed as the compositor took them; taken as the one before left them,
  that is its pictures, which are gone by then. The console comes back as
  the kernel brings it back once the card is let go of, rather than as
  Smithay would have set it.
- The first change of compositor after this is installed is not yet
  unseen: the holder running, of the version before, does not know
  `showing`, and only takes up its new version after it has closed the
  card. From the next change on, both know it.
- A change of compositor under an older relay waits a second before the
  session is shown, once, until the relay is started anew with the next
  login.
- Steps 2 and 3 touch the hardware backend, which no test runs (ADR 2);
  what they decide is kept out of it, and tested, as the rest is.
