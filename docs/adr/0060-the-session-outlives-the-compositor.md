# 60. The session outlives the compositor

- Status: accepted
- Date: 2026-10-26
- Amends: [13. Login session](0013-login-session.md) and
  [59. New versions taken up in the session, but for the compositor's own](0059-new-versions-taken-up-in-the-session.md)

## Context

ADR 59 takes up a new version of each program that comes with omorfos as
it is installed, and stops at the compositor: it "cannot be swapped under
its windows". What a window is lives in the compositor's memory, and a
program whose compositor goes closes, Qt 6 programs apart, which connect
again if the socket is where it was. So a fix to the compositor, however
urgent, waits for the user to log out, and a compositor that crashes takes
the session with it, every window and whatever was in it. No Wayland
desktop that ships today does otherwise.

More was asked for: that the user go on watching, writing or playing
while a fix to omorfos, the compositor included, lands within minutes of
being installed, and that omorfos find the way where the rest of the
system has not.

Two facts shape the answer.

- **An update and a crash are one event** to the session: the
  compositor's process ends. Whatever lets the session live through the
  one lets it live through the other, and an update is then a matter of
  choosing the moment.
- **What has to outlive the process is small, and can be named:** the
  socket programs connect to, each program's connection and the state of
  the protocol on it, the screen and the input devices, and the X
  server's window manager. All else is the compositor's own and can be
  made again.

Wayland was designed on the understanding that the compositor does not go
away: there is no connecting again in the protocol, and some of what
passes over it has no faithful replay, a drag in flight or a key held
down at the moment. So "no program notices" cannot be promised in full.
"Almost none, almost ever" can.

## Decision

**The compositor is allowed to stop.** The session is no longer the
compositor's process (ADR 13) but what outlives it; the compositor
becomes a program that is started again, with its windows kept, whenever
it ends, by a crash or because a new version was installed and the moment
was right.

It is done as three programs in place of one, in the order below, each
worth having by itself.

### The holder: `omorfos-hold`

A small program, the one the login screen starts. It holds what must not
be lost, and decides nothing.

- **The socket programs connect to.** The holder binds it, under the
  session's name, and hands it to the compositor, which accepts programs
  on it. The name holds for the whole login, so what D-Bus and systemd
  were told at login (ADR 13) stays true, the IPC socket, named after it,
  stays where it was, and a program that can connect again finds the
  session where it was.
- **The compositor.** Started with the socket, and started again when it
  ends. A new version installed meanwhile is the one started. The version
  that was running is kept open as a file, so that a new one which does
  not come up is followed by the one that did, and the user told, as a
  configuration that does not load is followed by the last that did
  (ADR 3).
- **Whether the session is locked, and which X display it has.** Two
  words, said by the compositor as they change. A compositor started
  while the session was locked comes up locked, and starts the locker
  before anything else is shown: a crash never unlocks. It asks for the
  X display number the session had, waiting for it as ADR 27 does when
  the X server is started again, so that `DISPLAY` stays right wherever
  it was told.
- **Nothing else.** No configuration, no policy and no service. The
  compositor goes on starting the components, the X server and what
  starts with the session (ADR 35). The components and the X server end
  with it and are started again by the next; what starts with the
  session is started by the login's first compositor alone, which the
  holder tells it, since a program that outlived the compositor must not
  be started twice.

The holder is told a few words by the compositor over a pipe: that it is
up and serving the socket, that the session is locked or unlocked, which
X display it has, and that it is stepping down. The pipe is the
compositor's alone, closed in everything it starts, so that no program it
started can say the session is unlocked. The holder reads the words as it
would anything a program says, and passes over one it does not know: a
compositor and a holder of different versions meet whenever one is
installed, so words are added and never changed, and are written down in
`docs/IPC.md` beside the socket's protocol, the other surface between
versions.

What the compositor's ending means is what ADR 28 says of any program's.
One that ends without failing, and had not said it was stepping down,
has ended the session, as logging out does, and the holder ends with it.
One that said so, or that failed, is started again, from the place the
holder was told at start, beside itself, and from nowhere else. A start
that follows a failure waits a second, for the seat to be let go. One
that ends before it has said it is up did not come up. After three
failures in a minute, as ADR 28 counts them, the version held open is
started in its place, if it is another file, and the user told; when
that fails as often, or there is no other, the login screen has the user
back. A session that keeps nothing up is one to leave, not to loop in.

The holder is written to hold more than one program, the compositor
today and the relay beside it later, so that the relay's coming changes
what the holder is handed and not what it is. It looks for its own new
version each time it is about to start a compositor, and takes it up
first: it checks that the new file starts at all, as a child asked only
to say so, then starts it in its own place, keeping the process the login
screen watches, and hands it the socket, the files it holds open, its
own among them, and what it has been told. A new holder that cannot take
what it is handed starts the old one back the same way. There is no
quiet moment to wait for: nothing on the screen is the holder's.

`omorfos` started on the hardware with no holder above it, from a console
or by a login screen that still runs it, starts the holder in its own
place, and the holder starts it anew under itself. A console and the
login screen behave the same, as ADR 13 wants, and the session entry
stays as it is. Nested, in a window of another desktop, there is no
holder, as before.

It is ordinary, covered code, tested by running it: with stand-ins for
the compositor, as the tests of ADR 59 run stand-ins for the programs,
and with the compositor itself for the handing over of the socket.

**A swap is asked for, never made, until programs are kept.** With the
holder alone, a compositor started again is one whose programs have
closed, Qt 6's apart. So nothing asks the compositor to step down but
the user, by an action, for whoever will take that cost: a developer
with nothing open, who installs a build and tries it without logging
out. ADR 59's word that the compositor's new version is used at the next
login stands until the relay is there.

### The relay: `omorfos-relay`

The program that programs connect to, in the compositor's place: the
holder hands it the socket. It keeps every connection and the state of
the protocol on it, in a form made to be replayed: which objects a
program has made, of what and with what; what each surface shows, a
buffer it holds a descriptor of; what has focus; which serials have been
used, which are its own, so that a program's do not change under it. To
the compositor it is one connection per program, so that a role (ADR 37)
and a sandbox (`security-context-v1`) are still a connection's and no
other's. When the compositor ends, the relay connects each program again
to the new one, makes its objects anew, hands over its buffers and says
what had focus. The program is sent nothing it would not have been sent,
and learns nothing.

Pixels never pass through it: a buffer is memory shared by a descriptor,
in shared memory or on the card, and the relay passes the descriptor.
Each request and event is read and written once more, a wakeup and tens
of microseconds against frames of several milliseconds; an idle desktop
sends no messages, so the relay does no work. It lives by the rules the
IPC socket does (ADR 8): nothing waits, every buffer is bounded, a
program that breaks a limit is dropped.

It is a second reader of what untrusted programs send, before the
compositor's, and is fuzzed from the first. It knows the protocols
omorfos offers and no others, from a list the compositor keeps in code:
each global it offers, with how a program's state in it is replayed, or
that there is none, and a test that what the compositor offers is that
list and nothing else. From now on a protocol is offered by being added
to the list with its note, so that the list is never a retrofit.

Its own new version is taken up as the holder's is: it hands its
connections, and its state, which is in a form to be handed over already,
to the new one, and ends.

With the relay there, the compositor is swapped under its windows. When
its new version is installed it picks a quiet moment itself, as ADR 59
has the portal and the prompt wait for one, and steps down: no window
fullscreen, nothing recording or playing (ADR 43), the user still for a
few seconds; and after a bound, which the configuration may set, the new
version goes in regardless and says so. The holder knows nothing of
moments.

Components are then kept as programs are, their connections being the
relay's, and a compositor started anew takes them as running rather than
starting them again. How it learns of them is the relay's to settle when
it is built.

It is to come before stage 5. By then omorfos is a desktop people live
in, and one should not lose an afternoon to it.

### What follows the two

- **Programs written for X11** share one X server, which is a program
  like the others and so is kept by the relay. But the compositor is its
  window manager, and Smithay's window manager takes up only the windows
  made after it starts. Until that is changed, which is asked of it now
  (`docs/UPSTREAM.md`), the X server is started again with the compositor
  and its programs close, as ADR 27 has it when the X server stops. Games
  are the programs most affected, since many run through it.
- **The screen stays lit.** The holder can hold the screen's device open
  too, so that the last frame stays on the glass while the next
  compositor comes up and the next draws over it, with no black moment.
  That is the hardware backend's, which no test runs, and is done last.
- **Programs that come back by themselves.** Qt 6 programs connect
  again today where the socket's name holds. A protocol in which a
  program is given a token as it connects, and connecting again with the
  token is given back what the relay holds for it, would let any toolkit
  do the same, and make the relay something a desktop could do without.
  omorfos is to propose it once the relay has shown what such a thing
  must hold. That is where this paves a path, and does not only mend
  omorfos.

## Alternatives

- **The compositor writes itself down and starts its new version in its
  own place**, keeping its descriptors, as systemd starts itself again.
  Nothing is paid while running. But what would be written down is
  Smithay's state and wayland-backend's, neither made to be written down
  or read back from outside: a fork of the protocol library, a serialiser
  for every handler, broken by every Smithay release, and nothing
  recovered from a crash. Rejected.
- **A session manager**: systemd's user manager, which can keep
  descriptors for a service across its restarts, or one of our own with
  configuration and services. ADR 35 keeps the compositor from depending
  on systemd, a machine without it has to work, and the holder needs
  nothing a manager has: what it is, it is in a few hundred lines.
- **Programs started again and put back where they were**, as the
  session-management protocol proposed for Wayland would have it. At best
  a complement: what a program held in memory is gone, and the program
  has to have been written for it.
- **Window management taken out of the compositor** into a program of
  its own, so that most updates never touch what draws. The domain is
  pure already (ADR 4), so it could be done. But once a compositor costs
  a blink to start again there is nothing to spare it, and a private
  protocol would be added for nothing gained here. Set aside, not
  refused.
- **Leave it**, and log out, as every desktop asks. What that costs is
  what was asked about.

## Consequences

- ADR 13 is amended: the session is the holder's process, and the
  compositor runs under it. The compositor is still what starts
  everything else, and `omorfos-session` still speaks to systemd for it
  (ADR 35). ADR 59
  is amended once the relay is there: the compositor's new version is
  taken up like the others', at a quiet moment, and the word about
  logging in again goes.
- One new program now and one later, each small, each in the packaging
  and `release-please-config.json`. The session entry stays as it is,
  since `omorfos` becomes the holder where there is none. The holder is
  `omorfos-hold` and the relay `omorfos-relay`, each named as the others
  are, by what it does.
- With the holder alone, a crash costs the windows, as it does today, but
  not the login: the session is back in a second, locked if it was
  locked, under its name, and a Qt 6 program with it. Components start as
  they do at login, and what each had in memory goes as ADR 59 says it
  does for an install; the clipboard kept from closed programs goes with
  a crash, which an install spares it. What starts with the session is
  not started again, so a program of the user's that closed with the
  compositor stays closed until the next login, while one that did not
  is not started twice.
- A compositor that ends is a program that may be driven to end: a crash
  a hostile program causes is now a blink, not the session. The holder is
  the one piece that cannot be swapped under the session, only in its own
  place, so it is kept to what is written here.
- An old version started after a new one failed is a known version
  running where a fix was meant to be: the user is told so, as a problem,
  and not left to think the fix is in.
- The roadmap is reordered: the holder at the end of stage 3, the list of
  protocols with it, the relay before stage 5, and X11 windows kept and
  the screen kept lit in stage 8, the first waiting on Smithay.
- How the compositor is given the socket as a descriptor it inherits,
  the spelling of the words, and the bound on a quiet moment are the
  build's.

## What was built first, and what it settled

`omorfos-hold` came first, as this ADR orders it, with the compositor's part
of it and the action that asks the compositor to step down,
`restart_compositor`.

- **The compositor knows it is held by an argument**, `omorfos --held`, not
  by something in its environment. Whatever is in the environment every
  program the compositor starts inherits, and Smithay starts the X server
  itself, with nothing taken out; an argument goes to the compositor alone.
- **It takes the connection to the holder as what it reads from**, as the
  components take theirs (ADR 37), and at once puts what it reads from to
  nothing, before it starts anything, so that no program of the session
  can say to the holder that the session is unlocked. The holder hands the
  socket over that connection, as a descriptor, with what it was told.
- **`omorfos` becomes the holder only if one is beside it.** Started on the
  hardware with nothing above it, it runs the `omorfos-hold` in its own
  folder in its place; with none there it runs as it did before there was
  a holder. A build tried by hand would otherwise be handed to whatever
  holder is installed, which would start the compositor beside itself.
- **The holder starts the compositor from the file it holds open**, by its
  own name for it, `/proc/<holder>/fd/N`, opened just before. The file that
  runs is the file held, whatever is installed in its place meanwhile, and
  the one that last came up is the one kept to go back to. The process
  would then be listed by the last part of that name, `N`, so the
  compositor names itself `omorfos` as it takes the holder's connection.
- **The holder says the compositor is older**, as a line it tells it,
  `older`, when it went back to the one before; the compositor shows that
  as a problem, so that nobody thinks the fix is in.
- **What starts with the session is started by the login's first start**,
  not by the first compositor to say it is up. One that crashed before it
  said so may have started some of it, and starting it twice is what this
  was to prevent; the cost is that a first compositor that crashes that
  early leaves the user's programs unstarted until the next login.
- **The holder waits on the compositor without a timer.** Its connection to
  the compositor closes only when the compositor ends, since nothing the
  compositor starts has it, so it reads until then and waits for the
  process. A compositor that hangs without ending is waited for for ever,
  as it would hang the session without a holder; noticing one is left
  for later.
- **Its own new version is taken up between one compositor and the next**,
  checked first by running it with `--check`, and handed, as
  `omorfos-hold --taking`, the socket, its lock, the file held, its own old
  file and a few lines of what it knew. One that cannot take it runs the old
  file again with `--taking-back`. Becoming another program is in a file of
  its own, `exec.rs`, left out of the coverage gate (ADR 2): a process that
  becomes another never writes down what coverage it counted. What to
  become, and with what, is decided in covered code, and the tests run the
  holder taking its own place for real.
- **A compositor started again away from its virtual terminal waits for
  it.** One that crashes, or is ended, while the user is at another
  terminal is started again at once, and may not take the graphics card
  until its own terminal is shown again. Tried on real hardware, three
  such starts failed within seconds and the session was given up; so it
  waits for its terminal before it opens the card, rather than fail.
- **Qt 6 programs are told to connect again**, by `QT_WAYLAND_RECONNECT=1`
  in the session's environment. Qt does so only when told, and an omorfos
  session told nothing; it is told always, held or not, since a program
  whose socket has gone fails to connect and closes as it would have.
- **The limit on starting it again is the holder's own copy** of ADR 28's,
  three in a minute, not shared with the compositor's: the holder depends
  on nothing of the compositor's crate.
- **Any IPC peer may ask the compositor to step down**, as any may ask it to
  quit (ADR 8): a peer is the same user.
- **The list of protocols is in `omorfos-ipc`**, the crate the omorfos
  programs share what they agree on through, as the holder's words are:
  the compositor and the relay both need it, and the relay is to depend
  on nothing of the compositor's crate, as the holder does not. Each
  entry is a name, a version and a note, written out rather than taken
  from Smithay, so that a new version of Smithay offering a protocol at
  another version fails the test until the list, and with it the relay,
  has caught up. The compositor would rather choose each version itself,
  and raise it with the relay on purpose, but Smithay makes most globals
  at a version of its own; until it lets one be chosen, which is asked of
  it in `docs/UPSTREAM.md`, the update that raises a version brings the
  list up to date, so that a fix in Smithay never waits on the relay. The note says what is particular to that protocol; what
  holds for all of them, objects made again by the requests that made
  them, a program told only what changed, what is in flight ended as the
  protocol lets it end, is said once, above the list. The list does not
  say which programs are shown what: that is the compositor's, which
  goes on deciding it for each connection, and is tested where roles
  are. The X server's own protocol, which no other program is shown, is
  checked against the list from inside the compositor's crate.
- **The relay comes next**, straight after the holder and the list of
  protocols it is built from, not only before stage 5. Tried on real
  hardware, the holder did what it is for, and showed what it is not: a
  compositor started again closed every window but Qt 6's, a terminal and
  what ran in it among them. Nothing else on the roadmap spares the user
  as much.
