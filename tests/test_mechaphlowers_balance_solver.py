import logging

import numpy as np
import pandas as pd
import pytest
from numpy.testing import assert_allclose

pytest.importorskip("mechaphlowers.core.models.balance.engine")
from mechaphlowers.core.models.balance.engine import BalanceEngine  # noqa: E402
from mechaphlowers.core.models.balance.solvers.balance_solver import BalanceSolver  # noqa: E402
from mechaphlowers.data.catalog.catalog import sample_cable_catalog  # noqa: E402
from mechaphlowers.entities.arrays import SectionArray  # noqa: E402
from mechaphlowers.entities.errors import (  # noqa: E402
    ConvergenceError,
    SuspectedChainReversal,
)

import mphlowrs.mechaphlowers_patch as mp  # noqa: E402

CABLE = sample_cable_catalog.get_as_object(["ASTER600"])


def random_engine(n: int, seed: int, with_loads: bool) -> BalanceEngine:
    """Random section (same generator as benches/scenarios.py)."""
    rng = np.random.default_rng(seed)
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
    engine = BalanceEngine(cable_array=CABLE, section_array=section_array)
    if with_loads:
        rng = np.random.default_rng(seed + 1)
        has_load = np.arange(n - 1) % 3 == 1
        engine.set_loads(
            load_position_distance=np.where(
                has_load, rng.uniform(0.2, 0.8, n - 1) * span_length[:-1], 0.0
            ),
            load_mass=np.where(has_load, rng.uniform(100.0, 400.0, n - 1), 0.0),
        )
    return engine


def change_state(engine: BalanceEngine) -> None:
    n = len(engine.section_array.data)
    engine.solve_change_state(
        wind_pressure=np.full(n, -200.0),
        ice_thickness=np.full(n, 0.02),
        new_temperature=np.full(n, 5.0),
    )


def model_snapshot(engine: BalanceEngine) -> dict[str, np.ndarray]:
    """Everything a caller can read after a solve."""
    model = engine.balance_model
    out = {f"span/{k}": np.asarray(v, float) for k, v in engine.get_data_spans().items()}
    out["dxdydz"] = model.nodes.dxdydz.copy()
    out["Mx"], out["My"] = model.nodes.Mx.copy(), model.nodes.My.copy()
    out["Th"], out["Tv_d"], out["Tv_g"] = model.Th, model.Tv_d, model.Tv_g
    out["L_ref"] = np.asarray(engine.L_ref, float)
    out["tension_mean"] = engine.deformation_model.tension_mean.copy()
    out["cable_length"] = engine.deformation_model.cable_length.copy()
    return out


@pytest.mark.parametrize("n, seed", [(4, 0), (20, 0), (50, 1)])
@pytest.mark.parametrize("with_loads", [False, True])
def test_solve_leaves_the_same_model_state(n, seed, with_loads):
    expected_engine = random_engine(n, seed, with_loads)
    expected_engine.solve_adjustment()
    after_adjustment = model_snapshot(expected_engine)
    change_state(expected_engine)
    expected = model_snapshot(expected_engine)

    with mp.patched():
        engine = random_engine(n, seed, with_loads)
        engine.solve_adjustment()
        result_adjustment = model_snapshot(engine)
        change_state(engine)
        result = model_snapshot(engine)

    for snapshot, reference in ((result_adjustment, after_adjustment), (result, expected)):
        for key, value in reference.items():
            if key in ("Mx", "My"):
                # residual moments at convergence: ~0 ± 1e-7 N.m (moments involved ~1e4 N.m)
                atol = 1e-5
            else:
                atol = 1e-9 * np.nanmax(np.abs(value), initial=1.0)
            assert_allclose(snapshot[key], value, rtol=0, atol=atol, err_msg=key)


def test_rust_kernel_is_used(monkeypatch):
    calls = []
    original = mp._solve_balance_model
    monkeypatch.setattr(
        mp, "_solve_balance_model", lambda *a: calls.append(1) or original(*a)
    )
    with mp.patched():
        engine = random_engine(20, 0, True)
        engine.solve_adjustment()
        change_state(engine)
    assert len(calls) == 2


class Raised:
    """Exception raised by action(engine): type and message; a trailing
    `norm_d_param=np.float64(x)` is compared as a number (rtol 1e-6), since rounding
    differs on a non-converged iterate."""

    def __init__(self, engine_factory, action):
        engine = engine_factory()
        with pytest.raises(Exception) as info:
            action(engine)
        self.type = type(info.value)
        self.message = str(info.value)
        self.text, _, number = self.message.partition("norm_d_param=np.float64(")
        self.norm = float(number.rstrip(")")) if number else None

    def __eq__(self, other):
        same_norm = (
            self.norm is None
            if other.norm is None
            else self.norm is not None and np.isclose(self.norm, other.norm, rtol=1e-6)
        )
        return self.type is other.type and self.text == other.text and same_norm

    def __repr__(self):
        return f"{self.type.__name__}({self.message!r})"


def raised(engine_factory, action) -> Raised:
    return Raised(engine_factory, action)


def test_chain_reversal_like_python():
    # this random section reverses a chain during the adjustment
    factory = lambda: random_engine(10, 0, True)  # noqa: E731
    expected = raised(factory, BalanceEngine.solve_adjustment)
    assert expected.type is SuspectedChainReversal
    with mp.patched():
        assert raised(factory, BalanceEngine.solve_adjustment) == expected


def test_max_iter_like_python():
    def action(engine):
        engine.solve_adjustment()
        engine.solver_change_state.max_iter = 3
        change_state(engine)

    factory = lambda: random_engine(20, 0, False)  # noqa: E731
    expected = raised(factory, action)
    assert expected.type is ConvergenceError and "max iteration" in expected.message
    with mp.patched():
        assert raised(factory, action) == expected


@pytest.mark.parametrize("with_loads", [False, True])
def test_find_parameter_failure_like_python(with_loads, caplog):
    def action(engine):
        engine.solve_adjustment()
        engine.balance_model.find_param_solver.max_iter = 1
        engine.balance_model.load_model._find_param_solver.max_iter = 1
        change_state(engine)

    factory = lambda: random_engine(20, 0, with_loads)  # noqa: E731
    with caplog.at_level(logging.ERROR):
        expected = raised(factory, action)
    python_logs = [(r.name, r.getMessage()) for r in caplog.records]
    caplog.clear()
    assert expected.type is ConvergenceError
    with mp.patched(), caplog.at_level(logging.ERROR):
        assert raised(factory, action) == expected
    assert [(r.name, r.getMessage()) for r in caplog.records] == python_logs


def test_load_solver_failure_like_python():
    def action(engine):
        engine.solve_adjustment()
        engine.balance_model.load_solver.max_iter = 2
        change_state(engine)

    factory = lambda: random_engine(20, 0, True)  # noqa: E731
    expected = raised(factory, action)
    assert expected.type is ConvergenceError
    with mp.patched():
        assert raised(factory, action) == expected


def test_unsupported_model_falls_back_to_python(monkeypatch):
    class OtherSolver(BalanceSolver):
        pass

    engine = random_engine(20, 0, True)
    engine.balance_model.load_solver.__class__ = OtherSolver  # not the default solver
    calls = []
    monkeypatch.setattr(mp, "_solve_balance_model", lambda *a: calls.append(1))
    with mp.patched():
        engine.solve_adjustment()
        change_state(engine)
    assert calls == []
