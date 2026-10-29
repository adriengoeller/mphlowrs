"""Experiment: store the Masks booleans as numpy arrays instead of Python lists.

`Masks.compute_dx_dy_dz` indexes arrays with Python lists of bools, which numpy converts
on every indexing (~12 times per call).

Usage:
    uv run python -W ignore benches/experiments/numpy_masks.py
"""

import sys
import timeit
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from scenarios import make_engine, run_change_state  # noqa: E402

from mechaphlowers.core.models.balance.models.utils_model_ducloux import Masks  # noqa: E402

MASK_NAMES = ("is_suspension", "is_anchor_first", "is_anchor_last", "is_anchor")


def bench(n_supports: int) -> float:
    engine = make_engine(n_supports)
    run_change_state(engine)
    return min(timeit.repeat(lambda: run_change_state(engine), number=1, repeat=7)) * 1e3


def main() -> None:
    sizes = [20, 50, 200]
    baseline = {n: bench(n) for n in sizes}

    list_init = Masks.__init__

    def numpy_init(self, *args, **kwargs):
        list_init(self, *args, **kwargs)
        for name in MASK_NAMES:
            setattr(self, name, np.asarray(getattr(self, name), dtype=bool))

    Masks.__init__ = numpy_init
    for n in sizes:
        t = bench(n)
        print(f"{n:4d} supports: lists {baseline[n]:7.2f} ms | numpy masks {t:7.2f} ms | x{baseline[n] / t:.2f}")


if __name__ == "__main__":
    main()
