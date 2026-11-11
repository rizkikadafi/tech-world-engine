use thiserror::Error;
use tw_electrical::CircuitError;

use crate::diagnostics::Diagnostic;
use crate::document::{PartId, PinRef, WireId};

#[derive(Debug, Error)]
pub enum SchematicError {
    #[error("duplicate part id '{0}'")]
    DuplicatePartId(PartId),

    #[error("duplicate wire id '{0}'")]
    DuplicateWireId(WireId),

    #[error("unknown part '{0}'")]
    UnknownPart(PartId),

    #[error("unknown wire '{0}'")]
    UnknownWire(WireId),

    #[error("unknown pin '{pin}' on part '{part}'")]
    UnknownPin { part: PartId, pin: String },

    #[error("a wire cannot connect pin '{0}' to itself")]
    SelfWire(PinRef),

    #[error("invalid parameter for part '{part}': {message}")]
    InvalidParameter { part: PartId, message: String },

    #[error("validation failed: {0:?}")]
    Validation(Vec<Diagnostic>),

    #[error("electrical circuit error: {0}")]
    Circuit(#[from] CircuitError),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("unsupported document version {found} (maximum supported is {max})")]
    UnsupportedVersion { found: u32, max: u32 },

    #[error("invalid document: {0}")]
    InvalidDocument(String),
}

impl SchematicError {
    /// Error-level diagnostics, or an empty slice for other error kinds.
    pub fn validation_errors(&self) -> &[Diagnostic] {
        match self {
            SchematicError::Validation(diagnostics) => diagnostics,
            _ => &[],
        }
    }
}
