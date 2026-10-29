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
