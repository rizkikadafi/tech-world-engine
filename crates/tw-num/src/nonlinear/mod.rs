//! Nonlinear solvers and root-finding routines.

pub mod convergence;
pub mod newton;

pub use convergence::{check_convergence, ConvergenceConfig, VariableType};
pub use newton::{newton_scalar, NewtonConfig, NewtonResult, NewtonSolver};
