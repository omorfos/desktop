#!/bin/bash
# Holds one commit message, in `file`, to what AGENTS.md asks of it that
# `committed` does not check: a body after its subject, which says why,
# and no word `typos` knows to be misspelt. A line of git's own, a comment,
# is not part of it, and nor are trailers, as `Fixes #214`, which say no
# why. Run by the commit-msg hook, and by CI for each commit of a pull
# request.
#
#   tools/message.sh <file>
set -uo pipefail
file=${1:?message.sh <file>}
text=$(grep -v '^#' "$file")
# The body: what follows the subject, but trailers.
body=$(printf '%s\n' "$text" | sed '1d' |
    grep -v -E '^[A-Za-z][A-Za-z-]*: |^(Fixes|Closes|Resolves|Part of) #[0-9]+' |
    grep -c '[^[:space:]]')
failed=0
if [ "$body" -eq 0 ]; then
    echo "message.sh: the message has no body; say why the change is made"
    failed=1
fi
if ! printf '%s\n' "$text" | typos -; then
    echo "message.sh: a word of the message is misspelt"
    failed=1
fi
exit "$failed"
