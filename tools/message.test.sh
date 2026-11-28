#!/bin/bash
# The tests of message.sh, on messages written for them and thrown away.
# Run by the `commit` tier and by CI. Run from the top of the repository.
set -uo pipefail
script=$PWD/tools/message.sh
failed=0
place=$(mktemp -d)
trap 'rm -rf "$place"' EXIT

# Whether message.sh takes the message `text`, as `expected`, yes or no.
takes() {
    local name=$1 expected=$2 text=$3 got
    printf '%s' "$text" >"$place/message"
    if "$script" "$place/message" >/dev/null 2>&1; then got=yes; else got=no; fi
    if [ "$got" != "$expected" ]; then
        echo "FAILED: $name: expected $expected, got $got"
        failed=1
    fi
}

takes "a subject and a body" yes $'fix: a thing\n\nIt was wrong, and is now right.\n'
takes "a subject alone" no $'fix: a thing\n'
takes "a subject and blank lines" no $'fix: a thing\n\n\n'
takes "a subject and a trailer alone" no $'fix: a thing\n\nFixes #214\n'
takes "a subject and git's comments alone" no $'fix: a thing\n\n# Please enter the commit message\n'
takes "a body and a trailer" yes $'fix: a thing\n\nIt was wrong.\n\nFixes #214\n'
# A misspelling, put together here so that this file holds none.
wrong="t"
wrong+="eh"
takes "a word misspelt in the body" no $'fix: a thing\n\nThis is '"$wrong"$' reason.\n'
takes "a word misspelt in the subject" no $'fix: '"$wrong"$' thing\n\nIt was wrong.\n'

[ "$failed" -eq 0 ] && echo "message.sh: as expected"
exit "$failed"
