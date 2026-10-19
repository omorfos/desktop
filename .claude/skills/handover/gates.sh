#!/bin/bash
# The gates, in tiers that each stop at the first failure. See
# docs/TESTING.md, "Tiers".
#
#   .claude/skills/handover/gates.sh area <filter>...  the tests of what is being changed
#   .claude/skills/handover/gates.sh handover          what a build for trying by hand needs
#   .claude/skills/handover/gates.sh rest              the rest, run while it is tried
#   .claude/skills/handover/gates.sh all               handover, then rest
#
# Each tier's whole output is kept in target/gates/<tier>.log. `area` takes
# what to run: a crate, as `-p omorfos-core`, or tests of the
# integration binary by name, as `tiles::`; it builds what they need and
# runs only them. Run from the top of the repository.
set -uo pipefail
export PATH="$HOME/.cargo/bin:$PATH"

# Everything each step says is kept whole in a log of the tier's own, so
# that what a test that fails only sometimes said is not lost to a filter.
log=""

step() {
    local started=$SECONDS
    echo "=== $*" | tee -a "$log"
    if ! "$@" 2>&1 | tee -a "$log"; then
        echo "FAILED after $((SECONDS - started))s: $*, the whole of it in $log"
        exit 1
    fi
    echo "--- $((SECONDS - started))s" | tee -a "$log"
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
    step cargo fmt --all --check
    step cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
    # `cargo coverage` is an alias defined in .cargo/config.toml. This is
    # every test, once, with the coverage gate.
    step cargo coverage --locked
    step cargo build --profile nightly --locked -p omorfos -p omorfosctl -p omorfos-panel \
        -p omorfos-launcher -p omorfos-notifications -p omorfos-screenshot -p omorfos-clipboard \
        -p omorfos-recording -p xdg-desktop-portal-omorfos
    ls -l target/nightly/omorfos
}

rest() {
    step cargo clippy --workspace --all-targets --locked -- -D warnings
    # As CI has it, where there is no graphics card: what only the card
    # reaches counts here and not there.
    OMORFOS_TESTS_WITHOUT_CARD=1 step cargo coverage --locked
    step cargo test --workspace --all-features --doc --locked
    step cargo deny check
    RUSTDOCFLAGS="-D warnings" step cargo doc --workspace --all-features --no-deps --locked
}

tier=${1:-}
mkdir -p target/gates
log="target/gates/${tier:-none}.log"
: > "$log"
case "$tier" in
    area) shift; area "$@" ;;
    handover) handover ;;
    rest) rest ;;
    all) handover; rest ;;
    *) echo "usage: $0 area <filter>... | handover | rest | all" >&2; exit 2 ;;
esac
echo "PASSED: $tier"
