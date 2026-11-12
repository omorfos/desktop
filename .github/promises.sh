#!/bin/sh
# Finds each commit after $1 up to $2 that takes away something the
# configuration promises, a line of schemas/promised.txt, without being
# marked breaking, with `!` or a `BREAKING CHANGE:` footer (ADR 73), and
# fails if there is one, saying which and what it took away. What the
# commits say is printed indented, so that no line of theirs is read as
# one of the runner's commands.
set -eu

base=$1
head=$2
promises=schemas/promised.txt

# A branch pushed for the first time has nothing before it to compare.
if [ -z "$(printf '%s' "$base" | tr -d 0)" ]; then
    echo "Nothing before $head to compare with."
    exit 0
fi

broken=0
for commit in $(git rev-list --reverse "$base..$head"); do
    taken=$(git diff "$commit^" "$commit" -- "$promises" | grep '^-' | grep -v '^---' | cut -c2-) || true
    if [ -z "$taken" ]; then
        continue
    fi
    if git log -1 --format=%s "$commit" | grep -qE '^[a-z]+(\([^)]*\))?!:' ||
        git log -1 --format=%b "$commit" | grep -qE '^BREAKING[ -]CHANGE:'; then
        continue
    fi
    echo "::error::$(git rev-parse --short "$commit") takes away what the configuration promises, and is not marked breaking"
    git log -1 --format='  %s' "$commit"
    echo "  It takes away:"
    printf '%s\n' "$taken" | sed 's/^/    /'
    echo "  Mark it with \`!\` after its type, or a \`BREAKING CHANGE:\` footer (docs/RELEASING.md)."
    broken=1
done
exit "$broken"
