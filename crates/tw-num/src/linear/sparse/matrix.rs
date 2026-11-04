//! Sparse matrix representations and builders for `tw-num`.
//!
//! Designed for efficient assembly in MNA stamping (where multiple components
//! add conductances to the same matrix coordinates) and for Markowitz LU factorization.

use std::collections::HashMap;

use crate::error::NumericalError;
use crate::linear::matrix::Matrix;

/// An entry in a sparse matrix coordinate list.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Triplet {
    pub row: usize,
    pub col: usize,
    pub val: f64,
}

/// A sparse matrix with dynamic assembly and coordinate-based indexing.
#[derive(Debug, Clone, PartialEq)]
pub struct SparseMatrix {
    rows: usize,
    cols: usize,
    /// Internal map storing (row, col) -> value.
    /// This allows O(1) accumulation during stamping.
    entries: HashMap<(usize, usize), f64>,
}

impl SparseMatrix {
    /// Creates a new empty sparse matrix with the given dimensions.
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            entries: HashMap::new(),
        }
    }

    /// Number of rows.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Number of columns.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Returns true if the matrix is square.
    pub fn is_square(&self) -> bool {
        self.rows == self.cols
    }

    /// Number of non-zero entries currently stored.
    pub fn non_zero_count(&self) -> usize {
        self.entries.len()
    }

    /// Gets the value at `(row, col)`. Returns `0.0` if not present.
    pub fn get(&self, row: usize, col: usize) -> f64 {
        if row >= self.rows || col >= self.cols {
            return 0.0;
        }
        self.entries.get(&(row, col)).copied().unwrap_or(0.0)
    }

    /// Adds `val` to the entry at `(row, col)` (useful for MNA stamping).
    pub fn add_element(&mut self, row: usize, col: usize, val: f64) -> Result<(), NumericalError> {
        if row >= self.rows || col >= self.cols {
            return Err(NumericalError::IndexOutOfBounds);
        }
        if val == 0.0 {
            return Ok(());
        }
        let entry = self.entries.entry((row, col)).or_insert(0.0);
        *entry += val;
        if entry.abs() < 1e-30 {
            self.entries.remove(&(row, col));
        }
        Ok(())
    }

    /// Sets the value at `(row, col)`.
    pub fn set_element(&mut self, row: usize, col: usize, val: f64) -> Result<(), NumericalError> {
        if row >= self.rows || col >= self.cols {
            return Err(NumericalError::IndexOutOfBounds);
        }
        if val.abs() < 1e-30 {
            self.entries.remove(&(row, col));
        } else {
            self.entries.insert((row, col), val);
        }
        Ok(())
    }

    /// Returns an iterator over all non-zero triplets `(row, col, val)`.
    pub fn triplets(&self) -> impl Iterator<Item = Triplet> + '_ {
        self.entries
            .iter()
            .map(|(&(row, col), &val)| Triplet { row, col, val })
    }

    /// Converts this sparse matrix to a dense `Matrix`.
    pub fn to_dense(&self) -> Result<Matrix, NumericalError> {
        let mut dense = Matrix::zeros(self.rows, self.cols)?;
        for Triplet { row, col, val } in self.triplets() {
            dense[(row, col)] = val;
        }
        Ok(dense)
    }

    /// Creates a `SparseMatrix` from a dense `Matrix`.
    pub fn from_dense(dense: &Matrix) -> Self {
        let mut sparse = Self::new(dense.rows(), dense.cols());
        for i in 0..dense.rows() {
            for j in 0..dense.cols() {
                let v = dense[(i, j)];
                if v.abs() >= 1e-30 {
                    let _ = sparse.set_element(i, j, v);
                }
            }
        }
        sparse
    }
}
