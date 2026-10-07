#!/usr/bin/env bash
#
# Write a version into every place that states it, and make sure the revision
# history has something to say about it.
#
# The version is stated in four places on purpose — the manifest, the contract
# instance, and twice in the README, where a reader copies a command — and
# tests hold them equal. This is what keeps them equal.
#
# The revision history is not automated for a contract change. A minor or major
# bump means the contract itself moved, and why it moved is prose somebody
# should write; the release refuses rather than inventing a row. A patch counts
# a release of the suite, so its row is the list of what landed.
set -euo pipefail

cd "$(dirname "$0")/.."

version=${1:?usage: apply-version.sh <version> <kind>}
kind=${2:?usage: apply-version.sh <version> <kind>}

previous=$(grep -m1 '^version = ' Cargo.toml | cut -d'"' -f2)

sed -i.bak "s/^version = \"$previous\"/version = \"$version\"/" Cargo.toml
sed -i.bak "s/^version = \"$previous\"/version = \"$version\"/" contract/contract.toml
sed -i.bak "s/v$previous/v$version/g" README.md
rm -f Cargo.toml.bak contract/contract.toml.bak README.md.bak

# Keeps Cargo.lock's record of this package in step, so --locked still works.
cargo update --quiet --offline -p martian-robots-verify 2>/dev/null || cargo update --quiet -p martian-robots-verify

if grep -q "^| $version |" contract/template.md; then
  exit 0
fi

if [ "$kind" != "patch" ]; then
  echo "the contract moved, so $version needs a revision-history row in" >&2
  echo "contract/template.md saying why. A $kind bump is not a thing to" >&2
  echo "summarise from commit subjects." >&2
  exit 1
fi

last=$(git describe --tags --abbrev=0 --match 'v*' 2>/dev/null || true)
landed=$(git log --format='%s' "${last:+$last..}HEAD" | sed 's/^/\&bull; /' | tr '\n' ' ')
row="| $version | $(date -u +%Y-%m-%d) | A release of the suite: the contract is unchanged, so an implementation that conformed to $last still conforms. $landed |"

python3 - "$version" "$row" <<'PY'
import pathlib, sys
version, row = sys.argv[1], sys.argv[2]
p = pathlib.Path("contract/template.md")
s = p.read_text()
at = s.index("\n", s.index("|---|---|---|")) + 1
p.write_text(s[:at] + row + "\n" + s[at:])
PY
