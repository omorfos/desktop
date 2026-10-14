# 35. Where omorfos meets the rest of the system

- Status: accepted
- Date: 2026-10-13

## Context

A desktop is asked for things a compositor knows nothing of: to lock
before the machine sleeps, to say how full the battery is, to join a
network, to ask for the administrator's password, to let a browser share
the screen. On Linux each of these is a service that is spoken to over
D-Bus: logind, UPower, NetworkManager, polkit, the portals.

Today the compositor speaks to none of them. It takes its seat through
libseat, and tells D-Bus and systemd where the session is by running one
command (ADR 13). `omorfos-notifications` is the only program here that
is on D-Bus at all.

The danger in adding them is that the compositor comes to depend on all
of them. It is the one program that must not hang, must not crash, and
must be small enough to review. A call over D-Bus that is not answered
is a session that has stopped.

## Decision

### The compositor speaks to no service

It speaks Wayland to programs, its own IPC to components, and libseat
for the seat. Nothing in it waits on D-Bus. What it has to be told by
the system, a program of ours tells it, in a protocol it already speaks.

### Each service has one program of ours that speaks to it

| Service | Spoken to by | For |
|---|---|---|
| logind | `omorfos-session` (new) | Locking when asked and before sleep; sleep, restart and shut down; the brightness of the screen |
| systemd's user manager | `omorfos-session` | Saying that a graphical session is up, so that services which wait for one start |
| Notifications | `omorfos-notifications` | As now |
| polkit | `omorfos-ask` (new) | Asking for the administrator's password |
| GnuPG and SSH, which ask through pinentry | `omorfos-ask` | Asking for a passphrase |
| The portals | `xdg-desktop-portal-omorfos` (new) | Settings told to applications; screenshots and sharing the screen |
| UPower | the bar | The battery |
| NetworkManager | the bar; `omorfos-ask` for a network's password | The network |
| The system tray's watcher | the bar | Programs' icons |
| PAM | `omorfos-lock` (new) | Knowing the user's password is right |

Each is a program of its own for the reason every component is (ADR 18):
one that hangs or is fooled is one part missing, not the session.

### `omorfos-session`: what a session does besides draw

Started by the compositor with the others and started again if it stops
(ADR 28). It decides nothing about windows and draws nothing.

- **Locks when asked.** logind says "lock" (`loginctl lock-session`, or
  another program asking), and it starts the locker.
- **Locks before sleep.** It holds logind's leave to delay sleep. When
  told the machine is about to sleep it starts the locker, waits until
  the compositor says the session is locked and a locked frame has been
  shown (ADR 14), and only then lets sleep go ahead. logind waits a few
  seconds at most and then sleeps regardless. A locker that has not come
  up by then leaves the machine to wake unlocked, which is the worst
  this design can do: so the locker is started first of all, how long it
  takes to show is measured, and waking unlocked is said to the user as
  a problem.
- **Is never told to unlock.** logind can say "unlock". It is not
  heeded. A locked session is opened by its user's secret and nothing
  else.
- **Locks and sleeps when the user is away**, after the times the
  configuration gives, going by the compositor's own notice of idleness.
- **Sleeps, restarts and shuts down** when the menu asks, by asking
  logind, which decides whether this user may. `omorfos-session sleep`
  and the like are what the menu's entries run.
- **Leaves the lid and the power button to logind**, which handles them
  well and is where an administrator expects to set them. What omorfos
  adds is being locked by the time they take effect.
- **Starts what is to start with the session**: the programs in
  `~/.config/autostart` and the system's, once, as `autostart` in the
  configuration is.
- **Says the session is up** to systemd's user manager, where there is
  one, and that it is over when it ends. ADR 13 left this out until
  something needed it; portals do.
- **Warns when the battery is low**, as a notification.

### What asks for a secret, and what checks one

- `omorfos-ask` shows one prompt over a dimmed, blurred screen, and is
  what polkit, pinentry and later a network's password all come to. It
  never learns whether the secret was right for polkit: polkit's own
  helper is handed what was typed and answers.
- `omorfos-lock` is the lock screen. It asks PAM, so that whatever the
  system uses to know its user, a password or a finger, is what unlocks.
  PAM is a C library; it is reached through a crate that wraps it, since
  no `unsafe` is written here, and that crate is read before it is
  taken.
- How these are kept from being imitated, and who else may take the
  whole screen, is the next decision but one ("who may do what").

### Portals

