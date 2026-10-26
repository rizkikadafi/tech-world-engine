//! LU factorization with partial pivoting (Doolittle-style).
//!
//! Computes P * A = L * U where:
//! - L is lower triangular with unit diagonal
//! - U is upper triangular
//! - P is permutation matrix encoded as pivot vector
//!
//! Both L and U are stored in a single matrix `lu` (lower part = L, upper part = U, diagonal of L is implicit 1.0).

use crate::error::NumericalError;
use crate::linear::matrix::Matrix;

/// Result of LU factorization with partial pivoting.
#[derive(Clone, Debug)]
pub struct LuFactorization {
    /// Combined L and U storage (row-major). L has implicit unit diagonal.
    pub lu: Vec<f64>,
    /// Number of rows/cols
    pub n: usize,
    /// Pivot permutation vector
    pub pivot: Vec<usize>,
    /// Number of row swaps performed
    pub swaps: i32,
}

/// Perform LU factorization with partial pivoting.
/// Returns `LuFactorization` on success.
pub fn lu_factor(a: &Matrix) -> Result<LuFactorization, NumericalError> {
    if !a.is_square() {
        return Err(NumericalError::NotSquare {
            rows: a.rows(),
            cols: a.cols(),
        });
    }
    let n = a.rows();
    if n == 0 {
        return Err(NumericalError::InvalidSize);
    }

    let mut lu_mat: Vec<f64> = a.clone_to_vec();
    let mut pivot: Vec<usize> = (0..n).collect();
    let mut swaps: i32 = 0;

    for k in 0..n {
        // Find pivot in column k, rows k..n-1
        let mut max_row = k;
        let mut max_val = lu_mat[k * n + k].abs();
        for i in (k + 1)..n {
            let val = lu_mat[i * n + k].abs();
            if val > max_val {
                max_val = val;
                max_row = i;
                if max_val > 1e-18 {
                    // can continue
                }
            }
        }
        if max_val < 1e-18 {
            return Err(NumericalError::Singular { row: k, col: k });
        }
        if max_row != k {
            // Swap rows max_row and k
            for j in 0..n {
                let idx1 = k * n + j;
                let idx2 = max_row * n + j;
                lu_mat.swap(idx1, idx2);
            }
            pivot.swap(k, max_row);
            swaps += 1;
        }
        // Elimination
        for i in (k + 1)..n {
            let factor = lu_mat[i * n + k] / lu_mat[k * n + k];
            lu_mat[i * n + k] = factor; // store L
            for j in (k + 1)..n {
                lu_mat[i * n + j] -= factor * lu_mat[k * n + j];
            }
        }
    }

    Ok(LuFactorization {
        lu: lu_mat,
        n,
        pivot,
        swaps,
    })
}
