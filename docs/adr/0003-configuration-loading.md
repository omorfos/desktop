# 3. Configuration loading

- Status: accepted
- Date: 2026-10-09

## Context

Configuration is YAML in `~/.config/omorfos/`, reloaded live. A mistake must
never break a running session, and the user must be told exactly what is
wrong. Configuration files are untrusted input: they are often copied from
the internet, and YAML has well-known denial-of-service constructs.

`serde_yaml`, the usual choice, is archived and unmaintained.

## Decision

**Parser: `serde-saphyr`**, with only its `deserialize` feature. It is pure
Rust, reports a line and column for every error, and enforces budgets on
nodes, aliases and nesting depth. We verified that an alias bomb and a
5,000-level nesting are both rejected; a test keeps the former true. The
maintained forks of `serde_yaml` were rejected because they keep its
transpiled C parser and its `unsafe`. Booleans are strict (`true`/`false`
only) and duplicate keys are errors.

**Validation happens during deserialization.** Constrained values are types
(`Color`, `Bounded`, `KeyChain`, `Command`) that convert with `try_from`, so
every error carries the parser's position and a `Config` is valid by
construction. Unknown keys are errors, which catches typos.

**Files are independent.** Each of the five files is loaded on its own. An
absent or empty file means defaults; a file that fails keeps its previous
value while the others apply. This is more useful than rejecting the whole
reload: a broken theme should not block a keybinding fix. The cost is that
no validation spans files, so cross-file references (a rule naming workspace
9 when there are 4) must be handled where they are used.

**Files are capped at 1 MiB**, checked while reading.

**Watching uses inotify on the directory**, through the `inotify` crate. The
directory is watched rather than the files because editors save by renaming
a temporary file over the original. The watcher exposes a file descriptor
and a non-blocking `drain`, and the debouncer takes the time as an argument,
so the compositor's event loop owns all waiting and neither piece needs a
thread. The cross-platform `notify` crate was rejected as more than a
Linux-only project needs.

**`spawn` takes a list, not a string.** Programs are run directly without a
shell, so nothing in a configuration file is subject to shell expansion.

**JSON Schemas are generated from the same types** (`schemars`, behind the
`schema` feature so the compositor does not link it) and committed in
`schemas/`. A test fails if they are stale.

## Consequences

- Only the first error in each file is reported, because deserialization
  stops there. Errors in different files are all reported.
- A file symlinked from elsewhere is not seen to change when its target is
  edited in place. A manual reload command will cover this.
- Key names are not yet checked against real keysyms; that needs
  libxkbcommon and arrives with the compositor.
- `serde-saphyr` brings `encoding_rs` for input decoding, its largest
  transitive dependency.
