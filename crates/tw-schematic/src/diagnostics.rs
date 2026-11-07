use serde::{Deserialize, Serialize};

use crate::document::{PartId, WireId};

/// Diagnostic severity. Errors block compilation; warnings do not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// One validation finding, suitable for display in the editor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<PartId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wires: Vec<WireId>,
}

impl Diagnostic {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            code: code.into(),
            message: message.into(),
            parts: Vec::new(),
            wires: Vec::new(),
        }
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            code: code.into(),
            message: message.into(),
            parts: Vec::new(),
            wires: Vec::new(),
        }
    }

    pub fn with_part(mut self, part: impl Into<PartId>) -> Self {
        self.parts.push(part.into());
        self
    }

    pub fn with_wire(mut self, wire: impl Into<WireId>) -> Self {
        self.wires.push(wire.into());
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}
