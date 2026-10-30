"""Plug the Rust kernels into mechaphlowers, for tests and benchmarks.

Inside `with patched():`
- `model_ducloux.approx_parameter` is replaced by `mphlowrs.approx_parameter` (R1);
- `FindParamSolverForLoop.find_parameter` is replaced by `find_parameter_rust` (R1);
- `BalanceSolver.solve` is replaced by `balance_solver_solve_rust` (R2): the whole Newton
  loop on a `BalanceModel` (including the inner `find_parameter` and `LoadModel` solves)
  runs in Rust.

Cases the kernels do not cover (polynomial deformation, other span or deformation
models, another cubic solver, solvers other than the default ones) fall back to Python.
"""

import contextlib
import logging
from collections.abc import Iterator

import numpy as np

from mphlowrs._mphlowrs import approx_parameter as _approx_parameter
from mphlowrs._mphlowrs import find_parameter as _find_parameter
from mphlowrs._mphlowrs import solve_balance_model as _solve_balance_model

from mechaphlowers.core.models.balance.models import model_ducloux
from mechaphlowers.core.models.balance.models.model_ducloux import BalanceModel
from mechaphlowers.core.models.balance.solvers import find_parameter_solver
from mechaphlowers.core.models.balance.solvers.balance_solver import BalanceSolver
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
_patch_logger = logging.getLogger(__name__)

_python_approx_parameter = model_ducloux.approx_parameter
_python_find_parameter = FindParamSolverForLoop.find_parameter
_python_balance_solve = BalanceSolver.solve

_NODE_TYPES = {"suspension", "anchor_first", "anchor_last"}


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


def _supported_find_param_model(model) -> bool:
    return (
        type(model) is FindParamModel
        and type(model.span_model) is CatenarySpan
        and type(model.deformation_model) is DeformationRte
        and not model.deformation_model.is_polynomial
    )


def find_parameter_rust(self: FindParamSolverForLoop) -> np.ndarray:
    """Drop-in replacement for `FindParamSolverForLoop.find_parameter`."""
    model = self.model
    if not _supported_find_param_model(model):
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


def _solver_settings(prefix: str, solver: BalanceSolver) -> dict:
    return {
        f"{prefix}perturb": float(solver.perturb),
        f"{prefix}stop_condition": float(solver.stop_condition),
        f"{prefix}relax_ratio": float(solver.relax_ratio),
        f"{prefix}relax_power": float(solver.relax_power),
        f"{prefix}max_iter": int(solver.max_iter),
    }


def _find_param_settings(prefix: str, solver: FindParamSolverForLoop) -> dict:
    return {
        f"{prefix}param_step": float(solver.model.param_step),
        f"{prefix}stop_condition": float(solver.stop_condition),
        f"{prefix}max_iter": int(solver.max_iter),
    }


