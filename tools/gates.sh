#!/bin/bash
# The gates, in tiers that each stop at the first failure. See
# docs/TESTING.md, "Tiers".
#
#   tools/gates.sh area <filter>...  the tests of what is being changed
#   tools/gates.sh commit            what every commit needs: the tests of what it reaches, no coverage
#   tools/gates.sh handover          what a build for trying by hand needs
#   tools/gates.sh rest              the rest, run while it is tried
#   tools/gates.sh all               handover, then rest
#   tools/gates.sh commits [base [tip]]  each commit after base (main) alone, to tip (HEAD)
#   tools/gates.sh push              all on the last commit, in a worktree of its own,
#                                                      or what it reaches alone if it has built nothing new,
#                                                      then commits for each other commit not yet pushed
#   tools/gates.sh uncovered         what the last coverage run missed
#
# What passed is written down by the tree it passed on, in
# target/gates/passed, and `.githooks/pre-push` refuses to push a commit
# whose tree is not there (see docs/TESTING.md, "Tiers").
#
# Each tier's whole output is kept in target/gates/<tier>.log. `area` takes
# what to run: a crate, as `-p omorfos-core`, or tests of the
# integration binary by name, as `tiles::`; it builds what they need and
# runs only them. Run from the top of the repository.
# Where a commit is built in a worktree, in a subshell that goes into it,
# what it is built in is set for that subshell alone, as it is meant to be.
# shellcheck disable=SC2030,SC2031
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
    echo "$1 $2" >>"$top/target/gates/passed"
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
# worktree of its own: formatting, clippy, and once, without the coverage
# gate, the tests of each crate it reaches, as `commit` runs them. A
# commit that passes only with what came after it is found here, as one
# split badly is. The coverage gate is the last commit's alone (ADR 96):
# one that builds and passes its tests is what a bisect or a revert
# needs. One that passed `commit` as it was committed is not checked
# again.
commits() {
    local base=${1:-main} tip=${2:-HEAD} tree commit list
    list=$(git rev-list --reverse "$base".."$tip")
    [ -n "$list" ] || {
        echo "nothing after $base"
        return
    }
    # Outside the repository: cargo reads the configuration of every folder
    # above the one it runs in, and would take the coverage alias twice.
    tree="$(mktemp -d)/commit"
    for commit in $list; do
        # One whose tree has passed already, as a commit unchanged by a
        # rebase has, would pass again: it is not checked twice.
        local done_before
        done_before=$(git rev-parse "$commit^{tree}")
        if has_passed commit "$done_before" || has_passed commits "$done_before" ||
            has_passed push "$done_before" || has_passed all "$done_before"; then
            echo "=== $(git log -1 --format='%h %s' "$commit"): passed already, as it is" | tee -a "$log"
            continue
        fi
        git worktree remove --force "$tree" 2>/dev/null || rm -rf "$tree"
        git worktree add -q --detach "$tree" "$commit"
        echo "=== $(git log -1 --format='%h %s' "$commit")" | tee -a "$log"
        # One that changes nothing that is built or tested passes as the one
        # before it did, which has passed: `base` was pushed, and the tier
        # stops at the first that fails. Its spelling and links are checked.
        local crates
        crates=$(reached "$commit^" "$commit") || exit 1
        if [ -z "$crates" ]; then
            if ! (
                cd "$tree" &&
                    export CARGO_TARGET_DIR="$top/target/commits" &&
                    step typos &&
                    step cargo test -p omorfos-config --all-features --locked --test links
            ); then
                git worktree remove --force "$tree"
                rmdir "${tree%/*}"
                echo "FAILED at $(git log -1 --format='%h %s' "$commit"), the whole of it in $log"
                exit 1
            fi
            passed commits "$(git rev-parse "$commit^{tree}")"
            continue
        fi
        # Each is built where the one before was, so only what changed is.
        if ! (
            cd "$tree" &&
                export CARGO_TARGET_DIR="$top/target/commits" &&
                step cargo fmt --all --check &&
                step cargo clippy --workspace --all-targets --all-features --locked -- -D warnings &&
                reaching "$crates"
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

# The commit a push's change is counted from: the last before `commit`,
# back to what is pushed, on which everything has passed, as it had on
# what is pushed before it went.
passed_before() {
    local each
    for each in $(git rev-list "$1..$2~1" 2>/dev/null); do
        each=$(git rev-parse "$each^{tree}")
        if has_passed push "$each" || has_passed all "$each"; then
            echo "$each"
            return
        fi
    done
    git rev-parse "$1^{tree}"
}

# Whether what a change reaches, as `reached` says it, has anything that
# is built in it: a crate, or everything. One of a crate's tests alone,
# the scripts and the workflows are not.
built() {
    local word
    for word in $1; do
        case "$word" in
            scripts | workflows | *:*) ;;
            *) return 0 ;;
        esac
    done
    return 1
}

