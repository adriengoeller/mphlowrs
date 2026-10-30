// Derived from mechaphlowers `core/models/cable/span.py` (CatenarySpan).
// Copyright (c) 2026, RTE (http://www.rte-france.com)
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use numpy::ndarray::Zip;
use numpy::{IntoPyArray, PyArray2, PyReadonlyArray1, PyReadonlyArray2};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// Port of `CatenarySpan.compute_values` for one span: returns (x_m, x_n, L).
/// Operations follow the Python order so results match numpy to the last bits.
#[inline]
pub(crate) fn catenary_values(a: f64, b: f64, p: f64) -> (f64, f64, f64) {
    let x_m = -a / 2.0 + p * (b / (2.0 * p * (a / (2.0 * p)).sinh())).asinh();
    let x_n = a + x_m;
    let length = p * ((x_n / p).sinh() - (x_m / p).sinh());
    (x_m, x_n, length)
}

/// Port of `CatenarySpan.T_mean` for one span, from the values of `catenary_values`.
#[inline]
pub(crate) fn catenary_t_mean(
    p: f64,
    k_load: f64,
    linear_weight: f64,
    x_m: f64,
    x_n: f64,
    length: f64,
) -> f64 {
    let a = x_n - x_m;
    p * k_load * linear_weight * (a + ((2.0 * x_n / p).sinh() - (2.0 * x_m / p).sinh()) * p / 2.0)
        / length
        / 2.0
}

/// Port of `CatenarySpan.z_many_points` for a 2-D `x` of shape (resolution, n_spans):
/// z[i, j] = p[j] * (cosh(x[i, j] / p[j]) - 1).
#[pyfunction]
pub fn catenary_z_many_points<'py>(
    py: Python<'py>,
    x: PyReadonlyArray2<'py, f64>,
    p: PyReadonlyArray1<'py, f64>,
) -> PyResult<Bound<'py, PyArray2<f64>>> {
    let x = x.as_array();
    let p = p.as_array();
    if x.ncols() != p.len() {
        return Err(PyValueError::new_err(format!(
            "x has {} columns but p has {} spans",
            x.ncols(),
            p.len()
        )));
    }
    let z = Zip::from(&x)
        .and_broadcast(&p)
        .map_collect(|&xi, &pi| pi * ((xi / pi).cosh() - 1.0));
    Ok(z.into_pyarray(py))
}
