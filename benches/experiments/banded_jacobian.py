"""Experiment: banded finite-difference Jacobian in BalanceSolver (monkeypatched).

The balance Jacobian is banded (2 unknowns per support, coupling only to neighbours),
so columns more than 2*BW apart can be perturbed together (Curtis-Powell-Reid grouping):
2*BW + 1 model evaluations instead of n.

Usage:
    uv run python -W ignore benches/experiments/banded_jacobian.py
"""

import sys
import timeit
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from scenarios import make_engine, run_change_state  # noqa: E402

from mechaphlowers.core.models.balance.solvers.balance_solver import BalanceSolver  # noqa: E402

BW = 3  # half-bandwidth measured with jacobian_sparsity.py


def banded_jacobian(self, objective_vector, model, perturb=1e-4):
    n = len(objective_vector)
    jacobian = np.zeros((n, n))
    step = 2 * BW + 1
    for group in range(step):
        cols = np.arange(group, n, step)
        vector_perturb = np.zeros_like(objective_vector)
        vector_perturb[cols] = perturb
        df = (self._delta_d(model, vector_perturb) - objective_vector) / perturb
        for j in cols:
            lo, hi = max(0, j - BW), min(n, j + BW + 1)
            jacobian[lo:hi, j] = df[lo:hi]
    return jacobian


def run(n_supports: int) -> tuple[float, np.ndarray]:
    engine = make_engine(n_supports)
    run_change_state(engine)
    t = min(timeit.repeat(lambda: run_change_state(engine), number=1, repeat=5))
    return t * 1e3, engine.balance_model.state_vector.copy()


def main() -> None:
    dense_jacobian = BalanceSolver.jacobian
    for n in [20, 50, 200]:
        BalanceSolver.jacobian = dense_jacobian
        t_dense, s_dense = run(n)
        BalanceSolver.jacobian = banded_jacobian
        t_banded, s_banded = run(n)
        print(
            f"{n:4d} supports: dense {t_dense:8.2f} ms | banded {t_banded:7.2f} ms"
            f" | x{t_dense / t_banded:5.1f} | max |dstate| {np.max(np.abs(s_banded - s_dense)):.2e}"
        )


if __name__ == "__main__":
    main()
