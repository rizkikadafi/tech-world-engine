//! Packaging, abstraction, and registry for custom chips.
//!
//! A [`ChipDefinition`] packages an internal [`CircuitDocument`] with an external
//! pin interface, 3D package metadata, and empirical characterization results.
//!
//! When placed inside a schematic, a chip can either be compiled as an analog
//! [`Subcircuit`](tw_electrical::Subcircuit) (flattened down to transistors) or
//! simulated as a discrete event component.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tw_electrical::{DeviceTemplate, GenericCircuitCharacterization, MosfetType, Subcircuit};

use crate::document::{CircuitDocument, Part, PartKind, PinRole};
use crate::error::SchematicError;
use crate::net::{self, NetId};

/// Direction of an external chip pin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinDirection {
    Input,
    Output,
    Inout,
    Power,
    Ground,
}

impl PinDirection {
    pub fn role(self) -> PinRole {
        match self {
            PinDirection::Input | PinDirection::Output | PinDirection::Inout => PinRole::Signal,
            PinDirection::Power => PinRole::Power,
            PinDirection::Ground => PinRole::Ground,
        }
    }
}

/// Visual placement side of the pin on a 2D rectangular chip box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinSide {
    Left,
    Right,
    Top,
    Bottom,
}

/// An external interface pin on a packed chip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChipPin {
    pub name: String,
    pub direction: PinDirection,
    pub side: PinSide,
}

impl ChipPin {
    pub fn new(name: impl Into<String>, direction: PinDirection, side: PinSide) -> Self {
        Self {
            name: name.into(),
            direction,
            side,
        }
    }
}

/// 3D package metadata for the chip (footprint, dimensions, 3D pin numbers).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Package3DMetadata {
    /// Standard package designation, e.g. "DIP-4", "DIP-8", "DIP-14", "DIP-16", "SOIC-8".
    pub package_type: String,
    /// Physical number of pins on the 3D IC package.
    pub pin_count: usize,
    /// Optional resource path to a 3D model (e.g. "res://models/chips/dip8.glb").
    pub model_path: Option<String>,
    /// Package dimensions in millimeters: [width, length, height].
    pub dimensions_mm: [f64; 3],
    /// Mapping from logical pin name to physical pin number (1-indexed).
    pub pin_mapping: BTreeMap<String, usize>,
}

impl Default for Package3DMetadata {
    fn default() -> Self {
        Self {
            package_type: "DIP-8".to_string(),
            pin_count: 8,
            model_path: None,
            dimensions_mm: [9.8, 6.4, 3.4],
            pin_mapping: BTreeMap::new(),
        }
    }
}

/// A packaged integrated circuit (IC) definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChipDefinition {
    pub name: String,
    pub category: String,
    pub description: String,
    pub pins: Vec<ChipPin>,
    pub internal_schematic: CircuitDocument,
    pub package_3d: Package3DMetadata,
    pub characterization: Option<GenericCircuitCharacterization>,
}

impl ChipDefinition {
    /// Creates a new chip definition from an internal schematic and pin declarations.
    pub fn new(
        name: impl Into<String>,
        pins: Vec<ChipPin>,
        internal_schematic: CircuitDocument,
    ) -> Self {
        Self {
            name: name.into(),
            category: "Custom".to_string(),
            description: String::new(),
            pins,
            internal_schematic,
            package_3d: Package3DMetadata::default(),
            characterization: None,
        }
    }

