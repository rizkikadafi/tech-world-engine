//! Subcircuit abstraction for modular, hierarchical circuit composition.

use std::collections::{HashMap, HashSet};

use crate::circuit::Circuit;
use crate::device::MosfetType;
use crate::error::CircuitError;
use crate::node::NodeId;

/// Maximum nesting depth allowed for recursive subcircuit instantiation.
///
/// This is a guard against pathological templates; real designs rarely exceed
/// a handful of levels.
pub const MAX_SUBCIRCUIT_DEPTH: usize = 64;

/// Device template stored in a subcircuit definition before instantiation.
#[derive(Debug, Clone, PartialEq)]
pub enum DeviceTemplate {
    Resistor {
        name: String,
        pos: String,
        neg: String,
        resistance: f64,
    },
    Capacitor {
        name: String,
        pos: String,
        neg: String,
        capacitance: f64,
    },
    VoltageSource {
        name: String,
        pos: String,
        neg: String,
        voltage: f64,
    },
    CurrentSource {
        name: String,
        pos: String,
        neg: String,
        current: f64,
    },
    Diode {
        name: String,
        pos: String,
        neg: String,
        is_sat: f64,
        vt: f64,
        emission: f64,
    },
    Mosfet {
        name: String,
        mos_type: MosfetType,
        drain: String,
        gate: String,
        source: String,
        vth: f64,
        beta: f64,
        lambda: f64,
    },
}

/// A nested subcircuit instance stored in a parent template.
///
/// `bindings` maps the child's port names to node names in the parent's local
/// namespace. Those parent nodes may be parent ports, parent device terminals,
/// or dedicated internal nodes created by the parent.
#[derive(Debug, Clone, PartialEq)]
pub struct SubcircuitInstance {
    pub instance_name: String,
    pub template: Box<Subcircuit>,
    pub bindings: Vec<(String, String)>,
}

/// Reusable subcircuit template with defined external interface ports.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Subcircuit {
    pub name: String,
    pub ports: Vec<String>,
    pub devices: Vec<DeviceTemplate>,
    pub subcircuits: Vec<SubcircuitInstance>,
}

