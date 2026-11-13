# 73. What a version promises

- Status: accepted
- Date: 2026-11-12

## Context

Releases are made from the commits' messages (ADR 6): a commit marked
breaking, with `!` or a `BREAKING CHANGE:` footer, raises the version
further than one that is not, so that whoever takes the release knows to
look. What counts as breaking was never written down. Nothing here is a
library others build on, so the usual answer, the public interface of the
crates, is not the one: what is depended on is what users write and
other programs read. Without saying so, a change is marked breaking or
not by whoever happens to write it, and CI cannot check what has no
rule.

## Decision

**A version promises what someone else writes down or starts by name;
what only a person reads may change in any release.** A change that
takes away or changes the meaning of anything below is breaking, and is
marked so. Adding to it is not.

- **The configuration**: the files' names, `config.yaml`, `keys.yaml`,
  `rules.yaml`, `theme.css` and `menu.yaml`, in `$XDG_CONFIG_HOME/omorfos`
  or `~/.config/omorfos`; each key and value the schemas in `schemas/`
  describe; and the actions' names. A file written for one version means
  the same to the next: a key is not renamed, nor a value given another
  meaning, without the change being marked.
- **The theme**: its parts, states, properties and variables, as
  the guide lists them, so that a stylesheet of the user's
  styles what it styled.
- **The IPC protocol**: each request and reply, and each field of them,
  as `docs/IPC.md` says. Fields may be added; one taken away or renamed
  raises `PROTOCOL` and is marked.
- **`omorfosctl`'s commands**, their arguments, the JSON that `state` and
  `watch` print, and the exit codes. What the rest print, `version`,
  `screens`, `frames`, `report` and every error, is written for people
  and may be reworded.
- **The programs' names**, `omorfos`, `omorfosctl`, `omorfos-session`,
  `omorfos-pinentry` and the rest: the login screen's session entry,
  `gpg-agent.conf`, `keys.yaml` and the user's scripts start them by name.
  Where they are installed is the packaging's.
- **The IPC socket's name**, `omorfos-$WAYLAND_DISPLAY.sock` in
  `$XDG_RUNTIME_DIR`, and `OMORFOS_SOCKET`, by which tools find it. The
  Wayland socket's name is not: programs follow `WAYLAND_DISPLAY`,
  whatever it says.
- **Where the session's log is kept**, `session.log` and
  `session.log.old` in `$XDG_STATE_HOME/omorfos` or
  `~/.local/state/omorfos` (ADR 60), which the guide sends people to. What
  is in it is not: its lines are written for people, and change as the
  code does.

**What comes built in is not promised, but is said.** A changed default
key binding, look or value of a setting is not breaking: what the user
wrote still means what it meant. It goes in as a `feat` or `fix` whose
subject says what changed, so that the changelog does. The defaults are
to be gone through together in stage 4, and are free to be.

**What omorfos's own programs say to each other is not a promise to
anyone else.** The holder, the relay and the compositor meet across
versions as one is installed, and keep to their own rule for it, that
lines are added and never changed (ADR 60), which their tests hold them
to.

Before 1.0 a breaking change raises the minor version, as ADR 6 has it;
the promise is the same, and the version says less.

## Alternatives

- **All that `omorfosctl` prints promised**, so that a script may read
  `screens` or `frames`. Every rewording of a line for people would be a
  breaking change, and the JSON of `state` already says what they say.
- **The defaults promised**, as someone may rely on one without having
  written it down. The review of the defaults that stage 4 plans would
  then be a string of breaking changes, and a better default would wait
  for a major version.
- **The session log's lines promised**, for whoever reads them with a
  script. It would hold still every message the compositor writes; what
  is to be read by programs is in the IPC protocol instead.

## Consequences

- `docs/RELEASING.md` says what a version promises, beside how the
  version is chosen, for whoever writes a commit to look up.
- The schemas, the IPC protocol and the theme's lists are what a change
  is checked against. CI that finds a breaking change not marked as one
  starts with the schemas (ROADMAP, stage 8).
- A rename that has to come is made by taking the new name beside the old
  for a release, and taking the old away, marked, after.
