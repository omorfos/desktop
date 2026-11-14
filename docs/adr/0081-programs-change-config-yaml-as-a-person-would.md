# 81. Programs change `config.yaml` as a person would

- Status: accepted
- Date: 2026-11-14

## Context

The configuration is files a person writes, with comments, an order of
their own and keys left out for their defaults, and the compositor and
the components follow each file as it is saved. The welcome card shown
at the first login offers a wallpaper and a scheme, and Settings (stage
7) is to offer more, written "to the files as they are made... what a
person could have written", keeping what it does not understand. Both
have to change a value in `config.yaml` without losing the rest of what
is written there. Reading the file into the configuration's types and
writing them back out would lose every comment and default left out.

## Decision

**`omorfos_config::edit` changes the text of the file, not its values:
it finds where a key is written and replaces only that, or adds the key
where a person would.**

- **Where things are** comes from `granit-parser`, which `serde-saphyr`
  reads YAML with: each key and value with where it starts and ends, and
  how deep a key is. A value written is replaced where it is written; a
  key not there goes after the last line of the mapping it belongs in,
  as deep as the keys beside it, and before any comment written above
  the next section; a section not there goes at the end; an empty file,
  or one of only comments, is written to.
- **A mapping of values** (a wallpaper's dark and light pictures) is
  written on one line, as `{ dark: a, light: b }`, where one value or
  one such line was; where the mapping is written a key to a line, each
  of its keys is set where it is.
- **A word is written as it is** where nothing could take it for
  something else, and in double quotes otherwise: a number, `yes`, `~`,
  a space, a `#`.
- **What would need a guess is refused**, and the file left alone, with
  why: a value given by an anchor or an alias or with a tag; a block of
  lines (`|`, `>`); a list; a key written twice; a key to add to a
  mapping written on one line; more than one document; a file that does
  not read as YAML.
- **Nothing is written that does not read back**: each change is read
  again and must hold what was set; the file must have read as
  configuration before, and must after. It is written whole, to a file
  beside it put in its place, so the watcher sees one change and a
  reader never half of one; through a link, the file linked to is
  written, with its permissions as they were.

## Alternatives

- **Reading into the types and writing them back.** Simple, and every
  comment, order and default left out would be lost on the first change.
- **A file of its own for what programs choose**, read under
  `config.yaml`. Nothing a person wrote would ever be touched, but one
  setting would then live in two places, and a value in `config.yaml`
  would silently win over a choice just made.
- **Writing `config.yaml` only where there is none.** The usual case at a
  first login, but no use to Settings, nor to the card brought back once
  the file has been written.

## Consequences

- `granit-parser` is a dependency of `omorfos-config` of its own, at the
  version and with the features `serde-saphyr` already brings; an update
  of one without the other fails `cargo deny`.
- A program offering a choice says, in its own words, when the file is
  written in a way it will not change, and the file stays as it was.
- Only `config.yaml` is changed this way; `keys.yaml`, `rules.yaml` and
  `menu.yaml` are lists more than values, and the theme is a stylesheet,
  each for Settings to decide on when it comes to them.
