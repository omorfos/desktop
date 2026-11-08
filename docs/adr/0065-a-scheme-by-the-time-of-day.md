# 65. A scheme by the time of day, worked out by each program

- Status: accepted
- Date: 2026-11-08

## Context

`appearance.scheme` chose dark or light. `auto` was to make the desktop
light by day and dark by night: from sunrise to sunset where the user is,
or between hours of the day without a location.

There is an open standard for telling programs the scheme: the settings
portal's `org.freedesktop.appearance` `color-scheme`, with a
`SettingChanged` signal, which GTK, Qt, Firefox, Chromium and Electron
follow, each free to keep a theme of its own instead. omorfos's portal
already gives it, worked out from the theme, and says when it changes. So
other people's programs follow whatever the desktop's scheme is.

What did not exist is how omorfos's own programs would know. The
compositor, the panel, the notifications, the wallpaper and the portal
each load the configuration themselves, watch its directory, and load it
again when a file changes. Under `auto`, nothing changes in a file at
sunrise.

## Decision

**Each program works the scheme out.** Loading the configuration resolves
`auto` against the time it is loaded at, and keeps the scheme it resolved
and when it may next change (`Config::scheme`, `Config::changes`). The
watcher of the configuration's directory is also a timer: told when the
store is next due, it wakes then as though a file had changed, and the
program loads the configuration again as it would for a file. The timer
goes by the wall clock and is told when the clock is set, so a suspend or
a change of clock is a reason to ask the time again. It is asked again by
every midnight as well, so that a day the sun does not set is followed by
one it does.

**The sun is worked out here**, by NOAA's general solar position
formulas, within a minute or two of an almanac's tables, which the tests
hold it to. There is no dependency for it; the local time, for the hours
of the day, comes from `jiff`, which was already in the tree.

**The location stays in `config.yaml`.** It is read to work out when the
sun rises, and is told to no program.

## Alternatives

- **Our programs follow the portal**, as other programs do. One place
  decides, by the standard. But every component would need a connection
  to the bus, would stop following when the portal is not running, and
  the compositor, which draws most of the desktop, would still have to
  work the scheme out itself: the portal reads it from the configuration.
- **The compositor announces it** over its socket, and every component
  subscribes, as the panel does. One clock, but the notifications, the
  launcher, the prompts, the lock screen, the wallpaper and the portal
  would all have to start listening, and none would follow without the
  compositor.

## Consequences

- The programs change scheme within a moment of each other, each by its
  own timer and the same clock, and the portal tells other programs at
  the same moment.
- The programs that load the configuration only as they start, the lock
  screen, the box that asks for a password and the launcher, keep the
  scheme they were opened in. A screen locked overnight is in the scheme
  it was locked in until it is unlocked.
- A change of time zone is seen at the next change, the next midnight, or
  the next file saved, not at once.
- `07:00` to `19:00` is the day without a location; like the other
  defaults, it is to be reviewed.
