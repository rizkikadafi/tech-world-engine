//! Dense vector (column vector) backed by `Vec<f64>`.

use crate::error::NumericalError;
use std::ops::{Add, AddAssign, Mul, Sub};

use std::fmt;

/// A dense real vector.
#[derive(Clone, PartialEq)]
pub struct Vector {
    data: Vec<f64>,
}

impl fmt::Debug for Vector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Vector (len={}):", self.data.len())?;
        write!(f, "  [")?;
        for (i, v) in self.data.iter().enumerate() {
            write!(f, "{:10.6}", v)?;
            if i < self.data.len() - 1 {
                write!(f, " ")?;
            }
        }
        writeln!(f, "]")
    }
}

impl Vector {
    /// Create a zero vector of length `n`.
    pub fn zeros(n: usize) -> Self {
        Self {
            data: vec![0.0f64; n],
        }
    }

    /// Create a vector from a slice.
    pub fn from_slice(s: &[f64]) -> Self {
        Self { data: s.to_vec() }
    }

    /// Create a vector from a `Vec<f64>`.
    pub fn from_vec(v: Vec<f64>) -> Self {
        Self { data: v }
    }

    /// Get length of vector.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get element at index `i`.
    pub fn get(&self, i: usize) -> Option<f64> {
        self.data.get(i).copied()
    }

    /// Set element at index `i`.
    pub fn set(&mut self, i: usize, val: f64) -> Result<(), NumericalError> {
        if i >= self.data.len() {
            return Err(NumericalError::IndexOutOfBounds);
        }
        self.data[i] = val;
        Ok(())
    }

    /// Get mutable access to underlying data.
    pub fn as_mut_slice(&mut self) -> &mut [f64] {
        &mut self.data
    }

    /// Get immutable access to underlying data.
    pub fn as_slice(&self) -> &[f64] {
        &self.data
    }

    /// Compute L2 norm.
    pub fn norm2(&self) -> f64 {
        let mut s = 0.0f64;
        for &x in &self.data {
            s += x * x;
        }
        s.sqrt()
    }
}

impl Add for Vector {
    type Output = Result<Vector, NumericalError>;

    fn add(self, other: Vector) -> Self::Output {
        if self.len() != other.len() {
            return Err(NumericalError::DimensionMismatch);
        }
        let mut out = vec![0.0f64; self.len()];
        for i in 0..self.len() {
            out[i] = self.data[i] + other.data[i];
        }
        Ok(Vector { data: out })
    }
}

impl AddAssign for Vector {
    fn add_assign(&mut self, other: Vector) {
        if self.len() == other.len() {
            for i in 0..self.len() {
                self.data[i] += other.data[i];
            }
        }
        // If dimensions differ, we silently do nothing to match simple behavior;
        // call sites should check dimensions explicitly.
    }
}

impl Sub for Vector {
    type Output = Result<Vector, NumericalError>;

    fn sub(self, other: Vector) -> Self::Output {
        if self.len() != other.len() {
            return Err(NumericalError::DimensionMismatch);
        }
        let mut out = vec![0.0f64; self.len()];
        for i in 0..self.len() {
            out[i] = self.data[i] - other.data[i];
        }
        Ok(Vector { data: out })
    }
}

impl Mul<f64> for Vector {
    type Output = Vector;

    fn mul(self, scalar: f64) -> Self::Output {
        let mut out = self.data.clone();
        for v in out.iter_mut().take(self.len()) {
            *v *= scalar;
        }
        Vector { data: out }
    }
}
