# 99. The standards there are, before any of our own

- Status: accepted
- Date: 2026-11-18

## Context

A Linux desktop is tiring where every desktop answers the same question
its own way: three themes to find for three toolkits, a setting each
program reads from somewhere else, a protocol that one family of
compositors speaks and another does not. omorfos is one more desktop.
It can add to that or take from it.

The redesign of windows (ADR 97) and the question of which programs
stand alone (ADR 98) both came to the same place: what programs are
told, and what they ask. Some of it is agreed and written down, some of
it is one desktop's way that the others have taken up, and some of it
nobody has agreed at all, and each compositor has its own.

There is no one board for it. Two bodies between them settle nearly
all of it, neither an ISO nor a series of RFCs:

- **wayland-protocols**, at freedesktop.org, where compositors and
  toolkits agree what a program and a compositor say to each other. Its
  members are projects; a protocol is proposed by a merge request,
  needs members to take it up, and passes from experimental through
  staging to stable. Its namespaces say how wide the agreement is.
- **freedesktop.org's specifications and the portals**
  (`xdg-specs`, `xdg-desktop-portal`), where what is not between a
  program and the compositor is agreed: where files go, what a desktop
  entry or an icon theme is, how a notification is sent, and what a
  program may ask of the desktop from inside a sandbox, its settings
  among them.

Beside them is what is neither: GNOME's settings, which every toolkit
reads; KDE's and wlroots's protocols, which others have taken up one at
a time.

## Decision

**omorfos speaks what is agreed, and makes nothing of its own where
something is.** A protocol in wayland-protocols before a family's own;
a freedesktop specification before a desktop's own; and a desktop's
own, where it is what programs in fact read, before one invented here.
What omorfos offers programs is the list in `omorfos-ipc` (ADR 60), and
a protocol of a family's is taken out of it once what the same programs
ask through is agreed, as KDE's blur is to be (ADR 64).

**Where nothing is agreed, omorfos says so, and takes it to those who
could agree it.** What it speaks that is not yet anyone's standard is
written down in [STANDARDS.md](../STANDARDS.md): what it is, who has
what today, and where it would be proposed. Each is on the roadmap to
be raised: with the project whose it would be, as a proposal others can
take up, argued from what omorfos and the other desktops each do and
not from what omorfos would like. A proposal that others have already
made is joined, not made again.

**It does not make a standard by shipping one.** Something of omorfos's
own, where there is nothing else, is said to be its own, kept as small
as will do, and given up for what is agreed when something is. Its IPC
is its own and is meant to be: how one compositor is driven is not
something the others need agree.

**This is part of what omorfos is for**, beside being small, fast and
safe: a desktop that leaves the others easier to be beside, for its
users and for theirs. It comes after the priorities in AGENTS.md and
does not outrank them: nothing is made less safe or less correct to be
agreed with.

## Alternatives

- **Take what works and say nothing.** What every small desktop does.
  The gaps stay gaps, and each new desktop finds them again.
- **Design the missing ones here and ship them.** Quicker, and one more
  of what this is against.
- **Wait for the others to agree.** Some of these have been open for
  years with nobody whose need it is to close them.

## Consequences

- A cost in time that buys omorfos nothing by itself: reading what the
  others do, writing it up, and being told no.
- Raising a proposal is the maintainer's to do, or an agent's the
  maintainer has asked, as reporting upstream is (UPSTREAM.md). What
  goes out is in the first person, and theirs.
- Some of what is in STANDARDS.md was written from what is known in
  this repository and not checked against each project as it stands.
  Each is checked against what the project it would go to says of it
  now before it is raised, as the head of STANDARDS.md asks.
- What omorfos does meanwhile is what programs read today, so that
  nothing waits on an agreement to work.
