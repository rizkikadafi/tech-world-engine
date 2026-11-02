//! Electrical Circuit Domain and MNA Solver (`tw-electrical`).
//!
//! Provides circuit representation (nodes, passive devices, independent sources)
//! and automated Modified Nodal Analysis (MNA) matrix assembly and solution
//! via `tw-num`.

pub mod analysis;
pub mod circuit;
pub mod device;
pub mod digital;
pub mod error;
pub mod gates;
pub mod mna;
pub mod node;
pub mod prelude;
pub mod state;
pub mod subcircuit;

pub use analysis::{dc_op, transient, TransientResult};
pub use circuit::Circuit;
pub use device::{Capacitor, CurrentSource, Diode, Mosfet, MosfetType, Resistor, VoltageSource};
pub use digital::{cmos_d_latch, cmos_dff, cmos_full_adder, cmos_half_adder};
pub use error::CircuitError;
pub use gates::{cmos_and, cmos_inverter, cmos_nand, cmos_nor, cmos_or, cmos_xor, CmosGateConfig};
pub use mna::MnaSystem;
pub use node::NodeId;
pub use state::CircuitState;
pub use subcircuit::{DeviceTemplate, Subcircuit, SubcircuitInstance};