def _balance_model_inputs(solver: BalanceSolver, model) -> dict | None:
    """Data of the `BalanceModel` for `solve_balance_model`, or None if the kernel does
    not cover this model (then the Python solver runs)."""
    if type(model) is not BalanceModel:
        return None
    find_solver = model.find_param_solver
    load_model = model.load_model
    load_find_solver = load_model._find_param_solver
    if not (
        type(model.span_model) is CatenarySpan
        and type(model.deformation_model) is DeformationRte
        and not model.deformation_model.is_polynomial
        and isinstance(model_ducloux._cubic_solver, AnalyticalRealSolver)
        and type(find_solver) is FindParamSolverForLoop
        and type(load_find_solver) is FindParamSolverForLoop
        and _supported_find_param_model(find_solver.model)
        and _supported_find_param_model(load_find_solver.model)
        and type(model.load_solver) is BalanceSolver
        and set(model.nodes.masks.nodes_type) <= _NODE_TYPES
    ):
        return None
    adjustment = bool(model.adjustment)
    if not adjustment and not hasattr(model, "L_ref"):
        return None  # Python raises AttributeError: let it
    nodes, span, deformation = model.nodes, model.span_model, model.deformation_model
    n_spans = len(nodes.span_length)
    # Only used in change state (find_parameter). Before the first adjustment it can have
    # one value per span, depending on how the SectionArray was built.
    deformation_temperature = (
        np.full(n_spans + 1, np.nan)
        if adjustment
        else _f64(deformation.current_temperature)
    )
    if deformation_temperature.shape != (n_spans + 1,):
        return None
    return {
        "adjustment": adjustment,
        # nodes
        "node_type": list(nodes.masks.nodes_type),
        "insulator_length": _f64(nodes.insulator_length),
        "z_arm": _f64(nodes.z_arm),
        "crossarm_length": _f64(nodes.crossarm_length),
        "line_angle": _f64(nodes.line_angle),
        "signed_insulator_weight": _f64(nodes.signed_insulator_weight),
        "signed_counterweight": _f64(nodes.signed_counterweight),
        "bundle_number": float(nodes.bundle_number),
        "dx": _f64(nodes.dx),
        "dy": _f64(nodes.dy),
        "dz": _f64(nodes.dz),
        # span and deformation models
        "span_parameter": _f64(span.parameter),
        "span_load_coefficient": _f64(span.load_coefficient),
        "deformation_temperature": deformation_temperature,
        # per span
        "span_length": _f64(nodes.span_length),
        "beta": _f64(model.beta),
        "k_load": _f64(model.k_load),
        "l_ref": _f64(model.L_ref) if not adjustment else np.zeros(n_spans),
        "current_temperature": _f64(model.current_temperature),
        "has_load_on_span": [bool(v) for v in nodes.has_load_on_span],
        "load_weight": _f64(nodes.load_weight),
        "load_position": _f64(nodes.load_position),
        # constants
        "approx_cable_section": float(model.cable_section),
        "approx_linear_weight": float(model.linear_weight),
        "approx_young_modulus": float(model.young_modulus),
        "approx_dilatation_coefficient": float(model.dilatation_coefficient),
        "linear_weight": float(span.linear_weight),
        "young_modulus": float(deformation.young_modulus),
        "cable_section_area": float(deformation.cable_section_area),
        "dilatation_coefficient": float(deformation.dilatation_coefficient),
        **_find_param_settings("find_param_", find_solver),
        **_find_param_settings("load_find_param_", load_find_solver),
        **_solver_settings("load_solver_", model.load_solver),
        **_solver_settings("solver_", solver),
    }


def balance_solver_solve_rust(self: BalanceSolver, model) -> None:
    """Drop-in replacement for `BalanceSolver.solve`."""
    inputs = _balance_model_inputs(self, model)
    if inputs is None:
        return _python_balance_solve(self, model)
    initial_dxdydz = model.nodes.dxdydz.copy()
    state, _, error, initial, norm_d_param = _solve_balance_model(
        inputs, _f64(model.state_vector)
    )

    if error in (None, "max_iter"):
        # Leave the model as the Python loop does: last update() + objective_function()
        model.state_vector = state
        model.update()
        model.objective_function()
        if error == "max_iter":
            # numpy scalar, as in Python: the message reads "norm_d_param=np.float64(...)"
            norm_d_param = np.float64(norm_d_param)
            raise ConvergenceError(
                "max iteration reached",
                origin="balance_solver",
                details=f"{norm_d_param=}",
            )
        return None

    # Replay the failing step in Python: it raises the same exception, with its logs.
    if error == "singular":
        model.state_vector = state
        model.update()
        raise np.linalg.LinAlgError("Singular matrix")
    if not initial:
        model.state_vector = state  # raises SuspectedChainReversal
    model.update()  # raises ConvergenceError (find_parameter, LoadModel solve)

    # Python did not fail where Rust did (floating-point difference at a threshold):
    # restart the solve in Python from the initial state.
    _patch_logger.warning(
        "Rust and Python disagree on a solver failure (%s): rerun in Python", error
    )
    model.nodes.dxdydz[:] = initial_dxdydz
    return _python_balance_solve(self, model)


@contextlib.contextmanager
def patched(find_parameter: bool = True, balance_solver: bool = True) -> Iterator[None]:
    """Use the Rust kernels in mechaphlowers inside the `with` block.

    `find_parameter`: R1 kernels (`approx_parameter`, `FindParamSolverForLoop.find_parameter`).
    `balance_solver`: R2 kernel (`BalanceSolver.solve` on a `BalanceModel`).
    """
    if find_parameter:
        model_ducloux.approx_parameter = approx_parameter_rust
        FindParamSolverForLoop.find_parameter = find_parameter_rust
    if balance_solver:
        BalanceSolver.solve = balance_solver_solve_rust
    try:
        yield
    finally:
        model_ducloux.approx_parameter = _python_approx_parameter
        FindParamSolverForLoop.find_parameter = _python_find_parameter
        BalanceSolver.solve = _python_balance_solve
