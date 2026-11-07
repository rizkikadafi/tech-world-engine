use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::compile::CompiledCircuit;
use crate::diagnostics::Diagnostic;
use crate::error::SchematicError;
use crate::net::NetMap;

/// Current on-disk document version.
pub const DOCUMENT_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// IDs
// ---------------------------------------------------------------------------

/// Stable identifier of a part inside a document.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PartId(pub String);

impl PartId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl From<&str> for PartId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for PartId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&String> for PartId {
    fn from(value: &String) -> Self {
        Self(value.clone())
    }
}

impl AsRef<str> for PartId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PartId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Stable identifier of a wire inside a document.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WireId(pub String);

impl WireId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl From<&str> for WireId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for WireId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl AsRef<str> for WireId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for WireId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Reference to one pin of one part.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PinRef {
    pub part: PartId,
    pub pin: String,
}

impl PinRef {
    pub fn new(part: impl Into<PartId>, pin: impl Into<String>) -> Self {
        Self {
            part: part.into(),
            pin: pin.into(),
        }
    }
}

impl std::fmt::Display for PinRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.part, self.pin)
    }
}

// ---------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------

/// Position in the 2D schematic editor.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

impl Point2 {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

// ---------------------------------------------------------------------------
// Part catalog
// ---------------------------------------------------------------------------

/// Electrical role of a pin, used for validation and UI hints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinRole {
    Passive,
    Power,
    Ground,
    Signal,
}

/// Static description of one pin on a part kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PinDefinition {
    pub name: &'static str,
    pub role: PinRole,
}

/// Builtin part kinds available at the circuit level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartKind {
    Resistor,
    Capacitor,
    VoltageSource,
    CurrentSource,
    Ground,
    Diode,
    Nmos,
    Pmos,
    Lamp,
    Switch,
    Probe,
}

const TWO_TERMINAL_PINS: &[PinDefinition] = &[
    PinDefinition {
        name: "a",
        role: PinRole::Passive,
    },
    PinDefinition {
        name: "b",
        role: PinRole::Passive,
    },
];

const SOURCE_PINS: &[PinDefinition] = &[
    PinDefinition {
        name: "pos",
        role: PinRole::Power,
    },
    PinDefinition {
        name: "neg",
        role: PinRole::Power,
    },
];

const GROUND_PINS: &[PinDefinition] = &[PinDefinition {
    name: "gnd",
    role: PinRole::Ground,
}];

const DIODE_PINS: &[PinDefinition] = &[
    PinDefinition {
        name: "anode",
        role: PinRole::Passive,
    },
    PinDefinition {
        name: "cathode",
        role: PinRole::Passive,
    },
];

const MOSFET_PINS: &[PinDefinition] = &[
    PinDefinition {
        name: "drain",
        role: PinRole::Passive,
    },
    PinDefinition {
        name: "gate",
        role: PinRole::Signal,
    },
    PinDefinition {
        name: "source",
        role: PinRole::Passive,
    },
];

const PROBE_PINS: &[PinDefinition] = &[PinDefinition {
    name: "node",
    role: PinRole::Signal,
}];

impl PartKind {
    pub const ALL: [PartKind; 11] = [
        PartKind::Resistor,
        PartKind::Capacitor,
        PartKind::VoltageSource,
        PartKind::CurrentSource,
        PartKind::Ground,
        PartKind::Diode,
        PartKind::Nmos,
        PartKind::Pmos,
        PartKind::Lamp,
        PartKind::Switch,
        PartKind::Probe,
    ];

