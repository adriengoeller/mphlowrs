# Architecture

```
mphlowrs/
├── CLAUDE.md              # constraints for Claude
├── Cargo.toml             # Rust crate, lib `_mphlowrs` (cdylib, abi3-py311), release LTO
├── Makefile               # local CD: wheels into dist/, checked in build/wheel-check (docs/wheels.md)
├── pyproject.toml         # maturin build backend, uv dev env
├── src/lib.rs             # PyO3 extension module `mphlowrs._mphlowrs` (registers functions)
├── src/<module>.rs        # ports, one Rust file per mechaphlowers module: span.rs, analytical_cubic.rs,
│                          #   find_parameter_solver.rs, balance_solver.rs, model_ducloux.rs, utils_model_ducloux.rs
├── scripts/setup_vendor.sh  # clones mechaphlowers into vendor/ at the pinned commit
├── python/mphlowrs/       # Python package: __init__.py re-exports, .pyi stubs, py.typed,
│                          #   mechaphlowers_patch.py (enable/disable/patched: plugs the kernels into mechaphlowers)
├── tests/                 # pytest: smoke + comparisons against mechaphlowers
├── benches/               # scenarios, profiling, bench_change_state, patch-stack scripts (stack_bench, validate_stack, make_patch),
│                          #   validation vs releases (check_release, compare_results, bench_release_notes, run_upstream_profiling_tests), experiments/
├── profiles/              # .prof outputs (git-ignored)
├── docs/                  # this wiki
└── vendor/mechaphlowers/  # upstream clone, git-ignored
```

## Package layout (maturin "mixed" project)
The compiled module is private (`mphlowrs._mphlowrs`). `python/mphlowrs/__init__.py`
re-exports the public functions, and `_mphlowrs.pyi` provides type hints for editors.
Each new Rust function must be:
1. registered in the `#[pymodule]` in `src/lib.rs`,
2. re-exported in `python/mphlowrs/__init__.py`,
3. declared in `python/mphlowrs/_mphlowrs.pyi`.

Arrays cross the boundary as `PyReadonlyArray1<f64>` (zero-copy read) and come back as
new NumPy arrays (`into_pyarray`).

## Integration principle
mechaphlowers is installed in editable mode from `vendor/` into the same uv environment as
`mphlowrs`. Tests call a mechaphlowers function and its Rust equivalent on the same inputs
and compare the results with `numpy.testing.assert_allclose`.

Conventions for ports:
- The Rust file mirrors the mechaphlowers module (`span.py` → `src/span.rs`) and keeps the
  MPL-2.0 header with a "Derived from …" line.
- Function name: `<model>_<method>`, e.g. `CatenarySpan.z_many_points` →
  `catenary_z_many_points`.
- Shape/validity errors are raised as `ValueError`.
- One test file per mechaphlowers module: `tests/test_mechaphlowers_<module>.py`, which uses
  `pytest.importorskip` on the mechaphlowers module.
