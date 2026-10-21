# Releasing

Releases are cut by merging a pull request. Nobody edits version numbers or
the changelog by hand.

## How a release happens

1. Commits land on `main` following
   [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/).
2. The **Release** workflow keeps a pull request open, titled
   `chore(main): release X.Y.Z`. It bumps the version in `Cargo.toml` and
   `Cargo.lock` and adds a section to `CHANGELOG.md`.
3. When you want to release, merge that pull request.
4. The workflow tags `vX.Y.Z`, creates the GitHub release, builds on amd64
   and arm64, and attaches to the release, for each architecture:
   - `omorfos_X.Y.Z-1_<arch>.deb`
   - `omorfos-X.Y.Z-<target>.tar.gz` (binary, licence, docs, schemas,
     login session entry)
   - a `.sha256` file for each
5. If enabled, it then publishes the `omorfos` package to the AUR.

## How the version is chosen

| Commits since the last release | Before 1.0 | From 1.0 |
|---|---|---|
| a breaking change (`!` or `BREAKING CHANGE:`) | minor | major |
| `feat` | minor | minor |
| `fix`, `perf` | patch | patch |
| only others (`docs`, `ci`, `build`, ...) | no release | no release |

To force a version, add a commit whose body contains
`Release-As: 1.0.0`.

## One-time setup on GitHub

- **Let Actions open pull requests:** Settings → Actions → General →
  "Allow GitHub Actions to create and approve pull requests".
- **Checks on the release pull request (recommended):** a pull request
  opened with the default token does not start other workflows, so it would
  show no lint or test results. Create a fine-grained personal access token
  for this repository with read and write access to *Contents* and *Pull
  requests*, and save it as the secret `RELEASE_PLEASE_TOKEN`.
- **Build provenance:** attestation runs only while the repository is
  public. In a private repository the step is skipped.

### Publishing to the AUR

This is off by default. It needs the repository to be public, because the
package builds from the release's source tarball.

1. Create an AUR account and add an SSH public key to it.
2. Create the `omorfos` package once by hand: render the files for a
   released version with `VERSION=X.Y.Z packaging/aur/render.sh out`, then
   push them to `ssh://aur@aur.archlinux.org/omorfos.git`.
3. Save the matching private key as the secret `AUR_SSH_PRIVATE_KEY`.
4. Set the repository variable `AUR_PUBLISH` to `true`.

## Trying the packaging locally

```sh
cargo build --release --locked -p omorfos -p omorfosctl -p omorfos-panel -p omorfos-launcher -p omorfos-notifications -p omorfos-screenshot -p omorfos-clipboard -p omorfos-recording -p omorfos-session -p xdg-desktop-portal-omorfos

VERSION=0.1.0 TARGET=x86_64-unknown-linux-gnu packaging/package.sh /tmp/dist
```

This needs `cargo-deb`, and `dpkg-dev` for the package's dependency list to
be complete.

To build the AUR package in a container without an Arch machine, see the
comments in `packaging/aur/render.sh`; it accepts `CONTAINER=podman`.

## Verifying a download

```sh
sha256sum -c omorfos_X.Y.Z-1_amd64.deb.sha256
gh attestation verify omorfos_X.Y.Z-1_amd64.deb --repo omorfos/desktop
```