    /// Stable JSON/ID string for this kind.
    pub fn id(self) -> &'static str {
        match self {
            PartKind::Resistor => "resistor",
            PartKind::Capacitor => "capacitor",
            PartKind::VoltageSource => "voltage_source",
            PartKind::CurrentSource => "current_source",
            PartKind::Ground => "ground",
            PartKind::Diode => "diode",
            PartKind::Nmos => "nmos",
            PartKind::Pmos => "pmos",
            PartKind::Lamp => "lamp",
            PartKind::Switch => "switch",
            PartKind::Probe => "probe",
        }
    }

    /// Human-readable name for the editor palette.
    pub fn display_name(self) -> &'static str {
        match self {
            PartKind::Resistor => "Resistor",
            PartKind::Capacitor => "Capacitor",
            PartKind::VoltageSource => "Voltage Source",
            PartKind::CurrentSource => "Current Source",
            PartKind::Ground => "Ground",
            PartKind::Diode => "Diode",
            PartKind::Nmos => "NMOS",
            PartKind::Pmos => "PMOS",
            PartKind::Lamp => "Lamp",
            PartKind::Switch => "Switch",
            PartKind::Probe => "Probe",
        }
    }

    pub fn pins(self) -> &'static [PinDefinition] {
        match self {
            PartKind::Resistor
            | PartKind::Capacitor
            | PartKind::Lamp
            | PartKind::Switch => TWO_TERMINAL_PINS,
            PartKind::VoltageSource | PartKind::CurrentSource => SOURCE_PINS,
            PartKind::Ground => GROUND_PINS,
            PartKind::Diode => DIODE_PINS,
            PartKind::Nmos | PartKind::Pmos => MOSFET_PINS,
            PartKind::Probe => PROBE_PINS,
        }
    }

    pub fn has_pin(self, name: &str) -> bool {
        self.pins().iter().any(|pin| pin.name == name)
    }

    pub fn is_ground(self) -> bool {
        matches!(self, PartKind::Ground)
    }
}

// ---------------------------------------------------------------------------
// Parts and wires
// ---------------------------------------------------------------------------

/// One placed component in the schematic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub id: PartId,
    pub kind: PartKind,
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
    #[serde(default)]
    pub position: Point2,
    #[serde(default)]
    pub rotation: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl Part {
    pub fn new(id: impl Into<PartId>, kind: PartKind, position: Point2) -> Self {
        Self {
            id: id.into(),
            kind,
            params: BTreeMap::new(),
            position,
            rotation: 0.0,
            label: None,
        }
    }

    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn param(&self, key: &str) -> Option<&Value> {
        self.params.get(key)
    }

    pub fn f64_param(&self, key: &str, default: f64) -> f64 {
        self.params
            .get(key)
            .and_then(Value::as_f64)
            .unwrap_or(default)
    }

    pub fn bool_param(&self, key: &str, default: bool) -> bool {
        self.params
            .get(key)
            .and_then(Value::as_bool)
            .unwrap_or(default)
    }
}

/// One wire connecting two pins.
///
/// `points` is presentation-only routing data. Connectivity is always derived
/// from `from` and `to`; the compiler ignores intermediate points.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Wire {
    pub id: WireId,
    pub from: PinRef,
    pub to: PinRef,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<Point2>,
}

