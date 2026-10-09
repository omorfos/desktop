# 6. Release process

- Status: accepted
- Date: 2026-10-09

## Context

The project commits to Conventional Commits and semantic versioning, and
wants `.deb` and AUR packages for amd64 and arm64. Releases should follow
from the commit history without manual version edits.

## Decision

**release-please drives releases.** It keeps a release pull request open and
tags when that is merged, so a person still decides *when* to release but
not *what number*. Tools that publish to crates.io were not chosen because
none of our crates is published there.

**The `simple` release type with explicit TOML updaters.** The version is
defined once, in `[workspace.package]`, and crates inherit it. release-please
is told to update that field and the matching entries in `Cargo.lock`, so
`--locked` builds keep working after a bump. Adding a crate to the workspace
means adding its `Cargo.lock` entry to `release-please-config.json`.

**Packages are built in the release workflow itself.** A release created
with the default token does not trigger other workflows, so building in a
separate workflow on the `release` event would silently never run.

**Native runners, no cross-compilation.** GitHub offers arm64 runners; using
them avoids cross toolchains and sysroots for the C libraries we link.

**The AUR package builds from source** at the release tag, as Arch users
expect, rather than repackaging our binary. Publishing is opt-in and uses a
pinned host key for the AUR.

**The software renderer is not shipped.** It is a test dependency, so
packages depend only on libxkbcommon, EGL and OpenGL ES.

**No login-screen session entry yet.** The compositor only runs nested until
the hardware backend exists.

## Consequences

- The release-please configuration, and everything after the tag (upload,
  attestation, AUR push), can only be proven by a real release.
- Attestation and the AUR need a public repository.
- The AUR package does not run the test suite when it builds.
- The Debian package's dependency list is produced by `dpkg-shlibdeps`, so
  it reflects the libraries on the build runner (Ubuntu 24.04).
