#!/bin/bash
# The gates, in tiers that each stop at the first failure. See
# docs/TESTING.md, "Tiers".
#
#   .claude/skills/handover/gates.sh area <filter>...  the tests of what is being changed
#   .claude/skills/handover/gates.sh handover          what a build for trying by hand needs
#   .claude/skills/handover/gates.sh rest              the rest, run while it is tried
#   .claude/skills/handover/gates.sh all               handover, then rest
#   .claude/skills/handover/gates.sh commits [base]    each commit after base (main) alone
#   .claude/skills/handover/gates.sh uncovered         what the last coverage run missed
#
# Each tier's whole output is kept in target/gates/<tier>.log. `area` takes
# what to run: a crate, as `-p omorfos-core`, or tests of the
# integration binary by name, as `tiles::`; it builds what they need and
# runs only them. Run from the top of the repository.
set -uo pipefail
export PATH="$HOME/.cargo/bin:$PATH"

# Everything each step says is kept whole in a log of the tier's own, so
# that what a test that fails only sometimes said is not lost to a filter.
top=$PWD
log=""

# Runs a step, and stops the tier if it fails. A step whose failure needs
# explaining is run with what explains it in `explain`.
step() {
    local started=$SECONDS
    echo "=== $*" | tee -a "$log"
    if ! "$@" 2>&1 | tee -a "$log"; then
        [ -z "${explain:-}" ] || "$explain" 2>&1 | tee -a "$log"
        echo "FAILED after $((SECONDS - started))s: $*, the whole of it in $log"
        exit 1
    fi
    echo "--- $((SECONDS - started))s" | tee -a "$log"
}

# Each line on which a region begins that the last coverage run did not
# reach, by its file, line and the column of the first such region, with
# the line itself: what the summary only counts. The
# files the gate leaves out are left out here too, as the alias in
# .cargo/config.toml names them.
uncovered() {
    local ignored tab
    tab=$(printf '\t')
    ignored=$(sed -n 's/.*"--ignore-filename-regex=\(.*\)",$/\1/p' .cargo/config.toml)
    # Written in TOML, a backslash is two.
    ignored=${ignored//\\\\/\\}
    # A run in which tests failed reaches less than it would have, and
    # what it did not reach says nothing.
    if grep -q "^test result: FAILED" "$log"; then
        echo "Tests failed, so what coverage did not reach is not listed."
        return
    fi
    echo "Not reached, by where each region begins:"
    cargo llvm-cov report --json --ignore-filename-regex="$ignored" 2>/dev/null |
        jq -r '.data[0].files[] | .filename as $file | .segments[]
            | select(.[2] == 0 and .[3] and .[4] and (.[5] | not))
            | "\($file)\t\(.[0])\t\(.[1])"' |
        sort -u -t "$tab" -k1,1 -k2,2n -k3,3n |
        # A line once, at the first region on it.
        awk -F "$tab" '!seen[$1 FS $2]++' |
        while IFS="$tab" read -r file line column; do
            printf '%s:%s:%s: %s\n' "${file#"$PWD"/}" "$line" "$column" \
                "$(sed -n "${line}s/^ *//p" "$file")"
        done
}

# Each commit after `base` by itself, oldest first, in a worktree of its
# own: formatting, clippy, and the coverage gate. A commit that passes
# only with what came after it is found here, as one split badly is. The
# last commit's tree is what the other tiers ran on.
commits() {
    local base=${1:-main} tree commit list
    list=$(git rev-list --reverse "$base"..HEAD)
    [ -n "$list" ] || { echo "nothing after $base"; return; }
    # Outside the repository: cargo reads the configuration of every folder
    # above the one it runs in, and would take the coverage alias twice.
    tree="$(mktemp -d)/commit"
    for commit in $list; do
        git worktree remove --force "$tree" 2>/dev/null || rm -rf "$tree"
        git worktree add -q --detach "$tree" "$commit"
        echo "=== $(git log -1 --format='%h %s' "$commit")" | tee -a "$log"
        # Each is built where the one before was, so only what changed is.
        if ! (
            cd "$tree" &&
                export CARGO_TARGET_DIR="$top/target/commits" &&
                step cargo fmt --all --check &&
                step cargo clippy --workspace --all-targets --all-features --locked -- -D warnings &&
                explain=uncovered step cargo coverage --locked -- $(card)
        ); then
            git worktree remove --force "$tree"
            rmdir "${tree%/*}"
            echo "FAILED at $(git log -1 --format='%h %s' "$commit"), the whole of it in $log"
            exit 1
        fi
    done
    git worktree remove --force "$tree"
    rmdir "${tree%/*}"
}

# What to ask the tests for besides what CI runs: the tests that need a
# graphics card, which are ignored unless asked for, where this user may
# use one. Where they may not, it is said, so that it is not taken for
# their having passed.
card() {
    if [ -r /dev/dri/renderD128 ] && [ -w /dev/dri/renderD128 ]; then
        echo --include-ignored
    else
        echo "No graphics card this user may use: the tests that need one are not run." >&2
    fi
}

integration() {
    cargo test -p omorfos --all-features --locked --test integration -- "$@"
}

area() {
    local crates=() tests=()
    while [ $# -gt 0 ]; do
        if [ "$1" = -p ]; then
            crates+=(-p "$2")
            shift 2
        else
            tests+=("$1")
            shift
        fi
    done
    [ ${#crates[@]} -eq 0 ] || step cargo test --all-features --locked "${crates[@]}"
    [ ${#tests[@]} -eq 0 ] || step integration "${tests[@]}"
}

handover() {
    step typos
    step .githooks/pre-commit --tree
    step cargo fmt --all --check
    step cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
    # `cargo coverage` is an alias defined in .cargo/config.toml. This is
    # every test, once, with the coverage gate.
    explain=uncovered step cargo coverage --locked -- $(card)
    step cargo build --profile nightly --locked -p omorfos -p omorfosctl -p omorfos-panel \
        -p omorfos-launcher -p omorfos-notifications -p omorfos-screenshot -p omorfos-clipboard \
        -p omorfos-recording -p xdg-desktop-portal-omorfos
    ls -l target/nightly/omorfos
}

rest() {
    step cargo clippy --workspace --all-targets --locked -- -D warnings
    # As CI has it, without the tests that need a graphics card: what only
    # the card reaches counts here and not there.
    explain=uncovered step cargo coverage --locked
    step cargo test --workspace --all-features --doc --locked
    step cargo deny check
    RUSTDOCFLAGS="-D warnings" step cargo doc --workspace --all-features --no-deps --locked
}

tier=${1:-}
mkdir -p target/gates
log="$top/target/gates/${tier:-none}.log"
: > "$log"
case "$tier" in
    area) shift; area "$@" ;;
    handover) handover ;;
    rest) rest ;;
    all) handover; rest ;;
    commits) shift; commits "$@" ;;
    uncovered) uncovered | tee -a "$log" ;;
    *)
        echo "usage: $0 area <filter>... | handover | rest | all | commits [base] | uncovered" >&2
        exit 2
        ;;
esac
echo "PASSED: $tier"
