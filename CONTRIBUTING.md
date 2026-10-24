# Contributing

Thank you for wanting to help. omorfos is the program between every other
program on the desktop and the person using it, so a change is held to
what that asks: it is secure first, then correct, then fast, then clean,
then pleasant, and lean throughout. [AGENTS.md](AGENTS.md) says what
each of those means here, and is the place to start, for people as much
as for agents.

Everyone taking part is expected to follow the
[code of conduct](CODE_OF_CONDUCT.md).

## Before you start

- **A security vulnerability** is reported privately, as
  [SECURITY.md](SECURITY.md) says, and never in an issue or a pull
  request.
- **A bug** is an issue, through the form that asks what is needed to
  bring it about.
- **Something larger than a fix**, a feature, a new setting, a new
  dependency or a change to how something works, is best talked over in
  an issue first. [docs/ROADMAP.md](docs/ROADMAP.md) says what is
  planned and in what order, and the
  [decisions](docs/adr/) say why things are as they are; a change that
  goes against one of them needs a new decision, not only code.
- **The defaults**, of keys and of settings, are to be reviewed
  together (see the roadmap), so a change to one alone may be asked to
  wait for that.

## Building and testing

[AGENTS.md](AGENTS.md) lists what is needed to build and to run the tests,
and [docs/TESTING.md](docs/TESTING.md) how the tests are written: through
a real Wayland client, asserting on what the protocol says and on the
pixels drawn. Before you open a pull request, all of these pass:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo coverage
cargo test --workspace --all-features --doc
cargo deny check
```

`.claude/skills/handover/gates.sh` runs them in tiers, the quickest
first. To try a change on your own machine, `make nightly` and
`sudo make install PROFILE=nightly` build and install it in seconds after
a change.

What is checked, and is not given way on:

- **Coverage is 100%** of lines, functions and regions, error paths
  among them. Code that cannot run in CI is kept free of decisions, and
  listed in [ADR 2](docs/adr/0002-coverage-policy.md) before it is left
  out.
- **No `unsafe`**, no `unwrap` or `panic!` outside tests, and every
  public item documented. A lint is not turned off to make code pass; a
  local `#[expect(..., reason = "...")]` with a real reason is the most
  that is accepted.
- **A new dependency** passes `cargo deny check`, and the commit that adds
  it says why it is worth having over the standard library or what is
  already used.
- **Tests that start programs or watch files** are run many times over
  before they are handed in, as `docs/TESTING.md` says, since a test that
  fails one time in fifty fails someone's pull request.

## Commits and pull requests

- Commits follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/):
  `type(scope): summary`, in lower case, at most 72 characters, with a
  body that says why the change is needed, what it does and what was
  given up for it. Releases and their notes are made from these, as
  [docs/RELEASING.md](docs/RELEASING.md) says.
- Each commit is one change, which builds and passes on its own. A
  refactor, a fix and a feature are three commits.
- What a user can set or see is in
  [docs/CONFIGURATION.md](docs/CONFIGURATION.md) in the same change; a
  finished item of the roadmap is ticked; and a decision someone will
  later ask "why" about gets a new ADR.
- What is drawn follows [docs/DESIGN.md](docs/DESIGN.md), and can be used
  with the keyboard alone, as [ACCESSIBILITY.md](ACCESSIBILITY.md) says.
- The pull request's template asks for what a reviewer needs. Small and
  focused is reviewed sooner than large.

How a change was written is yours to choose, an assistant included. What
is handed in is yours to answer for: you understand it, it was tested as
above, and it is your own or yours to give under the project's licence.

## Licence

By contributing, you agree that your code is under the
[MIT licence](LICENSE), and pictures of the brand under
[CC BY-SA 4.0](LICENSE-pictures), as the rest of omorfos is.
