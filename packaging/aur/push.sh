#!/usr/bin/env bash
# Pushes a rendered PKGBUILD and .SRCINFO to the AUR.
#
# Usage: VERSION=1.2.3 AUR_SSH_PRIVATE_KEY=... packaging/aur/push.sh DIR
#
# DIR is the output of render.sh. The key must belong to an AUR account that
# maintains the `omorfos` package.
set -euo pipefail

rendered="$(cd "${1:?usage: push.sh DIR}" && pwd)"
: "${VERSION:?VERSION is not set}"
: "${AUR_SSH_PRIVATE_KEY:?AUR_SSH_PRIVATE_KEY is not set}"

work="$(mktemp -d)"
umask 077
printf '%s\n' "${AUR_SSH_PRIVATE_KEY}" > "${work}/key"
# The AUR's host key is pinned rather than trusted on first use.
printf '%s\n' \
  'aur.archlinux.org ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIEuBKrPzbawxA/k2g6NcyV5jmqwJ2s+zpgZGZ7tpLIcN' \
  > "${work}/known_hosts"
export GIT_SSH_COMMAND="ssh -i ${work}/key -o IdentitiesOnly=yes -o StrictHostKeyChecking=yes -o UserKnownHostsFile=${work}/known_hosts"

git clone ssh://aur@aur.archlinux.org/omorfos.git "${work}/aur"
cp "${rendered}/PKGBUILD" "${rendered}/.SRCINFO" "${work}/aur/"
cd "${work}/aur"
git add PKGBUILD .SRCINFO
if git diff --cached --quiet; then
  echo "The AUR already has version ${VERSION}; nothing to push."
  exit 0
fi
git -c user.name='omorfos release workflow' -c user.email='noreply@github.com' \
  commit -m "Update to ${VERSION}"
git push origin HEAD:master
