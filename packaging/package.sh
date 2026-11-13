#!/usr/bin/env bash
# Collects the release packages for the current build into a directory:
# a .deb, a tarball of the binary with its documentation, and a SHA-256
# checksum file for each.
#
# Usage: VERSION=1.2.3 TARGET=x86_64-unknown-linux-gnu packaging/package.sh OUT
#
# Expects `cargo build --release -p omorfos-brand -p omorfos -p omorfos-hold -p omorfos-relay -p omorfosctl -p omorfos-panel -p omorfos-launcher -p omorfos-notifications -p omorfos-screenshot -p omorfos-clipboard -p omorfos-recording -p omorfos-session -p omorfos-ask -p omorfos-wallpaper -p omorfos-lock -p xdg-desktop-portal-omorfos` and
# `target/release/omorfos-brand target/release/share` and `cargo about
# generate --workspace --all-features --frozen about.hbs -o
# target/release/LICENSE-crates` to have run, and cargo-deb to be
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
cp target/release/omorfos target/release/omorfos-hold target/release/omorfos-relay target/release/omorfosctl target/release/omorfos-panel target/release/omorfos-launcher target/release/omorfos-notifications target/release/omorfos-screenshot target/release/omorfos-clipboard target/release/omorfos-recording target/release/omorfos-session target/release/omorfos-ask target/release/omorfos-pinentry target/release/omorfos-wallpaper target/release/omorfos-lock target/release/xdg-desktop-portal-omorfos LICENSE LICENSE-pictures LICENSE-space-grotesk LICENSE-lucide target/release/LICENSE-crates README.md \
  packaging/omorfos.desktop packaging/omorfos.portal packaging/omorfos-portals.conf packaging/omorfos-session.target \
  "${stage}/${name}/"
# The PAM services the locker asks, for /etc/pam.d: in a folder of their
# own, as one is named as the program that asks it is.
mkdir "${stage}/${name}/pam.d"
cp packaging/pam/omorfos-lock packaging/pam/omorfos-lock-fingerprint packaging/pam/omorfos-lock-key "${stage}/${name}/pam.d/"
cp -r schemas "${stage}/${name}/"
# The guide, a page for each file and each part of the desktop.
cp -r docs/guide "${stage}/${name}/"
# The manual pages, for man1 and man5 of wherever it is put.
mkdir "${stage}/${name}/man"
cp man/*.1 man/*.5 "${stage}/${name}/man/"
# The icon and the wallpapers, as they go under share/.
cp -r target/release/share "${stage}/${name}/"
# The built-in theme, to copy from.
cp crates/omorfos-config/src/default.css "${stage}/${name}/theme.css"
tar -C "${stage}" -czf "${out}/${name}.tar.gz" "${name}"

cd "${out}"
for file in *; do
  sha256sum "${file}" > "${file}.sha256"
done
ls -l
