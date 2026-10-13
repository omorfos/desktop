# 37. Who may do what

- Status: accepted
- Date: 2026-10-13

## Context

On X11 any program could read every key, see every window and act as the
user. Wayland's answer is that a program sees and controls only its own
windows, and anything more is a privilege the compositor grants.

omorfos grants several, and today to anyone who asks: any program may
put a surface on the screen as a panel or over everything, lock the
session, and list every window by title. More are coming that matter
more: capturing the screen, reading the clipboard at any time, typing
and clicking as the user, arranging the screens.

Two facts decide what can honestly be promised.

- **A program in a sandbox** (Flatpak and the like) cannot read the
  user's files or reach other programs. The compositor is one of the few
  ways out of the sandbox, so what it refuses there is truly refused.
- **A program not in a sandbox** runs as the user. It can already read
  the user's files, change the user's configuration, and start programs.
  Whatever the compositor refuses it, it can get by rewriting the file
  that says who is allowed. Refusing it is worth doing, to make
  privileges plain and to stop the careless and the casual, and it is
  not a wall.

## Decision

### Privilege comes from who started the program

The compositor grants a privilege to a program **it started itself, for
a role it was told to give**. Nothing is granted by a program's name,
its window's name or its process number, each of which a program can
choose or borrow.

- The compositor starts the program and hands it its connection. That
  connection, and no other, has the role.
- Components of ours are found beside the compositor's own executable,
  not by searching the user's `PATH`.
- A program started any other way, from a terminal or by another
  program, is an ordinary program.

### Three kinds of program

| | Ordinary windows, clipboard while it has focus, ask to be activated | Privileged things | The IPC socket |
|---|---|---|---|
| **Given a role** by the configuration | yes | what its role names, and nothing else | as now |
| **Ordinary**, not in a sandbox | yes | none; told how it could be given them | as now |
| **In a sandbox** | yes | none, ever; it asks the portals, which ask the user | cannot reach it |

A sandbox is recognised by `security-context-v1`, which is how the
sandbox itself tells the compositor "the programs on this connection are
mine, and this is the application". A program cannot say it of itself or
unsay it.

A program refused something is not shown that the thing exists. The user
is told once, among the notifications, what asked and how to allow it,
so that trying another panel is one line of configuration and not a
puzzle.

### The roles

A program is given roles where it is started, in `autostart` or where a
component is named:

```yaml
autostart:
  - run: [waybar]
    may: [panel]
```

| Role | What it may do | Who has it by default |
|---|---|---|
| `panel` | Put surfaces on the screen outside windows and keep room for them; list windows, and show, close and move them; know the workspaces | The bar, the dock, notifications |
| `overlay` | Cover the screen and take the keyboard while it does | The launcher |
| `lock` | Lock the session | `omorfos-lock`, and only one program at a time |
| `ask` | Show the sealed prompt (below) | `omorfos-ask` |
| `capture` | Take pictures of the screen and of windows | The portal backend, which asks the user each time |
| `clipboard` | Read and set the clipboard at any time | The clipboard's history |
| `input` | Type and click as the user; see what is typed into fields, as an input method must | Nobody. An input method is given it by the user |
| `screens` | Arrange the screens; change their colour, as a night light does | Settings |

Every one of ours can be replaced: name another program for the role and
ours is not started.

### What is not left to roles

Some things protect the user from a program that has a role, or from one
that got it by rewriting the configuration. They are the compositor's
and no role alters them.

- **A mark while the screen is being captured.** The compositor draws
  it, in a corner, over everything, for as long as any program is taking
  pictures, whoever the program is and however it was allowed. The mark
  is left out of the pictures. The bar may say more; the mark does not
  depend on the bar.
- **One locker.** While a session is locked nothing else is drawn or
  given input (ADR 14), and the lock is not lifted by anyone but the
  locker that holds it. A locker that dies leaves the session locked.
- **The sealed prompt cannot be imitated.** While something asks for a
  secret, the rest of the screen is dimmed and blurred by the
  compositor, which no program can do to the screen, and the keyboard
  goes to the prompt alone: no binding, no other program and no input
  method sees a key. A window that draws a password box is just a
  window, with the desktop sharp around it.
- **The edge that answers to a push** in a fullscreen workspace
  (ADR 36).
- **Keys typed into a window go to that window.** No role but `input`
  sees them, and `input` is shown in the same list as capture is: the
  user can always find out that something has it.
- **Focus is not taken.** A program may ask to be activated, and is, if
  the user just did something in the program that asks. Otherwise its
  window is marked as wanting attention.

### Asking the user

- A program in a sandbox reaches privileges only through the portals,
  and the portal backend asks, with the sealed prompt, saying which
  application asks and for what. "Always for this application" can be
  chosen for sharing a screen, and is listed in Settings to be taken
  back. Nothing is remembered for typing as the user.
- A program not in a sandbox is never asked about with a prompt. A
  prompt for every panel and screenshot tool teaches saying yes; the
  configuration says who has what, in one place that can be read.

