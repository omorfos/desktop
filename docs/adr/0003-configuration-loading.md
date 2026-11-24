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

**What loaded is kept for the session.** The previous value of a file lives
in the process, which a compositor started again has not got. So each file
that loads is copied to `$XDG_RUNTIME_DIR/omorfos/last-good/`, the user's
alone and emptied at logout, and a file that fails at startup is given its
copy there in place of the defaults: a compositor started again after a
crash, or for its new version, comes up as it was, though a file was saved
with a mistake meanwhile. The copies are not kept beside the files, where a
configuration kept in git or shared between machines would carry them, nor
across sessions, where one could be run long after the file it came from was
changed: a new session loads what is on disk. Each file stands alone, as in
a reload; a copy is read as any file is, and one that does not load is the
defaults. The directory is handed to the store by the hardware backend, not
read from the environment by it, so that tests keep their own. Every other
program of the session starts from the copies too: the parts the compositor
starts and starts again (ADR 28), the portal, the password box, the lock
screen, the launcher, the screenshot tool and the welcome card. Each finds
them from the `XDG_RUNTIME_DIR` it is given, by one way in,
`Store::open_in_session`, or is handed where they are by whoever found them
so, and writes none: the compositor, which runs for as long as they do, is
the one that keeps them, and a part that opens the configuration each time
it is asked, as the portal does, does not write files each time. The
welcome card shows what is chosen as the copies have it, as the desktop
behind it goes by them, but saves a choice to the file on disk alone, which
refuses it while the file does not load. That none opens the configuration
without the copies is held by a test that reads every program's source
(`crates/omorfos-config/tests/kept_copies.rs`): three once did, each by a
line that read as right alone.

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
- The directory itself may be taken away and put back, or be a link
  pointed elsewhere: the directory it is in is watched for its name, and
  the directory watched again wherever its path then leads (finding 2 of
  the review of 22 November 2026). Its going, or its being moved aside,
  is a change, and what it became is not watched. A link may lead nowhere
  for a while, as when the directory it points at is made again, and
  nothing need say when it leads somewhere again: so while it leads
  nowhere it is tried again each time the watcher is drained, and the
  watcher wakes of itself once a second meanwhile, so that it is. The
  compositor drains it every turn of its loop; the bar, the wallpaper,
  the notifications and the portal sleep until it can be read, and would
  otherwise stay with the defaults its going left them. It wakes so only
  where nothing else would say, for a link, or where the directory above
  may not be watched: a directory that is simply not there, as where
  nothing was ever configured, wakes nothing, and its name is heard of
  when it comes (finding 6 of the review of the pull requests merged on
  23 November 2026).
- The directory above being replaced, as `~/.config` moved aside and put
  back, is not seen: both watches stay on what was there before, and an
  edit is next seen when a program is started again. Seeing it would take
  watching every directory up to the root.
- Key names are not yet checked against real keysyms; that needs
  libxkbcommon and arrives with the compositor.
- `serde-saphyr` brings `encoding_rs` for input decoding, its largest
  transitive dependency.
