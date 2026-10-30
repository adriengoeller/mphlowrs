// Derived from mechaphlowers `core/models/balance/solvers/balance_solver.py` (BalanceSolver).
// Copyright (c) 2026, RTE (http://www.rte-france.com)
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

/// Why a model evaluation (`update()` + `objective_function()`) failed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum EvalError {
    /// `Masks.compute_dx_dy_dz` raises `SuspectedChainReversal`.
    ChainReversal,
    /// `FindParamSolverForLoop.find_parameter` raises `ConvergenceError`.
    FindParameter,
    /// The inner `LoadModel` solve failed (any reason).
    LoadSolver,
}

/// Why `solve` failed. Each variant carries the state of the failing step.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SolveError {
    /// The first `update()` (on the model as it is) failed.
    InitialEval(EvalError),
    /// A model evaluation failed at this state.
    Eval(EvalError, Vec<f64>),
    /// `np.linalg.solve` raises `LinAlgError` (exactly zero pivot); state of the iteration.
    Singular(Vec<f64>),
    /// `max_iter` reached (`ConvergenceError`); final state and last `norm_d_param`.
    MaxIter(Vec<f64>, f64),
}

/// A model solved by `BalanceSolver`: mirrors `IModelForSolver`. `evaluate` is
/// `state_vector = state; update(); return objective_function()`.
pub(crate) trait SolverModel {
    fn evaluate(&mut self, state: &[f64]) -> Result<Vec<f64>, EvalError>;
    fn jacobian_half_bandwidth(&self) -> Option<usize>;

    /// First evaluation of `solve`: `update(); objective_function()` on the model as it is,
    /// without going through the `state_vector` setter.
    fn evaluate_initial(&mut self, state: &[f64]) -> Result<Vec<f64>, EvalError> {
        self.evaluate(state)
    }
}

/// Constructor arguments of `BalanceSolver`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SolverParams {
    pub perturb: f64,
    pub stop_condition: f64,
    pub relax_ratio: f64,
    pub relax_power: f64,
    pub max_iter: usize,
}

fn norm(x: &[f64]) -> f64 {
    x.iter().map(|v| v * v).sum::<f64>().sqrt()
}

/// `relax_ratio ** (counter ** relax_power)`: the exponent is an exact integer in Python
/// when `relax_power` is an int (the default).
fn relaxation(relax_ratio: f64, counter: usize, relax_power: f64) -> f64 {
    let exponent = if relax_power.fract() == 0.0 && relax_power >= 0.0 {
        (counter as f64).powi(relax_power as i32)
    } else {
        (counter as f64).powf(relax_power)
    };
    relax_ratio.powf(exponent)
}

/// Port of `BalanceSolver.solve`. Returns the final state and objective.
pub(crate) fn solve<M: SolverModel>(
    model: &mut M,
    initial_state: &[f64],
    params: &SolverParams,
) -> Result<(Vec<f64>, Vec<f64>), SolveError> {
    let mut state = initial_state.to_vec();
    let mut objective = model
        .evaluate_initial(&state)
        .map_err(SolveError::InitialEval)?;

    for counter in 1..params.max_iter {
        let (jac, half_bandwidth) = jacobian(model, &mut state, &objective, params.perturb)?;
        let mem = norm(&objective);
        let correction = solve_banded(jac, &objective, half_bandwidth)
            .ok_or_else(|| SolveError::Singular(state.clone()))?;

        let factor = 1.0 - relaxation(params.relax_ratio, counter, params.relax_power);
        for (s, c) in state.iter_mut().zip(&correction) {
            *s -= c * factor;
        }
        objective = model
            .evaluate(&state)
            .map_err(|e| SolveError::Eval(e, state.clone()))?;

        let objective_norm = norm(&objective);
        let norm_d_param = (objective_norm * objective_norm - mem * mem).abs();
        if norm_d_param < params.stop_condition {
            break;
        }
        if counter == params.max_iter - 1 {
            return Err(SolveError::MaxIter(state, norm_d_param));
        }
    }
    Ok((state, objective))
}

