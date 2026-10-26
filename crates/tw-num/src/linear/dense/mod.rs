//! Dense linear solvers (inspired by ngspice maths/dense, adapted to Rust).

pub mod lu;
pub mod solve;

pub use lu::{lu_factor, LuFactorization};
pub use solve::solve;
