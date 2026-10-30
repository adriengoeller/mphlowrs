#!/usr/bin/env bash
# Validate each step of the mechaphlowers patch stack like upstream CI does.
#
# For k = 0..N: reset vendor/mechaphlowers to HEAD, apply patches 1..k, then run
# ruff format --check, ruff check, mypy (+ stubs) with the versions locked in upstream
# uv.lock, and pytest. The full stack is re-applied at exit.
#
# Usage: benches/validate_stack.sh [first_k]
set -uo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
V=$ROOT/vendor/mechaphlowers
PATCHES=("$V"/optim_patches/*.patch)
FIRST=${1:-0}

locked() { grep -A1 "^name = \"$1\"" "$V/uv.lock" | sed -n 's/version = "\(.*\)"/\1/p'; }
RUFF=$(locked ruff)
MYPY_WITH=()
for pkg in mypy pandas-stubs types-requests plotly-stubs types-pyyaml; do
    MYPY_WITH+=(--with "$pkg==$(locked "$pkg")")
done

reset_vendor() {
    git -C "$V" checkout -q HEAD -- src test
    git -C "$V" ls-files --others --exclude-standard -- src test | while read -r f; do rm "$V/$f"; done
}

apply_first() {
    reset_vendor
    for ((i = 0; i < $1; i++)); do
        git -C "$V" apply "${PATCHES[$i]}" || { echo "STEP $1: PATCH APPLY FAILED ${PATCHES[$i]}"; exit 1; }
    done
}

restore() {
    apply_first "${#PATCHES[@]}"
    echo "RESTORED: full stack (${#PATCHES[@]} patches) applied"
}
trap restore EXIT

check() {
    local name=$1; shift
    local out
    if out=$(cd "$V" && "$@" 2>&1); then
        echo "  $name: OK ($(echo "$out" | tail -1))"
    else
        echo "  $name: FAIL"
        echo "$out" | tail -15 | sed 's/^/    /'
    fi
}

for ((k = FIRST; k <= ${#PATCHES[@]}; k++)); do
    if ((k == 0)); then label="baseline"; else label=$(basename "${PATCHES[$((k - 1))]}" .patch); fi
    apply_first "$k"
    echo "STEP $k ($label)"
    check "ruff format" uvx "ruff@$RUFF" format --check
    check "ruff check" uvx "ruff@$RUFF" check
    check "mypy" uv run --project "$ROOT" "${MYPY_WITH[@]}" mypy .
    check "pytest" "$ROOT/.venv/bin/python" -m pytest test -q --ignore=test/benchmark -p no:cacheprovider
done
echo "DONE"
