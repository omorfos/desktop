# When something is wrong

## A setting does nothing

A file is read again each time it is saved, but one with a mistake does
not take effect: its contents from before are kept until the mistake is
put right, and the problem is shown among the notifications. `omorfosctl config check` prints every problem, with its
file, line and column, and fails while there is one:

```sh
omorfosctl config check
```

A setting omorfos does not know is a mistake too, so a misspelt one is
said to be, not passed over.

## A key does nothing, or something else

What each key does before you write any is in the table of
[the built-in bindings](keys.yaml.md#built-in-bindings); `omorfosctl
action` lists every action there is. A chord with Super that nothing is
bound to reaches the program with the keyboard as Ctrl, so it does what
that program does with Ctrl (see [Super](keys.yaml.md#super)). A program
that asked for every key, as a virtual machine may, has the desktop's too
while its window has the keyboard: `Super+Escape` takes them back.

## What programs say

What omorfos and the programs that come with it say, a configuration it
cannot read and why, a program it sent away for asking what Wayland does
not allow, and why, is kept in `~/.local/state/omorfos/session.log`, or
under `$XDG_STATE_HOME` if you set it, and what the login before said
beside it, in `session.log.old`. Only you can read them. Where they cannot
be written, what omorfos says goes where your login screen keeps what a
session says, with a line saying why.

```sh
tail -f ~/.local/state/omorfos/session.log
```

What the programs started in the session say, those you open and those
that start with it, is kept in the journal, each under its own name, where
systemd's journal runs:

```sh
journalctl --user -t foot
journalctl --user -t code --since today
```

So a program that complains, as most do now and then, does not fill
omorfos's log, and what it said is still there after you log out and in
again. Where there is no journal, they say it where omorfos does.

To report a fault, `omorfosctl report` gathers what a report needs into
one text: the versions of omorfos, the kernel and the system, the graphics
cards and screens, what the compositor says of its state, your
configuration, and the end of both logs. Window titles, and any path in
your home, are left out; `--whole` keeps them. It is only written out,
never sent: read it over before you hand it on. It is made without a
running compositor too, saying why that part is missing.

```sh
omorfosctl report > omorfos-report.txt
```