    /// Convert this chip's internal schematic into a `tw_electrical::Subcircuit`.
    pub fn to_subcircuit(&self, registry: Option<&ChipRegistry>) -> Result<Subcircuit, SchematicError> {
        let ports: Vec<String> = self.pins.iter().map(|p| p.name.clone()).collect();
        let mut subcircuit = Subcircuit::new(self.name.clone(), ports);

        let nets = net::resolve(&self.internal_schematic)?;

        // Map each net to an internal node name in the subcircuit.
        // If a net is connected to an IoPin, name the node after that IoPin port.
        // If it is Ground, name it "GND".
        // Otherwise, name it "N<id>".
        let mut net_to_node_name: HashMap<NetId, String> = HashMap::new();

        for resolved in &nets.nets {
            if resolved.is_ground {
                net_to_node_name.insert(resolved.id, "GND".to_string());
            } else {
                net_to_node_name.insert(resolved.id, format!("N{}", resolved.id.0));
            }
        }

        // Override node names with declared external port names where IoPins are attached.
        for part in self.internal_schematic.parts() {
            if part.kind == PartKind::IoPin {
                let default_name = part.label.clone().unwrap_or_else(|| part.id.0.clone());
                let pin_name = part.str_param("name", &default_name).to_string();
                let pin_ref = crate::document::PinRef::new(part.id.clone(), "pin");
                if let Some(net_id) = nets.net_for_pin(&pin_ref) {
                    net_to_node_name.insert(net_id, pin_name);
                }
            }
        }

        let node_of = |part: &Part, pin: &str| -> Result<String, SchematicError> {
            let pin_ref = crate::document::PinRef::new(part.id.clone(), pin);
            let net_id = nets.net_for_pin(&pin_ref).ok_or_else(|| SchematicError::UnknownPin {
                part: part.id.clone(),
                pin: pin.to_string(),
            })?;
            net_to_node_name
                .get(&net_id)
                .cloned()
                .ok_or_else(|| SchematicError::UnknownPin {
                    part: part.id.clone(),
                    pin: pin.to_string(),
                })
        };

        for part in self.internal_schematic.parts() {
            match part.kind {
                PartKind::Resistor => {
                    let resistance = part.f64_param("resistance", 1000.0);
                    subcircuit.devices.push(DeviceTemplate::Resistor {
                        name: part.id.0.clone(),
                        pos: node_of(part, "a")?,
                        neg: node_of(part, "b")?,
                        resistance,
                    });
                }
                PartKind::Capacitor => {
                    let capacitance = part.f64_param("capacitance", 1e-6);
                    subcircuit.devices.push(DeviceTemplate::Capacitor {
                        name: part.id.0.clone(),
                        pos: node_of(part, "a")?,
                        neg: node_of(part, "b")?,
                        capacitance,
                    });
                }
                PartKind::VoltageSource => {
                    let voltage = part.f64_param("voltage", 5.0);
                    subcircuit.devices.push(DeviceTemplate::VoltageSource {
                        name: part.id.0.clone(),
                        pos: node_of(part, "pos")?,
                        neg: node_of(part, "neg")?,
                        voltage,
                    });
                }
                PartKind::CurrentSource => {
                    let current = part.f64_param("current", 0.001);
                    subcircuit.devices.push(DeviceTemplate::CurrentSource {
                        name: part.id.0.clone(),
                        pos: node_of(part, "pos")?,
                        neg: node_of(part, "neg")?,
                        current,
                    });
                }
                PartKind::Diode => {
                    subcircuit.devices.push(DeviceTemplate::Diode {
                        name: part.id.0.clone(),
                        pos: node_of(part, "anode")?,
                        neg: node_of(part, "cathode")?,
                        is_sat: 1e-14,
                        vt: 0.02585,
                        emission: 1.0,
                    });
                }
                PartKind::Nmos => {
                    let vth = part.f64_param("vth", 0.8);
                    let beta = part.f64_param("beta", 1e-3);
                    subcircuit.devices.push(DeviceTemplate::Mosfet {
                        name: part.id.0.clone(),
                        mos_type: MosfetType::Nmos,
                        drain: node_of(part, "drain")?,
                        gate: node_of(part, "gate")?,
                        source: node_of(part, "source")?,
                        vth,
                        beta,
                        lambda: 0.0,
                    });
                }
                PartKind::Pmos => {
                    let vth = part.f64_param("vth", 0.8);
                    let beta = part.f64_param("beta", 1e-3);
                    subcircuit.devices.push(DeviceTemplate::Mosfet {
                        name: part.id.0.clone(),
                        mos_type: MosfetType::Pmos,
                        drain: node_of(part, "drain")?,
                        gate: node_of(part, "gate")?,
                        source: node_of(part, "source")?,
                        vth,
                        beta,
                        lambda: 0.0,
                    });
                }
                PartKind::Switch => {
                    let closed = part.bool_param("closed", false);
                    let on_res = part.f64_param("on_resistance", 1e-3);
                    let off_res = part.f64_param("off_resistance", 1e9);
                    let resistance = if closed { on_res } else { off_res };
                    subcircuit.devices.push(DeviceTemplate::Resistor {
                        name: part.id.0.clone(),
                        pos: node_of(part, "a")?,
                        neg: node_of(part, "b")?,
                        resistance,
                    });
                }
                PartKind::Lamp => {
                    let resistance = part.f64_param("resistance", 100.0);
                    subcircuit.devices.push(DeviceTemplate::Resistor {
                        name: part.id.0.clone(),
                        pos: node_of(part, "a")?,
                        neg: node_of(part, "b")?,
                        resistance,
                    });
                }
                PartKind::Chip => {
                    let child_type = part.str_param("chip_name", "");
                    let Some(reg) = registry else {
                        return Err(SchematicError::InvalidDocument(format!(
                            "Cannot compile nested chip '{}': no registry provided",
                            child_type
                        )));
                    };
                    let child_chip = reg.get(child_type).ok_or_else(|| {
                        SchematicError::InvalidDocument(format!(
                            "Unknown chip definition '{}'",
                            child_type
                        ))
                    })?;
                    let child_sub = child_chip.to_subcircuit(Some(reg))?;
                    let mut bindings = Vec::new();
                    for pin in &child_chip.pins {
                        let local_node = node_of(part, &pin.name)?;
                        bindings.push((pin.name.clone(), local_node));
                    }
                    let bindings_ref: Vec<(&str, &str)> = bindings
                        .iter()
                        .map(|(p, n)| (p.as_str(), n.as_str()))
                        .collect();
                    subcircuit.add_subcircuit(part.id.0.clone(), child_sub, &bindings_ref);
                }
                PartKind::Ground | PartKind::Probe | PartKind::IoPin => {}
            }
        }

        Ok(subcircuit)
    }

