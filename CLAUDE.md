# CLAUDE.md — mphlowrs

Rust acceleration experiments for [mechaphlowers](https://github.com/phlowers/mechaphlowers),
built as a Python extension module with **maturin + PyO3**.

## Constraints (from the user — always respect)

### Repository & GitHub
- Repo: `adriengoeller/mphlowrs`, **private** for now.
- **Never run `gh` commands yourself.** Give the user the exact command; the user runs it.
- Never commit or push. Workflow per step: do the work → `git add` the files → propose a
  commit message → **stop** and wait for the user to review and commit → then continue.

### Naming
- Repo, crate, Python package and module are all named `mphlowrs`
  (no `mphlow.rs`: dots are not valid in Python/crate names).

### mechaphlowers
- mechaphlowers is a plain **clone** of `phlowers/mechaphlowers` in `vendor/mechaphlowers/`,
  used only for testing, profiling and benchmarking.
- `vendor/` is git-ignored: **mechaphlowers code is never pushed** to this repo.
- mechaphlowers is MPL-2.0: any file ported/derived from it must keep the MPL-2.0 header.

### Build
- Build backend: maturin; bindings: PyO3 (+ rust-numpy for arrays).
- Python env managed with `uv`. Python range matches mechaphlowers: `>=3.11,<3.15`. But we could test if 3.15 pass on mechaphlowers side in order to profile with 3.15 profil tools. 
- Rust toolchain comes from Homebrew `rustup`; binaries live in `/opt/homebrew/opt/rustup/bin`
  (prefix `PATH` with it if `cargo` is not found).

### Documentation
- `docs/` is the project wiki. Update it **every time** something is learned or decided
  (build steps, profiling results, ported functions, decisions in `docs/decisions/log.md`).

## Future constraints
<!-- Append new constraints given by the user here, with the date. -->
