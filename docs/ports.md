# Rust ports

| mechaphlowers function | mphlowrs function | Status | Speedup | Validation test |
|------------------------|-------------------|--------|---------|-----------------|
| `CatenarySpan.z_many_points` (`core/models/cable/span.py`) | `catenary_z_many_points` (`src/span.rs`) | ✅ validated (rtol 1e-12) — 2-D `x` only | not measured | `tests/test_mechaphlowers_span.py` |
| `approx_parameter` (`core/models/balance/models/model_ducloux.py`) + `AnalyticalRealSolver` max root (`numeric/analytical_cubic.py`) | `approx_parameter` (`src/model_ducloux.rs`, `src/analytical_cubic.rs`) | ✅ validated (rtol 1e-12, NaN propagation) | part of R1 below | `tests/test_mechaphlowers_find_parameter.py` |
| `FindParamSolverForLoop.find_parameter` + `FindParamModel._delta` with `CatenarySpan` / linear `DeformationRte` (`core/models/balance/solvers/find_parameter_solver.py`) | `find_parameter` (`src/find_parameter_solver.rs`) | ✅ validated (rtol 1e-12, same model state left, same `ConvergenceError` + log) | 72–77 → 27–33 µs per call (×2.3–×2.7) | `tests/test_mechaphlowers_find_parameter.py` |

## Plugging into mechaphlowers
`mphlowrs.mechaphlowers_patch.patched()` (context manager) swaps
`model_ducloux.approx_parameter` and `FindParamSolverForLoop.find_parameter` for the Rust
versions; unsupported cases (polynomial deformation, other span/deformation models,
another cubic solver) fall back to Python. Used by:
- `benches/bench_change_state.py --rust`;
- `benches/pytest_rust_patch.py`, a pytest plugin that runs the **upstream suite** with the
  kernels plugged in (`cd vendor/mechaphlowers && PYTHONPATH=../../benches uv run --project
  ../.. pytest test -p pytest_rust_patch`).

## Notes
- `catenary_z_many_points`: upstream also accepts a 1-D `x` (the same abscissas for every
  span, broadcast to `(len(x), n_spans)`). The port only handles the 2-D
  `(resolution, n_spans)` input used by `get_coords`/`x()`. It was chosen as an integration
  example, not because profiling showed it to be a bottleneck.
- `find_parameter` keeps the Python semantics exactly:
  - global stop test `norm(decr(mem - p)) < stop * size` (the last element is ignored,
    which also happens for `LoadModel`'s combined 2N array);
  - the models are left in the state of the last `_delta` call (`p + step`): `memento.py`
    saves `deformation_model.tension_mean` / `cable_length`, so it is observable. The
    wrapper restores it with one Python `update_models` call (~14 µs of the ~30 µs).
- Formulas follow the numpy operation order; `np.sign(0) = 0` and NaN-propagating
  `np.maximum` are reimplemented (`f64::signum` / `f64::max` differ).
