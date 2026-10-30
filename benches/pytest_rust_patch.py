"""pytest plugin: run a test suite with the mphlowrs Rust kernels plugged in.

Runs the upstream mechaphlowers suite against the Rust kernels:
    cd vendor/mechaphlowers
    PYTHONPATH=../../benches uv run --project ../.. pytest test -p pytest_rust_patch
"""

import pytest

from mphlowrs.mechaphlowers_patch import patched

_context = patched()


def pytest_configure(config: pytest.Config) -> None:
    _context.__enter__()


def pytest_unconfigure(config: pytest.Config) -> None:
    _context.__exit__(None, None, None)


def pytest_report_header(config: pytest.Config) -> str:
    return (
        "mphlowrs: Rust kernels plugged into mechaphlowers "
        "(approx_parameter, find_parameter, BalanceSolver.solve on BalanceModel)"
    )
