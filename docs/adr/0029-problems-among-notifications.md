# 29. What is wrong is shown among the notifications

- Status: accepted
- Date: 2026-10-12

## Context

The compositor draws a notice in the top right-hand corner when something
is wrong: a mistake in a configuration file, a program it could not start.
The notifications (ADR 23) are in the same corner. One could cover the
other, and a user who saw one might never see the other.

The notice cannot simply be done away with. It is what says that the
notifications keep crashing, what shows a mistake found before they have
started, and what there is when they are turned off or another program
does them. Something has to remain that needs nothing else to be working.

## Decision

**Where the notifications are showing, they show what is wrong.** Each
problem is an entry in their stack, drawn and stacked as a notification
is, headed `omorfos` and looking as a critical one does. It stays until
it is clicked away or put right. Clicking one away dismisses that one
only, and it does not come back while it stays wrong; put right and gone
wrong again, it does.

**A problem is never the one to make room.** When there are too many
notifications it is a program's that goes. A program cannot replace or
close one either: they go by keys outside the numbers programs use.

**They are told over the compositor's own socket**, as the panel is told
of windows. `State` already carries `problems`. The notifications ask for
it with a new request, `show_problems`, which is `subscribe` with a
promise to show them.

**The compositor draws its notice only while no one has made that
promise.** It counts the peers that have, and stops counting one the
moment its connection closes. So the notice comes back by itself if the
notifications crash, are turned off, or were never there, and nothing has
to tell it so. Another notification program can make the promise too.

**Nothing new is trusted.** The socket is the user's own (ADR 8). What
arrives on it is clipped and counted like anything a program sends, and a
peer that breaks its promise only hides problems from the user it belongs
to.

## Consequences

- Errors and notifications are one stack. Nothing covers anything, and
  each stays until it is dealt with.
- A problem is given six lines, broken to fit, where the notice gave it
  one and cut it short.
- What is dismissed is remembered by whichever is showing. If the
  notifications are started again after a crash, what was dismissed in
  them is shown again.
- What merely happened and needs nothing done, such as the X server
  having been started again, is not a problem and is not kept as one.
  `State` has `events` beside `problems`: what has happened in the last
  ten seconds. The state is a picture of the session and not a stream, so
  something that happened is in it for a while and then is not, and a
  program that shows events shows each once, when it first sees it. The
  notifications show one as an ordinary notification, which goes after as
  long as those usually stay; the compositor's own notice, where that is
  what there is, shows it for as long as it is in the state.
- The notifications fail if the compositor's socket closes under them,
  which it does only when the compositor has gone.
- The protocol gains a request. A compositor that does not know it
  answers with an error, and goes on drawing its notice.
