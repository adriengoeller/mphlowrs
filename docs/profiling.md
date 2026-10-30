# Profiling

## Tools
| Script | Purpose |
|--------|---------|
| `benches/scenarios.py` | Reproducible workloads: `make_engine(n_supports)` builds a random section (seeded, point load on every third span, ASTER600) and runs `solve_adjustment`. `run_change_state` calls `solve_change_state(wind=-200, ice=0.02, T=5)`. |
| `benches/profile_balance.py` | Timing + cProfile for several sizes. Writes `profiles/balance_<n>.prof` (git-ignored). |
| `benches/experiments/*.py` | Monkeypatch experiments that test an optimisation without editing mechaphlowers. |

```sh
uv run python -W ignore benches/profile_balance.py --supports 4 20 50 200 --repeat 10
uv run python -W ignore benches/experiments/banded_jacobian.py
```
All timings below were measured on an Apple Silicon Mac with Python 3.13 and mechaphlowers
`fbce4fe`, best of N runs.

Known limit: with ≥ 500 supports, the random generator produces sections that raise
`SuspectedChainReversal`, so the generator needs tuning before going above 200.

## 2026-10-30 — `BalanceEngine.solve_change_state`

### Baseline scaling
| Supports | ms / call |
|---------:|----------:|
| 4   | 4.8 |
| 20  | 17.2 |
| 50  | 44.3 |
| 200 | 271.6 |

Time grows faster than linearly with section size.

### Where the time goes (200 supports, cumulative)
- `BalanceSolver.jacobian` accounts for **~96 %** (3.19 s of 3.34 s over 10 calls).
  It is a dense forward-difference Jacobian, so each column costs a full `model.update()` +
  `objective_function()` (`_delta_d`): **n evaluations of an O(n) model → O(n²)**.
- Inside each evaluation, the biggest self-time is `Masks.compute_dx_dy_dz`
  (`utils_model_ducloux.py:32`), then `CatenarySpan.compute_x_m`, `T_mean`,
  `state_vector`, `force_cable`, and `np.cross`.

### Finding 1: the Jacobian is banded (biggest lever, pure Python)
`experiments/jacobian_sparsity.py` (20 supports) gives a 40×40 Jacobian with 2 unknowns per
support and coupling to neighbours only: **half-bandwidth 3, 85 % zeros**, and 7 Jacobian
computations per `solve_change_state`.

Grouping columns that share no row (Curtis–Powell–Reid: columns `j ≡ g mod 7` perturbed
together) needs **7 evaluations instead of n**. Prototype in `experiments/banded_jacobian.py`:

| Supports | dense (ms) | banded (ms) | speedup | max \|Δ state\| |
|---------:|-----------:|------------:|--------:|---------------:|
| 20  | 17.2  | 5.0  | ×3.5  | 7e-14 |
| 50  | 45.8  | 5.9  | ×7.7  | 1e-12 |
| 200 | 278.8 | 11.2 | ×24.8 | 1e-12 |

Caveat: the bandwidth was **measured** on one scenario family. Before relying on it,
derive it from the model structure (which nodes and spans couple which unknowns, including
loads, anchors, and guying) or check it at runtime.

### Finding 2: boolean masks stored as Python lists
`Masks.is_suspension / is_anchor_first / is_anchor_last / is_anchor` are lists of `bool`.
NumPy converts them on each `arr[mask]` (~12× per `compute_dx_dy_dz` call). Converting them
to `np.ndarray` once (`experiments/numpy_masks.py`) gives:

| Supports | lists (ms) | numpy masks (ms) | speedup |
|---------:|-----------:|-----------------:|--------:|
| 20  | 17.3 | 15.7 | ×1.10 |
| 50  | 45.4 | 38.1 | ×1.19 |
| 200 | 270.5 | 178.9 | ×1.51 |

### Finding 3: eager debug logging (dominant once the Jacobian is banded)
After the banded Jacobian, at 200 supports the top self-time is `numpy` **array printing**.
It comes from:
- `engine.py` `solve_change_state`: `logger.debug(f"... {str(array)}")` on inputs,
  `L_ref`, and `get_displacement()`. The f-strings (and `get_displacement()`) are evaluated
  even when DEBUG is off.
