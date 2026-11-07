//! Editable circuit documents for the Tech World game layer.
//!
//! `tw-schematic` owns the user-facing schematic: parts, pins, wires,
//! positions, edit commands, validation, and persistence. It contains no
//! physics. A document is compiled into [`tw_electrical::Circuit`], which is
//! then solved by the electrical domain crate.
//!
//! Keeping authoring state separate from the compiled netlist means editor
//! concerns (wire routing, undo/redo, chip metadata, save versioning) never
//! leak into the solver.
//!
//! ```
//! use tw_schematic::{CircuitDocument, Part, PartKind, Point2};
//!
//! let mut doc = CircuitDocument::new("example");
//! doc.add_part(Part::new("r1", PartKind::Resistor, Point2::default()))
//!     .unwrap();
//! assert_eq!(doc.parts().len(), 1);
//! ```

pub mod command;
pub mod compile;
pub mod diagnostics;
pub mod document;
pub mod error;
pub mod net;
pub mod validate;

pub use command::{Command, SchematicSession};
pub use compile::CompiledCircuit;
pub use diagnostics::{Diagnostic, Severity};
pub use document::{
    CircuitDocument, Part, PartId, PartKind, PinDefinition, PinRef, PinRole, Point2, Wire, WireId,
    DOCUMENT_VERSION,
};
pub use error::SchematicError;
pub use net::{NetId, NetMap, ResolvedNet};
