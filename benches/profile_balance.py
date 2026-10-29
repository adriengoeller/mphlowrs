"""Profile BalanceEngine.solve_change_state with cProfile for several section sizes.

Usage:
    uv run python benches/profile_balance.py --supports 4 50 200 --repeat 20

Writes profiles/balance_<n>.prof (git-ignored, readable with pstats) and prints
timing plus the top functions by self time.
"""

import argparse
import cProfile
import pstats
import timeit
from pathlib import Path

from scenarios import make_engine, run_change_state

PROFILES_DIR = Path(__file__).resolve().parents[1] / "profiles"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--supports", type=int, nargs="+", default=[4, 50, 200])
    parser.add_argument("--repeat", type=int, default=20)
    parser.add_argument("--top", type=int, default=25)
    parser.add_argument("--no-loads", action="store_true")
    args = parser.parse_args()
    PROFILES_DIR.mkdir(exist_ok=True)

    for n in args.supports:
        engine = make_engine(n, with_loads=not args.no_loads)
        run_change_state(engine)  # warmup

        t = min(timeit.repeat(lambda: run_change_state(engine), number=1, repeat=args.repeat))
        print(f"\n=== {n} supports: {t * 1e3:.2f} ms per solve_change_state (best of {args.repeat}) ===")

        prof = cProfile.Profile()
        prof.enable()
        for _ in range(args.repeat):
            run_change_state(engine)
        prof.disable()
        out = PROFILES_DIR / f"balance_{n}.prof"
        prof.dump_stats(out)

        stats = pstats.Stats(prof)
        stats.strip_dirs().sort_stats("tottime").print_stats(args.top)
        print(f"profile written to {out}")


if __name__ == "__main__":
    main()
