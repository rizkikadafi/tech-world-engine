//! Linear algebra primitives for `tw-num`.
//!
//! This module is domain-agnostic. It provides dense `Matrix` and `Vector` types,
//! along with LU factorization and linear solve routines.

pub mod dense;
pub mod matrix;
pub mod vector;

pub use dense::{lu, solve, LuFactorization};
pub use matrix::Matrix;
pub use vector::Vector;
