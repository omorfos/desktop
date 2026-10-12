# 28. What runs for the whole session is started again if it crashes

- Status: accepted
- Date: 2026-10-12

## Context

The panel and the notifications are programs of their own (ADR 18), so that
a fault in one cannot take the session down. That is only half of what
being separate is good for. A panel that crashed stayed gone until the
user logged in again, and so did the notifications, silently.

ADR 27 started the X server again when it stops. The same is owed to what
omorfos itself ships.

## Decision

**The panel and the notifications are started again when they fail.** The
compositor already collects the programs it started when they exit. One of
these two that exits with a failure, or is killed, is started again there
and then.

**One that exits without failing is left stopped.** That is what a program
does when it means to stop, and what it does when the session ends.

**What the configuration has turned off since is left stopped too**, even
though turning it off does not stop one that is running.

**Nothing is started again for ever.** `Restarts` allows three in a minute,
for each program separately, and the X server now uses it too. After that
the program is left stopped and the user is told which, in the notice. A
restart that works is not announced: nothing was lost that the user could
see, beyond the bar blinking.

**Only what omorfos ships and runs for the whole session.** The programs a
user lists under `autostart` are theirs: whether each is meant to keep
running is not something the compositor knows. The launcher is started to
be used once and is not kept running at all.

## Consequences

- A crash in the panel or the notifications costs a moment, not the rest
  of the session.
- The notifications fail at once when another program already shows them.
  They are tried three times more and then left, with a notice that says
  so, which is also how the user learns of the clash. Turning ours off in
  the configuration ends it.
- A program that fails while the session is ending may be started once
  more as it goes. It finds no compositor and stops.
- Notifications that were showing when theirs crashed are lost: the new
  one starts empty.
