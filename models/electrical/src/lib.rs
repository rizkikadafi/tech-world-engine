//! Electrical Circuit Domain and MNA Solver (`tw-electrical`).
//!
//! Provides circuit representation (nodes, passive devices, independent sources)
//! and automated Modified Nodal Analysis (MNA) matrix assembly and solution
//! via `tw-num`.

pub mod analysis;
pub mod circuit;
pub mod device;
pub mod error;
pub mod mna;
pub mod node;
pub mod prelude;
pub mod state;

pub use analysis::{dc_op, transient, TransientResult};
pub use circuit::Circuit;
pub use device::{Capacitor, CurrentSource, Resistor, VoltageSource};
pub use error::CircuitError;
pub use mna::MnaSystem;
pub use node::NodeId;
pub use state::CircuitState;
