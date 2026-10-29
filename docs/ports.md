# Rust ports

| mechaphlowers function | mphlowrs function | Status | Speedup | Validation test |
|------------------------|-------------------|--------|---------|-----------------|
| `CatenarySpan.z_many_points` (`core/models/cable/span.py`) | `catenary_z_many_points` (`src/span.rs`) | ✅ validated (rtol 1e-12) — 2-D `x` only | not measured | `tests/test_mechaphlowers_span.py` |

## Notes
- `catenary_z_many_points`: upstream also accepts a 1-D `x` (the same abscissas for every
  span, broadcast to `(len(x), n_spans)`). The port only handles the 2-D
  `(resolution, n_spans)` input used by `get_coords`/`x()`. It was chosen as an integration
  example, not because profiling showed it to be a bottleneck.
