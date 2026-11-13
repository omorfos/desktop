# Installing a new version

Installing omorfos again, as `make install` or a package does, takes up
the new versions of the programs that come with it within a second or
two, without logging in again: the panel, the wallpaper, the
notifications, the portal, the clipboard keeper and the rest are each
started again as their new version. What is on the clipboard is handed to
the new keeper first. The portal waits while a screen is being shared,
and the agent that asks for passwords while it is asking.

omorfos itself is taken up too, its windows staying where and as they
were, at a moment you are not using the session: nothing fullscreen,
nothing recording, playing or keeping the screen awake, the prompt for a
password and the launcher closed, and no key pressed nor the pointer
moved for five seconds. The screen blinks as it starts again. It waits
while the screens are off, as they would come on, until you are back. If
no such moment comes within an hour, or an hour after you were last
back, it is taken up regardless, and a notice says so afterwards. `wait` sets how long it waits, in seconds, up to a
week; with 0 it waits for such a moment however long:

```yaml
update:
  wait: 0
```

While a program written for X11 has a window open it is not taken up at
all, as that program would close with its X server: a notice says it
waits for them to close, or for you to log in again. Where nothing keeps
the programs through it, as where omorfos is not the whole session, run by
`omorfos-hold` and its relay, a notice says the new version is used once
you log in again.
