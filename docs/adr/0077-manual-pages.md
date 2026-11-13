# 77. Manual pages

- Status: accepted
- Date: 2026-11-13

## Context

What there is could be found only by reading `CONFIGURATION.md` or the
source: there were no manual pages, and the roadmap asked for one for
each program and for each file a user writes, made from what `--help`
and the schemas already say, so that the two cannot disagree. Each
program's help was by then made from an `About` in `omorfos_ipc::help`,
but for the four that wrote theirs out by hand, and each file has a JSON
Schema made from the types that parse it, with a description of every key.

## Decision

**The pages are made from those, written in roff, and kept in `man/`,
made again by a test as the schemas are.**
`OMORFOS_BLESS=1 cargo test --workspace --all-features manual_page`
writes them; without it, the same tests fail while a page is not what it
would be made as.

- **Section 1, a page for each program installed**, from its `About`:
  its name and what it is, its usage, more of it, its commands and
  options, examples, and the other pages. Every `About` is in its crate's
  library, where the compositor's tests reach them all; `omorfosctl`,
  which has no library, keeps its own page in its own tests. `omorfos`,
  `omorfos-hold`, `omorfos-relay` and `omorfosctl` make their help from an
  `About` too, but keep reading their arguments their own way, so that
  `omorfosctl action spawn foot --help` is still `foot`'s.
- **Section 5, a page for each file**: `omorfos-config(5)` for
  `config.yaml`, and `omorfos-keys(5)`, `omorfos-rules(5)`,
  `omorfos-menu(5)` and `omorfos-theme(5)`. Each key of a YAML file is
  told by the path written to reach it, with its schema's description,
  the values it may take by name with theirs, its bounds and its default
  where the schema has one; then the guide's example of the file, which a
  test keeps valid, and which for `config.yaml` is every default but the
  workspaces' names. The theme's page lists its parts and states and
  shows the built-in theme, whose comments tell what each part is.
  `omorfos-keys(5)` and `omorfos-menu(5)` begin, before the keys, with
  what is there before anything is written: each built-in binding by its
  keys, its action as `keys.yaml` writes it and what that does, and the
  built-in menu as a `menu.yaml`, to begin one from. The actions are
  written by serde's `Serialize`, derived under the `schema` feature
  only, but for `spawn`, written by hand so that `run` comes before
  `may` as the guide has it; a test reads each back as the action it was
  written from. The guide's table of the built-in bindings is made from
  the same list, by the same command, between two marks in it.
- **No version and no date in a page.** Either would change with every
  release and leave the pages kept behind, failing the test on the
  release's own commit.
- **Neither hyphenated nor stretched to both edges**, as names and options
  are read whole.
- **Installed by every packaging**, into `share/man`: `make install`, the
  `.deb`, the AUR package and the release's tarball; a test checks each,
  and that every program installed has its page, and every page a page
  points to is there.

## Alternatives

- **Made as the packages are built**, by a program of their own or
  `build.rs`. Nothing kept in the repository to go stale, but a step more
  in every packaging, and a page nobody reads in review.
- **A library such as `clap_mangen`.** Made for `clap`, which omorfos does
  not use, and a dependency for what is a few lines of roff.
- **Short pages for the files, pointing to the guide.** Less to keep, but
  of little use where the guide is not to hand; the user chose every key.
- **`config.yaml(5)` and the like as the pages' names.** The section is
  shared by every program on the system, so each is named after omorfos.

## Consequences

- A change to what a program says of itself, or to a configuration type,
  is followed by making the pages again, as the schemas are.
- `omorfos-config`'s `schema` feature now brings in `omorfos-ipc`, for its
  roff, which only the tests turn on.
- What a key does is said by the comment on its type, which the guide
  says at more length; the pages are as good as those comments.
- Deriving `Serialize` lets schemars write the default of a key whose
  type it could not write before, as the numbers within bounds are: the
  schemas, and so the pages and editors, now give them.
- A theme's parts have no description of their own in the page but the
  built-in theme's comments, as `Part`'s documentation is not there to be
  read when the pages are made.
