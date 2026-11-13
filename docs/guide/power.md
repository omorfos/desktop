# Sleep, restart and shut down

Whatever puts the machine to sleep, the lid, the sleep key or a command,
omorfos locks the screen first with the `locker` you named, and the
machine sleeps only once nothing of the session is on any screen, so
that it never wakes unlocked. logind waits a few seconds for that, five
unless it is set otherwise; a locker that has not locked by then is let
be, and you are told when the machine wakes. With no `locker` named the
machine sleeps at once. `loginctl lock-session` locks the same way;
`loginctl unlock-session` does nothing, since only your password opens a
locked screen.

| Command | Does |
|---|---|
| `omorfos-session sleep` | Sleeps |
| `omorfos-session restart` | Starts the machine again |
| `omorfos-session shut-down` | Turns it off |

Each is asked of logind, which decides whether you may. What the lid,
the power button and the sleep key do is logind's to say, in
`/etc/systemd/logind.conf` (`HandleLidSwitch`, `HandlePowerKey`,
`HandleSuspendKey`); omorfos binds none of them, since logind has
already acted on them, and a binding as well would sleep twice. To
have one ask something else, set it to `ignore` there and bind the key:

```yaml
# keys.yaml
bindings:
  - keys: XF86PowerOff
    action: { spawn: [omorfos-session, shut-down] }
```

All this needs logind; without it, nothing locks before sleep.