impl Wire {
    pub fn new(id: impl Into<WireId>, from: PinRef, to: PinRef) -> Self {
        Self {
            id: id.into(),
            from,
            to,
            points: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Document
// ---------------------------------------------------------------------------

fn default_version() -> u32 {
    DOCUMENT_VERSION
}

fn default_name() -> String {
    "untitled-circuit".to_string()
}

/// Editable schematic document.
///
/// Fields are private so connectivity changes go through methods that keep
/// wires and parts consistent. `Part`/`Wire` themselves are plain data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CircuitDocument {
    #[serde(default = "default_version")]
    version: u32,
    #[serde(default = "default_name")]
    name: String,
    #[serde(default)]
    parts: Vec<Part>,
    #[serde(default)]
    wires: Vec<Wire>,
}

impl CircuitDocument {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            version: DOCUMENT_VERSION,
            name: name.into(),
            parts: Vec::new(),
            wires: Vec::new(),
        }
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    pub fn parts(&self) -> &[Part] {
        &self.parts
    }

    pub fn wires(&self) -> &[Wire] {
        &self.wires
    }

    pub fn part(&self, id: impl AsRef<str>) -> Option<&Part> {
        self.parts.iter().find(|part| part.id.0 == id.as_ref())
    }

    pub fn part_mut(&mut self, id: impl AsRef<str>) -> Option<&mut Part> {
        self.parts
            .iter_mut()
            .find(|part| part.id.0 == id.as_ref())
    }

    pub fn wire(&self, id: impl AsRef<str>) -> Option<&Wire> {
        self.wires.iter().find(|wire| wire.id.0 == id.as_ref())
    }

    pub fn add_part(&mut self, part: Part) -> Result<(), SchematicError> {
        if self.parts.iter().any(|existing| existing.id == part.id) {
            return Err(SchematicError::DuplicatePartId(part.id));
        }
        self.parts.push(part);
        Ok(())
    }

    /// Removes a part and every wire attached to it.
    pub fn remove_part(&mut self, id: impl AsRef<str>) -> Result<Part, SchematicError> {
        let index = self
            .parts
            .iter()
            .position(|part| part.id.0 == id.as_ref())
            .ok_or_else(|| SchematicError::UnknownPart(PartId::new(id.as_ref())))?;

        let part = self.parts.remove(index);
        self.wires
            .retain(|wire| wire.from.part != part.id && wire.to.part != part.id);
        Ok(part)
    }

    pub fn add_wire(&mut self, wire: Wire) -> Result<(), SchematicError> {
        if self.wires.iter().any(|existing| existing.id == wire.id) {
            return Err(SchematicError::DuplicateWireId(wire.id));
        }
        if wire.from == wire.to {
            return Err(SchematicError::SelfWire(wire.from));
        }
        self.ensure_pin_exists(&wire.from)?;
        self.ensure_pin_exists(&wire.to)?;
        self.wires.push(wire);
        Ok(())
    }

    /// Connects two pins with a freshly generated wire id.
    pub fn connect(&mut self, from: PinRef, to: PinRef) -> Result<WireId, SchematicError> {
        let id = self.fresh_wire_id();
        self.add_wire(Wire::new(id.clone(), from, to))?;
        Ok(id)
    }

    pub fn remove_wire(&mut self, id: impl AsRef<str>) -> Result<Wire, SchematicError> {
        let index = self
            .wires
            .iter()
            .position(|wire| wire.id.0 == id.as_ref())
            .ok_or_else(|| SchematicError::UnknownWire(WireId::new(id.as_ref())))?;
        Ok(self.wires.remove(index))
    }

    pub fn fresh_wire_id(&self) -> WireId {
        let mut index = 1u64;
        loop {
            let candidate = WireId::new(format!("w{index}"));
            if !self.wires.iter().any(|wire| wire.id == candidate) {
                return candidate;
            }
            index += 1;
        }
    }

    /// Returns every diagnostic, including warnings.
    pub fn validate(&self) -> Vec<Diagnostic> {
        crate::validate::validate(self)
    }

    pub fn resolve(&self) -> Result<NetMap, SchematicError> {
        crate::net::resolve(self)
    }

    pub fn compile(&self) -> Result<CompiledCircuit, SchematicError> {
        crate::compile::compile(self)
    }

    pub fn to_json_string(&self) -> Result<String, SchematicError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn from_json_str(json: &str) -> Result<Self, SchematicError> {
        let document: Self = serde_json::from_str(json)?;
        if document.version > DOCUMENT_VERSION {
            return Err(SchematicError::UnsupportedVersion {
                found: document.version,
                max: DOCUMENT_VERSION,
            });
        }
        Ok(document)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, SchematicError> {
        let text = std::fs::read_to_string(path)?;
        Self::from_json_str(&text)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), SchematicError> {
        std::fs::write(path, self.to_json_string()?)?;
        Ok(())
    }

    fn ensure_pin_exists(&self, pin: &PinRef) -> Result<(), SchematicError> {
        match self.parts.iter().find(|part| part.id == pin.part) {
            None => Err(SchematicError::UnknownPart(pin.part.clone())),
            Some(part) if !part.kind.has_pin(&pin.pin) => Err(SchematicError::UnknownPin {
                part: part.id.clone(),
                pin: pin.pin.clone(),
            }),
            Some(_) => Ok(()),
        }
    }
}
