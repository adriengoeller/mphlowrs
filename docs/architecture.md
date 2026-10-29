# Architecture

```
mphlowrs/
├── CLAUDE.md              # constraints for Claude
├── Cargo.toml             # Rust crate (cdylib) — step 2
├── pyproject.toml         # maturin build backend, uv dev env — step 2
├── src/lib.rs             # PyO3 module `mphlowrs` — step 2
├── tests/                 # pytest: smoke + comparisons against mechaphlowers
├── benches/               # profiling / benchmark scripts
├── docs/                  # this wiki
└── vendor/mechaphlowers/  # upstream clone, git-ignored
```

## Integration principle
mechaphlowers is installed in editable mode from `vendor/` into the same uv environment as
`mphlowrs`. Tests call a mechaphlowers function and its Rust equivalent on the same inputs
and compare the results with `numpy.testing.assert_allclose`.
