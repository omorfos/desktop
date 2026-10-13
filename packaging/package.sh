#!/usr/bin/env bash
# Collects the release packages for the current build into a directory:
# a .deb, a tarball of the binary with its documentation, and a SHA-256
# checksum file for each.
#
# Usage: VERSION=1.2.3 TARGET=x86_64-unknown-linux-gnu packaging/package.sh OUT
#
# Expects `cargo build --release -p omorfos -p omorfosctl -p omorfos-panel -p omorfos-launcher -p omorfos-notifications -p omorfos-screenshot` to have run, and cargo-deb to be
# installed. OUT must not exist yet.
set -euo pipefail

out="${1:?usage: package.sh OUT}"
: "${VERSION:?VERSION is not set}"
: "${TARGET:?TARGET is not set}"

cd "$(dirname "$0")/.."
mkdir "${out}"

cargo deb -p omorfos --no-build --output "${out}/"

name="omorfos-${VERSION}-${TARGET}"
stage="$(mktemp -d)"
mkdir "${stage}/${name}"
cp target/release/omorfos target/release/omorfosctl target/release/omorfos-panel target/release/omorfos-launcher target/release/omorfos-notifications target/release/omorfos-screenshot LICENSE README.md docs/CONFIGURATION.md \
  packaging/omorfos.desktop "${stage}/${name}/"
cp -r schemas "${stage}/${name}/"
# The built-in theme, to copy from.
cp crates/omorfos-config/src/default.css "${stage}/${name}/theme.css"
tar -C "${stage}" -czf "${out}/${name}.tar.gz" "${name}"

cd "${out}"
for file in *; do
  sha256sum "${file}" > "${file}.sha256"
done
ls -l
