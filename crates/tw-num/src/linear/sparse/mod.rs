//! Sparse linear algebra module for `tw-num`.
//!
//! Provides coordinate sparse matrix builder, Sparse LU with Markowitz pivoting,
//! and forward/backward solve.

pub mod lu;
pub mod matrix;
pub mod solve;

pub use lu::{sparse_lu, SparseLu, DEFAULT_MARKOWITZ_THRESHOLD};
pub use matrix::{SparseMatrix, Triplet};
pub use solve::{solve_sparse, solve_with_sparse_lu};
