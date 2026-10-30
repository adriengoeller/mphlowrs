// Derived from mechaphlowers `numeric/analytical_cubic.py` (AnalyticalRealSolver).
// Copyright (c) 2026, RTE (http://www.rte-france.com)
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

/// `np.sign`: 0 for 0, NaN for NaN (`f64::signum` returns ±1 for ±0).
#[inline]
fn np_sign(x: f64) -> f64 {
    if x == 0.0 { 0.0 } else { x.signum() }
}

/// `np.maximum(x, y)`: NaN propagates (`f64::max` would drop it).
#[inline]
fn np_maximum(x: f64, y: f64) -> f64 {
    if x.is_nan() || y.is_nan() {
        f64::NAN
    } else {
        x.max(y)
    }
}

/// Port of `AnalyticalRealSolver.solve(p, only_max_real=True)` for one polynomial
/// `c3·x³ + c2·x² + c1·x + c0 = 0`: largest real root.
pub(crate) fn max_real_root(c3: f64, c2: f64, c1: f64, c0: f64) -> f64 {
    // normalise to monic: x³ + a·x² + b·x + c = 0
    let inv_a0 = 1.0 / c3;
    let a = c2 * inv_a0;
    let b = c1 * inv_a0;
    let c = c0 * inv_a0;

    // depress: t³ + p_dep·t + q_dep = 0 with x = t − a/3
    let a3 = a / 3.0;
    let a3_sq = a3 * a3;
    let p_dep = b - 3.0 * a3_sq;
    let q_dep = a3 * (2.0 * a3_sq - b) + c;
    let d = 0.25 * q_dep * q_dep + p_dep * p_dep * p_dep / 27.0;

    // D ≥ 0: signed real cube roots (Cardano)
    let half_q = 0.5 * q_dep;
    let sqrt_d = np_maximum(d, 0.0).sqrt();
    let big_a = -half_q + sqrt_d;
    let big_b = -half_q - sqrt_d;
    let cardano_root = np_sign(big_a) * big_a.abs().cbrt() + np_sign(big_b) * big_b.abs().cbrt();

    // D < 0: trigonometric method
    let neg_p3 = np_maximum(-p_dep / 3.0, 0.0);
    let sqrt_neg_p3 = neg_p3.sqrt();
    let m = 2.0 * sqrt_neg_p3;
    let denom = 2.0 * neg_p3 * sqrt_neg_p3 + 1e-300;
    let cos_arg = (-q_dep / denom).clamp(-1.0, 1.0);
    let trig_root = m * (cos_arg.acos() / 3.0).cos();

    // np.where(D >= 0, ...): NaN D takes the trigonometric branch
    (if d >= 0.0 { cardano_root } else { trig_root }) - a3
}
