//! # tw-num
//!
//! Domain-agnostic numerical foundation for Tech World Engine (Phase 6).
//!
//! This crate is inspired by concepts from ngspice's `maths/` (dense LU, sparse ideas)
//! but implemented idiomatically in Rust with a clean, testable API.
//!
//! ## Design Principles
//! - **Domain-agnostic**: No knowledge of circuits, components, or simulation engine.
//! - **Independent**: Can be used without `tw-core`/`tw-sim`.
//! - **Minimal**: Start with dense LU + validation, expand only when needed.
//! - **Deterministic**: Explicit partial pivoting, stable operations.

pub mod error;
pub mod linear;
pub mod nonlinear;
pub mod ode;
pub mod prelude;

pub use error::NumericalError;
pub use linear::{solve, Matrix, Vector};
pub use nonlinear::{newton_scalar, NewtonConfig, NewtonResult, NewtonSolver};
pub use prelude::*;
