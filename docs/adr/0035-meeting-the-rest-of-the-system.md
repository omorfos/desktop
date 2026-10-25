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
- **Sleeps when the user is away**, after the time the configuration
  gives, when the compositor asks it to as the menu does. The compositor
  itself locks when the user is away: it is what knows they are, and
  what starts the locker whichever way the session is locked, so nothing
  is gained by its asking another program to ask it back. Sleep needs
  logind, and so goes through this program, which locks the session
  first as it does for any sleep.
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
- **The portal service is started afresh once ours has its name**, and
  the GTK backend before it. Started a moment before, the portal gave the
  Screenshot interface to another backend for the whole session. Started
  after the last session ended, by a program finishing up (Firefox does),
  it started a GTK backend that found no display, and the bus was never
  told that start had failed: for two minutes it held every request for
  the GTK backend, the next session's portal hung on it and was killed,
  and applications lost their settings and the screen to share. A backend
  that takes its name ends a start pending for it, so starting both
  afresh, whether or not they run, leaves nothing of the kind behind.

**The question is the backend's own.** Asked for a picture, it puts a
question in the middle of the screen, over everything and with the
keyboard, and only once the user has said yes takes the picture, after
taking the question down. The compositor starts it able to do both, and
nothing else is trusted with either. It learns what the compositor
offers before it takes its name on the bus, so that it is never asked
something before it knows whether it can ask the user. It names the
application that asks as its desktop entry does, found by the identifier
the sandbox gives it, which is looked up only if it is a plain name that
cannot reach outside the directory of entries; and by that identifier if
there is no entry.

**A colour is picked, not asked about.** Asked for a colour on the
screen, the backend takes a picture of the first screen and shows it
still, over all of that screen and with the keyboard: a click of the left
button, or Enter, gives the colour under the pointer, and Escape or
another button gives none. Over it the pointer is a crosshair, and a
round loupe beside it shows the fifteen pixels each way round it,
larger, with the one it would give outlined; the loupe is a surface of
its own over the picture, so that the pointer moving paints it again and
not the whole screen. Nothing is asked first, as GNOME and KDE ask
nothing: the application learns the one colour the user chose, by
choosing it, which is the consent; asking as well would be asking twice.
The picture is taken before anything of the backend's is up, so that it
is not in it, and shown pixel for pixel, so that the colour is the
screen's. A question or a picture is each put up anew, so that nothing of
the one before, its shape or where the pointer was on it, carries over.

**Sharing the screen goes through PipeWire, from the backend.** The
backend takes pictures of the screen as the compositor gives them, each
once something on it has changed, and puts each in a PipeWire stream of
its own, whose number the application is told. The compositor knows
nothing of PipeWire. The `pipewire` crate binds libpipewire, which needs
its headers and clang to build, and the stream is filled through its safe
calls alone: PipeWire makes the memory, which is why it is copied into
rather than given. Copying is the first way, and buffers of the GPU, which
copy nothing, are to come. The tests run a PipeWire of their own, and read
the stream as an application would.

**`omorfos-session` came next, to lock before sleep.** Run with no
command, as the compositor runs it where it is the whole session, it
holds logind's leave to delay sleep, and when told the machine is about
to sleep asks the compositor to lock, as anyone may over the socket for
scripts. The compositor's state says `locked` once a locker holds the
session and a frame with only the locker on it has been shown on every
screen, which is what is waited for; then sleep is let go, and on waking
the leave is taken again.

- **The locker is the one `config.yaml` names**, started by the
  compositor with the role to lock, until `omorfos-lock` takes its
  place. With none named, the compositor refuses the request, and the
  machine sleeps at once: nobody asked for it to be locked, and holding
  it awake for nothing helps nobody.
- **A screen that is off shows the lock.** Once the user has been away
  long enough for the screens to be turned off, nothing is drawn, so a
  lock asked for then would never be shown, and the machine would be
  kept awake to the end of logind's patience and said to have slept
  unlocked. A screen that is off shows nothing of the session, and the
  backend says when every screen is; the locker is then told the
  session is locked at once, and its picture is the first drawn when the
  screens come back on.
- **logind is waited on as long as it waits**, which it says
  (`InhibitDelayMaxUSec`), five seconds unless set otherwise. A locker
  that has not locked by then leaves the machine to sleep unlocked, and
  a warning says so, to be read on waking.
- **Only logind is heard.** The bus delivers what is said in its name
  only from whoever has the name, so another program cannot make the
  session lock, or think the machine has woken. `Lock` is heeded for
  this session alone; `Unlock` never is.
- **The lid and the keys for sleep and for turning off are not bound.**
  logind acts on them itself (`HandleLidSwitch`, `HandleSuspendKey`,
  `HandlePowerKey`), so a binding as well would sleep twice, or ask to
  turn off while the machine was already turning off. What they do is
  set where an administrator expects, in `logind.conf`, and whatever
  does it, the session is locked first. `omorfos-session sleep`,
  `restart` and `shut-down` ask logind, interactively, so that polkit
  can ask where it must, and are what the menu's entries are to run.
- **It ends with the compositor**: when the state it follows stops, the
  session is over. Without logind, no system bus or nobody with
  logind's name on it, it ends at once, there being nothing to lock
  before. Anything else is a failure, and it is started again: a logind
  that does not answer in time, as on a machine busy starting up, which
  would otherwise leave the whole session without locking before sleep,
  or a bus that goes away while it runs. It listens to logind before it
  holds sleep back, so that nothing said in between goes unheard.
- **Nested, it is not started**: the machine's sleep is the outer
  desktop's to lock before.

**What starts with the session is started by a command of
`omorfos-session`**, `autostart`, which the compositor runs once at
login, after the programs `autostart` in the configuration lists, and
only where it is the whole session. A command run once, and not the part
that runs all session, because that part is started again when it fails,
and would start everything again with it. It reads the desktop entries
of the user and of the system as the launcher reads its own, with what
the two share now in `omorfos-entries`, starts each as an ordinary
program and ends; the programs it started go on without it. The session's
environment was already told to D-Bus and systemd at login, before any
of them starts.

Programs started so outlived the session at first: one that never
speaks to the compositor is told of nothing that ends, and GeoClue's
agent, still running from one login, would not start at the next. Where
systemd's user manager runs, each program the configuration or the
autostart entries list is started by `systemd-run --user --scope`, in a
scope of `app-graphical.slice` that is part of `graphical-session.target`,
and so stops when the session's target does. It is the same process, which
`systemd-run` becomes, so a role handed to it over its standard input
reaches it. Without the user manager each is started as before.