The portal service itself is not ours; applications speak to it. It
passes each request to a backend, and `xdg-desktop-portal-omorfos` is
ours, for what only omorfos can answer:

- **Settings**: light or dark, the accent, reduced motion, high
  contrast (ADR 32), so that applications match.
- **Screenshot and sharing the screen**, with the compositor's capture
  protocol behind them and the user asked each time.

What any desktop can answer is left to a backend that exists, by the
file that says which backend serves what: choosing a file, above all,
until there is a file manager here to do it.

### Rules for all of them

1. **Everything is optional.** A machine without logind, UPower or
   NetworkManager has no sleep in the menu, no battery, no network on
   the bar. Nothing fails; the part is not there. Which services are
   there is found out by asking the bus, at start and when they come and
   go.
2. **Nothing of ours runs as root, and nothing is setuid.** What needs
   more than the user may do is asked of the service, which asks polkit.
3. **What comes over D-Bus is untrusted input**, as a Wayland client's
   requests are: checked, bounded in size, and never able to make a
   program wait for ever. Every call has a time after which it is given
   up.
4. **One D-Bus library**, `zbus`, which `omorfos-notifications` already
   uses.
5. **Tested against the real protocol.** The tests start a bus of their
   own, as the notification tests do, and a stand-in for the service
   written with the same library. A stand-in logind says "about to
   sleep" and the test sees the session lock before it is let go.
6. **Logic apart from plumbing**, as in every component: what to do when
   told the machine will sleep is a value made from what is known, and
   is tested without a bus.

## Alternatives

- **The compositor speaks to logind itself.** Fewer programs, and what
  several compositors do. Then a bus that does not answer is a frozen
  screen, and the largest dependency in the tree is in the one program
  that must stay small.
- **Use what exists**: `swayidle`, `swaylock`, a polkit agent from
  another desktop, a portal backend for wlroots. This is what a user of
  omorfos does today and it stays possible, since every one of ours can
  be replaced. But each looks like something else, each is configured
  somewhere else, and the lock screen and what asks for passwords are
  the two things that most have to be the desktop's own to be trusted.
- **One daemon for all of it.** Simpler to start. But the battery and
  the network belong to what shows them, and what holds secrets should
  hold nothing else.

## Consequences

- Four new programs, each small: `omorfos-session`, `omorfos-lock`,
  `omorfos-ask`, `xdg-desktop-portal-omorfos`. Each goes into
  `release-please-config.json` and the packages when it is written.
- The compositor's IPC gains what they need to be told: that the session
  is locked, and that a locked frame has been shown.
- ADR 13 said "no session manager", and the compositor is still the
  session: it starts `omorfos-session`, not the other way about. What
  that ADR left undone, the session target and programs to start, is
  done here.
- The bar comes to speak to three services. It is the component most
  exposed to the rest of the system and is written to lose any of them
  without stopping.
- A wrapper for PAM is a dependency with `unsafe` inside it, taken with
  care for the one program that needs it.
- Sound is not decided here. Volume keys and the bar's volume speak to
  PipeWire, and how is decided with that item.

## What was built first, and what it settled

`xdg-desktop-portal-omorfos` came first, answering how the desktop looks.

- **It ends with the session.** The bus belongs to the user, not to the
  session, and outlives it; a program that only spoke to the bus was
  left answering for a session that had ended, holding the name the
  next one needed. So it keeps the connection the compositor hands it
  and ends when that closes, as the other components do.
- **A new copy takes the name over.** Should one be left over all the
  same, after a crash say, the new one is given the name and the old one
  is told and stops, rather than the new one being refused it.
- **systemd is told when the session begins and ends**, by
  `omorfos-session.target`, which brings up its graphical session. The
  portal services belong to that, so they stop when the session does.
  Left running, they outlived it: the GTK backend lost its display and
  was started again without one, which left a start pending on the bus
  that the next session's portal waited on until it was killed, and
  every GTK application with it. The target is started again at login
  in case a session ended without stopping it.
- **The portal service is started again once ours has its name**, if it
  is running by then. Started a moment before, it gave the Screenshot
  interface to another backend for the whole session.

**The question is the backend's own.** Asked for a picture, it puts a
question in the middle of the screen, over everything and with the
keyboard, and only once the user has said yes takes the picture, after
taking the question down. The compositor starts it able to do both, and
nothing else is trusted with either. It learns what the compositor
offers before it takes its name on the bus, so that it is never asked
something before it knows whether it can ask the user.
