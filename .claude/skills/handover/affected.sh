#!/bin/bash
# What a change can reach of what is built and tested, from the paths it
# changes, one to a line on standard input. Says, one to a line:
#
#   a crate's name      each crate whose files changed, and each that
#                       depends on one of them, as a dependency of any
#                       kind, with every feature on, as the gates build them
#   crate:target        one of a crate's tests alone, `lib` for those in
#                       its library, where that is all that reads a path
#                       and the crate itself is not reached
#   scripts             the gates' own scripts and the hooks
#   workflows           what CI runs
#
# or nothing, if nothing that is built or tested has changed, or
# `everything`. The tiers all ask this, `commit` and `push` alike, so that
# there is one list. See
# docs/adr/0094-the-commit-tier-tests-what-a-change-can-reach.md.
#
#   git diff --no-renames --name-only HEAD | .claude/skills/handover/affected.sh
#
# A path inside a crate is the crate's. A path outside the crates reaches
# everything, unless it is one of those below: a path left off these lists
# costs a longer run, where one put on them wrongly lets a test go unrun.
# affected.test.sh holds them to what the tests are found to read. The
# links between documents are a test of every file there is, so the gates
# run that one whatever changed, as they run the formatter, clippy and the
# check of spelling: what only those read is on the first list.
#
# Run from the top of the repository. AFFECTED_METADATA names a file of
# what `cargo metadata` says, in place of asking it.
set -euo pipefail

# Read by nothing that is built or tested, but the links between documents
# and the tools every tier runs. A directory ends in a slash; a name that
# ends in a star is every path that begins so.
unread=(
    docs/adr/
    docs/reviews/
    docs/ARCHITECTURE.md
    docs/BRAND.md
    docs/DESIGN.md
    docs/IPC.md
    docs/RELEASING.md
    docs/ROADMAP.md
    docs/STANDARDS.md
    docs/TESTING.md
    docs/UPSTREAM.md
    docs/ACCESSIBILITY.md
    AGENTS.md
    CLAUDE.md
    docs/CODE_OF_CONDUCT.md
    docs/CONTRIBUTING.md
    docs/SECURITY.md
    _typos.toml
    committed.toml
    clippy.toml
    rustfmt.toml
    .editorconfig
    .gitignore
    .release-please-manifest.json
    release-please-config.json
)

# Read by less than everything: each path, and after it what it reaches,
# as above. The first that fits a path is the one taken.
only=(
    # Built into the programs: what depends on the crate is reached too.
    "schemas/promised.txt omorfos-ipc"
    "docs/CREDITS.md omorfos-core"
    # Read by a test or two, which hold them to the code.
    "schemas/ omorfos-config:schemas"
    "docs/guide/ omorfos-config:documentation omorfos:packaging"
    "man/ omorfos-config:documentation omorfos:manuals omorfos:packaging omorfosctl"
    "docs/brand/ omorfos-brand:docs"
    "Makefile omorfos-brand:lib omorfos:packaging"
    "packaging/ omorfos:packaging"
    "README.md omorfos-config:documentation omorfos:packaging"
    "LICENSE omorfos:packaging"
    "LICENSES/ omorfos:packaging"
    "about.hbs omorfos:packaging"
    "about.toml omorfos:packaging"
    "deny.toml omorfos:packaging"
    ".github/ omorfos:packaging workflows"
    # Built into nothing: their own tests, and shellcheck.
    ".claude/ scripts"
    ".githooks/ scripts"
)

if [ "${1:-}" = --unread ]; then
    printf '%s\n' "${unread[@]}"
    exit 0
fi
if [ "${1:-}" = --only ]; then
    printf '%s\n' "${only[@]}"
    exit 0
fi

if [ -n "${AFFECTED_METADATA:-}" ]; then
    metadata=$(cat "$AFFECTED_METADATA")
else
    metadata=$(cargo metadata --format-version 1 --all-features --locked --offline)
fi

jq -r --arg top "$PWD" \
    --argjson unread "$(printf '%s\n' "${unread[@]}" | jq -R . | jq -s .)" \
    --argjson only "$(printf '%s\n' "${only[@]}" | jq -R 'split(" ") | {path: .[0], to: .[1:]}' | jq -s .)" \
    --argjson paths "$(jq -R . | jq -s 'map(select(length > 0))')" '
    .workspace_members as $members
    | [.packages[] | select(.id as $id | $members | index($id))
        | {id, name, dir: (.manifest_path | ltrimstr($top + "/") | rtrimstr("Cargo.toml"))}]
        as $crates
    | [.resolve.nodes[] | select(.id as $id | $members | index($id))
        | {id, deps: [.deps[].pkg | select(. as $pkg | $members | index($pkg))]}]
        as $nodes
    | def fits($path; $it):
        if $it | endswith("/") then $path | startswith($it)
        elif $it | endswith("*") then $path | startswith($it | rtrimstr("*"))
        else $path == $it end;
    def read($path): $unread | any(. as $it | fits($path; $it)) | not;
    # A crate by its name, as the lists have it; one that is not there, as
    # a crate taken out is not, reaches everything.
    def named($name): first($crates[] | select(.name == $name) | .id) // "everything";
    def owner($path): first($crates[] | select(.dir as $dir | $path | startswith($dir)) | .id)
        // "everything";
    # What a path reaches: a crate, by its id; `crate:target`, `scripts` or
    # `workflows`, as they are; or `everything`.
    def reaches($path):
        (first($only[] | select(.path as $it | fits($path; $it)) | .to) // null) as $to
        | if $to then $to[] | if test(":") or . == "scripts" or . == "workflows" then .
                              else named(.) end
          else owner($path) end;
    [$paths[] | select(read(.)) | reaches(.)] as $all
    | if $all | index("everything") then "everything"
      else
        # Each crate that depends on one reached is reached, until no more are.
        def reach: . as $set
            | ($set + [$nodes[] | select(.deps | any(. as $dep | $set | index($dep))) | .id]
                | unique) as $more
            | if ($more | length) == ($set | length) then $set else $more | reach end;
        ([$all[] | select(. as $it | $crates | any(.id == $it))] | unique | reach) as $reached
        | [$crates[] | select(.id as $id | $reached | index($id)) | .name] as $names
        # One test of a crate that is reached whole is run with the rest.
        | [$all[] | select(. as $it | $crates | any(.id == $it) | not)
            | select((split(":")[0]) as $crate | $names | index($crate) | not)] as $more
        | ($names + $more | unique)[]
      end
    ' <<<"$metadata" | sort