impl Subcircuit {
    /// Creates a new empty subcircuit template with declared external port names.
    pub fn new(
        name: impl Into<String>,
        ports: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            name: name.into(),
            ports: ports.into_iter().map(Into::into).collect(),
            devices: Vec::new(),
            subcircuits: Vec::new(),
        }
    }

    /// Adds a nested subcircuit instance to this template.
    ///
    /// `bindings` maps the child's ports to local node names in this template.
    /// For example, composing a NAND and an inverter into an AND gate:
    ///
    /// ```ignore
    /// sub.add_subcircuit(
    ///     "NAND1",
    ///     cmos_nand(config),
    ///     &[
    ///         ("A", "A"),
    ///         ("B", "B"),
    ///         ("OUT", "nand_out"),
    ///         ("VDD", "VDD"),
    ///         ("GND", "GND"),
    ///     ],
    /// );
    /// ```
    pub fn add_subcircuit(
        &mut self,
        instance_name: impl Into<String>,
        template: Subcircuit,
        bindings: &[(&str, &str)],
    ) -> &mut Self {
        self.subcircuits.push(SubcircuitInstance {
            instance_name: instance_name.into(),
            template: Box::new(template),
            bindings: bindings
                .iter()
                .map(|(port, node)| (port.to_string(), node.to_string()))
                .collect(),
        });
        self
    }

    /// Validates this template and all nested subcircuits.
    ///
    /// Checks for duplicate port names, duplicate/empty instance names,
    /// unknown or duplicate child-port bindings, and missing child-port
    /// bindings. It does not require parent-side node names to exist yet;
    /// unknown parent names are created as internal nodes during instantiation.
    pub fn validate(&self) -> Result<(), CircuitError> {
        self.validate_depth(0)
    }

    fn validate_depth(&self, depth: usize) -> Result<(), CircuitError> {
        if depth > MAX_SUBCIRCUIT_DEPTH {
            return Err(CircuitError::InvalidElement(format!(
                "Subcircuit '{}' exceeds maximum nesting depth of {}",
                self.name, MAX_SUBCIRCUIT_DEPTH
            )));
        }

        let mut port_names = HashSet::new();
        for port in &self.ports {
            if !port_names.insert(port.as_str()) {
                return Err(CircuitError::InvalidElement(format!(
                    "Subcircuit '{}': duplicate port '{}'",
                    self.name, port
                )));
            }
        }

        let mut instance_names = HashSet::new();
        for instance in &self.subcircuits {
            if instance.instance_name.is_empty() {
                return Err(CircuitError::InvalidElement(format!(
                    "Subcircuit '{}': subcircuit instance name cannot be empty",
                    self.name
                )));
            }
            if !instance_names.insert(instance.instance_name.as_str()) {
                return Err(CircuitError::InvalidElement(format!(
                    "Subcircuit '{}': duplicate subcircuit instance '{}'",
                    self.name, instance.instance_name
                )));
            }

            let child = &instance.template;
            let mut bound_ports = HashSet::new();
            for (child_port, _parent_node) in &instance.bindings {
                if !child.ports.iter().any(|port| port == child_port) {
                    return Err(CircuitError::InvalidElement(format!(
                        "Subcircuit '{}' instance '{}': child '{}' has no port '{}'",
                        self.name, instance.instance_name, child.name, child_port
                    )));
                }
                if !bound_ports.insert(child_port.as_str()) {
                    return Err(CircuitError::InvalidElement(format!(
                        "Subcircuit '{}' instance '{}': duplicate binding for child port '{}'",
                        self.name, instance.instance_name, child_port
                    )));
                }
            }

            for child_port in &child.ports {
                if !bound_ports.contains(child_port.as_str()) {
                    return Err(CircuitError::InvalidElement(format!(
                        "Subcircuit '{}' instance '{}': child '{}' port '{}' is not bound",
                        self.name, instance.instance_name, child.name, child_port
                    )));
                }
            }

            child.validate_depth(depth + 1)?;
        }

        Ok(())
    }

    /// Adds a resistor to the subcircuit template.
    pub fn add_resistor(
        &mut self,
        name: impl Into<String>,
        pos: impl Into<String>,
        neg: impl Into<String>,
        resistance: f64,
    ) -> &mut Self {
        self.devices.push(DeviceTemplate::Resistor {
            name: name.into(),
            pos: pos.into(),
            neg: neg.into(),
            resistance,
        });
        self
    }

    /// Adds a capacitor to the subcircuit template.
    pub fn add_capacitor(
        &mut self,
        name: impl Into<String>,
        pos: impl Into<String>,
        neg: impl Into<String>,
        capacitance: f64,
    ) -> &mut Self {
        self.devices.push(DeviceTemplate::Capacitor {
            name: name.into(),
            pos: pos.into(),
            neg: neg.into(),
            capacitance,
        });
        self
    }

    /// Adds a diode to the subcircuit template.
    pub fn add_diode(
        &mut self,
        name: impl Into<String>,
        pos: impl Into<String>,
        neg: impl Into<String>,
    ) -> &mut Self {
        self.devices.push(DeviceTemplate::Diode {
            name: name.into(),
            pos: pos.into(),
            neg: neg.into(),
            is_sat: 1e-14,
            vt: 0.02585,
            emission: 1.0,
        });
        self
    }

    /// Adds an NMOS transistor to the subcircuit template.
    pub fn add_nmos(
        &mut self,
        name: impl Into<String>,
        drain: impl Into<String>,
        gate: impl Into<String>,
        source: impl Into<String>,
        vth: f64,
        beta: f64,
    ) -> &mut Self {
        self.devices.push(DeviceTemplate::Mosfet {
            name: name.into(),
            mos_type: MosfetType::Nmos,
            drain: drain.into(),
            gate: gate.into(),
            source: source.into(),
            vth: vth.abs(),
            beta,
            lambda: 0.02,
        });
        self
    }

    /// Adds a PMOS transistor to the subcircuit template.
    pub fn add_pmos(
        &mut self,
        name: impl Into<String>,
        drain: impl Into<String>,
        gate: impl Into<String>,
        source: impl Into<String>,
        vth: f64,
        beta: f64,
    ) -> &mut Self {
        self.devices.push(DeviceTemplate::Mosfet {
            name: name.into(),
            mos_type: MosfetType::Pmos,
            drain: drain.into(),
            gate: gate.into(),
            source: source.into(),
            vth: -vth.abs(),
            beta,
            lambda: 0.02,
        });
        self
    }

    /// Instantiates this subcircuit into a target `Circuit`.
    ///
    /// - `instance_name`: Prefix for devices and internal nodes to prevent collision.
    /// - `port_bindings`: Mapping from template port names to caller's `NodeId`.
    ///
    /// Nested subcircuits are instantiated recursively, with their internal
    /// nodes and device names prefixed by `instance_name` plus the child
    /// instance name.
    pub fn instantiate(
        &self,
        circuit: &mut Circuit,
        instance_name: &str,
        port_bindings: &[(&str, NodeId)],
    ) -> Result<(), CircuitError> {
        self.validate()?;
        self.instantiate_depth(circuit, instance_name, port_bindings, 0)
    }

    fn instantiate_depth(
        &self,
        circuit: &mut Circuit,
        instance_name: &str,
        port_bindings: &[(&str, NodeId)],
        depth: usize,
    ) -> Result<(), CircuitError> {
        if depth > MAX_SUBCIRCUIT_DEPTH {
            return Err(CircuitError::InvalidElement(format!(
                "Subcircuit '{}' exceeds maximum nesting depth of {}",
                self.name, MAX_SUBCIRCUIT_DEPTH
            )));
        }

        let mut binding_map: HashMap<&str, NodeId> = HashMap::new();
        for &(port, node) in port_bindings {
            if binding_map.insert(port, node).is_some() {
                return Err(CircuitError::InvalidElement(format!(
                    "Subcircuit '{}' instance '{}': duplicate binding for port '{}'",
                    self.name, instance_name, port
                )));
            }
        }

        // Verify all declared ports are bound, and reject unknown bindings.
        for port in &self.ports {
            if !binding_map.contains_key(port.as_str()) {
                return Err(CircuitError::InvalidElement(format!(
                    "Subcircuit '{}' instance '{}': port '{}' is not bound",
                    self.name, instance_name, port
                )));
            }
        }
        for port in binding_map.keys() {
            if !self.ports.iter().any(|declared| declared == port) {
                return Err(CircuitError::InvalidElement(format!(
                    "Subcircuit '{}' instance '{}': unknown port '{}'",
                    self.name, instance_name, port
                )));
            }
        }

        // Map node names to NodeId
        let mut node_map: HashMap<String, NodeId> = HashMap::new();
        for &(port, node) in port_bindings {
            node_map.insert(port.to_string(), node);
        }
        node_map.insert("0".to_string(), NodeId::GROUND);
        node_map.insert("GND".to_string(), NodeId::GROUND);
        node_map.insert("gnd".to_string(), NodeId::GROUND);

        // Pre-discover all internal nodes and register them in circuit.
        for dev in &self.devices {
            let names: &[&str] = match dev {
                DeviceTemplate::Resistor { pos, neg, .. } => &[pos, neg],
                DeviceTemplate::Capacitor { pos, neg, .. } => &[pos, neg],
                DeviceTemplate::VoltageSource { pos, neg, .. } => &[pos, neg],
                DeviceTemplate::CurrentSource { pos, neg, .. } => &[pos, neg],
                DeviceTemplate::Diode { pos, neg, .. } => &[pos, neg],
                DeviceTemplate::Mosfet {
                    drain,
                    gate,
                    source,
                    ..
                } => &[drain, gate, source],
            };
            for &name in names {
                if !node_map.contains_key(name) {
                    let full_internal_name = format!("{}_{}", instance_name, name);
                    let id = circuit.get_or_add_node(&full_internal_name);
                    node_map.insert(name.to_string(), id);
                }
            }
        }

        // Parent-side nodes used by nested subcircuit bindings are internal
        // nodes of this template unless they are already ports/devices.
        for instance in &self.subcircuits {
            for (_child_port, parent_node) in &instance.bindings {
                if !node_map.contains_key(parent_node) {
                    let full_internal_name = format!("{}_{}", instance_name, parent_node);
                    let id = circuit.get_or_add_node(&full_internal_name);
                    node_map.insert(parent_node.clone(), id);
                }
            }
        }

        // Instantiate all devices
        for dev in &self.devices {
            match dev {
                DeviceTemplate::Resistor {
                    name,
                    pos,
                    neg,
                    resistance,
                } => {
                    let p = *node_map.get(pos).unwrap();
                    let n = *node_map.get(neg).unwrap();
                    let full_name = format!("{}_{}", instance_name, name);
                    circuit.add_resistor(full_name, p, n, *resistance);
                }
                DeviceTemplate::Capacitor {
                    name,
                    pos,
                    neg,
                    capacitance,
                } => {
                    let p = *node_map.get(pos).unwrap();
                    let n = *node_map.get(neg).unwrap();
                    let full_name = format!("{}_{}", instance_name, name);
                    circuit.add_capacitor(full_name, p, n, *capacitance);
                }
                DeviceTemplate::VoltageSource {
                    name,
                    pos,
                    neg,
                    voltage,
                } => {
                    let p = *node_map.get(pos).unwrap();
                    let n = *node_map.get(neg).unwrap();
                    let full_name = format!("{}_{}", instance_name, name);
                    circuit.add_voltage_source(full_name, p, n, *voltage);
                }
                DeviceTemplate::CurrentSource {
                    name,
                    pos,
                    neg,
                    current,
                } => {
                    let p = *node_map.get(pos).unwrap();
                    let n = *node_map.get(neg).unwrap();
                    let full_name = format!("{}_{}", instance_name, name);
                    circuit.add_current_source(full_name, p, n, *current);
                }
                DeviceTemplate::Diode {
                    name,
                    pos,
                    neg,
                    is_sat,
                    vt,
                    emission,
                } => {
                    let p = *node_map.get(pos).unwrap();
                    let n = *node_map.get(neg).unwrap();
                    let full_name = format!("{}_{}", instance_name, name);
                    let d =
                        crate::device::Diode::with_params(full_name, p, n, *is_sat, *vt, *emission);
                    circuit.diodes.push(d);
                }
                DeviceTemplate::Mosfet {
                    name,
                    mos_type,
                    drain,
                    gate,
                    source,
                    vth,
                    beta,
                    lambda,
                } => {
                    let d = *node_map.get(drain).unwrap();
                    let g = *node_map.get(gate).unwrap();
                    let s = *node_map.get(source).unwrap();
                    let full_name = format!("{}_{}", instance_name, name);
                    let mut m = match mos_type {
                        MosfetType::Nmos => {
                            crate::device::Mosfet::new_nmos(full_name, d, g, s, *vth, *beta)
                        }
                        MosfetType::Pmos => {
                            crate::device::Mosfet::new_pmos(full_name, d, g, s, *vth, *beta)
                        }
                    };
                    m.lambda = *lambda;
                    circuit.mosfets.push(m);
                }
            }
        }

        // Instantiate nested subcircuits recursively.
        for instance in &self.subcircuits {
            let child_instance_name = format!("{}_{}", instance_name, instance.instance_name);
            let child_bindings: Vec<(&str, NodeId)> = instance
                .bindings
                .iter()
                .map(|(child_port, parent_node)| {
                    (child_port.as_str(), *node_map.get(parent_node).unwrap())
                })
                .collect();
            instance.template.instantiate_depth(
                circuit,
                &child_instance_name,
                &child_bindings,
                depth + 1,
            )?;
        }

        Ok(())
    }
}