# What a push needs of a commit that has built nothing new since one on
# which everything passed: the code is that code, and the coverage gate,
# its tests and its documentation would say what they said (ADR 96). What
# the change can reach is run, by the one list every tier asks, with what
# is quick and reads the whole tree.
light() {
    echo "=== nothing that is built has changed since everything passed:" \
        "the coverage gate and the tests of the code are not run again" | tee -a "$log"
    step typos
    step .githooks/pre-commit --tree
    step cargo fmt --all --check
    step cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
    step tools/affected.test.sh
    step cargo test -p omorfos-config --all-features --locked --test links
    # The workflows are checked below whether or not they changed.
    reaching "${1//workflows/}"
    step cargo deny check
    workflows
}

# Before a push: everything on the last commit, and each commit not yet
# pushed but the last alone, as the last is what everything ran on. The
# last commit is checked out in a worktree of its own, built beside the
# commits', so that the working copy can go on changing while it runs:
# what passes is the commit, whatever is done meanwhile. Nothing is
# written down for a commit added meanwhile. A commit that has built
# nothing new since one on which everything passed is not built and
# tested again: what it reaches is.
push() {
    local upstream commit worktree reach
    upstream=$(git rev-parse --abbrev-ref --symbolic-full-name '@{upstream}' 2>/dev/null || echo origin/main)
    commit=$(git rev-parse HEAD)
    tree=$(git rev-parse "$commit^{tree}")
    echo "=== $(git log -1 --format='%h %s' "$commit")" | tee -a "$log"
    reach=$(reached "$(passed_before "$upstream" "$commit")" "$tree") || exit 1
    # Outside the repository, as in `commits`.
    worktree="$(mktemp -d)/push"
    git worktree add -q --detach "$worktree" "$commit"
    if ! (
        cd "$worktree" &&
            export CARGO_TARGET_DIR="$top/target/commits" &&
            if built "$reach"; then checked && rest; else light "$reach"; fi
    ); then
        git worktree remove --force "$worktree"
        rmdir "${worktree%/*}"
        exit 1
    fi
    git worktree remove --force "$worktree"
    rmdir "${worktree%/*}"
    commits "$upstream" "$commit~1"
    passed push "$tree"
}

