# Security

omorfos is the program that stands between every other program on the
desktop and the user: what they may see of each other's windows, what
they may type into, what they may capture, and whether the screen is
locked. A flaw in it is a flaw in all of that, so reports of one are
welcome, and handled before anything else.

## Reporting a vulnerability

Report it privately, through GitHub's
[private vulnerability reporting](https://github.com/omorfos/desktop/security/advisories/new),
or, without a GitHub account or by choice, by email to
[security@omorfos.io](mailto:security@omorfos.io); never in a public issue. Say what is affected, how to bring it about, and
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

## Who is trusted

The roles of [Who may do what](guide/permissions.md) hold back what
programs reach through Wayland: a program sees and works its own windows,
and only one omorfos started with a role may capture the screen, be the
input method or lock the session. They are not a wall between programs
of yours that run outside a sandbox, and are not meant to be.

**The IPC socket** (`docs/IPC.md`) is for programs of the user's own:
it is made readable and writable by its owner alone, and the compositor
refuses a peer that runs as another user. A peer on it may do anything
a key can: perform any action, among them `spawn`, which starts a
program with any roles it names, and `quit`; close windows; write the
clipboard; keep the screen awake; and read the state, which lists every
window by its title, those marked private among them. Nothing it asks is
held to a role.

This gives it no more than it has already. A program running as you,
outside a sandbox, can write `keys.yaml` to bind a `spawn` with any role
and have the configuration read again, or start a program of its own
with your files and your session's bus. What stands between it and the
rest of your session is the operating system's, not omorfos's.

**A sandbox** is where the line is. A Flatpak, or another program inside
`security-context-v1`, is held to less (ADR 37), and reaches neither
the IPC socket nor the configuration unless it is given them: a sandbox
given `--filesystem=xdg-run` (or the socket by name, under
`$XDG_RUNTIME_DIR`) reaches the socket, and one given write access to
`~/.config/omorfos` (or `xdg-config`, or `home`) can give itself any
role through the configuration. Either is as good as running outside
it, and is to be given only to what is trusted that far.

Should components written by others come, which a user installs and
runs as their own, requests held to a role will be weighed then.

## Versions

omorfos is not released yet. Only the newest commit on `main` is
supported.
