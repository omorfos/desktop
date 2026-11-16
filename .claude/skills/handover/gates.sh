#!/bin/bash
# The gates, in tiers that each stop at the first failure. See
# docs/TESTING.md, "Tiers".
#
#   .claude/skills/handover/gates.sh area <filter>...  the tests of what is being changed
#   .claude/skills/handover/gates.sh commit            what every commit needs: every test once, no coverage
#   .claude/skills/handover/gates.sh handover          what a build for trying by hand needs
#   .claude/skills/handover/gates.sh rest              the rest, run while it is tried
#   .claude/skills/handover/gates.sh all               handover, then rest
#   .claude/skills/handover/gates.sh commits [base [tip]]  each commit after base (main) alone, to tip (HEAD)
#   .claude/skills/handover/gates.sh push              all, then commits for each commit not yet pushed
#   .claude/skills/handover/gates.sh uncovered         what the last coverage run missed
#
# What passed is written down by the tree it passed on, in
# target/gates/passed, and `.githooks/pre-push` refuses to push a commit
# whose tree is not there (see docs/TESTING.md, "Tiers").
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

# Runs a command that runs tests. Where the directory X servers keep
# their sockets in cannot be written to, as under WSL, every test of a
# program written for X11 would fail; the command then has one of its own,
# through bubblewrap, and sees the rest of the machine as it is.
tested() {
    if [ -w /tmp/.X11-unix ] || [ ! -e /tmp/.X11-unix ]; then
        "$@"
    elif command -v bwrap >/dev/null; then
        bwrap --dev-bind / / --tmpfs /tmp/.X11-unix -- "$@"
    else
        echo "/tmp/.X11-unix cannot be written to and bwrap is not installed:" \
            "the tests of programs written for X11 will fail." >&2
        "$@"
    fi
}

# The tree of what is in the working copy now, untracked files that are
# not ignored included: what a tier ran on, and what a commit of it all
# would hold. Worked out in an index of its own, so that what is staged
# is left as it is.
tree_now() {
    local index
    index=$(mktemp)
    rm -f "$index"
    GIT_INDEX_FILE=$index git read-tree HEAD &&
        GIT_INDEX_FILE=$index git add -A &&
        GIT_INDEX_FILE=$index git write-tree
    rm -f "$index"
}

# Writes down that `tier` passed on `tree`, for the hook that checks a
# push and for `rest`, which need not run again what `handover` ran.
passed() {
    echo "$1 $2" >> "$top/target/gates/passed"
}