/// Port of `BalanceSolver.jacobian` (dense) and `_banded_jacobian`. Returns the row-major
/// jacobian and the half-bandwidth to use in `solve_banded` (n - 1 when dense).
///
/// The state is perturbed in place and restored with `-= perturb`, as `_delta_d` does: the
/// restored value is `(s + perturb) - perturb`, which can differ from `s` in the last bit.
fn jacobian<M: SolverModel>(
    model: &mut M,
    state: &mut [f64],
    objective: &[f64],
    perturb: f64,
) -> Result<(Vec<f64>, usize), SolveError> {
    let n = objective.len();
    let mut jac = vec![0.0; n * n];
    let (step, half_bandwidth) = match model.jacobian_half_bandwidth() {
        Some(hb) if 2 * hb + 1 < n => (2 * hb + 1, hb),
        _ => (1, n.saturating_sub(1)),
    };
    let banded = step > 1;
    // banded: columns j ≡ group (mod step) perturbed together; dense: one column at a time
    let groups: Vec<Vec<usize>> = if banded {
        (0..step).map(|g| (g..n).step_by(step).collect()).collect()
    } else {
        (0..n).map(|j| vec![j]).collect()
    };
    for columns in groups {
        for &j in &columns {
            state[j] += perturb;
        }
        let f_perturb = model
            .evaluate(state)
            .map_err(|e| SolveError::Eval(e, state.to_vec()))?;
        for &j in &columns {
            state[j] -= perturb;
        }
        for &j in &columns {
            let (first_row, last_row) = if banded {
                (
                    j.saturating_sub(half_bandwidth),
                    (j + half_bandwidth + 1).min(n),
                )
            } else {
                (0, n)
            };
            for r in first_row..last_row {
                jac[r * n + j] = (f_perturb[r] - objective[r]) / perturb;
            }
        }
    }
    Ok((jac, half_bandwidth))
}

/// Gaussian elimination with partial pivoting (as LAPACK `dgesv`, used by
/// `np.linalg.solve`), restricted to the band: entries more than `half_bandwidth` below the
/// diagonal are zero, and pivoting widens the upper band to `2 * half_bandwidth`.
/// Returns None for an exactly zero pivot (numpy raises `LinAlgError: Singular matrix`).
fn solve_banded(mut a: Vec<f64>, b: &[f64], half_bandwidth: usize) -> Option<Vec<f64>> {
    let n = b.len();
    let mut x = b.to_vec();
    let kl = half_bandwidth;
    let upper = (2 * half_bandwidth).min(n.saturating_sub(1));
    for k in 0..n {
        let last_row = (k + kl).min(n - 1);
        let last_col = (k + upper).min(n - 1);
        // first row with the largest |pivot| (idamax)
        let mut pivot_row = k;
        for r in k + 1..=last_row {
            if a[r * n + k].abs() > a[pivot_row * n + k].abs() {
                pivot_row = r;
            }
        }
        let pivot = a[pivot_row * n + k];
        if pivot == 0.0 {
            return None;
        }
        if pivot_row != k {
            for c in k..=last_col {
                a.swap(k * n + c, pivot_row * n + c);
            }
            x.swap(k, pivot_row);
        }
        for r in k + 1..=last_row {
            let factor = a[r * n + k] / pivot;
            if factor != 0.0 {
                for c in k + 1..=last_col {
                    a[r * n + c] -= factor * a[k * n + c];
                }
                x[r] -= factor * x[k];
            }
        }
    }
    for k in (0..n).rev() {
        let last_col = (k + upper).min(n - 1);
        let mut sum = x[k];
        for c in k + 1..=last_col {
            sum -= a[k * n + c] * x[c];
        }
        x[k] = sum / a[k * n + k];
    }
    Some(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banded_solve_matches_dense_solution() {
        // tridiagonal system with a known solution, needing a row swap
        let n = 6;
        let mut a = vec![0.0; n * n];
        for i in 0..n {
            a[i * n + i] = 0.1 + i as f64;
            if i + 1 < n {
                a[i * n + i + 1] = 2.0;
                a[(i + 1) * n + i] = 5.0;
            }
        }
        let x_true: Vec<f64> = (0..n).map(|i| 1.0 + i as f64).collect();
        let b: Vec<f64> = (0..n)
            .map(|i| (0..n).map(|j| a[i * n + j] * x_true[j]).sum())
            .collect();
        let x = solve_banded(a.clone(), &b, 1).unwrap();
        let x_dense = solve_banded(a, &b, n - 1).unwrap();
        for i in 0..n {
            assert!((x[i] - x_true[i]).abs() < 1e-12);
            assert!((x_dense[i] - x_true[i]).abs() < 1e-12);
        }
    }

    #[test]
    fn singular_matrix_is_detected() {
        assert!(solve_banded(vec![0.0; 4], &[1.0, 1.0], 1).is_none());
    }
}
