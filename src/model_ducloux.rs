// Derived from mechaphlowers `core/models/balance/models/model_ducloux.py`
// (approx_parameter, BalanceModel, Nodes, LoadModel).
// Copyright (c) 2026, RTE (http://www.rte-france.com)
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use numpy::ndarray::Zip;
use numpy::{IntoPyArray, PyArray1, PyReadonlyArray1};
use pyo3::exceptions::{PyKeyError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::analytical_cubic::max_real_root;
use crate::balance_solver::{EvalError, SolveError, SolverModel, SolverParams, solve};
use crate::find_parameter_solver::{
    Cable, FindParamInputs, FindParamSettings, find_parameter_core,
};
use crate::span::catenary_values;
use crate::utils_model_ducloux::{NodeType, Projection, compute_dx_dy_dz, force_cable, np_sign};

/// Cable constants used by `approx_parameter` (attributes of `BalanceModel` / `LoadModel`).
#[derive(Clone, Copy)]
pub(crate) struct ApproxCable {
    pub section: f64,
    pub linear_weight: f64,
    pub young_modulus: f64,
    pub dilatation_coefficient: f64,
}

/// `approx_parameter` for one span (see the Python wrapper below).
#[inline]
pub(crate) fn approx_parameter_one(
    a: f64,
    b: f64,
    l0: f64,
    k: f64,
    temperature: f64,
    cable: &ApproxCable,
) -> f64 {
    // numpy: (a**2 + b**2) ** 0.5 and a**4 use the square / sqrt / pow fast paths
    let circle_chord = (a * a + b * b).sqrt();
    let factor = cable.linear_weight * k / cable.young_modulus / cable.section;
    let p3 = factor * l0;
    let p2 = l0 - circle_chord + cable.dilatation_coefficient * temperature * l0;
    let p1 = 0.0 * l0;
    let p0 = -a.powi(4) / 24.0 / circle_chord;
    max_real_root(p3, p2, p1, p0)
}

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
    let cable = ApproxCable {
        section: cable_section,
        linear_weight,
        young_modulus,
        dilatation_coefficient,
    };
    let roots = Zip::from(&a)
        .and(&b)
        .and(&l0)
        .and(&k)
        .and(&t)
        .map_collect(|&a, &b, &l0, &k, &t| approx_parameter_one(a, b, l0, k, t, &cable));
    Ok(roots.into_pyarray(py))
}

/// Port of `LoadModel` (state `[x_i0, z_i0, x_i1, z_i1, ...]`, one load per loaded span).
/// After `evaluate`, the outputs of the last evaluation are kept, as in the Python span
/// models `span_model_left` / `span_model_right`.
pub(crate) struct LoadModel {
    // per load
    pub l_ref: Vec<f64>,
    pub a_prime: Vec<f64>,
    pub b_prime: Vec<f64>,
    pub k_load: Vec<f64>,
    pub temperature: Vec<f64>,
    pub load_weight: Vec<f64>,
    pub load_position: Vec<f64>,
    // constants
    pub approx_cable: ApproxCable,
    pub cable: Cable,
    pub find_param: FindParamSettings,
    pub bundle_number: f64,
    // outputs of the last evaluation
    pub parameter_left: Vec<f64>,
    pub th_left: Vec<f64>,
    pub x_m_left: Vec<f64>,
    pub x_n_right: Vec<f64>,
}

impl SolverModel for LoadModel {
    fn jacobian_half_bandwidth(&self) -> Option<usize> {
        Some(1)
    }

