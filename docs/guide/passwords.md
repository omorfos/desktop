# Passwords and passphrases

## Asking for the administrator's password

When a program wants to do what needs the administrator, installing
software or changing the date, polkit asks the desktop to ask you, and
`omorfos-ask` does, which omorfos starts with the session. A box in the
middle of the screen says what is asked for and whose password it wants:
yours, if you are allowed to say yes, as an administrator is, and
otherwise the first of those who are. Type it and press Enter; Escape or
Cancel gives up. Backspace held down and Ctrl+A or Super+A work as on the
lock screen. The eye at the end of the field shows what you typed, or
hides it again, and says so when the pointer rests on it. Tab moves from
the field to the eye, Cancel and Authenticate in turn, and Shift+Tab
back; Enter presses the one you are on, which looks
`question-button:focus`, and typing goes back to the field. A wrong
password is asked for again, and the box shakes; a theme that has the
`question` part change at once, `question { transition: none; }`, keeps it
still.

While it asks, everything else is dimmed and blurred, and the keyboard and the
pointer reach the box alone: no key binding works, no other program and
no input method sees a key. Only a program omorfos started with the `ask`
role can do that, which is how you know the box is omorfos's own and not
a window imitating it. The dim is the theme's `sealed` part, blurred as
far as its `backdrop-filter` says, and the box
is drawn as the `question` parts, its field as `question-input`.

What you type goes only to polkit's own helper, which checks it, and is
not kept once it has been handed over. Programs written for X11, and one in a
sandbox, ask through polkit like any other.

## Asking for a key's passphrase

GnuPG asks for the passphrase of a key, to sign, decrypt or use a key it
holds for SSH, through a pinentry. Name omorfos's in `gpg-agent.conf`, and
the same sealed box asks:

```sh
echo "pinentry-program /usr/local/bin/omorfos-pinentry" >> ~/.gnupg/gpg-agent.conf
gpg-connect-agent reloadagent /bye
```

It says only what omorfos says: that GnuPG asks for the passphrase of a
key, or for a new one twice, with the key's name and fingerprint as GnuPG
lists them. What GnuPG gives the box to say is not shown, as any program
can give it words to say there, such as "Type your login password"; it is
in the session's log, under `omorfos-ask`, if you want to read it. A wrong
passphrase is asked for again. Asked to confirm something, it says no,
and GnuPG goes on as if you had. GnuPG's question gives nothing but its
own words to say what is to be confirmed, and any program can ask one
with words of its own, so the box does not ask it. A new passphrase
GnuPG thinks too weak, which it asks you to confirm, is not taken: choose
a stronger one, or set `min-passphrase-len` and `min-passphrase-nonalpha`
in `gpg-agent.conf`. An SSH key added with `ssh-add -c`, which asks to be
confirmed each time it is used, cannot be used: add it without `-c`.

Outside an omorfos session, or with `omorfos-ask` not running, it asks
nothing and GnuPG says so; name another pinentry there. Only
`omorfos-pinentry` started by `gpg-agent` is heard.
