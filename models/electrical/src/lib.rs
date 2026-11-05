//! Electrical Circuit Domain and MNA Solver (`tw-electrical`).
//!
//! Provides circuit representation (nodes, passive devices, independent sources)
//! and automated Modified Nodal Analysis (MNA) matrix assembly and solution
//! via `tw-num`.

pub mod analysis;
pub mod bridge;
pub mod characterization;
pub mod circuit;
pub mod derived;
pub mod device;
pub mod digital;
pub mod error;
pub mod fidelity;
pub mod gates;
pub mod mna;
pub mod node;
pub mod prelude;
pub mod state;
pub mod subcircuit;
pub mod time_base;

pub use analysis::{
    dc_op, integrate_source_energy, transient, transient_adaptive, TransientResult,
};
pub use bridge::{BridgeConfig, BridgeInput, BridgeOutput, CircuitComponent};
pub use characterization::{
    CharacterizationConfig, Characterizer, CircuitTemplate, GateCharacterization, GateKind,
    LogicLevels, Measurement, TemplateInput, TimingMetrics, TruthTable,
};
pub use circuit::Circuit;
pub use derived::{DerivedDff, DerivedError, DerivedGate, TimingTicks};
pub use device::{Capacitor, CurrentSource, Diode, Mosfet, MosfetType, Resistor, VoltageSource};
pub use digital::{cmos_d_latch, cmos_dff, cmos_full_adder, cmos_half_adder};
pub use error::CircuitError;
pub use fidelity::{ComponentSpec, Fidelity, FidelityError, FidelityFactory, Model};
pub use gates::{cmos_and, cmos_inverter, cmos_nand, cmos_nor, cmos_or, cmos_xor, CmosGateConfig};
pub use mna::MnaSystem;
pub use node::NodeId;
pub use state::CircuitState;
pub use subcircuit::{DeviceTemplate, Subcircuit, SubcircuitInstance};
pub use time_base::TimeBase;
