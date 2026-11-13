# The clipboard

What you copy is kept after the program you copied it from closes, by
`omorfos-clipboard`, which omorfos starts with the session. It reads what
is copied, in every form the program offers it in, as soon as it is
copied, and offers it in the program's place.

Some things are left with the program that copied them, and go when it
closes: what a password manager copies, which it marks as secret, so that
it goes when the password manager takes it away; what is only selected,
for the middle button; and what is larger than 64 MiB, or not handed over
in any form within two seconds. To keep nothing, or have another clipboard manager do
it, with `may: [clipboard]`:

```yaml
clipboard:
  enabled: false
```

What is kept can also be forgotten after a while, for passwords copied
from somewhere that does not say they are secret: with `forget: 60`, the
clipboard is emptied a minute after something was copied, even if the
program that copied it is still open, unless something else was copied
since. 0, as it comes, keeps it for as long as the session lasts.

Like `autostart`, these take effect the next time you log in.

A program written for X11 that copies, as `xsel` and `xclip` do for an
editor in a terminal, goes on holding what it copied for the others
written for X11, so that copying and pasting among them works as it does
anywhere. A program written for X11 may read what a program written for
Wayland copied only while one of its windows has the keyboard, so `xsel`
pasting into an editor in a terminal, which has no window, gets nothing
from them: give such an editor `wl-copy` and `wl-paste` (`wl-clipboard`)
instead, as Neovim prefers them where they are installed.
