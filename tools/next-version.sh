#!/usr/bin/env bash
#
# The version the next release should carry and what kind of bump it is, as
# `<version> <kind>`, or nothing if HEAD is already released.
#
# Both come from here so there is one statement of the rule. The workflow
# computing the kind for itself from the two version strings is how that rule
# gets two implementations, one of which is wrong.
#
# The bump is not derived from commit types, because this contract's own policy
# in §2.7 does not use them: a `feat:` that adds a case is a *patch* here,
# since the patch component counts releases of the suite. What the policy does
# say is mechanically checkable, and this repository owns the instrument.
#
#   patch  nothing under contract/ changed, so nothing about conformance can
#          have changed: the suite got better at detecting, which is what a
#          patch counts.
#   minor  the contract changed, and an implementation that conformed to the
#          previous release still conforms to this one.
#   major  the contract changed and the previous release's conforming program
#          no longer passes, or a commit subject declares a break with `!`.
#
# The compatibility check grades the *previous* tag's probe with *this* tree's
# suite. A failure is proof of a major. A pass is not proof of a minor — the
# probe is one conforming program, not every conforming program — so `!` in a
# subject overrides it.
set -euo pipefail

cd "$(dirname "$0")/.."

if [ -n "$(git tag --points-at HEAD --list 'v*')" ]; then
  exit 0
fi

last=$(git describe --tags --abbrev=0 --match 'v*' 2>/dev/null || true)
if [ -z "$last" ]; then
  echo "$(grep -m1 '^version = ' Cargo.toml | cut -d'"' -f2) major"
  exit 0
fi

# Bump from the last release, never from the tree. The automation owns this
# number: deriving it from Cargo.toml would double-count a hand edit, which is
# exactly what happened the first time this script ran.
current=${last#v}

bump() {
  local part=$1 major minor patch
  IFS=. read -r major minor patch <<<"$current"
  case "$part" in
    major) echo "$((major + 1)).0.0 major" ;;
    minor) echo "$major.$((minor + 1)).0 minor" ;;
    patch) echo "$major.$minor.$((patch + 1)) patch" ;;
  esac
}

if git log --format=%s "$last..HEAD" | grep -qE '^[a-z]+(\([^)]*\))?!:'; then
  bump major
  exit 0
fi

if [ -z "$(git diff --name-only "$last..HEAD" -- contract/)" ]; then
  bump patch
  exit 0
fi

# The contract changed. Ask whether what conformed still conforms.
worktree=$(mktemp -d)
trap 'git worktree remove --force "$worktree" >/dev/null 2>&1 || true' EXIT
git worktree add -q --detach "$worktree" "$last"

cargo build --quiet --release --bin martian-robots-verify
if ( cd "$worktree" && cargo build --quiet --release --bin probe ) \
  && ./target/release/martian-robots-verify \
       --bin "$worktree/target/release/probe" --quiet >/dev/null 2>&1; then
  bump minor
else
  bump major
fi