    /// `LoadModel.update()` + `objective_function()`.
    fn evaluate(&mut self, state: &[f64]) -> Result<Vec<f64>, EvalError> {
        let m = self.l_ref.len();
        // left semi-spans then right semi-spans, as the combined 2N span model
        let mut a_both = vec![0.0; 2 * m];
        let mut b_both = vec![0.0; 2 * m];
        let mut l_ref_both = vec![0.0; 2 * m];
        let mut k_both = vec![0.0; 2 * m];
        let mut t_both = vec![0.0; 2 * m];
        for i in 0..m {
            let (x_i, z_i) = (state[2 * i], state[2 * i + 1]);
            let l_ref_left = self.load_position[i] * self.l_ref[i];
            a_both[i] = x_i;
            a_both[m + i] = self.a_prime[i] - x_i;
            b_both[i] = z_i;
            b_both[m + i] = self.b_prime[i] - z_i;
            l_ref_both[i] = l_ref_left;
            l_ref_both[m + i] = self.l_ref[i] - l_ref_left;
            k_both[i] = self.k_load[i];
            k_both[m + i] = self.k_load[i];
            t_both[i] = self.temperature[i];
            t_both[m + i] = self.temperature[i];
        }
        let p0: Vec<f64> = (0..2 * m)
            .map(|i| {
                approx_parameter_one(
                    a_both[i],
                    b_both[i],
                    l_ref_both[i],
                    k_both[i],
                    t_both[i],
                    &self.approx_cable,
                )
            })
            .collect();
        let inputs = FindParamInputs {
            span_length: &a_both,
            elevation_difference: &b_both,
            load_coefficient: &k_both,
            temperature: &t_both,
            l_ref: &l_ref_both,
        };
        let (parameter, _, converged) =
            find_parameter_core(&p0, &inputs, &self.cable, &self.find_param);
        if !converged {
            return Err(EvalError::FindParameter);
        }

        let lw = self.cable.linear_weight;
        let mut objective = vec![0.0; 2 * m];
        self.parameter_left.resize(m, 0.0);
        self.th_left.resize(m, 0.0);
        self.x_m_left.resize(m, 0.0);
        self.x_n_right.resize(m, 0.0);
        for i in 0..m {
            let (p_left, p_right) = (parameter[i], parameter[m + i]);
            let k = self.k_load[i];
            let (x_m_left, x_n_left, _) = catenary_values(a_both[i], b_both[i], p_left);
            let (x_m_right, x_n_right, _) = catenary_values(a_both[m + i], b_both[m + i], p_right);
            let th_left = p_left * k * lw;
            let th_right = p_right * k * lw;
            let tv_d_loc = -(th_left * (x_n_left / p_left).sinh());
            let tv_g_loc = th_right * (x_m_right / p_right).sinh();
            objective[2 * i] = th_left - th_right;
            objective[2 * i + 1] =
                tv_d_loc + tv_g_loc - k * self.load_weight[i] / self.bundle_number;
            self.parameter_left[i] = p_left;
            self.th_left[i] = th_left;
            self.x_m_left[i] = x_m_left;
            self.x_n_right[i] = x_n_right;
        }
        Ok(objective)
    }
}

/// Port of `BalanceModel` + `Nodes` for `BalanceSolver` (state
/// `[dz_0, dy_0, dx_1, dy_1, ..., dz_n, dy_n]`). Arrays are named after the mechaphlowers
/// attributes they come from; "per span" arrays have n - 1 values.
pub(crate) struct BalanceModel {
    pub adjustment: bool,
    // nodes (one value per support)
    pub node_type: Vec<NodeType>,
    pub insulator_length: Vec<f64>,
    pub z_arm: Vec<f64>,
    pub crossarm_length: Vec<f64>,
    pub line_angle: Vec<f64>,
    pub signed_insulator_weight: Vec<f64>,
    pub signed_counterweight: Vec<f64>,
    pub bundle_number: f64,
    /// `Nodes.dxdydz` when `solve` starts
    pub initial_dxdydz: [Vec<f64>; 3],
    // span model and deformation model (one value per support, NaN-padded)
    pub span_parameter: Vec<f64>,
    pub span_load_coefficient: Vec<f64>,
    pub deformation_temperature: Vec<f64>,
    // per span
    pub span_length: Vec<f64>,
    pub beta: Vec<f64>,
    pub k_load: Vec<f64>,
    pub l_ref: Vec<f64>,
    pub current_temperature: Vec<f64>,
    pub has_load_on_span: Vec<bool>,
    pub load_weight: Vec<f64>,
    pub load_position: Vec<f64>,
    // constants
    pub approx_cable: ApproxCable,
    pub cable: Cable,
    pub find_param: FindParamSettings,
    pub load_find_param: FindParamSettings,
    pub load_solver: SolverParams,
}

