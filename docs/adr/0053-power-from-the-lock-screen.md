# 53. Sleep, restart, shut down and log out from the lock screen, asking nobody

- Status: accepted
- Date: 2026-10-23
- Amends: [52. The lock screen](0052-the-lock-screen.md),
  [35. Where omorfos meets the rest of the system](0035-meeting-the-rest-of-the-system.md)

## Context

A locked screen used to offer one thing: unlocking. To turn the machine
off, or let someone else at it, the user had to unlock first. Login
screens and other lock screens offer Sleep, Restart and Shut Down, and
often Log Out, without it.

Each of those is done by someone other than the lock screen: sleep,
restart and shut down by logind, which asks polkit whether this user may;
logging out by the compositor, which ends the session when asked to quit.
`omorfos-session` already asks logind for the first three, for the menu
and the keys, and speaks to the compositor's socket.

What the lock screen cannot do is show anything over itself. A polkit
prompt is shown by `omorfos-ask`, a window of the session, which a locked
screen hides. An action that needed a password would wait on a prompt
nobody can see.

## Decision

- **Asked of `omorfos-session`, as a program.** The lock screen runs
  `omorfos-session offered` to learn what may be done, and
  `omorfos-session sleep|restart|shut-down --locked` or `log-out` to do it,
  each on a thread of its own, as it runs a helper for each way of
  unlocking. It speaks to neither logind nor the compositor itself: it
  stays without D-Bus, and one that hangs does not hang it.
- **Only what needs nobody's password.** `offered` says `can` of each of
  sleep, restart and shut down only where logind's `CanSuspend`,
  `CanReboot` and `CanPowerOff` answer `yes`; `challenge`, which would ask
  for a password, is not offered, nor is what logind does not answer.
  `--locked` asks logind with `interactive` false, so that should polkit
  want a password after all, the request fails at once instead of
  waiting. Log Out is offered wherever there is a compositor to ask.
- **What loses work is asked about first.** Restart, Shut Down and Log
  Out close every program of the session, and what is not saved in them,
  so each asks "...? What is not saved in open programs is lost." with a
  button for yes, drawn as `question-button:active`, and Cancel; Escape
  says no too, and so does leaving the screen alone until it shows only
  the time. Sleep loses nothing, keeps the session locked, and is done at
  once.
- **Who keeps the machine on is said.** logind lists the programs that
  block sleep or shut down (`ListInhibitors`). While one does, logind asks
  for a password to go ahead, so the button is not offered, and the
  screen says "Kept from turning off by Firefox: Downloading" instead, so
  that a missing button is not a mystery. Delays, as the session's own
  before sleep, hold nothing and are not shown.
- **Shown with the rest.** The buttons are along the bottom of every
  screen, and come into view, and go, with the field: never while only
  the time shows. A click that wakes the screen presses nothing.
- **`lock.power: false`** offers none of them, and runs nothing, for a
  machine others use.

## Risks

- **Whoever is at the screen can end the session.** Shutting down,
  restarting or logging out closes the user's programs; what is not saved
  is lost. That is no more than holding the power button does, which
  logind treats as shutting down. The question asked first makes it a
  choice, not a slip; `lock.power: false` takes it away.
- **Text from any program is shown on the lock screen.** The reasons in
  logind's list of inhibitors are any program's to set, and could be made
  to read as an instruction ("Type your password to continue"). They are
  shown on one line, no longer than 100 characters, with control
  characters taken out, and always after "Kept from ... by", so that they
  read as what they are. To be looked at again if that proves too little:
  the names alone, without the reasons, would be safer and say less.
- **logind decides, not omorfos.** What `CanPowerOff` answers is polkit's
  rules: on a machine where other users are logged in, or where an
  administrator has said so, nothing may be offered. That is as it should
  be, and is what is tested; the real answers are only seen on real
  machines.
- **Log Out ends the session as `omorfosctl action quit` does,** without
  asking programs to save: there is no session manager to ask them. The
  question says so.

## Consequences

- `omorfos-session` has two new commands, `offered` and `log-out`, and
  `--locked` after sleep, restart and shut down.
- `omorfos-lock` draws buttons and follows the pointer.
- Keyboard access to the buttons, Tab to reach them, is not there yet,
  and is on the roadmap.
- The built-in theme gives `question-button:hover` a look of its own.
