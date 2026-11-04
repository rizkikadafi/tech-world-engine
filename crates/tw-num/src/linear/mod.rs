//! Linear algebra primitives for `tw-num`.
//!
//! This module is domain-agnostic. It provides dense and sparse matrix/vector types,
//! along with LU factorization and linear solve routines.

pub mod dense;
pub mod matrix;
pub mod sparse;
pub mod vector;

pub use dense::{lu, solve, LuFactorization};
pub use matrix::Matrix;
pub use sparse::{solve_sparse, solve_with_sparse_lu, sparse_lu, SparseLu, SparseMatrix, Triplet};
pub use vector::Vector;