### Programs written for X11

They all speak through one X server and can read each other's windows,
keys and clipboard, as X11 has always allowed. They cannot see or reach
programs written for Wayland. The X server is an ordinary program here
and is given no role.

### The IPC socket

It stays as ADR 8 has it: the user's own programs may read the state and
ask for actions, among them starting a program. That makes it as
powerful as a shell, and it is guarded as one: owned by the user, closed
to others, and not reachable from a sandbox. A program given a role
through it gains nothing it could not have had by editing the
configuration.

## What was built first, and what it settled

The first three roles: `panel`, `overlay` and `lock`. The others are
names the configuration accepts, with nothing behind them yet.

**Handed over as what the program reads from.** The compositor makes
both ends of a connection, keeps one as a client it knows the roles of,
and gives the program the other as its standard input, with
`WAYLAND_SOCKET=0` to say so. Every Wayland program understands that.
Our own components take a copy of their standard input, which needs
nothing `unsafe`, as taking a descriptor by its number would.

**Refused, not hidden.** The decision had a refused program "not shown
that the thing exists", and also had the user told what asked. The two
do not go together: a program that is not shown a thing never asks for
it. For a program that is not in a sandbox, being told matters more, so
it is shown that panels and locking exist, asks, and is refused, and the
user is told once with what to write. The list of windows is the
exception: it is simply not offered to a program without the `panel`
role, since offering it is giving it.

**Hidden, in a sandbox.** A sandbox makes a place to connect of its own
and tells the compositor so (`security-context-v1`); whatever connects
there is inside it. Such a program is not shown panels, locking or the
list of windows at all. It could not be let have them by anything the
user writes, so there is nothing to tell the user, and a program that
sees less has less to try. It is not shown the way a sandbox is made
either, or it could make one and say it is some other program. What the
sandbox says the program is called is not kept yet: nothing reads it
until the portals do.

**Capture is refused like a panel, and marked.** An ordinary program
that asks for pictures of the screen is told there will be none, and the
user is told once what asked; a program in a sandbox is not shown that
pictures can be asked for. The mark is on the screen being captured from
the moment a program may take pictures of it until a second and a half
after it stops, so that one picture cannot be taken unseen. It is drawn
after a picture has been read back, which is what keeps it out of the
pictures without a second way of drawing the screen. While the session
is locked a picture asked for fails.

**The clipboard at any time is not offered, like the list of windows.**
A program without the `clipboard` role is not shown
`ext-data-control-v1` at all, in a sandbox or out of one: what is copied
has passwords in it, and offering it is giving it. So nothing is refused
and the user is told nothing; a program that needs it and has not got
it says so itself, as `omorfos-screenshot` does.

**`overlay` is the keyboard.** It was to be "cover the screen and take
the keyboard". Which layer a surface is on does not say whether it
covers the screen, and the notifications that come with omorfos are on
the top layer of all without covering anything. So any layer is the
`panel` role's, and `overlay` adds the one thing that reaches further:
taking the keyboard from whatever has it.

**`spawn` takes `may` too**, in a key binding or a menu entry, since
those are the configuration as much as `autostart` is.

**Locking has an action.** With locking refused to ordinary programs, a
key bound to `spawn: [swaylock]` and an idle daemon that starts a locker
both stop working. `locker` in the configuration names the program and
the `lock` action starts it with the role, from a key or over IPC, which
is what an idle daemon is now told to do.

**A socket can be served with roles.** `Session::serve` takes the roles
its clients have. The session's own socket is served with none. It is
there for tests, which run a component against a socket as a user would
have before this, and for whoever builds on the library.

## Alternatives

- **Leave it open, as most small compositors do.** Any program that runs
  can then show a false lock screen or record the screen unseen.
- **Know programs by their executable.** The compositor can learn which
  file a connected program is. But a program not in a sandbox can make a
  trusted file do its bidding, so this is the same soft line as now with
  more to go wrong, and it breaks for scripts and wrappers.
- **Ask about everything.** See above: it teaches yes.
- **Trust only ours.** Then omorfos could not be given another panel or
  launcher, which ADR 18 promises.

## Consequences

- Programs that work today by being started from a terminal, a panel or
  a wallpaper setter tried by hand, stop until they are named in the
  configuration. The notification that says how is part of this, not an
  extra.
- `autostart` entries gain `may`, and components gain a place to be
  replaced. The compositor starts components by handing them their
  connection, not by their finding the socket.
- Every privileged protocol is behind a check of the role, in one place,
  and a protocol added later is refused to everyone until it is given
  to a role.
- The portal backend and `omorfos-ask` (ADR 35) carry the asking.
- The mark for capture is drawn by the compositor and is tested as
  pixels: there while a capture is live, gone after, absent from the
  capture.
- What is promised is written in the guide as it is here: a hard line
  round sandboxes, a plain and honest one round everything else.
