//! Dense matrix backed by row-major `Vec<f64>`.

use crate::error::NumericalError;
use std::fmt;

/// A dense real matrix in row-major order.
#[derive(Clone, PartialEq)]
pub struct Matrix {
    rows: usize,
    cols: usize,
    data: Vec<f64>, // data[r * cols + c]
}

impl fmt::Debug for Matrix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Matrix {}x{}:", self.rows, self.cols)?;
        for r in 0..self.rows {
            write!(f, "  [")?;
            for c in 0..self.cols {
                let val = self.data[r * self.cols + c];
                write!(f, "{:10.6}", val)?;
                if c < self.cols - 1 {
                    write!(f, " ")?;
                }
            }
            writeln!(f, "]")?;
        }
        Ok(())
    }
}

impl Matrix {
    /// Create a zero matrix of size `rows x cols`.
    pub fn zeros(rows: usize, cols: usize) -> Result<Self, NumericalError> {
        if rows == 0 || cols == 0 {
            return Err(NumericalError::InvalidSize);
        }
        Ok(Self {
            rows,
            cols,
            data: vec![0.0f64; rows * cols],
        })
    }

    /// Create an identity matrix of size `n x n`.
    pub fn identity(n: usize) -> Result<Self, NumericalError> {
        if n == 0 {
            return Err(NumericalError::InvalidSize);
        }
        let mut m = Self::zeros(n, n)?;
        for i in 0..n {
            m.data[i * n + i] = 1.0f64;
        }
        Ok(m)
    }

    /// Create a matrix from row vectors.
    pub fn from_rows(rows: Vec<Vec<f64>>) -> Result<Self, NumericalError> {
        if rows.is_empty() {
            return Err(NumericalError::InvalidSize);
        }
        let r = rows.len();
        let c = rows[0].len();
        if c == 0 {
            return Err(NumericalError::InvalidSize);
        }
        for row in &rows {
            if row.len() != c {
                return Err(NumericalError::DimensionMismatch);
            }
        }
        let mut data = vec![0.0f64; r * c];
        for i in 0..r {
            for j in 0..c {
                data[i * c + j] = rows[i][j];
            }
        }
        Ok(Self { rows: r, cols: c, data })
    }

    /// Get number of rows.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Get number of columns.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Check if square.
    pub fn is_square(&self) -> bool {
        self.rows == self.cols
    }

    /// Get element at (row, col).
    pub fn get(&self, r: usize, c: usize) -> Result<f64, NumericalError> {
        if r >= self.rows || c >= self.cols {
            return Err(NumericalError::IndexOutOfBounds);
        }
        Ok(self.data[r * self.cols + c])
    }

    /// Set element at (row, col).
    pub fn set(&mut self, r: usize, c: usize, val: f64) -> Result<(), NumericalError> {
        if r >= self.rows || c >= self.cols {
            return Err(NumericalError::IndexOutOfBounds);
        }
        self.data[r * self.cols + c] = val;
        Ok(())
    }

    /// Get immutable slice of underlying data (row-major).
    pub fn as_slice(&self) -> &[f64] {
        &self.data
    }

    /// Get mutable slice of underlying data (row-major).
    pub fn as_mut_slice(&mut self) -> &mut [f64] {
        &mut self.data
    }

    /// Create a clone as mutable for factorization.
    pub fn clone_to_vec(&self) -> Vec<f64> {
        self.data.clone()
    }

    /// Build matrix from raw row-major data (caller ensures dimensions match).
    pub fn from_row_major(rows: usize, cols: usize, data: Vec<f64>) -> Result<Self, NumericalError> {
        if rows == 0 || cols == 0 {
            return Err(NumericalError::InvalidSize);
        }
        if data.len() != rows * cols {
            return Err(NumericalError::DimensionMismatch);
        }
        Ok(Self { rows, cols, data })
    }
}

impl std::ops::Index<(usize, usize)> for Matrix {
    type Output = f64;

    #[inline]
    fn index(&self, index: (usize, usize)) -> &Self::Output {
        let (r, c) = index;
        assert!(r < self.rows && c < self.cols, "matrix index out of bounds: ({}, {}) for {}x{}", r, c, self.rows, self.cols);
        &self.data[r * self.cols + c]
    }
}

impl std::ops::IndexMut<(usize, usize)> for Matrix {
    #[inline]
    fn index_mut(&mut self, index: (usize, usize)) -> &mut Self::Output {
        let (r, c) = index;
        assert!(r < self.rows && c < self.cols, "matrix index out of bounds: ({}, {}) for {}x{}", r, c, self.rows, self.cols);
        &mut self.data[r * self.cols + c]
    }
}

