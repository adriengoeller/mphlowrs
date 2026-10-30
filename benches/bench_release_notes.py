"""Reproduce the v0.8.0 release-notes table (critical change state, balance engine).

The release notes compare v0.7.0 and v0.8.0 for 5/10/30 spans and four load configs
(no load, 1 load, 20 % and 50 % of spans loaded). The script that produced them was not
committed, so the scenario is rebuilt here (assumptions in docs/profiling.md):
- uniform section as upstream test/benchmark/test_model_ducloux_profiling.py
  (500 m spans, 50 m altitude, anchors at both ends), cable ASTER600,
  `spans` = number of spans (supports = spans + 1);
- loads of 500 kg at 40 % of the span, added with `add_loads` (present in every version),
  spread evenly along the section;
- change state of upstream profile_balance.py: wind -200 Pa, ice 2 cm, 5 °C.

Only uses APIs available from v0.7.0 on, so it runs against any installed mechaphlowers:
    <venv>/bin/python -W ignore benches/bench_release_notes.py --label v0.7.0 --out r.json
"""

import argparse
import json
import time

import numpy as np
import pandas as pd

from mechaphlowers.core.models.balance.engine import BalanceEngine
from mechaphlowers.data.catalog.catalog import sample_cable_catalog
from mechaphlowers.entities.arrays import SectionArray

CONFIGS = ["No load", "1 load", "20% loaded", "50% loaded"]


def loaded_spans(config: str, n_spans: int) -> np.ndarray:
    if config == "No load":
        return np.array([], dtype=int)
    if config == "1 load":
        return np.array([n_spans // 2])
    ratio = {"20% loaded": 0.2, "50% loaded": 0.5}[config]
    k = max(1, round(ratio * n_spans))
    return np.unique(np.linspace(0, n_spans - 1, k).round().astype(int))


def make_engine(n_spans: int, config: str) -> BalanceEngine:
    n = n_spans + 1
    section_array = SectionArray(
        pd.DataFrame(
            {
                "name": [str(i) for i in range(n)],
                "suspension": [False] + [True] * (n - 2) + [False],
                "conductor_attachment_altitude": [50.0] * n,
                "crossarm_length": [0.0] * n,
                "line_angle": [0.0] * n,
                "insulator_length": [3.0] * n,
                "span_length": [500.0] * (n - 1) + [np.nan],
                "insulator_mass": [500 / 9.81] * n,
            }
        ),
        sagging_parameter=2000,
        sagging_temperature=15,
    )
    cable = sample_cable_catalog.get_as_object(["ASTER600"])
    engine = BalanceEngine(cable_array=cable, section_array=section_array)
    spans = loaded_spans(config, n_spans)
    if len(spans):
        position = np.zeros(n)
        mass = np.zeros(n)
        position[spans] = 0.4 * 500.0
        mass[spans] = 500.0
        engine.add_loads(load_position_distance=position, load_mass=mass)
    engine.solve_adjustment()
    return engine


def run_change_state(engine: BalanceEngine, n: int) -> None:
    engine.solve_change_state(
        wind_pressure=np.full(n, -200.0),
        ice_thickness=np.full(n, 0.02),
        new_temperature=np.full(n, 5.0),
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--spans", type=int, nargs="+", default=[5, 10, 30])
    parser.add_argument("--repeat", type=int, default=3)
    parser.add_argument("--slow-ms", type=float, default=3000.0,
                        help="calls slower than this are timed once (no warmup)")
    parser.add_argument("--label", default="")
    parser.add_argument("--out", help="json file with timings and solved states")
    args = parser.parse_args()

    results = {"label": args.label, "numpy": np.__version__, "cases": []}
    print(f"# {args.label} — numpy {np.__version__}")
    print("| config | spans | change state (ms) | runs | state checksum |")
    print("|---|---:|---:|---:|---:|")
    for config in CONFIGS:
        for n_spans in args.spans:
            n = n_spans + 1
            engine = make_engine(n_spans, config)
            t0 = time.perf_counter()
            run_change_state(engine, n)  # first call doubles as warmup
            times = [time.perf_counter() - t0]
            if times[0] * 1e3 < args.slow_ms:
                times = []
                for _ in range(args.repeat):
                    t0 = time.perf_counter()
                    run_change_state(engine, n)
                    times.append(time.perf_counter() - t0)
            state = np.asarray(engine.balance_model.state_vector, dtype=float)
            ms = min(times) * 1e3
            print(f"| {config} | {n_spans} | {ms:.1f} | {len(times)} | "
                  f"{np.sum(np.abs(state)):.10e} |", flush=True)
            results["cases"].append(
                {"config": config, "spans": n_spans, "ms": ms,
                 "runs": len(times), "state": state.tolist()}
            )
    if args.out:
        with open(args.out, "w") as f:
            json.dump(results, f)


if __name__ == "__main__":
    main()
