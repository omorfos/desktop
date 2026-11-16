#!/bin/bash
# The tests of affected.sh, against the workspace as it is. Run by the
# `commit` tier. Run from the top of the repository.
set -uo pipefail
here=${0%/*}
failed=0
metadata=$(mktemp)
trap 'rm -f "$metadata"' EXIT
cargo metadata --format-version 1 --all-features --locked --offline > "$metadata" || exit 1
export AFFECTED_METADATA=$metadata

# What is reached from `paths`, one to a line, as a line of names; or
# that it could not be said, which is never taken for nothing.
reached() {
    local said
    said=$(printf '%s\n' "$@" | "$here/affected.sh" 2>/dev/null) || { echo "(failed)"; return; }
    echo $said
}

says() {
    local name=$1 expected=$2 got
    shift 2
    got=$(reached "$@")
    if [ "$got" != "$expected" ]; then
        echo "FAILED: $name: expected '$expected', got '$got'"
        failed=1
    fi
}

# Whether what is reached from `paths` has `crate` in it or not.
has() {
    local name=$1 want=$2 crate=$3 got
    shift 3
    got=" $(reached "$@") "
    case "$got" in *" $crate "*) found=yes ;; *) found=no ;; esac
    if [ "$found" != "$want" ]; then
        echo "FAILED: $name: $crate reached: $found"
        failed=1
    fi
}

says "a program nothing depends on reaches only itself" \
    omorfosctl crates/omorfosctl/src/main.rs
says "a library reaches what depends on it, through what depends on that" \
    "omorfos omorfos-launcher omorfos-sums" crates/omorfos-sums/src/lib.rs
has "the toolkit reaches the panel" yes omorfos-panel crates/omorfos-ui/src/lib.rs
has "the toolkit reaches the compositor" yes omorfos crates/omorfos-ui/src/lib.rs
has "the toolkit does not reach what it depends on" no omorfos-config crates/omorfos-ui/src/lib.rs
has "the toolkit does not reach what does not use it" no omorfosctl crates/omorfos-ui/src/lib.rs
says "a test of a crate reaches as its code does" \
    omorfosctl crates/omorfosctl/tests/executable.rs
says "the lockfile reaches everything" everything Cargo.lock
says "the workspace's manifest reaches everything" everything Cargo.toml
says "the README, which the tests read, reaches everything" everything README.md
says "the workflows, which a test reads, reach everything" everything .github/workflows/test.yml
says "the guide reaches everything" everything docs/guide/config.yaml.md
says "a crate that is not in the workspace reaches everything" everything crates/omorfos-new/src/lib.rs
says "a decision reaches nothing" "" docs/adr/0001-fresh-implementation-on-smithay.md
says "the roadmap and a skill reach nothing" "" docs/ROADMAP.md .claude/skills/handover/SKILL.md
says "what reaches nothing adds nothing to what does" \
    omorfosctl docs/ROADMAP.md crates/omorfosctl/src/main.rs
says "nothing changed reaches nothing" ""
AFFECTED_METADATA=/nonexistent says "what cannot be worked out is a failure, not nothing" \
    "(failed)" crates/omorfosctl/src/main.rs

# Each path said to be read by nothing is there: one moved would leave its
# new place reaching everything and its old one on the list for nothing.
while read -r path; do
    [ -e "$path" ] || { echo "FAILED: $path is listed as read by nothing, but is not there"; failed=1; }
done < <("$here/affected.sh" --unread)

[ $failed -eq 0 ] && echo "affected.sh: every test passed"
exit $failed
