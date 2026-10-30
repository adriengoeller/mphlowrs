"""Plug the Rust kernels into mechaphlowers, for tests and benchmarks.

Inside `with patched():`
- `model_ducloux.approx_parameter` is replaced by `mphlowrs.approx_parameter`;
- `FindParamSolverForLoop.find_parameter` is replaced by `find_parameter_rust`.

Cases the kernels do not cover (polynomial deformation, other span or deformation
models, another cubic solver) fall back to the Python implementation.
"""

import contextlib
import logging
from collections.abc import Iterator

import numpy as np

from mphlowrs._mphlowrs import approx_parameter as _approx_parameter
from mphlowrs._mphlowrs import find_parameter as _find_parameter

from mechaphlowers.core.models.balance.models import model_ducloux
from mechaphlowers.core.models.balance.solvers import find_parameter_solver
from mechaphlowers.core.models.balance.solvers.find_parameter_solver import (
    FindParamModel,
    FindParamSolverForLoop,
)
from mechaphlowers.core.models.cable.deformation import DeformationRte
from mechaphlowers.core.models.cable.span import CatenarySpan
from mechaphlowers.entities.errors import ConvergenceError
from mechaphlowers.numeric.analytical_cubic import AnalyticalRealSolver

# same logger as the Python solver, so the error message is identical
logger = logging.getLogger(find_parameter_solver.__name__)

_python_approx_parameter = model_ducloux.approx_parameter
_python_find_parameter = FindParamSolverForLoop.find_parameter


def _f64(array) -> np.ndarray:
    return np.asarray(array, dtype=np.float64)


def approx_parameter_rust(
    a, b, L0, k_load, cable_section, linear_weight, young_modulus,
    dilatation_coefficient, cable_temperature,
) -> np.ndarray:
    """Drop-in replacement for `model_ducloux.approx_parameter`."""
    if not isinstance(model_ducloux._cubic_solver, AnalyticalRealSolver):
        return _python_approx_parameter(
            a, b, L0, k_load, cable_section, linear_weight, young_modulus,
            dilatation_coefficient, cable_temperature,
        )
    return _approx_parameter(
        _f64(a), _f64(b), _f64(L0), _f64(k_load), float(cable_section),
        float(linear_weight), float(young_modulus), float(dilatation_coefficient),
        _f64(cable_temperature),
    )


def find_parameter_rust(self: FindParamSolverForLoop) -> np.ndarray:
    """Drop-in replacement for `FindParamSolverForLoop.find_parameter`."""
    model = self.model
    if not (
        type(model) is FindParamModel
        and type(model.span_model) is CatenarySpan
        and type(model.deformation_model) is DeformationRte
        and not model.deformation_model.is_polynomial
    ):
        return _python_find_parameter(self)
    span, deformation = model.span_model, model.deformation_model
    parameter, last_evaluated, converged = _find_parameter(
        _f64(model.initial_value),
        _f64(span.span_length),
        _f64(span.elevation_difference),
        _f64(span.load_coefficient),
        _f64(deformation.current_temperature),
        _f64(model.L_ref),
        float(span.linear_weight),
        float(deformation.young_modulus),
        float(deformation.cable_section_area),
        float(deformation.dilatation_coefficient),
        float(model.param_step),
        float(self.stop_condition),
        int(self.max_iter),
    )
    # The Python loop leaves both models in the state of its last _delta call;
    # it is observable (memento.py saves deformation_model.tension_mean / cable_length).
    model.update_models(last_evaluated)
    if not converged:
        logger.error(
            f"Maximum number of iterations reached in {find_parameter_solver.__name__}"
        )
        raise ConvergenceError("Find parameter solver did not converge")
    return parameter


@contextlib.contextmanager
def patched() -> Iterator[None]:
    """Use the Rust kernels in mechaphlowers inside the `with` block."""
    model_ducloux.approx_parameter = approx_parameter_rust
    FindParamSolverForLoop.find_parameter = find_parameter_rust
    try:
        yield
    finally:
        model_ducloux.approx_parameter = _python_approx_parameter
        FindParamSolverForLoop.find_parameter = _python_find_parameter