- `utils.check_time`: `options.log.perfs` defaults to `True` and formats `args`/`kw` into a
  debug f-string on every decorated call.

From the profile, this is ≈ 10 ms of ≈ 20 ms per call (under cProfile). The fix is to guard
with `logger.isEnabledFor(logging.DEBUG)` or use lazy `%s` arguments, and to default
`perfs` to `False`. Not yet measured in isolation.

### What this means for Rust
- The O(n²) → O(n) gain comes from the algorithm, not the language. Do it first (in
  Python, upstream).
- After findings 1–3, the remaining cost is a constant number (~7 × iterations) of
  `model.update()` + `objective_function()` calls, each made of many small NumPy operations
  on arrays of ~n elements. That per-call overhead is the right target for a Rust
  port: a single fused `update + objective` kernel per evaluation. Re-profile first.

## 2026-10-30 — Python pass 1: upstream patch stack (with loads, ≤ 100 supports)

The first-round numbers above were measured **without loads**: `SectionArray` load
columns are ignored by `BalanceEngine`, so `scenarios.py` now uses `engine.set_loads()`.
With loads, the baseline is much worse because the inner `LoadModel` solve also has a dense
jacobian: **14.9 s per `solve_change_state` at 100 supports**.

Five stacked patches for mechaphlowers, delivered as patches (no commits) in
`vendor/mechaphlowers/optim_patches/`. Full report (per PR: bottleneck, changes, figures,
complexity, risks, reproduction) is in `vendor/mechaphlowers/OPTIM_REPORT.md`.

| # | Patch | Main gain @100 supports |
|---|-------|-------------------------|
| 01 | banded jacobian (`BalanceModel`, half-bandwidth 3) | adjustment ×23, change state ×22 |
| 02 | numpy boolean masks | adjustment ×1.49 |
| 03 | lazy debug logging | change state no loads ×1.41 |
| 04 | block-diagonal jacobian (`LoadModel`, half-bandwidth 1) | change state with loads ×12.9 |
| 05 | explicit cross products in `compute_moment` | adjustment ×1.12 |

Whole stack @100 supports: adjustment 1 676 → 43 ms (×39), change state with loads
14 906 → 50 ms (**×296**), without loads 143 → 6.3 ms (×23). The state checksum is identical
at every step, and every step passes ruff, mypy and the 865+ upstream tests.

Time is now almost flat in n (36–50 ms from 4 to 100 supports): a constant number of
evaluations × per-evaluation Python/NumPy overhead. That overhead is the Rust target.

Tooling:
- `benches/stack_bench.sh`: benchmark steps 0..N of the patch stack (with and without loads).
- `benches/validate_stack.sh`: per step, upstream CI checks (ruff, mypy + stubs, pytest).
- `benches/make_patch.sh NN-name`: next stacked patch = worktree minus existing patches.
- `benches/experiments/*` are first-round monkeypatch prototypes, kept for history (01 and 02
  are now real patches).

## 2026-10-30 — Stack validation and comparison with v0.8.0

