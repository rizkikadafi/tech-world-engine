use serde::{Deserialize, Serialize};
use serde_json::Value;
use tw_electrical::CircuitState;

use crate::chip::ChipRegistry;
use crate::compile::{compile_with_registry, CompiledCircuit};
use crate::document::{CircuitDocument, Part, PartId, PinRef, Point2, Wire, WireId};
use crate::error::SchematicError;

/// One reversible edit to a [`CircuitDocument`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Command {
    AddPart {
        part: Part,
    },
    RemovePart {
        id: PartId,
    },
    MovePart {
        id: PartId,
        position: Point2,
    },
    SetParam {
        id: PartId,
        key: String,
        #[serde(default)]
        value: Option<Value>,
    },
    RenamePart {
        id: PartId,
        #[serde(default)]
        label: Option<String>,
    },
    Connect {
        #[serde(default)]
        id: Option<WireId>,
        from: PinRef,
        to: PinRef,
    },
    Disconnect {
        id: WireId,
    },
}

impl Command {
    pub fn apply(&self, document: &mut CircuitDocument) -> Result<(), SchematicError> {
        match self {
            Command::AddPart { part } => document.add_part(part.clone()),
            Command::RemovePart { id } => {
                document.remove_part(id)?;
                Ok(())
            }
            Command::MovePart { id, position } => {
                let part = document
                    .part_mut(id)
                    .ok_or_else(|| SchematicError::UnknownPart(id.clone()))?;
                part.position = *position;
                Ok(())
            }
            Command::SetParam { id, key, value } => {
                let part = document
                    .part_mut(id)
                    .ok_or_else(|| SchematicError::UnknownPart(id.clone()))?;
                match value {
                    Some(value) => {
                        part.params.insert(key.clone(), value.clone());
                    }
                    None => {
                        part.params.remove(key);
                    }
                }
                Ok(())
            }
            Command::RenamePart { id, label } => {
                let part = document
                    .part_mut(id)
                    .ok_or_else(|| SchematicError::UnknownPart(id.clone()))?;
                part.label = label.clone();
                Ok(())
            }
            Command::Connect { id, from, to } => {
                let id = id.clone().unwrap_or_else(|| document.fresh_wire_id());
                document.add_wire(Wire::new(id, from.clone(), to.clone()))
            }
            Command::Disconnect { id } => {
                document.remove_wire(id)?;
                Ok(())
            }
        }
    }
}

/// A document plus undo/redo, compilation cache, and the last DC state.
#[derive(Debug)]
pub struct SchematicSession {
    document: CircuitDocument,
    compiled: Option<CompiledCircuit>,
    state: Option<CircuitState>,
    undo_stack: Vec<CircuitDocument>,
    redo_stack: Vec<CircuitDocument>,
    revision: u64,
    registry: ChipRegistry,
}

impl SchematicSession {
    pub fn new(document: CircuitDocument) -> Self {
        Self {
            document,
            compiled: None,
            state: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            revision: 0,
            registry: ChipRegistry::new(),
        }
    }

    pub fn with_registry(document: CircuitDocument, registry: ChipRegistry) -> Self {
        Self {
            document,
            compiled: None,
            state: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            revision: 0,
            registry,
        }
    }

    pub fn registry(&self) -> &ChipRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut ChipRegistry {
        &mut self.registry
    }

    pub fn document(&self) -> &CircuitDocument {
        &self.document
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn state(&self) -> Option<&CircuitState> {
        self.state.as_ref()
    }

    /// Apply a command and return the new revision.
    pub fn apply(&mut self, command: &Command) -> Result<u64, SchematicError> {
        let mut next = self.document.clone();
        command.apply(&mut next)?;

        self.undo_stack
            .push(std::mem::replace(&mut self.document, next));
        self.redo_stack.clear();
        self.invalidate();
        self.revision += 1;
        Ok(self.revision)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        match self.undo_stack.pop() {
            Some(previous) => {
                let current = std::mem::replace(&mut self.document, previous);
                self.redo_stack.push(current);
                self.invalidate();
                self.revision += 1;
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        match self.redo_stack.pop() {
            Some(next) => {
                let current = std::mem::replace(&mut self.document, next);
                self.undo_stack.push(current);
                self.invalidate();
                self.revision += 1;
                true
            }
            None => false,
        }
    }

    /// Compile lazily and cache the result until the next edit.
    pub fn compile(&mut self) -> Result<&CompiledCircuit, SchematicError> {
        if self.compiled.is_none() {
            self.compiled = Some(compile_with_registry(&self.document, Some(&self.registry))?);
        }
        Ok(self.compiled.as_ref().expect("compiled was just set"))
    }

    /// Compile (if needed) and solve the DC operating point.
    pub fn run_dc(&mut self) -> Result<&CircuitState, SchematicError> {
        if self.compiled.is_none() {
            self.compiled = Some(compile_with_registry(&self.document, Some(&self.registry))?);
        }
        let state = self
            .compiled
            .as_ref()
            .expect("compiled was just set")
            .run_dc()?;
        self.state = Some(state);
        Ok(self.state.as_ref().expect("state was just set"))
    }

    fn invalidate(&mut self) {
        self.compiled = None;
        self.state = None;
    }
}
