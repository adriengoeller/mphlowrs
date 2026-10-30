#!/usr/bin/env bash
# Benchmark a stack of mechaphlowers patches step by step.
#
# For k = 0..N: reset vendor/mechaphlowers to HEAD, apply the first k patches of
# vendor/mechaphlowers/optim_patches/*.patch, run benches/bench_change_state.py
# (with and without loads). At exit, the full stack is re-applied.
#
# Usage: benches/stack_bench.sh [extra args for bench_change_state.py]
set -euo pipefail
cd "$(dirname "$0")/.."
V=vendor/mechaphlowers
PATCHES=("$V"/optim_patches/*.patch)

reset_vendor() {
    git -C "$V" checkout -q HEAD -- src test
    # remove files created by patches (untracked, outside optim_patches/)
    git -C "$V" ls-files --others --exclude-standard -- src test | while read -r f; do rm "$V/$f"; done
}

apply_first() {
    reset_vendor
    local k=$1
    for ((i = 0; i < k; i++)); do
        git -C "$V" apply "${PATCHES[$i]#"$V"/}"
    done
}

restore() {
    apply_first "${#PATCHES[@]}"
    echo "RESTORED: full stack (${#PATCHES[@]} patches) applied"
}
trap restore EXIT

for ((k = 0; k <= ${#PATCHES[@]}; k++)); do
    if ((k == 0)); then label="baseline"; else label="+ $(basename "${PATCHES[$((k - 1))]}" .patch)"; fi
    apply_first "$k"
    uv run python -u -W ignore benches/bench_change_state.py --label "$label" "$@"
    uv run python -u -W ignore benches/bench_change_state.py --label "$label" --no-loads "$@"
done
echo "DONE"
