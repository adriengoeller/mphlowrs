"""Reproducible mechaphlowers workloads used for profiling and benchmarks.

Section data follows the shape of mechaphlowers' test/benchmark/profile_balance.py,
generated for an arbitrary number of supports.
"""

import numpy as np
import pandas as pd

from mechaphlowers.core.models.balance.engine import BalanceEngine
from mechaphlowers.data.catalog.catalog import sample_cable_catalog
from mechaphlowers.entities.arrays import SectionArray


def make_section_array(n_supports: int, seed: int = 0) -> SectionArray:
    rng = np.random.default_rng(seed)
    n = n_supports
    suspension = np.ones(n, dtype=bool)
    suspension[[0, -1]] = False

    span_length = rng.uniform(250.0, 550.0, n)
    span_length[-1] = np.nan

    section_array = SectionArray(
        pd.DataFrame(
            {
                "name": [str(i) for i in range(n)],
                "suspension": suspension,
                "conductor_attachment_altitude": rng.uniform(30.0, 70.0, n),
                "crossarm_length": rng.choice([-10.0, 0.0, 10.0], n),
                "line_angle": rng.choice([0.0, 0.0, 0.0, 10.0], n),
                "insulator_length": np.full(n, 3.0),
                "span_length": span_length,
                "insulator_mass": np.where(suspension, 500.0, 1000.0),
            }
        )
    )
    section_array.add_units({"line_angle": "grad"})
    section_array.set_sagging_parameter(2000)
    section_array.set_sagging_temperature(15)
    return section_array


def make_engine(n_supports: int, with_loads: bool = True, seed: int = 0) -> BalanceEngine:
    section_array = make_section_array(n_supports, seed)
    cable = sample_cable_catalog.get_as_object(["ASTER600"])
    engine = BalanceEngine(cable_array=cable, section_array=section_array)
    if with_loads:
        # a point load on every third span (load columns of SectionArray are not
        # used by BalanceEngine: loads must go through set_loads)
        rng = np.random.default_rng(seed + 1)
        span_length = section_array.data.span_length.to_numpy()[:-1]
        has_load = np.arange(n_supports - 1) % 3 == 1
        engine.set_loads(
            load_position_distance=np.where(has_load, rng.uniform(0.2, 0.8, n_supports - 1) * span_length, 0.0),
            load_mass=np.where(has_load, rng.uniform(100.0, 400.0, n_supports - 1), 0.0),
        )
    engine.solve_adjustment()
    return engine


def run_change_state(engine: BalanceEngine) -> None:
    n = len(engine.section_array.data)
    engine.solve_change_state(
        wind_pressure=np.full(n, -200.0),
        ice_thickness=np.full(n, 0.02),
        new_temperature=np.full(n, 5.0),
    )
