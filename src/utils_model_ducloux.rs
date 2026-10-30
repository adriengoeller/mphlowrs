// Derived from mechaphlowers `core/models/balance/models/utils_model_ducloux.py`
// (Masks, VectorProjection).
// Copyright (c) 2026, RTE (http://www.rte-france.com)
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use crate::balance_solver::EvalError;

/// Node types of `Masks` (`nodes_type` in mechaphlowers).
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum NodeType {
    Suspension,
    AnchorFirst,
    AnchorLast,
}

/// Port of `Masks.compute_dx_dy_dz`, in place: dz of suspension chains and dx of anchor
/// chains from the chain length (Pythagoras). Raises `SuspectedChainReversal` when a
/// square root argument is negative (the check runs on all nodes before any update).
pub(crate) fn compute_dx_dy_dz(
    node_type: &[NodeType],
    l_chain: &[f64],
    dx: &mut [f64],
    dy: &[f64],
    dz: &mut [f64],
) -> Result<(), EvalError> {
    let remainder = |i: usize| {
        let other = match node_type[i] {
            NodeType::Suspension => dx[i],
            NodeType::AnchorFirst | NodeType::AnchorLast => dz[i],
        };
        l_chain[i] * l_chain[i] - other * other - dy[i] * dy[i]
    };
    let remainders: Vec<f64> = (0..node_type.len()).map(remainder).collect();
    if remainders.iter().any(|&r| r < 0.0) {
        return Err(EvalError::ChainReversal);
    }
    for (i, r) in remainders.into_iter().enumerate() {
        match node_type[i] {
            NodeType::Suspension => dz[i] = -r.sqrt(),
            NodeType::AnchorFirst => dx[i] = r.sqrt(),
            NodeType::AnchorLast => dx[i] = -r.sqrt(),
        }
    }
    Ok(())
}

/// `np.sign`: 0 for 0, NaN for NaN.
#[inline]
pub(crate) fn np_sign(x: f64) -> f64 {
    if x == 0.0 { 0.0 } else { x.signum() }
}

/// Inputs of `VectorProjection` (one value per span, except `line_angle`: one per node).
pub(crate) struct Projection<'a> {
    pub th: &'a [f64],
    pub tv_d: &'a [f64],
    pub tv_g: &'a [f64],
    pub alpha: &'a [f64],
    pub beta: &'a [f64],
    pub proj_angle: &'a [f64],
    pub line_angle: &'a [f64],
    pub bundle_number: f64,
}

/// Port of `VectorProjection.force_cable`: (Fx, Fy, Fz), one value per node.
pub(crate) fn force_cable(p: &Projection) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let m = p.th.len();
    let n = m + 1;
    // T_line_plane_left / T_line_plane_right, per span
    let mut left = vec![[0.0; 3]; m];
    let mut right = vec![[0.0; 3]; m];
    for j in 0..m {
        let (sin_b, cos_b) = (p.beta[j].sin(), p.beta[j].cos());
        let (sin_a, cos_a) = (p.alpha[j].sin(), p.alpha[j].cos());
        let (sin_t, cos_t) = (p.proj_angle[j].sin(), p.proj_angle[j].cos());
        let th = p.th[j];

        let vg = p.tv_g[j] * cos_b - th * sin_b * sin_a;
        let hg = p.tv_g[j] * sin_b + th * cos_b * sin_a;
        let lg = th * cos_a;
        left[j] = [
            (lg * cos_t - hg * sin_t) * p.bundle_number,
            (lg * sin_t + hg * cos_t) * p.bundle_number,
            vg * p.bundle_number,
        ];

        let vd = p.tv_d[j] * cos_b + th * sin_b * sin_a;
        let hd = p.tv_d[j] * sin_b - th * cos_b * sin_a;
        let ld = -th * cos_a;
        right[j] = [
            (ld * cos_t - hd * sin_t) * p.bundle_number,
            (ld * sin_t + hd * cos_t) * p.bundle_number,
            vd * p.bundle_number,
        ];
    }

    let mut fx = vec![0.0; n];
    let mut fy = vec![0.0; n];
    let mut fz = vec![0.0; n];
    let g0 = p.line_angle[0] / 2.0;
    let [s_left, t_left, z_left] = left[0];
    fx[0] = s_left * g0.cos() - t_left * g0.sin();
    fy[0] = t_left * g0.cos() + s_left * g0.sin();
    fz[0] = z_left;
    // suspension nodes: right side of span j + left side of span j + 1 (np.roll(..., -1))
    for j in 0..m.saturating_sub(1) {
        let gamma = p.line_angle[j + 1] / 2.0;
        let [s_right, t_right, z_right] = right[j];
        let [s_rolled, t_rolled, z_rolled] = left[j + 1];
        fx[j + 1] = (s_right + s_rolled) * gamma.cos() - (-t_right + t_rolled) * gamma.sin();
        fy[j + 1] = (t_right + t_rolled) * gamma.cos() - (s_right - s_rolled) * gamma.sin();
        fz[j + 1] = z_right + z_rolled;
    }
    let gamma_last = p.line_angle[n - 1] / 2.0;
    let [s_right, t_right, z_right] = right[m - 1];
    fx[n - 1] = s_right * gamma_last.cos() - (-t_right) * gamma_last.sin();
    fy[n - 1] = t_right * gamma_last.cos() - s_right * gamma_last.sin();
    fz[n - 1] = z_right;
    (fx, fy, fz)
}
