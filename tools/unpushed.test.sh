#!/bin/bash
# The tests of unpushed.sh, in a repository made for them and thrown away.
# Run by the `commit` tier and by CI. Run from the top of the repository.
set -uo pipefail
script=$PWD/tools/unpushed.sh
failed=0
repo=$(mktemp -d)
trap 'rm -rf "$repo"' EXIT
cd "$repo" || exit 1
export GIT_AUTHOR_NAME=test GIT_AUTHOR_EMAIL=test@example.invalid
export GIT_COMMITTER_NAME=test GIT_COMMITTER_EMAIL=test@example.invalid
git init -q -b main .
git config commit.gpgsign false

commit() {
    git commit -q --allow-empty -m "$1"
}

# The subjects of what unpushed.sh lists, on one line.
says() {
    local name=$1 expected=$2 got
    shift 2
    got=$("$script" "$@" | while read -r each; do git log -1 --format=%s "$each"; done | paste -sd' ')
    if [ "$got" != "$expected" ]; then
        echo "FAILED: $name: expected '$expected', got '$got'"
        failed=1
    fi
}

# main as it was pushed; a branch from it, pushed; then main gaining two
# and the branch rebased onto them, its upstream left as it was.
commit base
git update-ref refs/remotes/origin/main HEAD
git switch -q -c branch
commit one
commit two
git update-ref refs/remotes/origin/branch HEAD
says "a branch counted from its upstream lists what came after it" "" origin/branch HEAD
says "a branch counted from main lists its own commits" "one two" origin/main HEAD
commit three
says "only what is not yet pushed is listed, oldest first" "three" origin/branch HEAD
git switch -q main
commit main-a
commit main-b
git update-ref refs/remotes/origin/main HEAD
git switch -q branch
git rebase -q main
says "after a rebase, main's new commits are left out" "one two three" origin/branch HEAD
says "the tip itself can be left out" "one two" origin/branch HEAD~1

# A clone with no origin/main has only the upstream to go by.
git update-ref -d refs/remotes/origin/main
says "with no origin/main, what is after the upstream is listed" "main-a main-b one two three" origin/branch HEAD

[ "$failed" -eq 0 ] && echo "unpushed.sh: as expected"
exit "$failed"
