# Decision log

## 2026-10-29 — Project bootstrap
- Name `mphlowrs` for the repo, crate and Python module (dots aren't valid in module names).
- mechaphlowers is a plain clone in git-ignored `vendor/` (no fork/submodule): never pushed.
- Build: maturin + PyO3, env via uv, Python `>=3.11,<3.14` (aligned with mechaphlowers).
- Repo private on GitHub under `adriengoeller`; the user runs all `gh` commands.
