# Locking the screen

omorfos does not lock by itself; it starts a screen locker to. Name the
locker in `config.yaml`, and the `lock` action starts it, as the one
program let lock, as does being away for `idle.lock` seconds:

```yaml
# config.yaml
locker: [swaylock, -f]
idle:
  lock: 300
```

```yaml
# keys.yaml
bindings:
  - keys: Super+l
    action: lock
```

A machine left locked still holds the keys you gave it: those
`ssh-agent` holds, and the passphrases `gpg-agent` has kept for a while.
`lock.forget` has them forgotten as the session locks, however it is
locked and whatever the locker:

```yaml
# config.yaml
lock:
  forget: [ssh, gpg]
```

`ssh` has `ssh-add -D` forget every key `ssh-agent` holds; `gpg` has
`gpg-agent` forget every passphrase it has kept, of GnuPG's keys and of
the SSH keys it holds, and ends its smartcard daemon, so that a key on a
card such as a YubiKey asks for the card's PIN again. It starts no
`gpg-agent` that is not running. Where `gpg-agent` is your SSH agent too,
`gpg` is the one that matters: a key on a card cannot be taken out of an
agent, only locked again.
Nothing is forgotten unless named, since each key is to be given again
after unlocking. What could not be forgotten, as where there is no agent,
is in the journal.

An idle daemon of your own, as `swayidle`, still works: it is told when
you have been away, and asks omorfos to lock with `omorfosctl action
lock`. It does not start the locker itself: a locker it started would be
an ordinary program, and refused.
While the screen is locked nothing else is shown and no key binding
works, except Ctrl+Alt+F1 to F12. If the locker ends with the screen still
locked, as one that crashed does, omorfos starts it again, up to three
times in a minute; after that the screen stays black and locked, and
`omorfosctl action lock` from another virtual terminal starts it again.

omorfos comes with a lock screen of its own, `omorfos-lock`. Name it as
the locker to use it:

```yaml
# config.yaml
locker: [omorfos-lock]
lock:
  fingerprint: true
  key: true
```

It shows the time on every screen until you press a key or a mouse
button. Then the rest comes into view: your face, your name, a field for
your password and what each way of unlocking says. The key that wakes it
types nothing, so start typing your password after it. Backspace held
down goes on taking out, at the pace of `keyboard.repeat_rate` and
`repeat_delay`, and Ctrl+A, or Super+A, chooses all of what you typed,
which `question-input:active` marks, for Backspace to take it out at once
or what you type next to take its place. Escape, or being
left alone for 30 seconds, empties the field and shows only the time
again, the rest going the way it came. Your name is
your full name where your system has one (the first part of the comment
in `/etc/passwd`, which `chfn` sets), and your face is the picture your
login screen shows (`/var/lib/AccountsService/icons/` and your name),
or else `~/.face` or `~/.face.icon`, a PNG or a JPEG. Without one, your
initials are shown in a circle instead. However long your name, it is
shown whole, and cut short only where it would be wider than the screen.

It asks PAM, so whatever your system's login accepts unlocks it. A wrong
password shakes the field, as the administrator's prompt does, unless the
theme has nothing move (`question { transition: none; }`), and then
nothing comes into view or goes gradually either. A finger on
the reader and a security key, such as a YubiKey, are asked for at the
same time, each through a PAM service of its own, and whichever you use
first unlocks: you never wait for a finger to time out before typing.
Each is offered only where its module is installed and you have
something enrolled: `pam_fprintd` with a finger enrolled by `fprintd-enroll`,
and `pam_u2f` with a key registered by `pamu2fcfg`. Turn either off with
`fingerprint: false` or `key: false` under `lock:`, as in the example
above. The password is always
offered, so that a reader that breaks or a key that is lost never keeps
you out. The screen says nothing of the other ways until they speak: a
reader once it asks for a finger, and a key once it is plugged in and asks
to be touched, so that it does not tell whoever is at your desk what else
would unlock it.

Along the bottom are Sleep, Restart, Shut Down and Log Out, so that the
machine can be put to sleep or turned off, or the session ended, without
unlocking. Each is there only where your system lets it be done without a
password, as it does for whoever is at the machine when nobody else is
logged in; a password could not be asked for over the lock screen.
Restart, Shut Down and Log Out ask first, since the programs that are open
close and what is not saved in them is lost; Cancel or Escape says no.
Sleep is done at once, and you come back to the lock screen. Tab moves
from the field to each button in turn, and Shift+Tab back; Enter presses
the one you are on, which looks `question-button:focus`. Asked whether to
go ahead, you are on Cancel, so that pressing Enter twice loses nothing;
typing goes back to the field. When a
program keeps the machine from turning off, such as a download, the
screen says which, and that button is not there. On a machine others use,
`power: false` under `lock:` takes them all away:

```yaml
# config.yaml
lock:
  power: false
```

The services are files the package installs in `/etc/pam.d`, yours to
change: `omorfos-lock` for the password (as your system's login asks),
`omorfos-lock-fingerprint` and `omorfos-lock-key`. Any one way unlocks, so
the session is as hard to get into as the weakest of them. To need a key
and the password both, put the key's module in `omorfos-lock`, before the
password's, and turn `lock.key` off: they are then asked in turn. A key
that asks for its PIN belongs there too, since only the password's field
is typed into.
