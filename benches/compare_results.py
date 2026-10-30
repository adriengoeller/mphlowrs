"""Regression check of solver outputs between two states of vendor/mechaphlowers.

`dump` runs the benches/scenarios.py workloads (4/20/50/100 supports, with and without
loads, several seeds), then stores state vectors, L_ref and every get_data_spans() field
after solve_adjustment and after solve_change_state. `compare` reports the largest
relative difference per field between two dumps.

    uv run python -W ignore benches/compare_results.py dump baseline.npz   # vendor at HEAD
    uv run python -W ignore benches/compare_results.py dump stack.npz      # patches applied
    uv run python -W ignore benches/compare_results.py dump rust.npz --rust  # + Rust kernels
    uv run python benches/compare_results.py compare baseline.npz stack.npz
"""

import sys

import numpy as np

SUPPORTS = [4, 20, 50, 100]
SEEDS = [0, 1, 3]  # seed 2 raises SuspectedChainReversal at 100 supports with loads


def snapshot(engine, prefix: str, out: dict) -> None:
    out[f"{prefix}/state_vector"] = np.asarray(engine.balance_model.state_vector, float)
    out[f"{prefix}/L_ref"] = np.asarray(engine.L_ref, float)
    for key, value in engine.get_data_spans().items():
        out[f"{prefix}/{key}"] = np.asarray(value, float)


def dump(path: str) -> None:
    from scenarios import make_engine, run_change_state

    out: dict[str, np.ndarray] = {}
    for n in SUPPORTS:
        for with_loads in (False, True):
            for seed in SEEDS:
                prefix = f"n{n}/loads{int(with_loads)}/seed{seed}"
                engine = make_engine(n, with_loads=with_loads, seed=seed)
                snapshot(engine, f"{prefix}/adjustment", out)
                run_change_state(engine)
                snapshot(engine, f"{prefix}/change_state", out)
                print(prefix, flush=True)
    np.savez(path, **out)


def compare(path_a: str, path_b: str) -> None:
    a, b = np.load(path_a), np.load(path_b)
    assert set(a.files) == set(b.files), set(a.files) ^ set(b.files)
    worst: dict[str, tuple[float, str]] = {}
    for key in a.files:
        field = key.rsplit("/", 1)[1]
        x, y = a[key], b[key]
        scale = max(np.nanmax(np.abs(x), initial=0.0), 1e-12)
        rel = float(np.nanmax(np.abs(x - y), initial=0.0) / scale)
        assert np.array_equal(np.isnan(x), np.isnan(y)), key
        if rel >= worst.get(field, (-1.0, ""))[0]:
            worst[field] = (rel, key)
    print(f"{len(a.files)} arrays compared")
    print("| field | max relative diff | worst case |")
    print("|---|---:|---|")
    for field, (rel, key) in sorted(worst.items(), key=lambda kv: -kv[1][0]):
        print(f"| {field} | {rel:.1e} | {key.rsplit('/', 1)[0]} |")


if __name__ == "__main__":
    if sys.argv[1] == "dump":
        if "--rust" in sys.argv:  # dump with the mphlowrs kernels plugged in
            from mphlowrs.mechaphlowers_patch import patched

            with patched():
                dump(sys.argv[2])
        else:
            dump(sys.argv[2])
    else:
        compare(sys.argv[2], sys.argv[3])
