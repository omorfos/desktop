#!/bin/bash
# Which of the workspace's crates a change can reach, from the paths it
# changes, one to a line on standard input: each crate whose files changed,
# and each that depends on one of them, as a dependency of any kind, with
# every feature on, as the gates build them. Says one name to a line,
# nothing if nothing that is built or tested has changed, or `everything`.
# See docs/adr/0094-the-commit-tier-tests-what-a-change-can-reach.md.
#
#   git diff --no-renames --name-only HEAD | .claude/skills/handover/affected.sh
#
# A path outside the crates reaches everything, unless it is one of those
# below: the tests read files all over the repository (the guide, the
# manual pages, the schemas, the packaging, the workflows, the README), and
# a path left off this list costs a longer run, where one put on it wrongly
# lets a test go unrun. The links between documents are a test of every
# file there is, so the gates run that one whatever changed.
#
# Run from the top of the repository. AFFECTED_METADATA names a file of
# what `cargo metadata` says, in place of asking it.
set -euo pipefail

# Read by nothing that is built or tested, but the links between documents.
# A directory ends in a slash.
unread=(
    .claude/
    .githooks/
    docs/adr/
    docs/reviews/
    docs/ARCHITECTURE.md
    docs/RELEASING.md
    docs/ROADMAP.md
    docs/TESTING.md
    docs/UPSTREAM.md
    ACCESSIBILITY.md
    CODE_OF_CONDUCT.md
    CONTRIBUTING.md
    _typos.toml
    committed.toml
)

if [ "${1:-}" = --unread ]; then
    printf '%s\n' "${unread[@]}"
    exit 0
fi

if [ -n "${AFFECTED_METADATA:-}" ]; then
    metadata=$(cat "$AFFECTED_METADATA")
else
    metadata=$(cargo metadata --format-version 1 --all-features --locked --offline)
fi

jq -r --arg top "$PWD" \
    --argjson unread "$(printf '%s\n' "${unread[@]}" | jq -R . | jq -s .)" \
    --argjson paths "$(jq -R . | jq -s 'map(select(length > 0))')" '
    .workspace_members as $members
    | [.packages[] | select(.id as $id | $members | index($id))
        | {id, name, dir: (.manifest_path | ltrimstr($top + "/") | rtrimstr("Cargo.toml"))}]
        as $crates
    | [.resolve.nodes[] | select(.id as $id | $members | index($id))
        | {id, deps: [.deps[].pkg | select(. as $pkg | $members | index($pkg))]}]
        as $nodes
    | def read($path): $unread | any(. as $it
        | if endswith("/") then $path | startswith($it) else $path == $it end) | not;
    def owner($path): first($crates[] | select(.dir as $dir | $path | startswith($dir)) | .id)
        // "everything";
    [$paths[] | select(read(.)) | owner(.)] as $changed
    | if $changed | index("everything") then "everything"
      else
        # Each crate that depends on one reached is reached, until no more are.
        def reach: . as $set
            | ($set + [$nodes[] | select(.deps | any(. as $dep | $set | index($dep))) | .id]
                | unique) as $more
            | if ($more | length) == ($set | length) then $set else $more | reach end;
        ($changed | unique | reach) as $reached
        | $crates[] | select(.id as $id | $reached | index($id)) | .name
      end
    ' <<<"$metadata" | sort
