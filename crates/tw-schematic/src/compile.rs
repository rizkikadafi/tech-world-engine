use std::collections::BTreeMap;

use tw_electrical::{dc_op, Circuit, CircuitError, CircuitState, NodeId};

use crate::chip::ChipRegistry;
use crate::document::{CircuitDocument, Part, PartId, PartKind, PinRef};
use crate::error::SchematicError;
use crate::net::{self, NetId, NetMap};

/// A document compiled into a `tw-electrical` netlist.
///
/// The compiled artifact is transient: it is rebuilt from the document after
/// every edit. Keep the document as the source of truth.
#[derive(Debug, Clone)]
pub struct CompiledCircuit {
    pub circuit: Circuit,
    pub nets: NetMap,
    pub pin_nodes: BTreeMap<PinRef, NodeId>,
    pub net_nodes: BTreeMap<NetId, NodeId>,
    pub probes: BTreeMap<PartId, NodeId>,
}

impl CompiledCircuit {
    pub fn pin_node(&self, pin: &PinRef) -> Option<NodeId> {
        self.pin_nodes.get(pin).copied()
    }

    pub fn net_node(&self, net: NetId) -> Option<NodeId> {
        self.net_nodes.get(&net).copied()
    }

    pub fn probe_node(&self, part: impl AsRef<str>) -> Option<NodeId> {
        self.probes
            .iter()
            .find(|(id, _)| id.0 == part.as_ref())
            .map(|(_, node)| *node)
    }

    pub fn probe_voltage(&self, state: &CircuitState, part: impl AsRef<str>) -> Option<f64> {
        self.probe_node(part).map(|node| state.voltage(node))
    }

    pub fn net_voltage(&self, state: &CircuitState, net: NetId) -> Option<f64> {
        self.net_node(net).map(|node| state.voltage(node))
    }

    pub fn pin_voltage(&self, state: &CircuitState, pin: &PinRef) -> Option<f64> {
        self.pin_node(pin).map(|node| state.voltage(node))
    }

    /// Solve the DC operating point of the compiled netlist.
    pub fn run_dc(&self) -> Result<CircuitState, CircuitError> {
        dc_op(&self.circuit)
    }
}

/// Compile a validated document into a `tw-electrical::Circuit`.
pub fn compile(document: &CircuitDocument) -> Result<CompiledCircuit, SchematicError> {
    compile_with_registry(document, None)
}

/// Compile a validated document into a `tw-electrical::Circuit` with a chip registry.
pub fn compile_with_registry(
    document: &CircuitDocument,
    registry: Option<&ChipRegistry>,
) -> Result<CompiledCircuit, SchematicError> {
    let errors: Vec<_> = document
        .validate()
        .into_iter()
        .filter(|diagnostic| diagnostic.is_error())
        .collect();
    if !errors.is_empty() {
        return Err(SchematicError::Validation(errors));
    }

    let nets = net::resolve(document)?;
    let mut circuit = Circuit::new();
    let mut net_nodes = BTreeMap::new();

    for resolved in &nets.nets {
        let node = if resolved.is_ground {
            NodeId::GROUND
        } else {
            circuit.get_or_add_node(&format!("N{}", resolved.id.0))
        };
        net_nodes.insert(resolved.id, node);
    }

    let mut pin_nodes = BTreeMap::new();
    for (pin, net_id) in &nets.pin_to_net {
        let node = *net_nodes
            .get(net_id)
            .ok_or_else(|| SchematicError::UnknownPin {
                part: pin.part.clone(),
                pin: pin.pin.clone(),
            })?;
        pin_nodes.insert(pin.clone(), node);
    }

    let mut parts: Vec<&Part> = document.parts().iter().collect();
    parts.sort_by(|left, right| left.id.cmp(&right.id));

    let mut probes = BTreeMap::new();
    for part in parts {
        compile_part(part, &pin_nodes, &mut circuit, &mut probes, registry)?;
    }

    Ok(CompiledCircuit {
        circuit,
        nets,
        pin_nodes,
        net_nodes,
        probes,
    })
}

