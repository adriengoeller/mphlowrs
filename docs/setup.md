# Setup

## Toolchain
- Rust: Homebrew `rustup`, stable toolchain (1.99.1 at creation).
  The binaries are in `/opt/homebrew/opt/rustup/bin`, which is not on the PATH by default:
  ```sh
  export PATH="/opt/homebrew/opt/rustup/bin:$PATH"   # add to ~/.zshrc
  ```
- Python: managed with `uv` (Python `>=3.11,<3.14`, same as mechaphlowers).

- Python: the dev env is pinned to 3.13 (`.python-version`), the newest version upstream
  mechaphlowers accepts (`<3.14`).

## Build & test
```sh
uv sync          # creates .venv, builds the extension (maturin, release profile) and installs it
uv run pytest    # runs tests/
```
`[tool.uv] cache-keys` in `pyproject.toml` makes `uv sync` / `uv run` rebuild the extension
whenever `Cargo.toml` or `src/**/*.rs` changes.

For a fast edit/compile loop you can also build in place with maturin:
```sh
uv run maturin develop --uv            # debug build (fast compile, slow code)
uv run maturin develop --uv --release  # use this for any benchmark
```

## Versions (at creation)
| Tool | Version |
|------|---------|
| rustc / cargo | 1.99.1 |
| pyo3 / numpy (crate) | 0.30.1 / 0.30.0 |
| maturin | 1.16.0 |
| numpy (Python) | 2.5.4 |
