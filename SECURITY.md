# Security

omorfos is the program that stands between every other program on the
desktop and the user: what they may see of each other's windows, what
they may type into, what they may capture, and whether the screen is
locked. A flaw in it is a flaw in all of that, so reports of one are
welcome, and handled before anything else.

## Reporting a vulnerability

Report it privately, through GitHub's
[private vulnerability reporting](https://github.com/omorfos/desktop/security/advisories/new),
not in a public issue. Say what is affected, how to bring it about, and
what an attacker would gain; a program or configuration that shows it
helps most.

You will have an answer within a week. Once it is fixed, the fix is
released and an advisory published, crediting you unless you would
rather not be.

## What counts

Anything that lets a program, a peer on the IPC socket or a
configuration file do what it should not, such as:

- seeing, capturing or sharing a window or screen without the user's
  choice, or one marked private;
- reading or injecting input meant for another program;
- getting past the lock screen, or showing anything over it;
- taking a role (panel, launcher, portal) it was not given;
- making the compositor crash or hang, which ends the session and, with
  it, the lock.

## Versions

omorfos is not released yet. Only the newest commit on `main` is
supported.