fn compile_part(
    part: &Part,
    pin_nodes: &BTreeMap<PinRef, NodeId>,
    circuit: &mut Circuit,
    probes: &mut BTreeMap<PartId, NodeId>,
    registry: Option<&ChipRegistry>,
) -> Result<(), SchematicError> {
    let node = |pin: &str| node_for(pin_nodes, part, pin);

    match part.kind {
        PartKind::Resistor => {
            let resistance = part.f64_param("resistance", 1000.0);
            circuit.add_resistor(
                part.id.0.clone(),
                node("a")?,
                node("b")?,
                resistance,
            );
        }
        PartKind::Capacitor => {
            let capacitance = part.f64_param("capacitance", 1e-6);
            circuit.add_capacitor(
                part.id.0.clone(),
                node("a")?,
                node("b")?,
                capacitance,
            );
        }
        PartKind::VoltageSource => {
            let voltage = part.f64_param("voltage", 5.0);
            circuit.add_voltage_source(
                part.id.0.clone(),
                node("pos")?,
                node("neg")?,
                voltage,
            );
        }
        PartKind::CurrentSource => {
            let current = part.f64_param("current", 0.001);
            circuit.add_current_source(
                part.id.0.clone(),
                node("pos")?,
                node("neg")?,
                current,
            );
        }
        PartKind::Diode => {
            circuit.add_diode(part.id.0.clone(), node("anode")?, node("cathode")?);
        }
        PartKind::Nmos => {
            let vth = part.f64_param("vth", 0.8);
            let beta = part.f64_param("beta", 1e-3);
            circuit.add_nmos(
                part.id.0.clone(),
                node("drain")?,
                node("gate")?,
                node("source")?,
                vth,
                beta,
            );
        }
        PartKind::Pmos => {
            let vth = part.f64_param("vth", 0.8);
            let beta = part.f64_param("beta", 1e-3);
            circuit.add_pmos(
                part.id.0.clone(),
                node("drain")?,
                node("gate")?,
                node("source")?,
                vth,
                beta,
            );
        }
        PartKind::Lamp => {
            let resistance = part.f64_param("resistance", 100.0);
            circuit.add_resistor(
                part.id.0.clone(),
                node("a")?,
                node("b")?,
                resistance,
            );
        }
        PartKind::Switch => {
            let closed = part.bool_param("closed", false);
            let on_resistance = part.f64_param("on_resistance", 1e-3);
            let off_resistance = part.f64_param("off_resistance", 1e9);
            let resistance = if closed {
                on_resistance
            } else {
                off_resistance
            };
            circuit.add_resistor(
                part.id.0.clone(),
                node("a")?,
                node("b")?,
                resistance,
            );
        }
        PartKind::Ground | PartKind::IoPin => {}
        PartKind::Probe => {
            probes.insert(part.id.clone(), node("node")?);
        }
        PartKind::Chip => {
            let chip_name = part.str_param("chip_name", "");
            let Some(reg) = registry else {
                return Err(SchematicError::InvalidDocument(format!(
                    "Cannot compile chip '{}': no chip registry provided",
                    chip_name
                )));
            };
            let chip = reg.get(chip_name).ok_or_else(|| {
                SchematicError::InvalidDocument(format!("Unknown chip '{}'", chip_name))
            })?;
            let subcircuit = chip.to_subcircuit(Some(reg))?;
            let mut bindings = Vec::new();
            for pin in &chip.pins {
                let pin_node = node(&pin.name)?;
                bindings.push((pin.name.as_str(), pin_node));
            }
            subcircuit
                .instantiate(circuit, &part.id.0, &bindings)
                .map_err(SchematicError::Circuit)?;
        }
    }

    Ok(())
}

fn node_for(
    pin_nodes: &BTreeMap<PinRef, NodeId>,
    part: &Part,
    pin: &str,
) -> Result<NodeId, SchematicError> {
    pin_nodes
        .get(&PinRef::new(part.id.clone(), pin))
        .copied()
        .ok_or_else(|| SchematicError::UnknownPin {
            part: part.id.clone(),
            pin: pin.to_string(),
        })
}
