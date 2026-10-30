# Decision log

## 2026-10-29 — Project bootstrap
- Name `mphlowrs` for the repo, crate and Python module (dots aren't valid in module names).
- mechaphlowers is a plain clone in git-ignored `vendor/` (no fork/submodule): never pushed.
- Build: maturin + PyO3, env via uv, Python `>=3.11,<3.14` (aligned with mechaphlowers).
- Repo private on GitHub under `adriengoeller`; the user runs all `gh` commands.

## 2026-10-30 — Rust package baseline
- maturin mixed layout (`python/` + private `_mphlowrs` extension) so Python wrappers and
  type stubs can sit next to the compiled code.
- No `abi3`: wheels are built per Python version. That's fine for a local experiment and
  avoids limited-API restrictions on the numpy crate.
- `requires-python = ">=3.11,<3.15"`; dev env pinned to 3.13 because upstream mechaphlowers
  is `<3.14`. **TODO:** try mechaphlowers on 3.14/3.15 (overriding its upper bound) so the
  newer profiling tools can be used.
- Release profile uses fat LTO + `codegen-units = 1` (slower builds, faster code for benchmarks).
- First function `catenary_z` (z = p·(cosh(x/p) − 1)) was chosen as a numpy-in/numpy-out
  example close to mechaphlowers' domain.
- Rust edition: `2024` (cargo 1.99.1 does not support `2026`).

## 2026-10-30 — mechaphlowers integration
- mechaphlowers is installed as an editable `dev` dependency through `[tool.uv.sources]`
  path `vendor/mechaphlowers`. `scripts/setup_vendor.sh` clones it at a pinned commit
  (`fbce4fe`, v0.13.2) so results are reproducible. Bump the pin deliberately.
- Consequence: `uv sync` needs `vendor/` to exist, so run the script first after a fresh clone.
- uv locked the env even though mechaphlowers declares `<3.14` and mphlowrs `<3.15`
  (path sources are not range-checked the same way). Running on 3.14+ remains untested.
- First real port: `CatenarySpan.z_many_points` → `catenary_z_many_points`, validated
  against mechaphlowers on a 4-span section.

## 2026-10-30 — First profiling round (BalanceEngine.solve_change_state)
- Profiling harness lives in `benches/` (cProfile + timeit, no extra dependencies).
  Optimisations are first tested as **monkeypatch experiments** in `benches/experiments/`
  so the mechaphlowers clone stays untouched.
- Results (see profiling.md): the dominant cost is algorithmic (dense finite-difference
  Jacobian, O(n²)), not raw computation. Rule adopted: **fix algorithmic and Python-level
  issues first, then port to Rust what remains hot**. Otherwise the Rust speedup is
  measured against a baseline that is easy to beat in Python.
- No Rust code written this round. Pending a decision on the upstream fixes.

## 2026-10-30 — Upstream Python fixes (round 1, option "a")
- The fixes live in the vendor clone (working tree, on local branch `perf/mphlowrs-round1`
  at `fbce4fe`, no commits). Superseded by the patch-stack workflow below.
- **numpy pinned to 2.2.5** (`[tool.uv] constraint-dependencies`), matching upstream's
  `uv.lock`. With numpy 2.5.4, 82 upstream tests fail (2-D `np.cross` removed), so the
  suite cannot serve as a safety net. With 2.2.5, all 865 pass. Upstream issue to report:
  `numpy>=2.2.5,<3` allows a broken numpy.
- The banded Jacobian is **opt-in per model** (`IModelForSolver.jacobian_half_bandwidth`,
  default `None` = dense). Only `BalanceModel` sets it (3, derived from the node/span
  structure). `LoadModel`, which uses the same `BalanceSolver`, is unchanged.
- Benchmark scenario fixed: `SectionArray` `load_mass`/`load_position` columns are
  **ignored** by `BalanceEngine`, so loads must be set with `engine.set_loads(...)`. The first
  profiling round therefore ran **without loads**. Load masses were reduced to 100–400 kg
  because 300–1000 kg produced `SuspectedChainReversal` at 50 and 200 supports (in both
  baseline and fixed code).

## 2026-10-30 — Python pass 1 as a stack of upstream PRs
- User decisions: **stacked** PRs `refactor/optim-<name>`, **no commits** in vendor
  (patches + `optim_patches/apply_stack.sh` that the user runs), report
  `vendor/mechaphlowers/OPTIM_REPORT.md` (not committed), sections ≤ 100 supports.
- 5 patches: banded jacobian, numpy masks, lazy logging, LoadModel block-diagonal jacobian,
  explicit cross products. A 6th Python candidate was considered and left out (per-component
  micro-optimisations of `force_cable`, `find_parameter`, …: 5–10 % each, worse
  readability). That per-evaluation overhead goes to Rust.
- Every step is validated like upstream CI: `ruff format --check`, `ruff check` (0.6.1),
  `mypy .` (1.11.2 **with upstream stub packages** `pandas-stubs`, `plotly-stubs`,
  `types-requests`, `types-pyyaml`: without them, the unmodified baseline already reports
  76 errors) and the full pytest suite. **No `-W ignore`**: it hides a warning that
  `test_duplicated_warning` asserts.
- `apply_stack.sh` was tested on a throwaway clone in the scratchpad (5 stacked commits
  created as expected); it never ran on the user's vendor clone.
- Scripts that swap vendor files must use absolute paths in their exit hook: one
  interrupted run left the vendor at step 1 because the hook ran from inside `vendor/`.

## 2026-10-30 — Validation against released versions
- Regression is checked on **outputs**, not just on the test suite: `compare_results.py`
  compares state vectors and every `get_data_spans()` field, baseline vs stack. Tolerance
  in practice: ~1e-11 relative (finite-difference rounding).
- The v0.8.0 release-notes figures came from an uncommitted script. The scenario was rebuilt
  and judged on whether it reproduces the published v0.7.0 → v0.8.0 **ratios**, not the
  absolute times (different machine). Old releases run in their own venvs (from a
  `git worktree` of the vendor clone, in the scratchpad), never in the mphlowrs env.
- `compare_results.py` skips seed 2: it raises `SuspectedChainReversal` at 100 supports
  with loads, both before and after the stack.

## 2026-10-30 — Rust R1 (`approx_parameter` + `find_parameter`)
- Base for the Rust work: vendor on `refactor/optim-explicit-cross-product` (full Python
  stack, pushed upstream as stacked draft PRs).
- Integration for experiments: **monkeypatch** (`mphlowrs.mechaphlowers_patch.patched()`),
  no change in mechaphlowers. How mechaphlowers would call mphlowrs for real (optional
  dependency, injection point) is still to decide with the user. Note: upstream has an
  injection point (`find_param_solver_type`), but `BalanceModel` does not pass it on to
  `LoadModel`, and `BalanceEngine` does not expose it.
- Ports reproduce Python **semantics**, including quirks (global stop test ignoring the
  last element, model state left at the last evaluation). Rule: a port is validated against
  the Python function (rtol 1e-12) **and** by the upstream suite run with the port plugged in.
- Lesson: never write `contextmanager().__enter__()` without keeping the object. Once it is
  garbage collected, its generator is closed and the `finally` undoes the patch. The first
  `--rust` benchmark silently measured Python.

## 2026-10-30 — Rust R2 (`BalanceSolver.solve` on `BalanceModel`)
- User decision: R2 on a separate branch, `rust/r2-balance-model-kernel` (from `main`).
- Scope: the whole Newton loop, not only the jacobian evaluations. Possible because an
  evaluation is a pure function of the state; it removes every Python evaluation but one.
- The model is left exactly as after the Python loop by one Python evaluation at the final
  state, rather than writing back each attribute from Rust: no attribute can be forgotten,
  at the cost of one evaluation (~0.1 ms without loads, a Python `LoadModel` solve with).
- Failures are replayed in Python instead of rebuilding exceptions in Rust: same exception
  types, messages and logs for free. Only `max_iter` and `LinAlgError` are raised by the
  wrapper. If the replay does not fail, the solve reruns in Python (warning logged).
- PyO3 `extension-module` became a crate feature enabled by maturin only, so that
  `cargo test` links (Rust unit tests for the banded solver).
- Numerical equivalence is checked on outputs (1e-11), not bit-for-bit: the linear solve
  and the norm summation order differ from LAPACK/BLAS.
