# Rust ports

| mechaphlowers function | mphlowrs function | Status | Speedup | Validation test |
|------------------------|-------------------|--------|---------|-----------------|
| `CatenarySpan.z_many_points` (`core/models/cable/span.py`) | `catenary_z_many_points` (`src/span.rs`) | ✅ validated (rtol 1e-12) — 2-D `x` only | not measured | `tests/test_mechaphlowers_span.py` |
| `approx_parameter` (`core/models/balance/models/model_ducloux.py`) + `AnalyticalRealSolver` max root (`numeric/analytical_cubic.py`) | `approx_parameter` (`src/model_ducloux.rs`, `src/analytical_cubic.rs`) | ✅ validated (rtol 1e-12, NaN propagation) | part of R1 below | `tests/test_mechaphlowers_find_parameter.py` |
| `FindParamSolverForLoop.find_parameter` + `FindParamModel._delta` with `CatenarySpan` / linear `DeformationRte` (`core/models/balance/solvers/find_parameter_solver.py`) | `find_parameter` (`src/find_parameter_solver.rs`) | ✅ validated (rtol 1e-12, same model state left, same `ConvergenceError` + log) | 72–77 → 27–33 µs per call (×2.3–×2.7) | `tests/test_mechaphlowers_find_parameter.py` |
| **R2:** `BalanceSolver.solve` on a `BalanceModel` (`core/models/balance/solvers/balance_solver.py`), with each evaluation: `Nodes` state setter + `Masks.compute_dx_dy_dz`, `BalanceModel.update_span` / `update_tensions` / `update_projections`, `VectorProjection.force_cable`, `Nodes.compute_moment`, and in change state `approx_parameter` + `find_parameter` and the inner `LoadModel` solve | `solve_balance_model` (`src/balance_solver.rs`, `src/model_ducloux.rs`, `src/utils_model_ducloux.rs`) | ✅ validated: model state after solve within 1e-9 (relative), 720 output arrays within 1e-11, same exceptions and logs, upstream suite passes | adjustment ×15–×19, change state ×5.4–×8.8 with loads, ×2.9–×3.1 without (vs Python stack) | `tests/test_mechaphlowers_balance_solver.py` |

## Plugging into mechaphlowers
`mphlowrs.mechaphlowers_patch.patched(find_parameter=True, balance_solver=True)` (context
manager) swaps `model_ducloux.approx_parameter`, `FindParamSolverForLoop.find_parameter`
(R1) and `BalanceSolver.solve` (R2) for the Rust versions; unsupported cases (polynomial
deformation, other span/deformation models, another cubic solver, non-default solver
classes, `LoadModel` solves called directly) fall back to Python. Used by:
- `benches/bench_change_state.py --rust r1|r2`;
- `benches/compare_results.py dump out.npz --rust`;
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
- `solve_balance_model` (R2) reproduces the Python loop, including:
  - the **first** `update()` of `BalanceSolver.solve` runs on `Nodes.dxdydz` as it is,
    without the `state_vector` setter: after `solve_adjustment` resets `dxdydz` to 0, the
    suspension chains have dz = 0 (not -L) for that evaluation only, and it is the
    reference of the first jacobian. The kernel receives `dx`/`dy`/`dz` for it;
  - `_delta_d` restores the state with `-= perturb`, i.e. `(s + p) - p`;
  - the linear solve is Gaussian elimination with partial pivoting restricted to the band
    (same pivots as `np.linalg.solve`, different rounding); exact zero pivot →
    `LinAlgError("Singular matrix")`;
  - `ConvergenceError(..., details="norm_d_param=np.float64(...)")` on `max_iter`.
- After the Rust solve, the wrapper runs one Python `state_vector = state; update();
  objective_function()`: every model attribute ends as after the Python loop (its last
  action is the same evaluation). On a failure, it replays the failing evaluation in Python,
  which raises the same exception with the same logs; if Python does not fail there
  (floating-point difference at a threshold), it logs a warning and reruns the whole solve
  in Python.
- Moments `Mx`/`My` at convergence are residuals (~1e-9 to 1e-7 N·m); they differ between
  Python and Rust at that level, which is why the tests compare them with `atol=1e-5`.
- Remaining Python cost with R2 (change state with loads): 2.4–3.3 ms per call out of
  3–7 ms (input extraction, final Python evaluation, which runs the `LoadModel` solve in
  Python, and engine overhead). The Rust solve itself is 0.65 ms at 20 supports and 3.75 ms
  at 100.
