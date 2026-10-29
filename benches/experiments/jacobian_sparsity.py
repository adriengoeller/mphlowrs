"""Experiment: capture the BalanceSolver Jacobian and print its sparsity pattern.

Usage:
    uv run python -W ignore benches/experiments/jacobian_sparsity.py [n_supports]
"""

import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from scenarios import make_engine, run_change_state  # noqa: E402

from mechaphlowers.core.models.balance.solvers.balance_solver import BalanceSolver  # noqa: E402


def main() -> None:
    n_supports = int(sys.argv[1]) if len(sys.argv) > 1 else 20
    captured = []
    dense_jacobian = BalanceSolver.jacobian

    def spy(self, objective_vector, model, perturb=1e-4):
        jacobian = dense_jacobian(self, objective_vector, model, perturb)
        captured.append(jacobian)
        return jacobian

    BalanceSolver.jacobian = spy
    engine = make_engine(n_supports)
    captured.clear()
    run_change_state(engine)

    jacobian = captured[0]
    nonzero = np.abs(jacobian) > 1e-9 * np.abs(jacobian).max()
    rows, cols = np.nonzero(nonzero)
    print(f"jacobian calls per solve_change_state: {len(captured)}, shape: {jacobian.shape}")
    print(f"nonzero fraction: {nonzero.mean():.3f}, half-bandwidth max|i-j|: {np.abs(rows - cols).max()}")
    for row in nonzero[:12, :14]:
        print("".join("#" if v else "." for v in row))


if __name__ == "__main__":
    main()