# The tests that need a graphics card, which CI does not run, are
# ignored unless asked for, and run here where this user may use one: the
# node OMORFOS_TEST_CARD names, or the first render node. They are run
# without the coverage gate, which runs as CI has it: what only the card
# reaches is left out of it (ADR 2), so the gate would be the same with
# them, and a second run of it would cost minutes. Where there is no card,
# it is said, so that it is not taken for their having passed.
card() {
    local node=${OMORFOS_TEST_CARD:-/dev/dri/renderD128}
    if [ -r "$node" ] && [ -w "$node" ]; then
        step tested cargo test -p omorfos --all-features --locked --test integration \
            -- --ignored gpu:: sharing:: relayed::
    else
        echo "=== No graphics card this user may use: the tests that need one are not run." | tee -a "$log"
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

# The crates a change from `from` to `to`, two trees or commits, can
# reach, as affected.sh says: names, nothing, or `everything`. It stops the
# tier if that cannot be said, rather than take it for nothing.
reached() {
    local said
    said=$(git diff --no-renames --name-only "$1" "$2" | "$top/tools/affected.sh") || {
        echo "FAILED: what the change reaches could not be said" | tee -a "$log"
        exit 1
    }
    echo "$said"
}

# What every commit needs, in a minute or two: the checks that are quick,
# and once each, without the coverage gate, which `push` runs, the tests of
# every crate the change can reach (docs/adr/0094-the-commit-tier-tests-
# what-a-change-can-reach.md). The links between documents are a test of
# every file, and run whatever changed.
commit() {
    local crates
    step typos
    step .githooks/pre-commit --tree
    step cargo fmt --all --check
    step cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
    step tools/affected.test.sh
    step cargo test -p omorfos-config --all-features --locked --test links
    crates=$(reached HEAD "$tree") || exit 1
    reaching "$crates"
}

# Once, without the coverage gate, the tests of what `reached` said: each
# crate, one of a crate's tests where that is all a path is read by, and
# the checks of the scripts and of the workflows.
reaching() {
    local crates=() word
    case "$1" in
        everything)
            step tested cargo test --workspace --all-features --locked
            return
            ;;
        *[![:space:]]*) ;;
        *)
            echo "=== nothing that is built or tested has changed: no more tests" | tee -a "$log"
            return
            ;;
    esac
    # One word to each, as the names have no spaces.
    for word in $1; do
        case "$word" in
            scripts) scripts ;;
            workflows) workflows ;;
            *:lib) step tested cargo test --all-features --locked -p "${word%:lib}" --lib ;;
            *:*) step tested cargo test --all-features --locked -p "${word%%:*}" --test "${word#*:}" ;;
            *) crates+=(-p "$word") ;;
        esac
    done
    [ ${#crates[@]} -eq 0 ] || step tested cargo test --all-features --locked "${crates[@]}"
}

# The scripts of the gates, the hooks and the packaging, by shellcheck,
# and by shfmt as .editorconfig says they are written. Where either is not
# installed, it is said, so that it is not taken for their having passed.
scripts() {
    local found=() each
    # Those there are in the commit being checked: one made before a
    # script was moved or written has it elsewhere, or not at all.
    for each in tools/*.sh .githooks/pre-commit .githooks/pre-push \
        packaging/package.sh packaging/aur/*.sh; do
        [ -e "$each" ] && found+=("$each")
    done
    if command -v shellcheck >/dev/null; then
        step shellcheck "${found[@]}"
    else
        echo "=== shellcheck is not installed: the scripts are not checked by it." | tee -a "$log"
    fi
    if command -v shfmt >/dev/null; then
        step shfmt -d "${found[@]}"
    else
        echo "=== shfmt is not installed: how the scripts are written is not checked." | tee -a "$log"
    fi
}

# The workflows, as CI checks them but for what needs the network: each
# action they use has a commit in the lockfile.
workflows() {
    step zizmor --offline --no-progress .
    step gh actions-lock --verify-local
}

handover() {
    checked
    step make nightly
    ls -l target/nightly/omorfos
}

# What `handover` checks, which `push` checks too, but for the build.
checked() {
    step typos
    step .githooks/pre-commit --tree
    step cargo fmt --all --check
    step cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
    # `cargo coverage` is an alias defined in .cargo/config.toml. This is
    # every test, once, with the coverage gate, as CI runs it.
    explain=uncovered step tested cargo coverage --locked
    passed coverage "$tree"
    card
}

rest() {
    step cargo clippy --workspace --all-targets --locked -- -D warnings
    # As CI has it: the run `handover` made, if it made one on this tree.
    if ! has_passed coverage "$tree"; then
        explain=uncovered step tested cargo coverage --locked
    else
        echo "=== the coverage gate passed on this tree in handover: not run again" | tee -a "$log"
    fi
    step cargo test --workspace --all-features --doc --locked
    step cargo deny check
    workflows
    RUSTDOCFLAGS="-D warnings" step cargo doc --workspace --all-features --no-deps --locked
}

tier=${1:-}
mkdir -p target/gates
# Two runs of the coverage gate at once share the machine, and tests that
# wait on a clock time out: a second waits for the first to end. The lock
# is held by flock, not by a descriptor of this script's, which a program
# a test starts and leaves behind, as a gpg-agent, would keep held.
case "$tier" in
    handover | rest | all | commits | push)
        if [ -z "${GATES_LOCKED:-}" ]; then
            flock -n target/gates/running.lock true ||
                echo "Another run of the gates is going; waiting for it to end." >&2
            GATES_LOCKED=1 exec flock -o target/gates/running.lock "$0" "$@"
        fi
        ;;
esac
log="$top/target/gates/${tier:-none}.log"
: >"$log"
# What the tier runs on. A tier whose tree changes while it runs has
# passed on neither, and is not written down as passed.
tree=$(tree_now)
case "$tier" in
    area)
        shift
        area "$@"
        ;;
    commit) commit ;;
    handover) handover ;;
    rest) rest ;;
    all)
        handover
        rest
        ;;
    commits)
        shift
        commits "$@"
        ;;
    push) push ;;
    uncovered) uncovered | tee -a "$log" ;;
    *)
        echo "usage: $0 area <filter>... | commit | handover | rest | all | commits [base [tip]] | push | uncovered" >&2
        exit 2
        ;;
esac
case "$tier" in
    commit | handover | rest | all)
        if [ "$(tree_now)" = "$tree" ]; then
            passed "$tier" "$tree"
        else
            echo "The working copy changed while the gates ran: run them again." >&2
            exit 1
        fi
        ;;
esac
echo "PASSED: $tier"