Script: `benches/check_release.sh OUT_DIR`, with `REL_VENVS="v0.7.0=<venv> v0.8.0=<venv>"`
(one venv per release, created from `git worktree` checkouts with each release's pins).
It runs `compare_results.py`, upstream `test/benchmark/profile_balance.py`,
`run_upstream_profiling_tests.py` (skip-marked `test_model_ducloux_profiling.py`) and
`bench_release_notes.py`, at baseline and full stack.

- **No regression:** 868 upstream tests pass. Over 720 output arrays
  (`compare_results.py`, 4–100 supports, 3 seeds, with and without loads), the max
  relative difference vs baseline is 5.6e-12 (`state_vector`) and ≤ 4.2e-13 for tensions,
  sags and slopes.
- **Upstream scripts:** `profile_balance.py` 3.89 → 2.85 ms (×1.36, 4 supports, in effect
  no loads); `test_many_spans` (50 supports) 886 → 134 ms (×6.6).
- **v0.8.0 release-notes scenario** (rebuilt: uniform 500 m spans, 500 kg loads at 40 %).
  It reproduces the published v0.7.0 → v0.8.0 ratios within a few points (11 of 12 cases).
  Stack vs v0.8.0: ×2–×2.4 at 5 spans, ×3.4–×8 at 10 spans, ×9–×391 at 30 spans. Every
  configuration is ≤ 31 ms after the stack (v0.8.0: up to 11.9 s).

Full tables are in `vendor/mechaphlowers/OPTIM_REPORT.md`, section "Validation of the whole
stack and comparison with v0.8.0".

Upstream test hygiene, found along the way:
- `test_load_all_spans_wind_ice_temp_profiling` is broken (stale `CableArray` schema).
- `test_many_spans_with_load` has no load.
- `profile_balance.py` loads are ignored.

None of the upstream profiling scripts exercises `LoadModel`.

## 2026-10-30 — Rust R1: `approx_parameter` + `find_parameter`

Profile of the full Python stack at 100 supports with loads: `LoadModel.update` takes
~2/3 of the change state, inside `find_parameter` → `_delta` → `CatenarySpan.compute_values` /
`T_mean`, and `approx_parameter` (cubic). Both models (`BalanceModel` in change state,
`LoadModel`) call this pair, so it was ported first (see ports.md).

Caveat: cProfile overstated this path (it charges per Python call, and these are many tiny
calls). Measured without profiler: 72–77 µs per Python `find_parameter` call vs 27–33 µs
with the Rust kernel (of which 12–18 µs is the Rust call itself and ~14 µs the Python
`update_models` that restores the model state).

`benches/bench_change_state.py` (ms, best of 3, full Python stack = vendor branch
`refactor/optim-explicit-cross-product`):

| supports | change state, loads: Python → Rust | no loads: Python → Rust | adjustment |
|---------:|-----------------------------------:|------------------------:|-----------:|
| 4   | 19.5 → 9.7 (×2.0)  | 3.84 → 3.05 (×1.26) | 27–29 (unchanged) |
| 20  | 29.9 → 14.0 (×2.1) | 4.00 → 3.11 (×1.29) | 27–29 (unchanged) |
| 50  | 36.3 → 18.9 (×1.9) | 4.11 → 3.28 (×1.25) | 29–31 (unchanged) |
| 100 | 39.5 → 21.5 (×1.8) | 4.37 → 4.11 (×1.06) | 33–35 (unchanged) |

State checksums are identical to 10 digits, and the upstream suite passes with the kernels
plugged in (868 passed, `benches/pytest_rust_patch.py`).

Adjustment does not call `find_parameter`, so it is now the slowest operation. Its profile
has no single hot spot: one `BalanceModel.update()` + `objective_function()` is ~0.2 ms,
spread over `Masks.compute_dx_dy_dz`, `force_cable`, `proj_d`/`proj_g`, `compute_moment`,
`compute_inter`, and it runs ~17 Newton iterations × 7 banded evaluations. The cost is
almost independent of n, so only a fused kernel for the whole evaluation would help.

## Bottlenecks found
| Date | Function (mechaphlowers) | Share of runtime | Fix | Rust? |
|------|--------------------------|------------------|-----|-------|
| 2026-10-30 | `BalanceSolver.jacobian` (dense FD) | ~96 % @200 | banded column grouping (×24.8 @200) | no, algorithmic |
| 2026-10-30 | `Masks.compute_dx_dy_dz` (list masks) | top self-time | numpy masks (×1.5 @200) | no |
| 2026-10-30 | eager `logger.debug(f"...")` + `check_time` | ~50 % after banded fix | lazy logging, `perfs=False` | no |
| 2026-10-30 | `LoadModel` dense jacobian (inner solve) | ~98 % with loads after 01–03 | block-diagonal jacobian (×12.9 @100) | no, algorithmic |
| 2026-10-30 | `Nodes.compute_moment` (`np.cross`) | ~11 % of adjustment | explicit components (×1.12) | no |
| 2026-10-30 | `find_parameter` + `approx_parameter` (both models) | ~50 % of change state with loads | Rust R1 (×1.8–×2.1 change state with loads) | ✅ Rust |
| 2026-10-30 | `BalanceModel.update()` + `objective_function()` (nodes, projections, moments) | ~all of adjustment | fused evaluation kernel | candidate R2 |
