# 59. New versions taken up in the session, but for the compositor's own

- Status: accepted
- Date: 2026-10-25

## Context

Installing a new version of omorfos (`make install`, or a package) put
new programs on the disk, and the session went on running the old ones
until the user logged out and in again. Most of what comes with omorfos
is a program of its own (ADR 18), started by the compositor and started
again if it fails, and could as well be started again when it changes.

Installing does not write over a program: `install`, and the package
managers, write a new file and put it in the old one's place, so that
what is running keeps the old. A watch on the program itself would never
hear of the new one; a watch on its directory does.

Not every program can be stopped at any moment. The keeper of the
clipboard holds what was copied from programs that have since closed, and
it would be gone with it. The portal shares a screen with a call, and the
call would lose it. `omorfos-ask` may be asking for a password, and the
user would be left typing into nothing. Two others hold a name on the
session's bus, the notifications and the portal, which a new one cannot
take while the old one holds it.

The compositor cannot be swapped under its windows. What a window is,
its place, its buffers and the protocol objects its program made, lives
in the compositor's memory, and most programs cannot rebuild it on
another compositor: only Qt 6 reconnects. A compositor that started again
within the login, its windows closed, would save the login screen and
nothing else.

## Decision

**The compositor watches the directories its programs are in** (inotify,
for a file written or put in place), and takes a program to be installed
a second after it last changed, so that what writes it has finished.

**Each program that runs for the whole session is then taken up**: the
one running is asked to end, and its new version started once it has,
so that it can take whatever name the old one held. A new version is not
held to how often the old one failed, and one given up on for failing is
started again by its new version. One turned off in the configuration is
left off.

**Three wait, or hand over:**

- **The keeper of the clipboard** is the other way round: the new one is
  started first, takes what the old one keeps as the first thing it finds
  on the clipboard, though it carries a keeper's mark, and the old one is
  asked to end three seconds later, longer than a keeper waits for what
  it reads.
- **The portal** waits while any program is taking pictures of a screen
  or a window, as sharing one does.
- **`omorfos-ask`** waits while the sealed prompt is up.

**The compositor's own new version is said**, once: that it is used once
the user logs in again. It is not started again in the session.

Programs started on demand, the launcher and the locker, need nothing:
each is started anew every time it is wanted, and is the new version from
then on.

## Alternatives

- **Asked for by `make install`**, over the session's socket. It runs as
  root, outside the session, and would have to find the user's runtime
  directory and speak to it; and a package manager would not ask. Hearing
  of the files covers both, and asks nothing of either.
- **Starting the compositor again within the login.** Its windows would
  close, with whatever they held, and it would save only the login screen;
  logging in again is clearer about what it costs.
- **Every program at once**, without waiting. A share would end mid-call,
  and a prompt mid-password, for an update that could have waited a
  minute.

## Consequences

- Installing omorfos takes up the new panel, wallpaper, notifications,
  portal, keeper, recording watcher, session watcher and agent within a
  second or two, each with a moment in which it is not there. A program
  that speaks to the old portal or notifications through the session's
  bus finds the new one under the same name.
- A program named without a directory, looked up in `PATH`, is not
  watched: which file it is depends on where it is looked for. omorfos
  names its programs by their place beside its own.
- What the old version of a program had in memory, but for the keeper's
  clipboard, is gone: a notification that was showing, the wallpaper's
  place in its turn. Each starts as it would at login.
- A watch the kernel will not give means new versions are taken up at the
  next login, as before.
