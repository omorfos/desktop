# Sound and light

## The volume, the brightness and what is playing

The keys a keyboard has for these work as they are: louder, quieter and
mute (`XF86AudioRaiseVolume`, `XF86AudioLowerVolume`, `XF86AudioMute`),
the microphone muted (`XF86AudioMicMute`), the screen brighter and darker
(`XF86MonBrightnessUp`, `XF86MonBrightnessDown`), and play or pause,
next, previous and stop. How loud or bright it is then shows as a
notification with a bar, which the next press changes in place.

They run `omorfos-session`, which can be bound to other keys or run from
a script as it is:

```yaml
# keys.yaml
bindings:
  - keys: Super+Up
    action: { spawn: [omorfos-session, volume, up] }
```

| Command | Does |
|---|---|
| `omorfos-session volume up`, `down`, `mute` | The sound that plays five hundredths louder, up to full, or quieter, or muted and back |
| `omorfos-session microphone mute` | The microphone muted, or heard again |
| `omorfos-session brightness up`, `down` | The screen a twentieth brighter or darker, never quite dark |
| `omorfos-session media play-pause`, `next`, `previous`, `stop` | Told to the player that is playing, or else to one that is paused |

The volume is set by WirePlumber's `wpctl`, as other mixers set it, so it
is the same wherever it is looked at; without WirePlumber the volume keys
do nothing. The brightness is set through logind, which lets whoever is
logged in at the machine set it. Players are told over D-Bus, as players
listen for (MPRIS); browsers and most music players do.

The keys do nothing while the screen is locked, and a key held down
changes the level once.

The radios, Wi-Fi, Bluetooth and the rest, are followed as the kernel
knows them, whatever turned them off or on: the key the laptop has for
them, which the kernel acts on itself, a switch on the side, or another
program. Each change is shown as the volume is, "Bluetooth off", and
"Aeroplane mode on" when every radio is off; the panel says "Aeroplane
mode", or which are off, for as long as they are. omorfos binds no key to
them, as the kernel has already done what the key asks.

## Night light

With `night_light.enabled: true` in `config.yaml`, the screens are made
warmer after dark, as though lit by a light of `night_light.temperature`
kelvin, 4000 unless you say otherwise: 6500 is daylight, as screens
are, and lower takes away more blue. Dark is as `appearance.scheme: auto`
has it, from sunset to sunrise where `appearance.location` says, or
outside the hours of `appearance.day`, whatever the scheme is; the
screens turn warm over the 20 minutes after dusk, and back over the 20
before dawn.

```yaml
night_light:
  enabled: true
  temperature: 3400
```

The `night_light` action switches it by hand until it next turns: off
for the rest of the evening, or on now, as warm as it says, even where it
is not enabled. Each change fades in over two seconds rather than
jumping at you, but for the first after the session starts, which is
there at once. It is done through each screen's gamma table, after
everything is drawn, so pictures of the screen, and a screen shared, keep
their colours. A screen whose table cannot be set says so once, in the
session's log.
