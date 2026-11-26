#!/bin/bash
# The commits after `from` up to `tip`, oldest first, one to a line, but
# those `origin/main` has: what is on it was checked where it was made.
#
#   tools/unpushed.sh <from> <tip>
#
# `from` is what `push` counts from, a branch's upstream. A branch of a
# pull request that has been pushed and then rebased onto a newer `main`
# still has the remote branch as it was for its upstream, and every
# commit `main` gained since lies between the two: on 26 November 2026 a
# push set out to check twenty of them again, at a quarter of an hour
# each. Where there is no `origin/main`, as in a clone of no remote,
# `from` alone is left out.
set -uo pipefail
from=${1:?unpushed.sh <from> <tip>}
tip=${2:?unpushed.sh <from> <tip>}
pushed=()
git rev-parse -q --verify origin/main >/dev/null && pushed=(origin/main)
git rev-list --reverse "$tip" --not "$from" "${pushed[@]}"
