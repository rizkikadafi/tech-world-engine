//! Nonlinear solvers and root-finding routines.

pub mod newton;

pub use newton::{newton_scalar, NewtonConfig, NewtonResult, NewtonSolver};
