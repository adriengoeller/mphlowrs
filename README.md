# mphlowrs

Experimental Rust (PyO3 + maturin) acceleration of hot paths in
[mechaphlowers](https://github.com/phlowers/mechaphlowers).

The repo contains the Rust extension, the tests that compare it with mechaphlowers, and
profiling tools. mechaphlowers itself is cloned locally into `vendor/` and is never committed.

## Quickstart
```sh
./scripts/setup_vendor.sh   # clone mechaphlowers into vendor/ (pinned commit)
uv sync                     # build the Rust extension + install mechaphlowers (editable)
uv run pytest
```

See [docs/index.md](docs/index.md) for the wiki.
