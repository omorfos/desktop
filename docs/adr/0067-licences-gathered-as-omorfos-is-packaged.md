# 67. The licences of the crates gathered as omorfos is packaged

- Status: accepted
- Date: 2026-11-09

## Context

omorfos is under the MIT licence, and so are most of the crates it is
built from; nearly all the others are under Apache 2.0, the BSD
licences, ISC, Zlib or Unicode's. Every one of them asks that its
copyright notice and licence go with copies of the program, the binary
too. The packages carried omorfos's own licences and nothing of the
crates', some 250 of them, Smithay among them. `cargo deny` checks that
each crate's licence is one that may be shipped with an MIT binary, but
does not write their texts out.

## Decision

**`cargo about` gathers them as omorfos is packaged**, into
`LICENSE-crates`: each licence's text once, with the crates under it and
their versions, from the files the crates were downloaded with, which
the packages carry beside omorfos's own. `make` does it, as do the
PKGBUILD and the release workflow, with what `about.toml` and
`about.hbs` say. It accepts the licences `deny.toml` allows, and a test
keeps the two lists the same, so that a crate under another licence
fails `cargo deny` first.

cargo-about is by Embark Studios, who make cargo-deny, and is pinned,
0.9.3, where it is installed.

## Alternatives

- **The file kept in the repository**, written again by hand and checked
  by a gate. Packages would need no new tool, but every update of a
  crate, as Dependabot makes each week, would fail the check until it
  was written again, which Dependabot cannot do.
- **`cargo deny list`**, which is installed already, names each crate's
  licence but not its text or its copyright.

## Consequences

- Building packages needs cargo-about. It runs offline, from what cargo
  has downloaded, and so reads only the licence files crates carry: one
  that carries none is given the licence's usual text, without its
  copyright line.
- The file is about 200 KB, and as current as the `Cargo.lock` it was
  gathered from.
