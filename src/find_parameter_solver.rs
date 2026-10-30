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

/// Cable constants seen by `FindParamModel` (span model `linear_weight`, deformation model
/// `young_modulus`, `cable_section_area`, `dilatation_coefficient`; scalars in mechaphlowers).
#[derive(Clone, Copy)]
pub(crate) struct Cable {
    pub linear_weight: f64,
    pub young_modulus: f64,
    pub section: f64,
    pub dilatation_coefficient: f64,
}

/// Settings of `FindParamSolverForLoop` / `FindParamModel`.
#[derive(Clone, Copy)]
pub(crate) struct FindParamSettings {
    pub param_step: f64,
    pub stop_condition: f64,
    pub max_iter: usize,
}

/// Inputs of `FindParamModel` for a batch of spans (one value per span).
pub(crate) struct FindParamInputs<'a> {
    pub span_length: &'a [f64],
    pub elevation_difference: &'a [f64],
    pub load_coefficient: &'a [f64],
    pub temperature: &'a [f64],
    pub l_ref: &'a [f64],
}

/// `(parameter, last_evaluated, converged)`, see `find_parameter`.
type FindParameterResult<'py> = (Bound<'py, PyArray1<f64>>, Bound<'py, PyArray1<f64>>, bool);

/// Port of `FindParamModel._delta` for one span:
/// (L(p) - L_ref) / L_ref - (epsilon_mecha(p) + epsilon_therm_0).
#[inline]
fn delta(inputs: &FindParamInputs, i: usize, cable: &Cable, p: f64) -> f64 {
    let (x_m, x_n, length) =
        catenary_values(inputs.span_length[i], inputs.elevation_difference[i], p);
    let t_mean = catenary_t_mean(
        p,
        inputs.load_coefficient[i],
        cable.linear_weight,
        x_m,
        x_n,
        length,
    );
    let eps_mecha = t_mean / (cable.young_modulus * cable.section);
    let eps_therm = inputs.temperature[i] * cable.dilatation_coefficient;
    let l_ref = inputs.l_ref[i];
    (length - l_ref) / l_ref - (eps_mecha + eps_therm)
}

/// Port of `FindParamSolverForLoop.find_parameter` (Newton with a forward-difference
/// derivative of step `param_step`). See the Python wrapper `find_parameter` for the
/// semantics of the stop test and of the returned values.
pub(crate) fn find_parameter_core(
    initial_parameter: &[f64],
    inputs: &FindParamInputs,
    cable: &Cable,
    settings: &FindParamSettings,
) -> (Vec<f64>, Vec<f64>, bool) {
    let n = initial_parameter.len();
    let mut parameter = initial_parameter.to_vec();
    let mut last_evaluated = parameter.clone();
    let threshold = settings.stop_condition * n as f64;
    for _ in 0..settings.max_iter {
        let mut sum_sq = 0.0;
        for i in 0..n {
            let mem = parameter[i];
            let d = delta(inputs, i, cable, mem);
            let shifted = mem + settings.param_step;
            let d_prime = (delta(inputs, i, cable, shifted) - d) / settings.param_step;
            parameter[i] = mem - d / d_prime;
            last_evaluated[i] = shifted;
            if i + 1 < n {
                let diff = mem - parameter[i];
                sum_sq += diff * diff;
            }
        }
        if sum_sq.sqrt() < threshold {
            return (parameter, last_evaluated, true);
        }
    }
    (parameter, last_evaluated, false)
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
    let p0 = initial_parameter.as_array().to_vec();
    let (a, b, k, t, l) = (
        span_length.as_array().to_vec(),
        elevation_difference.as_array().to_vec(),
        load_coefficient.as_array().to_vec(),
        temperature.as_array().to_vec(),
        l_ref.as_array().to_vec(),
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
    let inputs = FindParamInputs {
        span_length: &a,
        elevation_difference: &b,
        load_coefficient: &k,
        temperature: &t,
        l_ref: &l,
    };
    let cable = Cable {
        linear_weight,
        young_modulus,
        section,
        dilatation_coefficient,
    };
    let settings = FindParamSettings {
        param_step,
        stop_condition,
        max_iter,
    };
    let (parameter, last_evaluated, converged) =
        find_parameter_core(&p0, &inputs, &cable, &settings);
    Ok((
        parameter.into_pyarray(py),
        last_evaluated.into_pyarray(py),
        converged,
    ))
}
