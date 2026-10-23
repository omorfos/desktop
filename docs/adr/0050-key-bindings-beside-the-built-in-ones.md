# 50. Key bindings are made beside the built-in ones

- Status: accepted
- Date: 2026-10-23
- Amends: the `keys.yaml` of [ADR 3](0003-configuration-loading.md)

## Context

`bindings` in `keys.yaml` replaced every built-in binding. To bind one
key, `Super+l` to `lock`, a user had to copy all sixty built-in bindings
into the file, and keep the copy up to date as they changed, or lose the
terminal, the launcher, the workspaces and the volume keys. The
maintainer met it as a user does: one line added, and the desktop's keys
gone.

## Decision

**What is written is made beside the built-in bindings.** A built-in
binding goes on working on every key the file leaves alone. One written on
keys a built-in binding uses takes its place, as does a chain that begins
with them, which the built-in one would otherwise leave unreachable; that
is what writing it means, and is not a conflict. Conflicts are still
refused among what the file says itself.

**A built-in binding is done without by naming its keys in `unbind`.**
`action: none` was the other way, but `Action` is what keys, the IPC
socket and the menu share, and an action that does nothing would be one
for all of them to know of; and a list says plainly what it is for. A key
in `unbind` that no built-in binding uses is refused, since it is a
mistake that would otherwise do nothing.

**`defaults: false` has only what is written,** as before.

## Consequences

- A breaking change: a `keys.yaml` that left built-in bindings out on
  purpose gets them back, unless it says `defaults: false`. Each file
  that bound the same keys as the built-in ones behaves as before.
- The bindings in force are those written first and then the built-in
  ones kept, in their order; where an action has two keys, the first
  found is the user's.
- A new built-in binding, in a release, reaches every user whose file
  does not take its keys, which is what the defaults are for.
