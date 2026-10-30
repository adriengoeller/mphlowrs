// Derived from mechaphlowers `core/models/balance/solvers/find_parameter_solver.py`
// (FindParamSolverForLoop + FindParamModel, with CatenarySpan and the linear DeformationRte).
// Copyright (c) 2026, RTE (http://www.rte-france.com)
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use numpy::{IntoPyArray, PyArray1, PyReadonlyArray1};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use crate::span::{catenary_t_mean, catenary_values};

/// Constant inputs of `FindParamModel._delta` for one span.
struct SpanInputs {
    a: f64,
    b: f64,
    k_load: f64,
    temperature: f64,
    l_ref: f64,
}

/// Cable constants (scalars in mechaphlowers).
struct Cable {
    linear_weight: f64,
    young_modulus: f64,
    section: f64,
    dilatation_coefficient: f64,
}

/// `(parameter, last_evaluated, converged)`, see `find_parameter`.
type FindParameterResult<'py> = (Bound<'py, PyArray1<f64>>, Bound<'py, PyArray1<f64>>, bool);

/// Port of `FindParamModel._delta` for one span:
/// (L(p) - L_ref) / L_ref - (epsilon_mecha(p) + epsilon_therm_0).
#[inline]
fn delta(s: &SpanInputs, cable: &Cable, p: f64) -> f64 {
    let (x_m, x_n, length) = catenary_values(s.a, s.b, p);
    let t_mean = catenary_t_mean(p, s.k_load, cable.linear_weight, x_m, x_n, length);
    let eps_mecha = t_mean / (cable.young_modulus * cable.section);
    let eps_therm = s.temperature * cable.dilatation_coefficient;
    (length - s.l_ref) / s.l_ref - (eps_mecha + eps_therm)
}

/// Port of `FindParamSolverForLoop.find_parameter` (Newton with a forward-difference
/// derivative of step `param_step`).
///
/// The stop test is global, as in Python: `norm(decr(mem - parameter)) < stop * size`, where
/// `decr` drops the last element. Returns `(parameter, last_evaluated, converged)`:
/// `last_evaluated` is the parameter of the last `_delta` call (`mem + param_step`), which is
/// the state Python leaves in the span and deformation models. `converged` is False when
/// `max_iter` is reached (Python raises `ConvergenceError`; the caller does it).
#[pyfunction]
#[allow(clippy::too_many_arguments)]
pub fn find_parameter<'py>(
    py: Python<'py>,
    initial_parameter: PyReadonlyArray1<'py, f64>,
    span_length: PyReadonlyArray1<'py, f64>,
    elevation_difference: PyReadonlyArray1<'py, f64>,
    load_coefficient: PyReadonlyArray1<'py, f64>,
    temperature: PyReadonlyArray1<'py, f64>,
    l_ref: PyReadonlyArray1<'py, f64>,
    linear_weight: f64,
    young_modulus: f64,
    section: f64,
    dilatation_coefficient: f64,
    param_step: f64,
    stop_condition: f64,
    max_iter: usize,
) -> PyResult<FindParameterResult<'py>> {
    let p0 = initial_parameter.as_array();
    let (a, b, k, t, l) = (
        span_length.as_array(),
        elevation_difference.as_array(),
        load_coefficient.as_array(),
        temperature.as_array(),
        l_ref.as_array(),
    );
    let n = p0.len();
    if [a.len(), b.len(), k.len(), t.len(), l.len()]
        .iter()
        .any(|&len| len != n)
    {
        return Err(PyValueError::new_err(
            "all arrays must have the same length",
        ));
    }
    let spans: Vec<SpanInputs> = (0..n)
        .map(|i| SpanInputs {
            a: a[i],
            b: b[i],
            k_load: k[i],
            temperature: t[i],
            l_ref: l[i],
        })
        .collect();
    let cable = Cable {
        linear_weight,
        young_modulus,
        section,
        dilatation_coefficient,
    };

    let mut parameter: Vec<f64> = p0.to_vec();
    let mut last_evaluated = parameter.clone();
    let mut converged = false;
    let threshold = stop_condition * n as f64;
    for _ in 0..max_iter {
        let mut sum_sq = 0.0;
        for (i, s) in spans.iter().enumerate() {
            let mem = parameter[i];
            let d = delta(s, &cable, mem);
            let shifted = mem + param_step;
            let d_prime = (delta(s, &cable, shifted) - d) / param_step;
            parameter[i] = mem - d / d_prime;
            last_evaluated[i] = shifted;
            if i + 1 < n {
                let diff = mem - parameter[i];
                sum_sq += diff * diff;
            }
        }
        if sum_sq.sqrt() < threshold {
            converged = true;
            break;
        }
    }
    Ok((
        parameter.into_pyarray(py),
        last_evaluated.into_pyarray(py),
        converged,
    ))
}
