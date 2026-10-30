import numpy as np
import numpy.typing as npt

def version() -> str: ...
def catenary_z(x: npt.NDArray[np.float64], p: float) -> npt.NDArray[np.float64]: ...
def catenary_z_many_points(
    x: npt.NDArray[np.float64], p: npt.NDArray[np.float64]
) -> npt.NDArray[np.float64]: ...
def approx_parameter(
    a: npt.NDArray[np.float64],
    b: npt.NDArray[np.float64],
    L0: npt.NDArray[np.float64],
    k_load: npt.NDArray[np.float64],
    cable_section: float,
    linear_weight: float,
    young_modulus: float,
    dilatation_coefficient: float,
    cable_temperature: npt.NDArray[np.float64],
) -> npt.NDArray[np.float64]: ...
def find_parameter(
    initial_parameter: npt.NDArray[np.float64],
    span_length: npt.NDArray[np.float64],
    elevation_difference: npt.NDArray[np.float64],
    load_coefficient: npt.NDArray[np.float64],
    temperature: npt.NDArray[np.float64],
    l_ref: npt.NDArray[np.float64],
    linear_weight: float,
    young_modulus: float,
    section: float,
    dilatation_coefficient: float,
    param_step: float,
    stop_condition: float,
    max_iter: int,
) -> tuple[npt.NDArray[np.float64], npt.NDArray[np.float64], bool]: ...
def solve_balance_model(
    inputs: dict[str, object], initial_state: npt.NDArray[np.float64]
) -> tuple[
    npt.NDArray[np.float64],  # state
    npt.NDArray[np.float64],  # objective (empty on error)
    str | None,  # error
    bool,  # error at the first evaluation
    float,  # norm_d_param (max_iter)
]: ...
