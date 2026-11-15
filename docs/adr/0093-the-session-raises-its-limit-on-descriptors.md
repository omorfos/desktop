# 93. The session raises its limit on descriptors

- Status: accepted
- Date: 2026-11-15

## Context

A login starts each program allowed 1024 open descriptors, and allows it
to ask for many times that: the hard limit is commonly 524288. Nothing in
omorfos asked. Yet the compositor keeps a descriptor for every pool of
memory a program shares with it, for every buffer of the graphics card
and each of its planes, for keymaps and pipes; and the relay keeps one
beside each pool for the next compositor, for every program at once. One
program that made a thousand small pools took every descriptor the
compositor had, and a long session of many windows comes nearer to it
than it should.

A review
([finding 4](../reviews/2026-11-15-what-a-client-can-cost.md#4-descriptors-run-out-in-the-compositor-and-in-the-relay))
showed what running out did: the next program to connect ended the
session. That is fixed apart from this: a socket that cannot be accepted
from is now listened to again a moment later. This record is about not
running out so soon.

## Decision

**The compositor, the relay and the holder each raise their soft limit
on descriptors to the hard limit, first thing as they start**, through
`omorfos_ipc::limits::raise`. This is what other compositors do, and
what the limit's being soft is for.

What they start inherits it: the programs of the session, a terminal
and what is run in it among them. Setting it back for each, between
`fork` and `exec`, needs `pre_exec`, which is `unsafe`, and is not done.

## Alternatives

- **Leave it, and hold each program to a most.** The relay does hold a
  program to a most number of descriptors kept (finding 4, to come), but
  an honest session of many windows needs more than 1024 between them.
- **Raise it only as far as some number, as 65536.** Any number is a
  guess at what the hard limit already says better.
- **Set it back for what is started**, as above: `unsafe`, for a cost
  that is small.

## Consequences

- A program started from the session may have as many descriptors as
  the hard limit allows. One that still uses `select`, which cannot
  watch a descriptor numbered 1024 or above, fails once it has that many
  open; such a program fails the same way under every desktop that does
  this, and few are left.
- Running out still has to be survived, and is: the compositor and its
  IPC socket listen again once descriptors are free, and the relay is to
  do the same.