impl BalanceModel {
    /// `LoadModel` solve of `BalanceModel.update_tensions` (change state with loads).
    /// Returns, per loaded span: parameter, Th, x_m (left semi-span), x_n (right semi-span).
    #[allow(clippy::type_complexity)]
    fn solve_loads(
        &self,
        loaded: &[usize],
        x_i: &[f64],
        z_i: &[f64],
        a_prime: &[f64],
        b_prime: &[f64],
    ) -> Result<(Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>), EvalError> {
        let pick = |v: &[f64]| loaded.iter().map(|&j| v[j]).collect::<Vec<f64>>();
        let mut load_model = LoadModel {
            l_ref: pick(&self.l_ref),
            a_prime: pick(a_prime),
            b_prime: pick(b_prime),
            k_load: pick(&self.k_load),
            temperature: pick(&self.current_temperature),
            load_weight: pick(&self.load_weight),
            load_position: pick(&self.load_position),
            approx_cable: self.approx_cable,
            cable: self.cable,
            find_param: self.load_find_param,
            bundle_number: self.bundle_number,
            parameter_left: vec![],
            th_left: vec![],
            x_m_left: vec![],
            x_n_right: vec![],
        };
        let mut state = Vec::with_capacity(2 * loaded.len());
        for &j in loaded {
            state.push(x_i[j]);
            state.push(z_i[j]);
        }
        solve(&mut load_model, &state, &self.load_solver).map_err(|_| EvalError::LoadSolver)?;
        Ok((
            load_model.parameter_left,
            load_model.th_left,
            load_model.x_m_left,
            load_model.x_n_right,
        ))
    }
}

impl SolverModel for BalanceModel {
    fn jacobian_half_bandwidth(&self) -> Option<usize> {
        Some(3)
    }

    /// `BalanceModel.state_vector = state; update(); objective_function()`.
    fn evaluate(&mut self, state: &[f64]) -> Result<Vec<f64>, EvalError> {
        let n = self.node_type.len();
        // Nodes.state_vector setter
        let mut dx = vec![0.0; n];
        let mut dz = vec![0.0; n];
        let dy: Vec<f64> = (0..n).map(|i| state[2 * i + 1]).collect();
        for i in 0..n {
            match self.node_type[i] {
                NodeType::Suspension => dx[i] = state[2 * i],
                NodeType::AnchorFirst | NodeType::AnchorLast => dz[i] = state[2 * i],
            }
        }
        compute_dx_dy_dz(
            &self.node_type,
            &self.insulator_length,
            &mut dx,
            &dy,
            &mut dz,
        )?;
        self.evaluate_nodes(&dx, &dy, &dz)
    }

    /// The first `update()` of `BalanceSolver.solve` uses `Nodes.dxdydz` as it is (after
    /// `solve_adjustment` resets it to 0, dz of suspension chains is 0, not -L).
    fn evaluate_initial(&mut self, _state: &[f64]) -> Result<Vec<f64>, EvalError> {
        let [dx, dy, dz] = self.initial_dxdydz.clone();
        self.evaluate_nodes(&dx, &dy, &dz)
    }
}

