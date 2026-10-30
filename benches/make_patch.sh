#!/usr/bin/env bash
# Write the next stacked patch: vendor/mechaphlowers working tree minus the patches already
# in optim_patches/ (no commit needed: uses a temporary git index).
#
# Usage: benches/make_patch.sh NN-short-name
set -euo pipefail
cd "$(dirname "$0")/../vendor/mechaphlowers"
name=$1
out="optim_patches/$name.patch"
[ -e "$out" ] && { echo "$out already exists" >&2; exit 1; }

export GIT_INDEX_FILE
GIT_INDEX_FILE=$(mktemp)
trap 'rm -f "$GIT_INDEX_FILE"' EXIT
git read-tree HEAD
for p in optim_patches/*.patch; do
    git apply --cached "$p"
done
# include new (untracked) files in the diff, outside optim_patches/
git ls-files --others --exclude-standard -- src test | xargs -r git add -N --
git diff -- src test > "$out"
echo "wrote $out ($(wc -l < "$out") lines)"
