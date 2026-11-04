//! Prelude for convenient imports.

pub use crate::error::NumericalError;
pub use crate::linear::{
    solve, solve_sparse, solve_with_sparse_lu, sparse_lu, Matrix, SparseLu, SparseMatrix, Triplet,
    Vector,
};
pub use crate::nonlinear::{
    check_convergence, newton_scalar, ConvergenceConfig, NewtonConfig, NewtonResult, NewtonSolver,
    VariableType,
};
pub use crate::ode::{evaluate_step_lte, IntegratorConfig, StepControlConfig, StepDecision};
