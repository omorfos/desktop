# 83. What the launcher does is on unless turned off

- Status: accepted
- Date: 2026-11-15

## Context

The launcher began by starting programs, and has been given more to do
with what is typed: sums worked out, their answers copied. Stage 6 has
more waiting: files in the home directory, windows to go to, shortcuts
such as a web search. Each is useful to most people and unwanted by some,
who would rather the launcher only started programs, or not look through
their files.

The maintainer asked for sums to be on by default and for a way to turn
them off, and for that to be how each thing the launcher does is added
from now on.

## Decision

**Each thing the launcher does, beyond starting programs, has a key of its
own under `launcher` in `config.yaml`, true by default; set to false, it
is not done at all.**

- `launcher.sums` is the first: off, what is typed is looked for among
  the programs as anything else is, and nothing is worked out.
- What only changes how a program is shown follows the same rule:
  `launcher.icons` and `launcher.descriptions` are on unless turned off.
- A thing that needs more than on or off, as which directories files are
  looked for in, is a mapping under its key with an `enabled` beside its
  other settings, as `notifications` has, so that turning it off stays
  one line.
- Off means not done: a feature turned off reads nothing, and works out
  nothing, rather than doing its work and hiding it.

## Alternatives

- **Off by default, turned on by whoever wants it.** Nothing surprises
  anyone, but what the launcher can do would be found only by reading
  the guide, and most of it would go unused.
- **One list of what is on, as `launcher.features: [sums, files]`.**
  Short, but a feature added later would be off for everyone who had
  written a list, and a misspelt name would need its own error.
- **No way to turn them off.** Fewer keys, but someone who does not want
  their files looked through, say, would have no choice.

## Consequences

- Each feature added to the launcher adds a key to `config.yaml`, its
  schema and `schemas/promised.txt`, and to the guide's example, which
  is checked against the defaults.
- A new feature is on for everyone the moment it ships, including those
  who wrote a configuration before it existed; its release notes have to
  say so, and how to turn it off.
