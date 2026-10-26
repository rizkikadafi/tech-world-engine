use thiserror::Error;

/// Errors that can occur during numerical operations.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum NumericalError {
    /// Matrix is not square but square matrix is required.
    #[error("matrix is not square: {rows}x{cols}")]
    NotSquare { rows: usize, cols: usize },
    /// Dimensions do not match for operation.
    #[error("dimension mismatch")]
    DimensionMismatch,
    /// Matrix is singular or near-singular at the given pivot location.
    #[error("singular matrix or near-singular at pivot ({row}, {col})")]
    Singular { row: usize, col: usize },
    /// Index out of bounds.
    #[error("index out of bounds")]
    IndexOutOfBounds,
    /// Invalid size (e.g., zero dimension).
    #[error("invalid size")]
    InvalidSize,
}
