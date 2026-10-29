# Architecture

```
mphlowrs/
├── CLAUDE.md              # constraints for Claude
├── Cargo.toml             # Rust crate, lib `_mphlowrs` (cdylib), release LTO
├── pyproject.toml         # maturin build backend, uv dev env
├── src/lib.rs             # PyO3 extension module `mphlowrs._mphlowrs`
├── python/mphlowrs/       # Python package: __init__.py re-exports, .pyi stubs, py.typed
├── tests/                 # pytest: smoke + comparisons against mechaphlowers
├── benches/               # profiling / benchmark scripts
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
