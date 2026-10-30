import logging

import numpy as np
import pandas as pd
import pytest
from numpy.testing import assert_allclose

import mphlowrs

model_ducloux = pytest.importorskip("mechaphlowers.core.models.balance.models.model_ducloux")
from mechaphlowers.core.models.balance.engine import BalanceEngine  # noqa: E402
from mechaphlowers.core.models.balance.solvers.find_parameter_solver import (  # noqa: E402
    FindParamModel,
    FindParamSolverForLoop,
)
from mechaphlowers.core.models.cable.deformation import deformation_model_builder  # noqa: E402
from mechaphlowers.core.models.cable.span import CatenarySpan  # noqa: E402
from mechaphlowers.data.catalog.catalog import sample_cable_catalog  # noqa: E402
from mechaphlowers.entities.arrays import SectionArray  # noqa: E402
from mechaphlowers.entities.errors import ConvergenceError  # noqa: E402

from mphlowrs.mechaphlowers_patch import patched  # noqa: E402

CABLE = sample_cable_catalog.get_as_object(["ASTER600"])


def cable_constants():
    data = CABLE.data
    return (
        float(data.section.iloc[0]),
        float(data.linear_weight.iloc[0]),
        float(data.young_modulus.iloc[0]),
        float(data.dilatation_coefficient.iloc[0]),
    )


@pytest.fixture
def spans():
    rng = np.random.default_rng(0)
    n = 12
    a = rng.uniform(100.0, 600.0, n)
    b = rng.uniform(-60.0, 60.0, n)
    k = rng.uniform(1.0, 2.5, n)
    temperature = rng.uniform(-20.0, 60.0, n)
    # unstressed length close to the arc length of a 2000 m parameter catenary
    reference = CatenarySpan(a, b, np.full(n, 2000.0), k, np.float64(cable_constants()[1]))
    L_ref = reference.compute_L() * (1 - 5e-4)
    return a, b, k, temperature, L_ref


def find_param_model(a, b, k, temperature, L_ref, p0):
    span = CatenarySpan(a, b, p0.copy(), k, np.float64(cable_constants()[1]))
    deformation = deformation_model_builder(CABLE, span, temperature)
    model = FindParamModel(span, deformation)
    model.set_attributes(initial_parameter=p0, L_ref=L_ref)
    return model


def test_approx_parameter_matches_mechaphlowers(spans):
    a, b, k, temperature, L_ref = spans
    expected = model_ducloux.approx_parameter(a, b, L_ref, k, *cable_constants(), temperature)
    result = mphlowrs.approx_parameter(a, b, L_ref, k, *cable_constants(), temperature)
    assert_allclose(result, expected, rtol=1e-12)


def test_approx_parameter_propagates_nan(spans):
    a, b, k, temperature, L_ref = (x.copy() for x in spans)
    a[-1] = b[-1] = L_ref[-1] = np.nan
    expected = model_ducloux.approx_parameter(a, b, L_ref, k, *cable_constants(), temperature)
    result = mphlowrs.approx_parameter(a, b, L_ref, k, *cable_constants(), temperature)
    assert np.isnan(result[-1]) and np.isnan(expected[-1])
    assert_allclose(result[:-1], expected[:-1], rtol=1e-12)


@pytest.mark.parametrize("nan_last", [False, True])
def test_find_parameter_matches_mechaphlowers(spans, nan_last):
    a, b, k, temperature, L_ref = (x.copy() for x in spans)
    if nan_last:  # BalanceModel arrays have one value per support, NaN for the last one
        a[-1] = b[-1] = L_ref[-1] = np.nan
    p0 = model_ducloux.approx_parameter(a, b, L_ref, k, *cable_constants(), temperature)

    python_model = find_param_model(a, b, k, temperature, L_ref, p0)
    expected = FindParamSolverForLoop(python_model).find_parameter()
    rust_model = find_param_model(a, b, k, temperature, L_ref, p0)
    with patched():
        result = FindParamSolverForLoop(rust_model).find_parameter()

    assert_allclose(result, expected, rtol=1e-12)
    # same state left in the models (last _delta evaluation)
    assert_allclose(rust_model.span_model.parameter, python_model.span_model.parameter, rtol=1e-12)
    for name in ("cable_length", "tension_mean"):
        assert_allclose(
            getattr(rust_model.deformation_model, name),
            getattr(python_model.deformation_model, name),
            rtol=1e-12,
        )


def test_find_parameter_raises_like_mechaphlowers(spans, caplog):
    a, b, k, temperature, L_ref = spans
    p0 = np.full(len(a), 500.0)  # far from the solution: 2 iterations are not enough
    with pytest.raises(ConvergenceError), caplog.at_level(logging.ERROR):
        FindParamSolverForLoop(find_param_model(a, b, k, temperature, L_ref, p0), max_iter=2).find_parameter()
    python_logs = [(r.name, r.levelno, r.getMessage()) for r in caplog.records]
    caplog.clear()
    with patched(), pytest.raises(ConvergenceError), caplog.at_level(logging.ERROR):
        FindParamSolverForLoop(find_param_model(a, b, k, temperature, L_ref, p0), max_iter=2).find_parameter()
    rust_logs = [(r.name, r.levelno, r.getMessage()) for r in caplog.records]
    assert rust_logs == python_logs
    assert "Maximum number of iterations reached" in python_logs[0][2]


def make_engine(with_loads: bool) -> BalanceEngine:
    n = 12
    section_array = SectionArray(
        pd.DataFrame(
            {
                "name": [str(i) for i in range(n)],
                "suspension": [False] + [True] * (n - 2) + [False],
                "conductor_attachment_altitude": [30.0, 50, 60, 65, 55, 40, 45, 60, 70, 50, 45, 40],
                "crossarm_length": [0.0, 10, -10, 0, 10, 0, -10, 0, 10, 0, 0, 0],
                "line_angle": [0.0, 10, 0, 0, 0, 10, 0, 0, 0, 0, 10, 0],
                "insulator_length": [3.0] * n,
                "span_length": [500.0, 300, 400, 450, 350, 500, 420, 380, 460, 300, 400, np.nan],
                "insulator_mass": [1000.0] + [500.0] * (n - 2) + [1000.0],
            }
        ),
        sagging_parameter=2000,
        sagging_temperature=15,
    )
    section_array.add_units({"line_angle": "grad"})
    engine = BalanceEngine(cable_array=CABLE, section_array=section_array)
    if with_loads:
        span_length = section_array.data.span_length.to_numpy()[:-1]
        has_load = np.arange(n - 1) % 3 == 1
        engine.set_loads(
            load_position_distance=np.where(has_load, 0.4 * span_length, 0.0),
            load_mass=np.where(has_load, 300.0, 0.0),
        )
    return engine


def solve(engine: BalanceEngine) -> dict[str, np.ndarray]:
    engine.solve_adjustment()
    engine.solve_change_state(
        wind_pressure=np.full(12, -200.0),
        ice_thickness=np.full(12, 0.02),
        new_temperature=np.full(12, 5.0),
    )
    out = {k: np.asarray(v, float) for k, v in engine.get_data_spans().items()}
    out["state_vector"] = np.asarray(engine.balance_model.state_vector, float)
    return out


@pytest.mark.parametrize("with_loads", [False, True])
def test_balance_engine_results_unchanged(with_loads):
    expected = solve(make_engine(with_loads))
    with patched():
        result = solve(make_engine(with_loads))
    for key, value in expected.items():
        assert_allclose(result[key], value, rtol=1e-9, atol=1e-9, err_msg=key)
