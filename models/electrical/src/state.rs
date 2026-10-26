use std::collections::HashMap;

use crate::circuit::Circuit;
use crate::node::NodeId;

/// State of an electrical circuit at a specific operating point or time step.
#[derive(Debug, Clone, PartialEq)]
pub struct CircuitState {
    node_voltages: HashMap<NodeId, f64>,
    branch_currents: HashMap<String, f64>,
}

impl CircuitState {
    pub fn new() -> Self {
        let mut state = Self {
            node_voltages: HashMap::new(),
            branch_currents: HashMap::new(),
        };
        // Ground is always 0V
        state.node_voltages.insert(NodeId::GROUND, 0.0);
        state
    }

    /// Sets the voltage for a given node.
    pub fn set_voltage(&mut self, node: NodeId, voltage: f64) {
        self.node_voltages.insert(node, voltage);
    }

    /// Sets the branch current for a given named voltage source.
    pub fn set_branch_current(&mut self, vsource_name: impl Into<String>, current: f64) {
        self.branch_currents.insert(vsource_name.into(), current);
    }

    /// Gets the voltage of a node. Returns 0.0 for Ground.
    pub fn voltage(&self, node: NodeId) -> f64 {
        if node.is_ground() {
            0.0
        } else {
            self.node_voltages.get(&node).copied().unwrap_or(0.0)
        }
    }

    /// Gets the voltage difference across two nodes: V(pos) - V(neg).
    pub fn voltage_diff(&self, pos: NodeId, neg: NodeId) -> f64 {
        self.voltage(pos) - self.voltage(neg)
    }

    /// Gets the voltage of a node by name.
    pub fn voltage_by_name(&self, circuit: &Circuit, name: &str) -> Option<f64> {
        let node = circuit.node_by_name(name)?;
        Some(self.voltage(node))
    }

    /// Gets the branch current through a named voltage source.
    pub fn current(&self, vsource_name: &str) -> Option<f64> {
        self.branch_currents.get(vsource_name).copied()
    }

    /// All node voltages.
    pub fn node_voltages(&self) -> &HashMap<NodeId, f64> {
        &self.node_voltages
    }

    /// All branch currents.
    pub fn branch_currents(&self) -> &HashMap<String, f64> {
        &self.branch_currents
    }
}

impl Default for CircuitState {
    fn default() -> Self {
        Self::new()
    }
}
