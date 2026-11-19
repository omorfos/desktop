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
    # Unquoted, so that the lines come out as one line of names.
    # shellcheck disable=SC2086
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
says "the toolchain reaches everything" everything rust-toolchain.toml
says "a file nothing is known of reaches everything" everything something-new.txt
says "the fuzzing, which is a workspace of its own, reaches everything" everything fuzz/Cargo.toml
says "a crate that is not in the workspace reaches everything" everything crates/omorfos-new/src/lib.rs
says "the README reaches the tests that read one of that name" \
    "omorfos-config:documentation omorfos:packaging" README.md
says "the workflows reach the test that reads them, and their own checks" \
    "omorfos:packaging workflows" .github/workflows/test.yml
says "the guide reaches the tests that hold it to the code" \
    "omorfos-config:documentation omorfos:packaging" docs/guide/config.yaml.md
says "a manual page reaches the tests that make it, and the program that keeps its own" \
    "omorfos-config:documentation omorfosctl omorfos:manuals omorfos:packaging" man/omorfos.1
says "a licence reaches the test of what is packaged" \
    omorfos:packaging LICENSES/lucide
says "the schemas reach their test" omorfos-config:schemas schemas/config.schema.json
has "what the schemas promise is built into the protocol, and reaches what speaks it" \
    yes omorfosctl schemas/promised.txt
has "a message is built into the programs that say it" yes omorfos-panel crates/omorfos-i18n/i18n/pl/panel.ftl
has "a message is not built into the model" no omorfos-core crates/omorfos-i18n/i18n/pl/panel.ftl
has "a message is not built into the relay" no omorfos-relay crates/omorfos-i18n/i18n/en/compositor.ftl
has "the credits are built into the compositor" yes omorfos docs/CREDITS.md
says "the Makefile reaches the tests that read it" \
    "omorfos-brand:lib omorfos:packaging" Makefile
says "a test of a crate that is reached whole is not named beside it" \
    "omorfos omorfos-config:documentation" docs/guide/windows.md crates/omorfos/src/paint.rs
says "the gates and the hooks reach their own checks" scripts \
    .claude/skills/handover/gates.sh .githooks/pre-push
says "a decision reaches nothing" "" docs/adr/0001-fresh-implementation-on-smithay.md
says "the roadmap, the agent guide and what tools are set by reach nothing" "" \
    docs/ROADMAP.md AGENTS.md docs/STANDARDS.md clippy.toml rustfmt.toml .gitignore
says "what reaches nothing adds nothing to what does" \
    omorfosctl docs/ROADMAP.md crates/omorfosctl/src/main.rs
says "nothing changed reaches nothing" ""
AFFECTED_METADATA=/nonexistent says "what cannot be worked out is a failure, not nothing" \
    "(failed)" crates/omorfosctl/src/main.rs

# Each path on either list is there: one moved would leave its new place
# reaching everything and its old one on a list for nothing. A name that
# ends in a star is the beginning of some.
there() {
    case "$1" in
        *\*) compgen -G "$1" >/dev/null ;;
        *) [ -e "$1" ] ;;
    esac
}
while read -r path; do
    there "$path" || { echo "FAILED: $path is listed as read by nothing, but is not there"; failed=1; }
done < <("$here/affected.sh" --unread)
while read -r path to; do
    there "$path" || { echo "FAILED: $path is listed as read by $to, but is not there"; failed=1; }
    # And each test it is said to reach is one there is.
    for word in $to; do
        case "$word" in
            *:lib) [ -e "crates/${word%:lib}/src/lib.rs" ] ;;
            *:*) [ -e "crates/${word%%:*}/tests/${word#*:}.rs" ] ;;
            scripts | workflows) true ;;
            *) [ -d "crates/$word" ] ;;
        esac || { echo "FAILED: $path is said to reach $word, which is not there"; failed=1; }
    done
done < <("$here/affected.sh" --only)

# What the code is found to read outside its own crate reaches the crate
# that reads it: a path put on a list wrongly lets a test go unrun. Found
# are the paths written out from two folders up, as `../../man`, which is
# how a crate reaches the top of the repository, in a string or in
# `include_str!`.
while IFS=: read -r file path; do
    crate=${file#crates/}
    crate=${crate%%/*}
    [ -e "$path" ] || continue
    # A directory is asked of by a file it might hold.
    [ -d "$path" ] && path="$path/x"
    got=" $(reached "$path") "
    case "$got" in
        *" everything "* | *" $crate "* | *" $crate:"*) ;;
        *)
            echo "FAILED: $file reads ${path%/x}, which is not said to reach $crate: '$got'"
            failed=1
            ;;
    esac
done < <(grep -rnoE '"(\.\./)+[A-Za-z_.][^"*]*"' crates --include='*.rs' |
    sed -E 's/^([^:]+):[0-9]+:"(\.\.\/)+([^"]*)"$/\1:\3/' | sort -u)

# And what a crate's tests name outright that is at the top of the
# repository, as the test of what is packaged names the files it reads.
# The test of the links reads every document, and is run whatever changed.
while IFS=: read -r file path; do
    crate=${file#crates/}
    crate=${crate%%/*}
    case "$path" in crates/* | target* | src | tests | . | .. | .git) continue ;; esac
    [ -e "$path" ] || continue
    [ -d "$path" ] && path="$path/x"
    got=" $(reached "$path") "
    case "$got" in
        *" everything "* | *" $crate "* | *" $crate:"*) ;;
        *)
            echo "FAILED: $file names ${path%/x}, which is not said to reach $crate: '$got'"
            failed=1
            ;;
    esac
done < <(grep -noE '"[A-Za-z_.][A-Za-z0-9_./-]*"' crates/*/tests/*.rs |
    grep -v '^crates/omorfos-config/tests/links.rs:' |
    sed -E 's/^([^:]+):[0-9]+:"([^"]*)"$/\1:\2/' | sort -u)

[ $failed -eq 0 ] && echo "affected.sh: every test passed"
exit $failed
