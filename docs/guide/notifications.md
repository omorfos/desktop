# Notifications

What programs have to tell you is shown in the top right-hand corner of the
screen the pointer is on, the newest first: a title, up to three lines
broken to fit, and a
picture or the program's icon if it sent one. A click dismisses one, and
tells the program that sent it, which is how clicking a browser's
notification brings up its tab; a click with the right button dismisses it
without that. Otherwise it goes after as long as its sender asked for, or
after `notifications.timeout` seconds. One its sender marked as critical
stays until it is clicked.

Each comes and goes over the theme's `notification` transition, and those
around it make room for it, or take the room it leaves, by gliding there
over 240 ms rather than jumping; a theme whose `notification` changes at
once, or `appearance.reduce_motion`, has them jump.

What omorfos itself has to say is wrong, such as a mistake in a
configuration file, is shown here too, headed `omorfos` and looking as a
critical notification does. It is given up to six lines, stays until you
click it away or put it right, and is never the one to go when there are
too many. What has merely happened and needs nothing done, such as the X
server having been started again, is shown the same way but looking as
an ordinary notification does, and goes by itself like one. If you turn
these notifications off, or use another program for them, the compositor
shows both in a notice of its own.

A picture may be a PNG or an SVG, given by its path, or an icon by name.
Icons are looked for in the `hicolor` and `Adwaita` themes; where a theme
has one only as a shape, it is drawn in the colour of the text.

`omorfos-notifications` is a program of its own, started with the session,
and started again if it crashes. If it stops more than three times in a
minute, as it will when another program is already showing notifications,
omorfos leaves it stopped and says so. It answers to the usual desktop notification service, so `notify-send` and
applications work with it as they are. It needs the session's D-Bus, which
a session started from a login manager has.

How notifications look is in `theme.css`, under `notification`,
`notification-title`, `notification-body` and `notification-level`; a
critical one is `notification:urgent`. Their width is `min-width`, and
`margin` is the room between them and around them.

A notification can say how full something is, out of 100, by the usual
`value` hint, as the volume and brightness keys do: it is drawn as a bar
of `notification-level` under what it says. One sent with the hint
`synchronous` (or `x-canonical-private-synchronous`, or
`x-dunst-stack-tag`) takes the place of the one showing with the same
tag, so that holding a key down shows one notification that changes
rather than a pile of them:

```sh
notify-send -h int:value:40 -h string:synchronous:volume Volume
```

Only one program can show notifications. To use another, or none, turn this
one off in `config.yaml`:

```yaml
notifications:
  enabled: false
```

Like `autostart`, this takes effect the next time you log in.
