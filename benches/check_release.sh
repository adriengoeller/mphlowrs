#!/usr/bin/env bash
# Regression check + upstream profiling scripts + v0.8.0 release-notes comparison.
#
# For vendor/mechaphlowers at baseline (HEAD) and with the full patch stack:
#   - benches/compare_results.py dump  (solver outputs, compared at the end)
#   - upstream test/benchmark/profile_balance.py (cProfile + benchmark)
#   - benches/run_upstream_profiling_tests.py (skip-marked test_model_ducloux_profiling.py)
#   - benches/bench_release_notes.py
# then bench_release_notes.py on released versions, each in its own venv:
#   REL_VENVS="v0.7.0=/path/venv v0.8.0=/path/venv" benches/check_release.sh OUT_DIR
# The full stack is re-applied at exit.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
V="$ROOT/vendor/mechaphlowers"
OUT="$(cd "${1:?usage: check_release.sh OUT_DIR}" && pwd)"
PATCHES=("$V"/optim_patches/*.patch)

reset_vendor() {
    git -C "$V" checkout -q HEAD -- src test
    git -C "$V" ls-files --others --exclude-standard -- src test | while read -r f; do rm "$V/$f"; done
}
apply_first() {
    reset_vendor
    for ((i = 0; i < $1; i++)); do git -C "$V" apply "${PATCHES[$i]}"; done
}
restore() {
    apply_first "${#PATCHES[@]}"
    echo "RESTORED: full stack (${#PATCHES[@]} patches) applied"
}
trap restore EXIT

run() { (cd "$ROOT" && uv run python -u -W ignore "$@"); }

for state in baseline stack; do
    if [[ $state == baseline ]]; then apply_first 0; else apply_first "${#PATCHES[@]}"; fi
    echo "== $state"
    run benches/compare_results.py dump "$OUT/results_$state.npz" > /dev/null
    (cd "$V" && uv run --project "$ROOT" python -u -W ignore test/benchmark/profile_balance.py) \
        > "$OUT/profile_balance_$state.log" 2>&1
    grep "^Benchmark" "$OUT/profile_balance_$state.log"
    run benches/run_upstream_profiling_tests.py --repeat 3 | tee "$OUT/ducloux_$state.md"
    run benches/bench_release_notes.py --label "$state" --out "$OUT/release_$state.json" \
        | tee "$OUT/release_$state.md"
done
run benches/compare_results.py compare "$OUT/results_baseline.npz" "$OUT/results_stack.npz" \
    | tee "$OUT/compare.md"

for entry in ${REL_VENVS:-}; do
    label="${entry%%=*}"; venv="${entry#*=}"
    echo "== $label"
    "$venv/bin/python" -u -W ignore "$ROOT/benches/bench_release_notes.py" --label "$label" \
        --out "$OUT/release_$label.json" | tee "$OUT/release_$label.md"
done
echo "ALL DONE"
