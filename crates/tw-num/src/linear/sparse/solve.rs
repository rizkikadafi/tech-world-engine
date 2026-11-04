//! Forward and backward substitution for Sparse LU factorization.
//!
//! Modeled after Sparse 1.3 `spsolve.c` in `ngspice-47/src/maths/sparse/`.

use crate::error::NumericalError;
use crate::linear::sparse::lu::{sparse_lu, SparseLu};
use crate::linear::sparse::matrix::SparseMatrix;
use crate::linear::vector::Vector;

/// Solves the linear system `A * x = b` using Sparse LU factorization with Markowitz pivoting.
pub fn solve_sparse(a: &SparseMatrix, b: &Vector) -> Result<Vector, NumericalError> {
    if !a.is_square() {
        return Err(NumericalError::NotSquare {
            rows: a.rows(),
            cols: a.cols(),
        });
    }
    if b.len() != a.rows() {
        return Err(NumericalError::DimensionMismatch);
    }

    let lu = sparse_lu(a, None)?;
    solve_with_sparse_lu(&lu, b)
}

/// Solves `A * x = b` given a precomputed `SparseLu`.
#[allow(clippy::needless_range_loop)]
pub fn solve_with_sparse_lu(lu: &SparseLu, b: &Vector) -> Result<Vector, NumericalError> {
    let n = lu.n;
    if b.len() != n {
        return Err(NumericalError::DimensionMismatch);
    }

    let b_slice = b.as_slice();

    // Step 1: Forward solve L * y = P * b
    let mut y = vec![0.0f64; n];
    for k in 0..n {
        let orig_row = lu.p_rows[k];
        let mut sum = b_slice[orig_row];
        for &(j, l_kj) in &lu.l_rows[k] {
            sum -= l_kj * y[j];
        }
        y[k] = sum;
    }

    // Step 2: Backward solve U * z = y
    let mut z = vec![0.0f64; n];
    for k in (0..n).rev() {
        let mut sum = y[k];
        for &(j, u_kj) in &lu.u_rows[k] {
            sum -= u_kj * z[j];
        }
        let diag = lu.u_diag[k];
        if diag.abs() < 1e-18 {
            return Err(NumericalError::Singular { row: k, col: k });
        }
        z[k] = sum / diag;
    }

    // Step 3: Permute solution vector x = Q * z
    let mut x = vec![0.0f64; n];
    for k in 0..n {
        let orig_col = lu.q_cols[k];
        x[orig_col] = z[k];
    }

    Ok(Vector::from_vec(x))
}
