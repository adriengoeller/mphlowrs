"""Rust acceleration experiments for mechaphlowers."""

from mphlowrs._mphlowrs import (
    approx_parameter,
    catenary_z,
    catenary_z_many_points,
    find_parameter,
    version,
)

__all__ = [
    "approx_parameter",
    "catenary_z",
    "catenary_z_many_points",
    "find_parameter",
    "version",
]
__version__ = version()
