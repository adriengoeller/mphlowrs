"""Time BalanceEngine.solve_adjustment and solve_change_state for several section sizes.

Benchmarks whatever is checked out in vendor/mechaphlowers (editable install), so two
states of the code can be compared by switching the vendor checkout.

Usage:
    uv run python -u -W ignore benches/bench_change_state.py --label baseline
    uv run python -u -W ignore benches/bench_change_state.py --no-loads --supports 20 100
"""

import argparse
import subprocess
import timeit
from pathlib import Path

import numpy as np

from scenarios import make_engine, run_change_state

VENDOR = Path(__file__).resolve().parents[1] / "vendor" / "mechaphlowers"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--supports", type=int, nargs="+", default=[4, 20, 50, 100])
    parser.add_argument("--repeat", type=int, default=3)
    parser.add_argument("--no-loads", action="store_true")
    parser.add_argument("--label", default="")
    args = parser.parse_args()
    with_loads = not args.no_loads

    commit = subprocess.run(
        ["git", "-C", str(VENDOR), "rev-parse", "--short", "HEAD"],
        capture_output=True, text=True, check=True,
    ).stdout.strip()
    print(
        f"# {args.label or 'run'} — loads={with_loads}, mechaphlowers {commit} (+ worktree), "
        f"numpy {np.__version__}, best of {args.repeat}"
    )
    print("| supports | solve_adjustment (ms) | solve_change_state (ms) | state checksum |")
    print("|---------:|----------------------:|------------------------:|---------------:|")
    for n in args.supports:
        engine = make_engine(n, with_loads=with_loads)  # includes a first solve_adjustment
        t_adj = min(timeit.repeat(engine.solve_adjustment, number=1, repeat=args.repeat))
        run_change_state(engine)  # warmup
        t_cs = min(timeit.repeat(lambda: run_change_state(engine), number=1, repeat=args.repeat))
        checksum = float(np.sum(np.abs(engine.balance_model.state_vector)))
        print(f"| {n} | {t_adj * 1e3:.2f} | {t_cs * 1e3:.2f} | {checksum:.10e} |")


if __name__ == "__main__":
    main()
