"""Time the skip-marked tests of upstream test/benchmark/test_model_ducloux_profiling.py.

They are marked `@pytest.mark.skip`, so pytest never runs them. They are called directly
here, with the `cable_array_AM600` fixture rebuilt as in test/conftest.py and `random`
seeded so the random change states are the same in every run.

    uv run python -W ignore benches/run_upstream_profiling_tests.py --repeat 3
"""

import argparse
import contextlib
import io
import random
import sys
import time
from pathlib import Path

VENDOR = Path(__file__).resolve().parents[1] / "vendor" / "mechaphlowers"
sys.path.insert(0, str(VENDOR / "test" / "benchmark"))

import test_model_ducloux_profiling as t  # noqa: E402
from mechaphlowers.data.catalog.catalog import sample_cable_catalog  # noqa: E402


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repeat", type=int, default=3)
    args = parser.parse_args()
    cases = {
        "test_load_all_spans_wind_ice_temp_profiling (4 supports, 10 change states)":
            lambda: t.test_load_all_spans_wind_ice_temp_profiling(),
        "test_many_spans (50 supports, 1 change state)":
            lambda: t.test_many_spans(sample_cable_catalog.get_as_object(["ASTER600"])),
        "test_many_spans_with_load (10 supports, 1 change state)":
            lambda: t.test_many_spans_with_load(sample_cable_catalog.get_as_object(["ASTER600"])),
    }
    print("| test | best (ms) | runs |")
    print("|---|---:|---:|")
    for name, fn in cases.items():
        try:
            random.seed(0)
            with contextlib.redirect_stdout(io.StringIO()):
                fn()
        except Exception as error:
            # test_load_all_spans_... builds a CableArray with a pre-thermal column set
            # that the current schema rejects: rerun it with the catalog cable
            print(f"| {name}: broken upstream ({type(error).__name__}) | - | 0 |")
            catalog_cable = sample_cable_catalog.get_as_object(["ASTER600"])
            t.CableArray = lambda *_args, **_kw: catalog_cable
            name += " [CableArray replaced by catalog ASTER600]"
        times = []
        for _ in range(args.repeat):
            random.seed(0)
            t0 = time.perf_counter()
            with contextlib.redirect_stdout(io.StringIO()):  # the tests print progress
                fn()
            times.append(time.perf_counter() - t0)
        print(f"| {name} | {min(times) * 1e3:.1f} | {len(times)} |", flush=True)


if __name__ == "__main__":
    main()