# Whether `tier` has passed on `tree`.
has_passed() {
    grep -qx "$1 $2" "$top/target/gates/passed" 2>/dev/null
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

# Each commit after `base` by itself, oldest first, up to `tip`, in a
# worktree of its own: formatting, clippy, and the coverage gate. A commit
# that passes only with what came after it is found here, as one split
# badly is. The last commit's tree is what the other tiers ran on: where
# `all` passed on it, `tip` can be the commit before, as `HEAD~1`, and the
# tier is a coverage run shorter.
commits() {
    local base=${1:-main} tip=${2:-HEAD} tree commit list
    list=$(git rev-list --reverse "$base".."$tip")
    [ -n "$list" ] || { echo "nothing after $base"; return; }
    # Outside the repository: cargo reads the configuration of every folder
    # above the one it runs in, and would take the coverage alias twice.
    tree="$(mktemp -d)/commit"
    for commit in $list; do
        # One whose tree has passed already, as a commit unchanged by a
        # rebase has, would pass again: it is not checked twice.
        local done_before
        done_before=$(git rev-parse "$commit^{tree}")
        if has_passed commits "$done_before" || has_passed push "$done_before" ||
            has_passed all "$done_before"; then
            echo "=== $(git log -1 --format='%h %s' "$commit"): passed already, as it is" | tee -a "$log"
            continue
        fi
        git worktree remove --force "$tree" 2>/dev/null || rm -rf "$tree"
        git worktree add -q --detach "$tree" "$commit"
        echo "=== $(git log -1 --format='%h %s' "$commit")" | tee -a "$log"
        # Each is built where the one before was, so only what changed is.
        if ! (
            cd "$tree" &&
                export CARGO_TARGET_DIR="$top/target/commits" &&
                step cargo fmt --all --check &&
                step cargo clippy --workspace --all-targets --all-features --locked -- -D warnings &&
                explain=uncovered step tested cargo coverage --locked -- $(card)
        ); then
            git worktree remove --force "$tree"
            rmdir "${tree%/*}"
            echo "FAILED at $(git log -1 --format='%h %s' "$commit"), the whole of it in $log"
            exit 1
        fi
        passed commits "$(git rev-parse "$commit^{tree}")"
    done
    git worktree remove --force "$tree"
    rmdir "${tree%/*}"
}

# Before a push: everything, on the tree as it is, which must be what
# is committed; and each commit not yet pushed but the last alone, as the
# last is what everything ran on.
push() {
    local upstream
    [ -z "$(git status --porcelain)" ] || {
        echo "Commit first: what is pushed is what is committed." >&2
        exit 1
    }
    upstream=$(git rev-parse --abbrev-ref --symbolic-full-name '@{upstream}' 2>/dev/null || echo origin/main)
    handover
    rest
    commits "$upstream" HEAD~1
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
    [ ${#tests[@]} -eq 0 ] ||
        step tested cargo test -p omorfos --all-features --locked --test integration -- "${tests[@]}"
}

# What every commit needs, in a few minutes: the checks that are quick,
# and every test once, without the coverage gate, which `push` runs.
commit() {
    step typos
    step .githooks/pre-commit --tree
    step cargo fmt --all --check
    step cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
    step tested cargo test --workspace --all-features --locked
}

handover() {
    step typos
    step .githooks/pre-commit --tree
    step cargo fmt --all --check
    step cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
    # `cargo coverage` is an alias defined in .cargo/config.toml. This is
    # every test, once, with the coverage gate.
    explain=uncovered step tested cargo coverage --locked -- $(card)
    passed coverage "$tree"
    step make nightly
    ls -l target/nightly/omorfos
}

rest() {
    step cargo clippy --workspace --all-targets --locked -- -D warnings
    # As CI has it, without the tests that need a graphics card: what only
    # the card reaches counts here and not there. Where no card was used,
    # that is the run `handover` made, if it made one on this tree.
    if [ -n "$(card 2>/dev/null)" ] || ! has_passed coverage "$tree"; then
        explain=uncovered step tested cargo coverage --locked
    else
        echo "=== the coverage gate passed on this tree in handover, with no card: not run again" | tee -a "$log"
    fi
    step cargo test --workspace --all-features --doc --locked
    step cargo deny check
    # The workflows, as CI checks them but for what needs the network:
    # each action they use has a commit in the lockfile.
    step zizmor --offline --no-progress .
    step gh actions-lock --verify-local
    RUSTDOCFLAGS="-D warnings" step cargo doc --workspace --all-features --no-deps --locked
}

tier=${1:-}
mkdir -p target/gates
log="$top/target/gates/${tier:-none}.log"
: > "$log"
# What the tier runs on. A tier whose tree changes while it runs has
# passed on neither, and is not written down as passed.
tree=$(tree_now)
case "$tier" in
    area) shift; area "$@" ;;
    commit) commit ;;
    handover) handover ;;
    rest) rest ;;
    all) handover; rest ;;
    commits) shift; commits "$@" ;;
    push) push ;;
    uncovered) uncovered | tee -a "$log" ;;
    *)
        echo "usage: $0 area <filter>... | commit | handover | rest | all | commits [base [tip]] | push | uncovered" >&2
        exit 2
        ;;
esac
case "$tier" in
    handover | rest | all | push)
        if [ "$(tree_now)" = "$tree" ]; then
            passed "$tier" "$tree"
        else
            echo "The working copy changed while the gates ran: run them again." >&2
            exit 1
        fi
        ;;
esac
echo "PASSED: $tier"
