//! Sparse LU Factorization using the Markowitz Pivoting strategy.
//!
//! Modeled after Sparse 1.3 (`spfactor.c` in `ngspice-47/src/maths/sparse/`).
//! Computes P * A * Q = L * U where:
//! - P is the row permutation vector
//! - Q is the column permutation vector
//! - L is unit lower triangular
//! - U is upper triangular

use std::collections::{HashMap, HashSet};

use crate::error::NumericalError;
use crate::linear::sparse::matrix::SparseMatrix;

/// Default threshold parameter for Markowitz pivoting (matches ngspice `spTHRESHOLD`).
pub const DEFAULT_MARKOWITZ_THRESHOLD: f64 = 0.001;

/// Minimum absolute pivot value considered non-singular.
pub const PIVOT_EPSILON: f64 = 1e-18;

/// Result of Sparse LU Factorization.
#[derive(Debug, Clone)]
pub struct SparseLu {
    /// Dimension of the square matrix.
    pub n: usize,
    /// Permutation vector for rows: `p_rows[k]` is the original row index chosen at step `k`.
    pub p_rows: Vec<usize>,
    /// Permutation vector for columns: `q_cols[k]` is the original column index chosen at step `k`.
    pub q_cols: Vec<usize>,
    /// L entries: for each permuted row `k`, a list of `(permuted_col j, value)` with `j < k`.
    pub l_rows: Vec<Vec<(usize, f64)>>,
    /// U entries: for each permuted row `k`, a list of `(permuted_col j, value)` with `j > k`.
    pub u_rows: Vec<Vec<(usize, f64)>>,
    /// Diagonal entries of U for each permuted row `k`: `U[k, k]`.
    pub u_diag: Vec<f64>,
    /// Number of fill-ins created during factorization.
    pub fill_ins: usize,
}

