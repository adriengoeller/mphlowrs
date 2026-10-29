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
