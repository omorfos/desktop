# 46. The volume, the brightness and what is playing

- Status: accepted
- Date: 2026-10-21

## Context

Keyboards have keys for the volume, the brightness of the screen and
what is playing, and a desktop is expected to make them work without
being told to. ADR 35 settled that the compositor speaks to no service,
that `omorfos-session` is what speaks to logind, which sets the
brightness, and left how the volume is set to be decided with this.

Each key needs three things: something to do what it asks, something to
show how loud or bright it is then, and a binding.

## Decision

**`omorfos-session` is a command, for now.** Each key runs it once:
`omorfos-session volume up`, `brightness down`, `media play-pause`. It
does what is asked, shows the level and ends. A command needs nothing
kept running for keys that are pressed a few times an hour, can be bound
to any key or run from a script, and is what ADR 35 already has the
menu's sleep and shut down run. The part that runs all session, which
locks before sleep, comes with that item and is the same program run
without a command.

**The volume is set by `wpctl`, WirePlumber's own tool, not by speaking
to PipeWire.** On a sound card WirePlumber does not set the volume of
the stream the card plays: it sets the card's own mixer, through the
card's route, on a cubic scale, and keeps it for next time; only where
there is no card does it set the stream's. Every other mixer, `wpctl`,
the PulseAudio server and the panels of other desktops, does the same,
so that the volume is one number wherever it is looked at. Setting it
by speaking to PipeWire ourselves would mean doing all of that again,
and a sound card's route can only be reached where there is a sound
card, which CI has not: the most important path would be the one never
tested. `wpctl` is there wherever WirePlumber is, which is wherever
PipeWire is set up as a desktop's sound. It is run with a time after
which it is given up, and what it says is read and checked.

**The brightness is set by logind** (`SetBrightness` on the session),
which lets whoever is logged in at the machine set it without being able
to write to the backlight. Which backlight, and how bright it is, is
read from `/sys/class/backlight`: one the firmware drives before one of
the platform, before one of the card itself. A step is a twentieth of
the whole, never less than one level, and never down to nothing, which
on some screens is off.

**What is playing is told over D-Bus**, as players listen for (MPRIS):
the one that is playing, or else one that is paused, or else any.

**The level is shown as a notification.** The notifications learnt two
hints that other notification services know: `value`, a level out of
100 drawn as a bar under the title, and `synchronous` (also written
`x-canonical-private-synchronous` and `x-dunst-stack-tag`), by which one
takes the place of the one showing with the same tag, whatever its
number. A command that ends after each key cannot remember the number
of the notification it sent before; the tag is how pressing a key five
times shows one notification that changes and not five. Because they
are the usual hints, a user of another notification service sees the
same, and anything else that sends a level is shown as one.

**The keys are bound by default.** These keys mean nothing else, and the
roadmap asks for them to work as they are. They run `omorfos-session` as
an ordinary program: it needs no role, since it changes nothing that is
the compositor's.

## Alternatives

- **A display of its own, in the middle of the screen.** What GNOME and
  macOS show. It is another surface and another program for what a
  notification already shows, and a theme can already move and size
  notifications. It belongs to the look (stage 4), if it is wanted.
- **Speaking to PipeWire for the volume.** Rejected above: what it would
  have to do again is what makes a volume right, and it could not be
  tested.
- **`pactl` or `amixer`.** The first speaks to PipeWire through its
  PulseAudio server, which may not be there; the second to the card
  alone, behind PipeWire's back.
- **The compositor doing it all.** Against ADR 35: a bus that does not
  answer would be a frozen screen.

## Consequences

- WirePlumber is recommended by the packages. Without it the volume keys
  say why in the session's log and do nothing.
- The keys do nothing while the screen is locked, as no binding does.
  Letting a few through, as other desktops do, is a hole to be made with
  care, and is left for the lock screen of our own.
- A key held down changes the level once: the compositor runs a binding
  when its key goes down, not again as it repeats.
- A command that does not finish within two seconds is given up, so a
  press is never kept waiting behind one that hangs.
