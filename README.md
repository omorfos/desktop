# omorfos

A lightweight Wayland compositor for Linux, written in Rust and inspired by
[Fluxbox](https://github.com/fluxbox/fluxbox): tabbed window groups, chained
keybindings, per-application rules and a small footprint, with a modern look.

> **Status:** early development. Nothing here is usable as a desktop yet.
> See [docs/ROADMAP.md](docs/ROADMAP.md).

## Goals

- **Secure:** no `unsafe` in our own crates, least-privilege IPC, vetted
  dependencies.
- **Fast:** damage-tracked rendering, no work when nothing changes.
- **Lean:** the compositor does window management; panels, launchers and
  notifications are separate clients.
- **Friendly:** YAML configuration in `~/.config/omorfos/`, reloaded live, and
  a broken config never takes the session down.

Supported targets: Linux on `x86_64` and `aarch64`.

## Development

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo llvm-cov --workspace --all-features --fail-under-lines 100 --fail-under-functions 100 --fail-under-regions 100
cargo deny check
```

The toolchain is pinned in `rust-toolchain.toml`. Commits follow
[Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/).
Architecture and conventions live in [AGENTS.md](AGENTS.md) and
[docs/](docs/).

## License

[MIT](LICENSE)