impl BalanceModel {
    /// `update()` + `objective_function()` for given chain displacements.
    fn evaluate_nodes(
        &mut self,
        dx: &[f64],
        dy: &[f64],
        dz: &[f64],
    ) -> Result<Vec<f64>, EvalError> {
        let n = self.node_type.len();
        let lw = self.cable.linear_weight;

        // update_span (compute_inter, a, b, a_prime, b_prime) and the angles of
        // update_projections (alpha, proj_angle)
        let proj = |i: usize| {
            let g = self.line_angle[i] / 2.0;
            let arm = self.crossarm_length[i] + dy[i];
            let proj_g = [
                -arm * g.sin() + dx[i] * g.cos(),
                arm * g.cos() + dx[i] * g.sin(),
            ];
            let proj_d = [
                arm * g.sin() + dx[i] * g.cos(),
                arm * g.cos() - dx[i] * g.sin(),
            ];
            (proj_g, proj_d)
        };
        let m = n - 1;
        let mut a_prime = vec![0.0; m];
        let mut b_prime = vec![0.0; m];
        let mut alpha = vec![0.0; m];
        let mut proj_angle = vec![0.0; m];
        for j in 0..m {
            let (_, proj_d_left) = proj(j);
            let (proj_g_right, _) = proj(j + 1);
            let inter1 = self.span_length[j] + (proj_g_right[0] - proj_d_left[0]);
            let inter2 = proj_g_right[1] - proj_d_left[1];
            proj_angle[j] = inter2.atan2(inter1);
            let a = (inter1 * inter1 + inter2 * inter2).sqrt();
            let b = (self.z_arm[j + 1] + dz[j + 1]) - (self.z_arm[j] + dz[j]);
            let sin_beta = self.beta[j].sin();
            let chord = (a * a + b * b * (sin_beta * sin_beta)).sqrt();
            a_prime[j] = chord;
            b_prime[j] = b * self.beta[j].cos();
            alpha[j] = -(a / chord).acos() * np_sign(self.beta[j]) * np_sign(b);
        }

        // update_tensions
        let mut parameter = vec![0.0; m];
        let mut th = vec![0.0; m];
        let mut x_m = vec![0.0; m];
        let mut x_n = vec![0.0; m];
        if self.adjustment {
            for j in 0..m {
                let p = self.span_parameter[j];
                let (xm, xn, _) = catenary_values(a_prime[j], b_prime[j], p);
                parameter[j] = p;
                th[j] = p * self.span_load_coefficient[j] * lw;
                x_m[j] = xm;
                x_n[j] = xn;
            }
        } else {
            // compute_Th_and_extremum: approx_parameter, then find_parameter on the
            // support-sized (NaN-padded) span model
            let mut p0: Vec<f64> = (0..m)
                .map(|j| {
                    approx_parameter_one(
                        a_prime[j],
                        b_prime[j],
                        self.l_ref[j],
                        self.k_load[j],
                        self.current_temperature[j],
                        &self.approx_cable,
                    )
                })
                .collect();
            p0.push(f64::NAN);
            let pad = |v: &[f64]| {
                let mut out = v.to_vec();
                out.push(f64::NAN);
                out
            };
            let (a_pad, b_pad, l_ref_pad) = (pad(&a_prime), pad(&b_prime), pad(&self.l_ref));
            let inputs = FindParamInputs {
                span_length: &a_pad,
                elevation_difference: &b_pad,
                load_coefficient: &self.span_load_coefficient,
                temperature: &self.deformation_temperature,
                l_ref: &l_ref_pad,
            };
            let (p, _, converged) =
                find_parameter_core(&p0, &inputs, &self.cable, &self.find_param);
            if !converged {
                return Err(EvalError::FindParameter);
            }
            for j in 0..m {
                let (xm, xn, _) = catenary_values(a_prime[j], b_prime[j], p[j]);
                parameter[j] = p[j];
                th[j] = p[j] * self.span_load_coefficient[j] * lw;
                x_m[j] = xm;
                x_n[j] = xn;
            }

            let loaded: Vec<usize> = (0..m).filter(|&j| self.has_load_on_span[j]).collect();
            if !loaded.is_empty() {
                // initial load positions: x_i on the chord, z_i on the catenary
                let z_one_point = |x: f64, p: f64| p * ((x / p).cosh() - 1.0);
                let x_i: Vec<f64> = (0..m).map(|j| self.load_position[j] * a_prime[j]).collect();
                let z_i: Vec<f64> = (0..m)
                    .map(|j| z_one_point(x_i[j] + x_m[j], p[j]) - z_one_point(x_m[j], p[j]))
                    .collect();
                let (p_left, th_left, x_m_left, x_n_right) =
                    self.solve_loads(&loaded, &x_i, &z_i, &a_prime, &b_prime)?;
                for (i, &j) in loaded.iter().enumerate() {
                    parameter[j] = p_left[i];
                    th[j] = th_left[i];
                    x_m[j] = x_m_left[i];
                    x_n[j] = x_n_right[i];
                }
            }
        }
        // Tv from the span model, with the parameter set above
        let mut tv_g = vec![0.0; m];
        let mut tv_d = vec![0.0; m];
        for j in 0..m {
            let p = parameter[j];
            let span_th = p * self.span_load_coefficient[j] * lw;
            tv_g[j] = span_th * (x_m[j] / p).sinh();
            tv_d[j] = -(span_th * (x_n[j] / p).sinh());
        }

        // objective_function: Nodes.compute_moment
        let (fx, fy, fz_cable) = force_cable(&Projection {
            th: &th,
            tv_d: &tv_d,
            tv_g: &tv_g,
            alpha: &alpha,
            beta: &self.beta,
            proj_angle: &proj_angle,
            line_angle: &self.line_angle,
            bundle_number: self.bundle_number,
        });
        let mut objective = vec![0.0; 2 * n];
        for i in 0..n {
            let fz = fz_cable[i] + self.signed_counterweight[i];
            let weight = self.signed_insulator_weight[i];
            let mx_cable = dy[i] * fz - dz[i] * fy[i];
            let my_cable = dz[i] * fx[i] - dx[i] * fz;
            objective[2 * i] = mx_cable + (dy[i] / 2.0) * weight;
            objective[2 * i + 1] = my_cable + -(dx[i] / 2.0) * weight;
        }
        Ok(objective)
    }
}

