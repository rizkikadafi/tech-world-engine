//! Multidimensional and scalar Newton-Raphson nonlinear solvers.

use crate::error::NumericalError;
use crate::linear::dense::solve;
use crate::linear::{Matrix, Vector};

/// Configuration for the Newton-Raphson solver.
#[derive(Debug, Clone, PartialEq)]
pub struct NewtonConfig {
    /// Maximum number of iterations before failing with `NoConvergence`.
    pub max_iter: usize,
    /// Relative convergence tolerance (SPICE default ~1e-3 to 1e-4).
    pub reltol: f64,
    /// Absolute convergence tolerance (SPICE default ~1e-9 to 1e-12).
    pub abstol: f64,
    /// Optional damping factor in (0.0, 1.0] to assist difficult convergence.
    pub damping: f64,
}

impl Default for NewtonConfig {
    fn default() -> Self {
        Self {
            max_iter: 100,
            reltol: 1e-4,
            abstol: 1e-9,
            damping: 1.0,
        }
    }
}

/// Result of a Newton-Raphson solve.
#[derive(Debug, Clone, PartialEq)]
pub struct NewtonResult {
    /// Converged solution vector.
    pub x: Vector,
    /// Number of iterations taken.
    pub iterations: usize,
    /// Final residual vector norm (L2).
    pub residual_norm: f64,
}

/// Multidimensional Newton-Raphson solver for systems $F(x) = 0$.
#[derive(Debug, Clone, Default)]
pub struct NewtonSolver {
    pub config: NewtonConfig,
}

impl NewtonSolver {
    pub fn new() -> Self {
        Self {
            config: NewtonConfig::default(),
        }
    }

    pub fn with_config(config: NewtonConfig) -> Self {
        Self { config }
    }

    /// Solves $F(x) = 0$ starting from initial guess `x0`.
    ///
    /// - `eval_f`: closure returning the residual vector $F(x)$.
    /// - `eval_j`: closure returning the Jacobian matrix $J(x) = \partial F / \partial x$.
    pub fn solve<F, J>(
        &self,
        x0: &Vector,
        mut eval_f: F,
        mut eval_j: J,
    ) -> Result<NewtonResult, NumericalError>
    where
        F: FnMut(&Vector) -> Result<Vector, NumericalError>,
        J: FnMut(&Vector) -> Result<Matrix, NumericalError>,
    {
        let mut x = x0.clone();
        let dim = x.len();

        for iter in 1..=self.config.max_iter {
            let f_val = eval_f(&x)?;
            if f_val.len() != dim {
                return Err(NumericalError::DimensionMismatch);
            }

            // Check if already converged by residual
            let res_norm = f_val.norm2();
            if res_norm < self.config.abstol {
                return Ok(NewtonResult {
                    x,
                    iterations: iter - 1,
                    residual_norm: res_norm,
                });
            }

            let jac = eval_j(&x)?;
            if jac.rows() != dim || jac.cols() != dim {
                return Err(NumericalError::DimensionMismatch);
            }

            // Newton step: J(x) * dx = -F(x)
            let mut neg_f = f_val.clone();
            for i in 0..dim {
                neg_f[i] = -neg_f[i];
            }

            let dx = solve(&jac, &neg_f)?;

            // Check step convergence using SPICE-style criterion:
            // |dx[i]| <= reltol * max(|x[i]|, |x_new[i]|) + abstol
            let mut step_converged = true;
            for i in 0..dim {
                let x_new_i = x[i] + dx[i];
                let max_mag = x[i].abs().max(x_new_i.abs());
                let tol = self.config.reltol * max_mag + self.config.abstol;
                if dx[i].abs() > tol {
                    step_converged = false;
                    break;
                }
            }

            // Update state with optional damping factor
            for i in 0..dim {
                x[i] += self.config.damping * dx[i];
            }

            if step_converged {
                let final_f = eval_f(&x)?;
                return Ok(NewtonResult {
                    x,
                    iterations: iter,
                    residual_norm: final_f.norm2(),
                });
            }
        }

        Err(NumericalError::NoConvergence {
            iterations: self.config.max_iter,
            reason: format!(
                "exceeded {} iterations without meeting tolerances (reltol={}, abstol={})",
                self.config.max_iter, self.config.reltol, self.config.abstol
            ),
        })
    }
}

/// Solves a scalar 1D equation $f(x) = 0$ using Newton-Raphson.
pub fn newton_scalar<F, DF>(
    x0: f64,
    mut f: F,
    mut df: DF,
    tol: f64,
    max_iter: usize,
) -> Result<f64, NumericalError>
where
    F: FnMut(f64) -> f64,
    DF: FnMut(f64) -> f64,
{
    let mut x = x0;
    for iter in 1..=max_iter {
        let fx = f(x);
        if fx.abs() < tol {
            return Ok(x);
        }
        let dfx = df(x);
        if dfx.abs() < 1e-15 {
            return Err(NumericalError::Singular { row: 0, col: 0 });
        }
        let dx = fx / dfx;
        x -= dx;
        if dx.abs() < tol {
            return Ok(x);
        }
        if !x.is_finite() {
            return Err(NumericalError::NoConvergence {
                iterations: iter,
                reason: "value diverged to non-finite".to_string(),
            });
        }
    }

    Err(NumericalError::NoConvergence {
        iterations: max_iter,
        reason: format!("exceeded {} iterations in scalar solve", max_iter),
    })
}
