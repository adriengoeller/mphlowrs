// Derived from mechaphlowers `core/models/balance/models/model_ducloux.py`.
// Copyright (c) 2026, RTE (http://www.rte-france.com)
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use numpy::ndarray::Zip;
use numpy::{IntoPyArray, PyArray1, PyReadonlyArray1};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use crate::analytical_cubic::max_real_root;

/// Port of `approx_parameter` (parabola approximation of the sagging parameter), with the
/// default cubic solver `AnalyticalRealSolver`. Same arguments as the Python function.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
#[allow(non_snake_case)]
pub fn approx_parameter<'py>(
    py: Python<'py>,
    a: PyReadonlyArray1<'py, f64>,
    b: PyReadonlyArray1<'py, f64>,
    L0: PyReadonlyArray1<'py, f64>,
    k_load: PyReadonlyArray1<'py, f64>,
    cable_section: f64,
    linear_weight: f64,
    young_modulus: f64,
    dilatation_coefficient: f64,
    cable_temperature: PyReadonlyArray1<'py, f64>,
) -> PyResult<Bound<'py, PyArray1<f64>>> {
    let (a, b, l0, k, t) = (
        a.as_array(),
        b.as_array(),
        L0.as_array(),
        k_load.as_array(),
        cable_temperature.as_array(),
    );
    let n = a.len();
    if [b.len(), l0.len(), k.len(), t.len()]
        .iter()
        .any(|&len| len != n)
    {
        return Err(PyValueError::new_err(
            "all arrays must have the same length",
        ));
    }
    let roots =
        Zip::from(&a)
            .and(&b)
            .and(&l0)
            .and(&k)
            .and(&t)
            .map_collect(|&a, &b, &l0, &k, &t| {
                // numpy: (a**2 + b**2) ** 0.5 and a**4 use the square / sqrt / pow fast paths
                let circle_chord = (a * a + b * b).sqrt();
                let factor = linear_weight * k / young_modulus / cable_section;
                let p3 = factor * l0;
                let p2 = l0 - circle_chord + dilatation_coefficient * t * l0;
                let p1 = 0.0 * l0;
                let p0 = -a.powi(4) / 24.0 / circle_chord;
                max_real_root(p3, p2, p1, p0)
            });
    Ok(roots.into_pyarray(py))
}