// ---- Python entry point ----------------------------------------------------------------

fn item<'py>(inputs: &Bound<'py, PyDict>, key: &str) -> PyResult<Bound<'py, PyAny>> {
    inputs
        .get_item(key)?
        .ok_or_else(|| PyKeyError::new_err(key.to_string()))
}

fn f64_array(inputs: &Bound<'_, PyDict>, key: &str) -> PyResult<Vec<f64>> {
    let array: PyReadonlyArray1<f64> = item(inputs, key)?.extract()?;
    Ok(array.as_array().to_vec())
}

fn f64_value(inputs: &Bound<'_, PyDict>, key: &str) -> PyResult<f64> {
    item(inputs, key)?.extract()
}

fn usize_value(inputs: &Bound<'_, PyDict>, key: &str) -> PyResult<usize> {
    item(inputs, key)?.extract()
}

fn solver_params(inputs: &Bound<'_, PyDict>, prefix: &str) -> PyResult<SolverParams> {
    Ok(SolverParams {
        perturb: f64_value(inputs, &format!("{prefix}perturb"))?,
        stop_condition: f64_value(inputs, &format!("{prefix}stop_condition"))?,
        relax_ratio: f64_value(inputs, &format!("{prefix}relax_ratio"))?,
        relax_power: f64_value(inputs, &format!("{prefix}relax_power"))?,
        max_iter: usize_value(inputs, &format!("{prefix}max_iter"))?,
    })
}

fn find_param_settings(inputs: &Bound<'_, PyDict>, prefix: &str) -> PyResult<FindParamSettings> {
    Ok(FindParamSettings {
        param_step: f64_value(inputs, &format!("{prefix}param_step"))?,
        stop_condition: f64_value(inputs, &format!("{prefix}stop_condition"))?,
        max_iter: usize_value(inputs, &format!("{prefix}max_iter"))?,
    })
}

fn balance_model_from_dict(inputs: &Bound<'_, PyDict>) -> PyResult<BalanceModel> {
    let node_type = item(inputs, "node_type")?
        .extract::<Vec<String>>()?
        .iter()
        .map(|t| match t.as_str() {
            "suspension" => Ok(NodeType::Suspension),
            "anchor_first" => Ok(NodeType::AnchorFirst),
            "anchor_last" => Ok(NodeType::AnchorLast),
            other => Err(PyValueError::new_err(format!(
                "unknown node type {other:?}"
            ))),
        })
        .collect::<PyResult<Vec<_>>>()?;
    let n = node_type.len();
    let model = BalanceModel {
        adjustment: item(inputs, "adjustment")?.extract()?,
        node_type,
        insulator_length: f64_array(inputs, "insulator_length")?,
        z_arm: f64_array(inputs, "z_arm")?,
        crossarm_length: f64_array(inputs, "crossarm_length")?,
        line_angle: f64_array(inputs, "line_angle")?,
        signed_insulator_weight: f64_array(inputs, "signed_insulator_weight")?,
        signed_counterweight: f64_array(inputs, "signed_counterweight")?,
        bundle_number: f64_value(inputs, "bundle_number")?,
        initial_dxdydz: [
            f64_array(inputs, "dx")?,
            f64_array(inputs, "dy")?,
            f64_array(inputs, "dz")?,
        ],
        span_parameter: f64_array(inputs, "span_parameter")?,
        span_load_coefficient: f64_array(inputs, "span_load_coefficient")?,
        deformation_temperature: f64_array(inputs, "deformation_temperature")?,
        span_length: f64_array(inputs, "span_length")?,
        beta: f64_array(inputs, "beta")?,
        k_load: f64_array(inputs, "k_load")?,
        l_ref: f64_array(inputs, "l_ref")?,
        current_temperature: f64_array(inputs, "current_temperature")?,
        has_load_on_span: item(inputs, "has_load_on_span")?.extract()?,
        load_weight: f64_array(inputs, "load_weight")?,
        load_position: f64_array(inputs, "load_position")?,
        approx_cable: ApproxCable {
            section: f64_value(inputs, "approx_cable_section")?,
            linear_weight: f64_value(inputs, "approx_linear_weight")?,
            young_modulus: f64_value(inputs, "approx_young_modulus")?,
            dilatation_coefficient: f64_value(inputs, "approx_dilatation_coefficient")?,
        },
        cable: Cable {
            linear_weight: f64_value(inputs, "linear_weight")?,
            young_modulus: f64_value(inputs, "young_modulus")?,
            section: f64_value(inputs, "cable_section_area")?,
            dilatation_coefficient: f64_value(inputs, "dilatation_coefficient")?,
        },
        find_param: find_param_settings(inputs, "find_param_")?,
        load_find_param: find_param_settings(inputs, "load_find_param_")?,
        load_solver: solver_params(inputs, "load_solver_")?,
    };
    let per_node = [
        model.insulator_length.len(),
        model.z_arm.len(),
        model.crossarm_length.len(),
        model.line_angle.len(),
        model.signed_insulator_weight.len(),
        model.signed_counterweight.len(),
        model.span_parameter.len(),
        model.span_load_coefficient.len(),
        model.deformation_temperature.len(),
        model.initial_dxdydz[0].len(),
        model.initial_dxdydz[1].len(),
        model.initial_dxdydz[2].len(),
    ];
    let per_span = [
        model.span_length.len(),
        model.beta.len(),
        model.k_load.len(),
        model.l_ref.len(),
        model.current_temperature.len(),
        model.has_load_on_span.len(),
        model.load_weight.len(),
        model.load_position.len(),
    ];
    if n < 2 || per_node.iter().any(|&l| l != n) || per_span.iter().any(|&l| l != n - 1) {
        return Err(PyValueError::new_err(
            "inputs must have one value per support (nodes, span model) or per span",
        ));
    }
    Ok(model)
}

