#!/usr/bin/env bash
# Clone mechaphlowers into vendor/ (git-ignored, never pushed) at the pinned commit.
set -euo pipefail

MECHAPHLOWERS_URL="https://github.com/phlowers/mechaphlowers.git"
MECHAPHLOWERS_REF="${1:-fbce4fecb3f682d30cdc966a30a2f09d0027845f}"

cd "$(dirname "$0")/.."
if [ ! -d vendor/mechaphlowers/.git ]; then
    git clone "$MECHAPHLOWERS_URL" vendor/mechaphlowers
fi
git -C vendor/mechaphlowers fetch --tags origin
git -C vendor/mechaphlowers checkout --detach "$MECHAPHLOWERS_REF"
