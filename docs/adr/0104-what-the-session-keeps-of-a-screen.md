# 104. What the session keeps of a screen

- Status: accepted
- Date: 2026-11-23
- Takes up: [ADR 100](0100-a-screens-scale-by-how-near-it-is-read.md),
  [ADR 103](0103-a-screen-known-by-what-it-says-it-is.md)

## Context

What is settled for a screen is to come in three layers, each over the
one before: what omorfos guesses, what the session has kept for that
screen, and the user's own files (roadmap, stage 3). Until now there
were two: a scale guessed each time a screen was taken on, by a rule
that changes between versions, as it did on 23 November for the
maintainer's laptop; and `config.yaml`. The way the display key kept
the screens in lasted until the next login, and the guide said so.

## Decision

**The session keeps, in `$XDG_STATE_HOME/omorfos/screens.yaml`, the scale
it guessed for each screen at each size it was driven at, and the way
the display key last kept the screens standing in.**

- A scale is guessed once for a screen at a size, and kept: guessed again
  for it later, by this version or another, the kept one is used. The
  user's files are laid over it, as over any guess; asking for it to be
  guessed again is the next item.
- A screen is kept by all it says it is (ADR 103), and by its socket too
  where it says no serial, as two alike are then told apart by nothing
  else. One that says nothing, as inside another desktop, keeps nothing.
- The way the display key keeps is kept for the screens it was kept
  for, by the names the configuration knows them by, and taken up at the
  next login; keeping the configuration's way forgets it.
- The mode is not kept: what a screen says suits it best is what it says
  each time, and one in `config.yaml` is the user's.
- The file is the session's, beside the session's log, not the user's
  configuration. It is written whole, aside and then put in place, and
  read as anything is that the user could have edited: one that cannot
  be read is as none, and every screen is guessed for again. It is not
  written by a compositor inside another desktop, which would have it
  speak for the session's screens.
- It is written by hand and read with the YAML reader already used; the
  YAML library's writer would bring three crates more.

## Alternatives

- **Guessing again at every start, as before.** A better rule then
  changes a screen the user has grown used to, unasked.
- **Keeping the guess in `config.yaml`.** The user's file would change
  under them, and a guess would read as a choice.
- **Keeping it with the relay.** It outlives the compositor, not a login.

## Consequences

- A rule changed in a later version reaches only screens not seen
  before, until the user asks for a screen to be guessed for again.
- Deleting the file starts over, and tells the user how in the guide.
- Placement and rotation are kept only as the display key's way keeps
  them, until there are settings to choose them in.