    /// Serialize this definition to JSON.
    pub fn to_json_string(&self) -> Result<String, SchematicError> {
        serde_json::to_string_pretty(self).map_err(SchematicError::Json)
    }

    /// Deserialize a chip definition from JSON.
    pub fn from_json_str(json: &str) -> Result<Self, SchematicError> {
        serde_json::from_str(json).map_err(SchematicError::Json)
    }

    /// Save definition to file (typically `.twchip`).
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), SchematicError> {
        let json = self.to_json_string()?;
        fs::write(path, json).map_err(SchematicError::Io)
    }

    /// Load definition from file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, SchematicError> {
        let text = fs::read_to_string(path).map_err(SchematicError::Io)?;
        Self::from_json_str(&text)
    }
}

/// Registry of known chip definitions in a project or session.
#[derive(Debug, Clone, Default)]
pub struct ChipRegistry {
    chips: BTreeMap<String, ChipDefinition>,
}

impl ChipRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, chip: ChipDefinition) {
        self.chips.insert(chip.name.clone(), chip);
    }

    pub fn get(&self, name: &str) -> Option<&ChipDefinition> {
        self.chips.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.chips.contains_key(name)
    }

    pub fn list(&self) -> Vec<String> {
        self.chips.keys().cloned().collect()
    }

    pub fn all(&self) -> impl Iterator<Item = &ChipDefinition> {
        self.chips.values()
    }

    pub fn load_file(&mut self, path: impl AsRef<Path>) -> Result<(), SchematicError> {
        let chip = ChipDefinition::load(path)?;
        self.register(chip);
        Ok(())
    }

    pub fn load_directory(&mut self, dir: impl AsRef<Path>) -> Result<usize, SchematicError> {
        let dir = dir.as_ref();
        if !dir.is_dir() {
            return Ok(0);
        }
        let mut count = 0;
        for entry in fs::read_dir(dir).map_err(SchematicError::Io)? {
            let entry = entry.map_err(SchematicError::Io)?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "twchip") {
                if let Ok(chip) = ChipDefinition::load(&path) {
                    self.register(chip);
                    count += 1;
                }
            }
        }
        Ok(count)
    }
}
