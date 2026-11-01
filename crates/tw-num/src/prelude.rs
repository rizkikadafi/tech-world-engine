//! Prelude for convenient imports.

pub use crate::error::NumericalError;
pub use crate::linear::{solve, Matrix, Vector};
pub use crate::nonlinear::{newton_scalar, NewtonConfig, NewtonResult, NewtonSolver};
pub use crate::ode::IntegratorConfig;
