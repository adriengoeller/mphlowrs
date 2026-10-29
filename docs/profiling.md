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

## Bottlenecks found
| Date | Function (mechaphlowers) | Share of runtime | Fix | Rust? |
|------|--------------------------|------------------|-----|-------|
| 2026-10-30 | `BalanceSolver.jacobian` (dense FD) | ~96 % @200 | banded column grouping (×24.8 @200) | no, algorithmic |
| 2026-10-30 | `Masks.compute_dx_dy_dz` (list masks) | top self-time | numpy masks (×1.5 @200) | no |
| 2026-10-30 | eager `logger.debug(f"...")` + `check_time` | ~50 % after banded fix | lazy logging, `perfs=False` | no |
| 2026-10-30 | `model.update()` + `objective_function()` | rest | fused kernel | **candidate** |
