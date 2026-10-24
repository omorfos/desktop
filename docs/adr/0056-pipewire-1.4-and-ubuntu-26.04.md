# 56. PipeWire 1.4 or later, and CI on Ubuntu 26.04

- Status: accepted
- Date: 2026-10-24

## Context

Every job ran on Ubuntu 24.04, whose PipeWire is 1.0.5, and the packages
were built there, so they installed on it. Much of what omorfos does with
PipeWire had to allow for 1.0: it does not say which process the daemon
is ([ADR 43](0043-what-records.md)), and it calls a stream's events on its
own thread while the thread that owns the stream is between turns of its
loop.

That last showed in the nightly stress run as a crash in
`omorfos-recording`'s tests, on both architectures, a few runs in thirty.
A stream that drives itself, as the portal's does, is told to go between
turns; PipeWire's data thread finds nobody in the main loop and calls the
main loop's `trigger_done` itself, while the owning thread goes through
the same stream's listeners. Both change that list at once, and it breaks.

The ways out are to keep the main loop entered for as long as PipeWire is
used, which the bindings allow only through `unsafe`; to move the portal
onto a thread loop of PipeWire's, a redesign of how it talks to PipeWire;
or to leave the oldest PipeWire behind. Ubuntu 26.04 ships PipeWire 1.6.2,
and GitHub has runners for it on both architectures. Debian 13 ships
1.4.2, and Ubuntu 25.10 1.4.7.

## Decision

omorfos needs PipeWire 1.4 or later. Every workflow runs on Ubuntu 26.04,
the packages among them, and no `unsafe` is added for PipeWire.

## Consequences

- Ubuntu 24.04 is no longer supported. The `.deb` is built on 26.04, so
  its libraries are 26.04's, and it says it needs libpipewire 1.4.
- CI tests PipeWire 1.6, not 1.4. What 1.4 does differently, Debian 13
  finds first.
- PipeWire 1.4 and 1.6 still run a loop's callbacks on the thread that
  asks, when nobody is in that loop, though no longer at once with another
  thread doing the same. Whether that ends the crash is for the stress run
  on 26.04 to say. If it does not, the thread loop is the way without
  `unsafe`.
- Allowances for PipeWire 1.0 in the code stay until each is looked at:
  they are not wrong with 1.4, only no longer needed.
