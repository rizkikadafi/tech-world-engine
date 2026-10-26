//! Solve linear system A*x = b using LU factorization with partial pivoting.

use crate::error::NumericalError;
use crate::linear::dense::lu::{lu_factor, LuFactorization};
use crate::linear::matrix::Matrix;
use crate::linear::vector::Vector;

/// Solve A*x = b using LU factorization with partial pivoting.
/// Returns x.
pub fn solve(a: &Matrix, b: &Vector) -> Result<Vector, NumericalError> {
    if !a.is_square() {
        return Err(NumericalError::NotSquare {
            rows: a.rows(),
            cols: a.cols(),
        });
    }
    let n = a.rows();
    if b.len() != n {
        return Err(NumericalError::DimensionMismatch);
    }
    let lu_f = lu_factor(a)?;
    solve_with_lu(&lu_f, b)
}

/// Solve using precomputed LU factorization.
pub fn solve_with_lu(lu_f: &LuFactorization, b: &Vector) -> Result<Vector, NumericalError> {
    let n = lu_f.n;
    if b.len() != n {
        return Err(NumericalError::DimensionMismatch);
    }
    // Apply permutation
    let mut x = vec![0.0f64; n];
    for i in 0..n {
        x[i] = b.as_slice()[lu_f.pivot[i]];
    }
    // Forward substitution: L*y = x (L has unit diagonal)
    let mut y = x.clone();
    for i in 0..n {
        let mut s = y[i];
        for j in 0..i {
            s -= lu_f.lu[i * n + j] * y[j];
        }
        y[i] = s;
    }
    // Backward substitution: U*x = y
    let mut sol = y.clone();
    if n > 0 {
        let mut i = (n as i32) - 1;
        while i >= 0 {
            let idx = i as usize;
            let mut s = sol[idx];
            for j in (idx + 1)..n {
                s -= lu_f.lu[idx * n + j] * sol[j];
            }
            let udiag = lu_f.lu[idx * n + idx];
            if udiag.abs() < 1e-18 {
                return Err(NumericalError::Singular { row: idx, col: idx });
            }
            sol[idx] = s / udiag;
            i -= 1;
        }
    }
    Ok(Vector::from_vec(sol))
}