/// Performs Sparse LU factorization with Markowitz threshold pivoting on `matrix`.
#[allow(clippy::needless_range_loop)]
pub fn sparse_lu(
    matrix: &SparseMatrix,
    threshold: Option<f64>,
) -> Result<SparseLu, NumericalError> {
    if !matrix.is_square() {
        return Err(NumericalError::NotSquare {
            rows: matrix.rows(),
            cols: matrix.cols(),
        });
    }
    let n = matrix.rows();
    if n == 0 {
        return Err(NumericalError::InvalidSize);
    }

    let u_threshold = threshold.unwrap_or(DEFAULT_MARKOWITZ_THRESHOLD);

    // Dynamic active submatrix representation:
    // row_entries[i] = map of col j -> A[i, j]
    let mut row_entries: Vec<HashMap<usize, f64>> = vec![HashMap::new(); n];
    // col_entries[j] = set of row indices i where A[i, j] != 0
    let mut col_entries: Vec<HashSet<usize>> = vec![HashSet::new(); n];

    for t in matrix.triplets() {
        if t.val.abs() > 1e-30 {
            row_entries[t.row].insert(t.col, t.val);
            col_entries[t.col].insert(t.row);
        }
    }

    let mut remaining_rows: HashSet<usize> = (0..n).collect();
    let mut remaining_cols: HashSet<usize> = (0..n).collect();

    let mut p_rows = Vec::with_capacity(n);
    let mut q_cols = Vec::with_capacity(n);

    // Multipliers L: l_multipliers[orig_row] = map of step_k -> multiplier
    let mut l_multipliers: Vec<HashMap<usize, f64>> = vec![HashMap::new(); n];
    // U entries for original row: u_row_data[orig_row] = map of orig_col -> val
    let mut u_row_data: Vec<HashMap<usize, f64>> = vec![HashMap::new(); n];
    let mut u_diag_data: Vec<f64> = vec![0.0; n];

    let mut fill_ins = 0;

    for k in 0..n {
        // Step 1: Find best pivot (p, q) in remaining_rows x remaining_cols
        // Compute column max values for the threshold test
        let mut col_max: HashMap<usize, f64> = HashMap::with_capacity(remaining_cols.len());
        for &c in &remaining_cols {
            let mut max_val = 0.0f64;
            for &r in &col_entries[c] {
                if remaining_rows.contains(&r) {
                    if let Some(&val) = row_entries[r].get(&c) {
                        let abs_val = val.abs();
                        if abs_val > max_val {
                            max_val = abs_val;
                        }
                    }
                }
            }
            col_max.insert(c, max_val);
        }

        let mut best_pivot: Option<(usize, usize)> = None;
        let mut min_markowitz = usize::MAX;
        let mut best_pivot_val = 0.0f64;

        // Search for pivot with minimum Markowitz product that satisfies the threshold test
        for &r in &remaining_rows {
            let r_len = row_entries[r]
                .iter()
                .filter(|(c, _)| remaining_cols.contains(c))
                .count();

            if r_len == 0 {
                return Err(NumericalError::Singular { row: r, col: k });
            }

            for (&c, &val) in &row_entries[r] {
                if !remaining_cols.contains(&c) {
                    continue;
                }
                let abs_val = val.abs();
                if abs_val < PIVOT_EPSILON {
                    continue;
                }

                let max_in_col = col_max.get(&c).copied().unwrap_or(0.0);
                if max_in_col > 0.0 && abs_val < u_threshold * max_in_col {
                    // Fails Markowitz threshold test
                    continue;
                }

                let c_len = col_entries[c]
                    .iter()
                    .filter(|row| remaining_rows.contains(row))
                    .count();

                let m_cost = (r_len.saturating_sub(1)) * (c_len.saturating_sub(1));

                let is_better = match best_pivot {
                    None => true,
                    Some((best_r, best_c)) => {
                        if m_cost < min_markowitz {
                            true
                        } else if m_cost == min_markowitz {
                            // Tie-breaker 1: prefer diagonal (preserves structural symmetry)
                            let is_diag = r == c;
                            let best_is_diag = best_r == best_c;
                            if is_diag && !best_is_diag {
                                true
                            } else if !is_diag && best_is_diag {
                                false
                            } else {
                                // Tie-breaker 2: lower row, then lower col
                                (r, c) < (best_r, best_c)
                            }
                        } else {
                            false
                        }
                    }
                };

                if is_better {
                    min_markowitz = m_cost;
                    best_pivot = Some((r, c));
                    best_pivot_val = val;

                    // If Markowitz cost is 0 (singleton row or singleton col), it cannot create fill-in
                    if min_markowitz == 0 {
                        break;
                    }
                }
            }

            if min_markowitz == 0 {
                break;
            }
        }

        let (p, q) = best_pivot.ok_or(NumericalError::Singular { row: k, col: k })?;
        if best_pivot_val.abs() < PIVOT_EPSILON {
            return Err(NumericalError::Singular { row: p, col: q });
        }

        // Record chosen pivot for step k
        p_rows.push(p);
        q_cols.push(q);
        u_diag_data[k] = best_pivot_val;

        // Copy row p entries for uneliminated cols to U
        let p_row_copy: Vec<(usize, f64)> = row_entries[p]
            .iter()
            .filter(|(&c, _)| remaining_cols.contains(&c) && c != q)
            .map(|(&c, &v)| (c, v))
            .collect();

        for &(c, v) in &p_row_copy {
            u_row_data[p].insert(c, v);
        }

        // Active rows in col q to eliminate
        let rows_to_eliminate: Vec<usize> = col_entries[q]
            .iter()
            .filter(|&&r| remaining_rows.contains(&r) && r != p)
            .copied()
            .collect();

        for r in rows_to_eliminate {
            let a_rq = row_entries[r].get(&q).copied().unwrap_or(0.0);
            if a_rq.abs() < 1e-30 {
                continue;
            }
            let factor = a_rq / best_pivot_val;
            l_multipliers[r].insert(k, factor);

            // Elimination: row[r] = row[r] - factor * row[p]
            for &(c, v_p) in &p_row_copy {
                let existing = row_entries[r].get(&c).copied();
                let was_zero = existing.is_none();
                let new_val = existing.unwrap_or(0.0) - factor * v_p;

                if new_val.abs() < 1e-30 {
                    row_entries[r].remove(&c);
                    col_entries[c].remove(&r);
                } else {
                    row_entries[r].insert(c, new_val);
                    col_entries[c].insert(r);
                    if was_zero {
                        fill_ins += 1;
                    }
                }
            }
        }

        // Remove row p and col q from active submatrix
        remaining_rows.remove(&p);
        remaining_cols.remove(&q);
    }

    // Build fast lookup arrays for forward and backward substitution
    // Map original col -> permuted col index
    let mut orig_col_to_perm: Vec<usize> = vec![0; n];
    for (perm_idx, &orig_col) in q_cols.iter().enumerate() {
        orig_col_to_perm[orig_col] = perm_idx;
    }

    let mut l_rows: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
    let mut u_rows: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
    let mut u_diag: Vec<f64> = vec![0.0; n];

    for (k, &p) in p_rows.iter().enumerate() {
        u_diag[k] = u_diag_data[k];

        // L multipliers for permuted row k (originally row p)
        let mut l_list: Vec<(usize, f64)> =
            l_multipliers[p].iter().map(|(&j, &val)| (j, val)).collect();
        l_list.sort_by_key(|&(j, _)| j);
        l_rows[k] = l_list;

        // U entries for permuted row k (originally row p)
        let mut u_list: Vec<(usize, f64)> = u_row_data[p]
            .iter()
            .map(|(&orig_col, &val)| (orig_col_to_perm[orig_col], val))
            .collect();
        u_list.sort_by_key(|&(j, _)| j);
        u_rows[k] = u_list;
    }

    Ok(SparseLu {
        n,
        p_rows,
        q_cols,
        l_rows,
        u_rows,
        u_diag,
        fill_ins,
    })
}