/// Result of `solve_balance_model`: (state, objective, error, initial, norm_d_param).
type BalanceSolveResult<'py> = (
    Bound<'py, PyArray1<f64>>,
    Bound<'py, PyArray1<f64>>,
    Option<&'static str>,
    bool,
    f64,
);

fn eval_error_name(e: EvalError) -> &'static str {
    match e {
        EvalError::ChainReversal => "chain_reversal",
        EvalError::FindParameter => "find_parameter",
        EvalError::LoadSolver => "load_solver",
    }
}

/// Port of `BalanceSolver.solve(balance_model)`, including the inner `find_parameter` and
/// `LoadModel` solves of each evaluation. `inputs` holds the `BalanceModel` data and the
/// solver settings (built by `mphlowrs.mechaphlowers_patch`).
///
/// Returns `(state, objective, error, initial, norm_d_param)`. `error` is None on success,
/// else one of:
/// - "chain_reversal", "find_parameter", "load_solver": the evaluation at `state` fails
///   (`initial` is True when it is the first `update()`, on `dx`/`dy`/`dz` as given);
/// - "singular": `np.linalg.solve` fails at iterate `state`;
/// - "max_iter": `state` is the last iterate and `norm_d_param` its stop value.
#[pyfunction]
pub fn solve_balance_model<'py>(
    py: Python<'py>,
    inputs: &Bound<'py, PyDict>,
    initial_state: PyReadonlyArray1<'py, f64>,
) -> PyResult<BalanceSolveResult<'py>> {
    let mut model = balance_model_from_dict(inputs)?;
    let params = solver_params(inputs, "solver_")?;
    let initial_state = initial_state.as_array().to_vec();
    if initial_state.len() != 2 * model.node_type.len() {
        return Err(PyValueError::new_err(
            "state must have 2 values per support",
        ));
    }
    let result = py.detach(|| solve(&mut model, &initial_state, &params));
    let (state, objective, error, initial, norm_d_param) = match result {
        Ok((state, objective)) => (state, objective, None, false, f64::NAN),
        Err(SolveError::InitialEval(e)) => (
            initial_state,
            vec![],
            Some(eval_error_name(e)),
            true,
            f64::NAN,
        ),
        Err(SolveError::Eval(e, state)) => {
            (state, vec![], Some(eval_error_name(e)), false, f64::NAN)
        }
        Err(SolveError::Singular(state)) => (state, vec![], Some("singular"), false, f64::NAN),
        Err(SolveError::MaxIter(state, norm)) => (state, vec![], Some("max_iter"), false, norm),
    };
    Ok((
        state.into_pyarray(py),
        objective.into_pyarray(py),
        error,
        initial,
        norm_d_param,
    ))
}
