#!/bin/bash
# Runs a command in what the Test job's runner has: Ubuntu 24.04 with only
# the packages `.github/workflows/test.yml` installs, read from it so that
# they cannot drift apart. Rust, the crates and what was built are kept in
# the podman volume `omorfos-ci`, so that a second run takes minutes.
#
#   .claude/skills/ci-failure/reproduce.sh [command...]
#
# Run from the top of the repository. The command runs in a copy of it,
# without `target`; it is `cargo coverage --locked` if none is given. What
# it leaves in `/cache` is kept, such as a report:
#
#   reproduce.sh cargo llvm-cov --locked -p omorfos-recording --text --output-path /cache/report.txt
#   podman run --rm -v omorfos-ci:/cache ubuntu:24.04 grep -n -B3 '\^0' /cache/report.txt
set -euo pipefail

workflow=.github/workflows/test.yml
packages=$(grep -o 'apt-get install -y --no-install-recommends [^&]*' "$workflow" |
    sed 's/.*--no-install-recommends //')
if [ -z "$packages" ]; then
    echo "no packages found in $workflow" >&2
    exit 1
fi
command=("$@")
[ ${#command[@]} -gt 0 ] || command=(cargo coverage --locked)

podman volume exists omorfos-ci || podman volume create omorfos-ci >/dev/null
podman run --rm -i -v "$PWD":/src:ro -v omorfos-ci:/cache docker.io/library/ubuntu:24.04 \
    bash -s -- "$packages" "${command[@]}" <<'INSIDE'
set -euo pipefail
packages=$1
shift
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
# shellcheck disable=SC2086
apt-get install -y -qq --no-install-recommends $packages \
    build-essential pkg-config curl ca-certificates rsync >/dev/null
# Where X servers put their sockets, as the job makes it.
mkdir -p -m 1777 /tmp/.X11-unix
export CARGO_HOME=/cache/cargo RUSTUP_HOME=/cache/rustup PATH=/cache/cargo/bin:$PATH
[ -x /cache/cargo/bin/rustup ] || curl -sSf https://sh.rustup.rs |
    sh -s -- -y -q --profile minimal --default-toolchain none
[ -x /cache/cargo/bin/cargo-llvm-cov ] || curl -sSfL \
    https://github.com/taiki-e/cargo-llvm-cov/releases/latest/download/cargo-llvm-cov-x86_64-unknown-linux-gnu.tar.gz |
    tar xz -C /cache/cargo/bin
mkdir -p /cache/work
rsync -a --delete --exclude target /src/ /cache/work/
cd /cache/work
"$@"
INSIDE
