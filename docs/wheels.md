# Wheels (local CD)

Builds two wheels into `dist/`:

| Wheel | Tag | Built by |
|-------|-----|----------|
| `mphlowrs-<v>-cp311-abi3-macosx_11_0_arm64.whl` | abi3: one wheel for Python ≥ 3.11 | maturin |
| `mechaphlowers-<v>-py3-none-any.whl` | pure Python, `rust` extra → `mphlowrs` | `uv build` (pdm-backend) in `vendor/` |

`pip install mechaphlowers` stays pure Python. `pip install "mechaphlowers[rust]"` also
installs mphlowrs, and mechaphlowers then plugs the Rust kernels in at import.

## Prerequisite: vendor on the Rust branch
The `rust` extra only exists on the vendor branch `poc/rust-connexion` (from upstream
`main`). Its change is `vendor/mechaphlowers/rust_patches/01-mphlowrs-extra.patch`:
- `pyproject.toml`: `rust = ["mphlowrs>=0.1,<0.2"]` in `[project.optional-dependencies]`;
- `src/mechaphlowers/_rust.py`: if `mphlowrs` can be imported, call
  `mphlowrs.mechaphlowers_patch.enable()`, unless `MECHAPHLOWERS_RUST=0`;
- `src/mechaphlowers/__init__.py`: imports `_rust` last (the modules it patches are
  loaded by then), and exposes `mechaphlowers.RUST_ENABLED`.

`uv.lock` is not updated: mphlowrs is not on PyPI, and `uv sync --frozen` (upstream
`make install`) keeps working without it.

## Make targets
```sh
make             # list the targets
make cd          # = wheels + install-wheels + check-wheels + test-wheels
make wheels      # build both wheels into dist/
make install-wheels [PY=3.11]   # clean venv build/wheel-check, install from dist/ only
make check-wheels               # kernels on by default, off with MECHAPHLOWERS_RUST=0
make test-wheels                # upstream suite on the installed wheels, Rust on then off
make clean-wheels
```

## The same, by hand
```sh
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"

# 1. wheels
uv run maturin build --release --out dist
(cd vendor/mechaphlowers && uv build --wheel --out-dir ../../dist)

# 2. clean venv, installed from dist/ (--find-links: mphlowrs is not on PyPI)
uv venv --python 3.13 build/wheel-check
uv pip install --python build/wheel-check/bin/python --find-links dist \
    "mechaphlowers[rust]" "numpy==2.2.5" pytest

# 3. check (outside the repo, so nothing is imported from the sources)
(cd build/wheel-check && bin/python -c "import mechaphlowers; print(mechaphlowers.RUST_ENABLED)")

# 4. upstream test suite against the installed wheels
cd vendor/mechaphlowers
../../build/wheel-check/bin/python -m pytest test -q -p no:cacheprovider \
    -o pythonpath= -o addopts= -m "not benchmark and not release_test"
MECHAPHLOWERS_RUST=0 ../../build/wheel-check/bin/python -m pytest test -q -p no:cacheprovider \
    -o pythonpath= -o addopts= -m "not benchmark and not release_test"
```
`-o pythonpath=` overrides the upstream `pythonpath = ["src"]`, so the tests import the
**installed** mechaphlowers (`test/conftest.py` only *appends* `src` to `sys.path`).

## Results (2026-10-30, mechaphlowers 0.13.2 @ `fbce4fe` + patch, macOS arm64, Python 3.13)
| Run | Result | Time |
|-----|--------|------|
| upstream suite, Rust on | 865 passed, 28 skipped | 101 s |
| upstream suite, `MECHAPHLOWERS_RUST=0` | 865 passed, 28 skipped | 178 s |
| `install-wheels check-wheels PY=3.11` | kernels enabled | — |

## Caveat for the dev env
The dev env installs mechaphlowers editable from `vendor/`. On the Rust branch, importing
mechaphlowers enables the kernels, so a "Python" reference is Rust too:
- `tests/conftest.py` sets `MECHAPHLOWERS_RUST=0` (tests plug the kernels in explicitly);
- for Python baselines in `benches/`, run with `MECHAPHLOWERS_RUST=0`.
