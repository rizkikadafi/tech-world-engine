use std::collections::HashMap;

use crate::device::{Capacitor, CurrentSource, Diode, Mosfet, Resistor, VoltageSource};
use crate::error::CircuitError;
use crate::node::NodeId;

/// Container for an electrical circuit netlist.
#[derive(Debug, Clone, Default)]
pub struct Circuit {
    node_names: Vec<String>,
    name_to_node: HashMap<String, NodeId>,
    pub resistors: Vec<Resistor>,
    pub capacitors: Vec<Capacitor>,
    pub voltage_sources: Vec<VoltageSource>,
    pub current_sources: Vec<CurrentSource>,
    pub diodes: Vec<Diode>,
    pub mosfets: Vec<Mosfet>,
}

impl Circuit {
    /// Creates a new circuit initialized with reference Ground (GND / "0").
    pub fn new() -> Self {
        let mut circuit = Self {
            node_names: Vec::new(),
            name_to_node: HashMap::new(),
            resistors: Vec::new(),
            capacitors: Vec::new(),
            voltage_sources: Vec::new(),
            current_sources: Vec::new(),
            diodes: Vec::new(),
            mosfets: Vec::new(),
        };
        // Ground is node 0
        circuit.node_names.push("GND".to_string());
        circuit.name_to_node.insert("GND".to_string(), NodeId::GROUND);
        circuit.name_to_node.insert("0".to_string(), NodeId::GROUND);
        circuit.name_to_node.insert("gnd".to_string(), NodeId::GROUND);
        circuit
    }

    /// Returns the Ground node.
    pub fn ground(&self) -> NodeId {
        NodeId::GROUND
    }

    /// Registers a new node by name or returns an error if already registered.
    pub fn add_node(&mut self, name: &str) -> Result<NodeId, CircuitError> {
        if let Some(&node) = self.name_to_node.get(name) {
            return Err(CircuitError::InvalidElement(format!(
                "Node '{}' already exists as {:?}",
                name, node
            )));
        }
        let id = NodeId(self.node_names.len());
        self.node_names.push(name.to_string());
        self.name_to_node.insert(name.to_string(), id);
        Ok(id)
    }

    /// Retrieves an existing node by name or registers it if absent.
    pub fn get_or_add_node(&mut self, name: &str) -> NodeId {
        if let Some(&id) = self.name_to_node.get(name) {
            id
        } else {
            let id = NodeId(self.node_names.len());
            self.node_names.push(name.to_string());
            self.name_to_node.insert(name.to_string(), id);
            id
        }
    }

    /// Look up a node by name.
    pub fn node_by_name(&self, name: &str) -> Option<NodeId> {
        self.name_to_node.get(name).copied()
    }

    /// Look up the name of a node.
    pub fn node_name(&self, node: NodeId) -> Option<&str> {
        self.node_names.get(node.0).map(|s| s.as_str())
    }

    /// Total count of all nodes including Ground.
    pub fn node_count(&self) -> usize {
        self.node_names.len()
    }

    /// List of all non-ground nodes.
    pub fn non_ground_nodes(&self) -> Vec<NodeId> {
        (1..self.node_names.len()).map(NodeId).collect()
    }

    /// Adds a resistor to the circuit.
    pub fn add_resistor(
        &mut self,
        name: impl Into<String>,
        pos: NodeId,
        neg: NodeId,
        resistance: f64,
    ) -> &Resistor {
        self.resistors.push(Resistor::new(name, pos, neg, resistance));
        self.resistors.last().unwrap()
    }

    /// Adds a capacitor to the circuit.
    pub fn add_capacitor(
        &mut self,
        name: impl Into<String>,
        pos: NodeId,
        neg: NodeId,
        capacitance: f64,
    ) -> &Capacitor {
        self.capacitors.push(Capacitor::new(name, pos, neg, capacitance));
        self.capacitors.last().unwrap()
    }

    /// Adds an independent voltage source.
    pub fn add_voltage_source(
        &mut self,
        name: impl Into<String>,
        pos: NodeId,
        neg: NodeId,
        voltage: f64,
    ) -> &VoltageSource {
        self.voltage_sources
            .push(VoltageSource::new(name, pos, neg, voltage));
        self.voltage_sources.last().unwrap()
    }

    /// Adds an independent current source.
    pub fn add_current_source(
        &mut self,
        name: impl Into<String>,
        pos: NodeId,
        neg: NodeId,
        current: f64,
    ) -> &CurrentSource {
        self.current_sources
            .push(CurrentSource::new(name, pos, neg, current));
        self.current_sources.last().unwrap()
    }

    /// Adds a semiconductor diode.
    pub fn add_diode(
        &mut self,
        name: impl Into<String>,
        anode: NodeId,
        cathode: NodeId,
    ) -> &Diode {
        self.diodes.push(Diode::new(name, anode, cathode));
        self.diodes.last().unwrap()
    }

    /// Adds an NMOS transistor.
    pub fn add_nmos(
        &mut self,
        name: impl Into<String>,
        drain: NodeId,
        gate: NodeId,
        source: NodeId,
        vth: f64,
        beta: f64,
    ) -> &Mosfet {
        self.mosfets.push(Mosfet::new_nmos(name, drain, gate, source, vth, beta));
        self.mosfets.last().unwrap()
    }

    /// Adds a PMOS transistor.
    pub fn add_pmos(
        &mut self,
        name: impl Into<String>,
        drain: NodeId,
        gate: NodeId,
        source: NodeId,
        vth: f64,
        beta: f64,
    ) -> &Mosfet {
        self.mosfets.push(Mosfet::new_pmos(name, drain, gate, source, vth, beta));
        self.mosfets.last().unwrap()
    }

    /// Checks if the circuit contains nonlinear elements (diodes or MOSFETs).
    pub fn is_nonlinear(&self) -> bool {
        !self.diodes.is_empty() || !self.mosfets.is_empty()
    }
}
