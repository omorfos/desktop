# 103. A screen known by what it says it is

- Status: accepted
- Date: 2026-11-23

## Context

`config.yaml` names a screen by the socket it is plugged into, as
`HDMI-A-1`, so another monitor on that socket takes the last one's
settings, and one moved to another socket loses its own (roadmap, stage
3). A screen says what it is in its EDID, the block of facts every
monitor and panel gives: who made it, by three letters registered for
the maker; the maker's number for it; and, where it says them, its
name and its serial, in descriptors of text, or its serial as a number.

Not every screen says all of that. The maintainer's laptop panel, a BOE
NV140QUM-N53, says its maker and number, and its model only as free
text; no name, and no serial of either kind. Two monitors of one model
that say no serial cannot be told apart by anything they say.

How other systems write a screen differs. sway and niri take one string,
the maker's full name, the model and the serial, which needs a table of
makers' names and reads oddly where a field is missing. KDE keeps a
fingerprint of the whole EDID, which no one can write by hand.

## Decision

**A screen is known by what it says, through names the user gives in
`screens`, which `outputs`, `arrangements` and `mirror` take in place of
a socket's name.**

```yaml
screens:
  laptop: { maker: BOE, product: 0x07c8 }
  office: { maker: DEL, name: DELL U2720Q, serial: 7X3KD13 }
outputs:
  - { name: office, scale: 1.5 }
  - { name: laptop, place: below }
```

- A screen in `screens` says any of `maker`, three capital letters, its
  `product`, its `name` and its `serial`, at least one; a screen plugged
  in is it if it says all of them.
- A name in `outputs`, `arrangements` or `mirror` that is in `screens`
  stands for a screen that is it; any other name is a socket's, as
  before, so every configuration written so far means what it did.
- Each screen plugged in is taken for the first such name used that it
  is, and that no screen plugged in before it has been taken for; one
  listed by its socket's name is that socket's, whatever it says. Two
  monitors alike are told apart by two names that say the same of them,
  in the order they were plugged in. Where every name has been taken,
  another alike is known by its socket.
- A name in `screens` may not end in a dash and a number, as sockets'
  names do, so that no name is both; and none is empty. These are
  refused as the file is read, as any mistake in it is, and the last
  configuration that loaded kept meanwhile.
- The first arrangement whose names, so taken, are just the screens
  plugged in is used, as before; the names are worked out again only as
  a screen comes or goes and the configuration changes, so looking up
  a screen's settings allocates nothing.

What a screen says is read from the first block of its EDID by
`omorfos-core`, tested, from bytes the hardware backend hands it: the
header has to be as the standard has it, the maker three letters, and
text is taken only if it is printable, as a screen may say anything. A
serial given in text is taken over the number, and a number of 0 is
none. `omorfosctl screens`, and the IPC state, show what each screen says,
written as `screens` takes it, and the name it is known by.

## Alternatives

- **A match written in each entry of `outputs`.** What was first
  proposed. It says the same of a screen again in every arrangement it is
  in, and leaves `mirror` with no way to name it.
- **One string per screen, as sway and niri.** Configurations could be
  copied from them, but their makers' names come from a table, and a
  panel that says no name or serial is a string with gaps in it.
- **A fingerprint of the EDID.** Tells almost any two apart, but cannot
  be written or read by the user; it may yet be what the session keeps
  of a screen, which the user does not write.
- **A name not in `screens` and not written as a socket's an error.**
  Sockets are not all written so: a backend inside another desktop
  names its own, and the tests do. A name that matches nothing is a
  screen not plugged in, as a socket's always was.

## Consequences

- Settings follow a monitor from one socket, or dock, to another.
- `wallpaper.screens` in `config.yaml` names screens by their sockets
  still; it is read by a program of its own, which would need what each
  screen says passed to it.
- A screen inside another desktop, and one that gives no EDID, says
  nothing, and is known by its socket alone.
- What the session keeps of a screen it has seen, the next item on the
  roadmap, can be kept by what it says, with its socket only where it
  says too little to be told from another.
