#!/bin/sh
# Says each job of each workflow that has no `timeout-minutes`, and fails
# if there is one. A job without it may run for GitHub's six hours: a test
# or a step that waits for good then holds a runner that long before
# anyone is told, where with it the job fails in a few times what it takes.
#
#   tools/timeouts.sh [workflow...]
#
# With no workflow named, every one in .github/workflows. A job is a key
# two spaces in under `jobs:`; its own keys are four in.
set -eu

[ "$#" -gt 0 ] || set -- .github/workflows/*.yml
missing=0
for workflow in "$@"; do
    # What is in single quotes is awk's, not the shell's.
    # shellcheck disable=SC2016
    untimed=$("${AWK:-awk}" '
        /^jobs:/ { jobs = 1; next }
        jobs && /^[^ #]/ { jobs = 0 }
        !jobs { next }
        /^  [A-Za-z0-9_-]+:[ ]*$/ {
            if (job != "" && !timed) print job
            job = $1
            sub(/:$/, "", job)
            timed = 0
            next
        }
        /^    timeout-minutes:/ { timed = 1 }
        END { if (job != "" && !timed) print job }
    ' "$workflow")
    for job in $untimed; do
        echo "::error file=$workflow::the job $job has no timeout-minutes"
        missing=1
    done
done
exit "$missing"
