use crate::node::NodeId;

/// Linear two-terminal resistor: V = I * R
#[derive(Debug, Clone, PartialEq)]
pub struct Resistor {
    pub name: String,
    pub node_pos: NodeId,
    pub node_neg: NodeId,
    pub resistance: f64,
}

impl Resistor {
    pub fn new(name: impl Into<String>, pos: NodeId, neg: NodeId, resistance: f64) -> Self {
        Self {
            name: name.into(),
            node_pos: pos,
            node_neg: neg,
            resistance,
        }
    }

    /// Conductance G = 1 / R
    #[inline]
    pub fn conductance(&self) -> f64 {
        1.0 / self.resistance
    }
}

/// Linear two-terminal capacitor: I = C * dV/dt
#[derive(Debug, Clone, PartialEq)]
pub struct Capacitor {
    pub name: String,
    pub node_pos: NodeId,
    pub node_neg: NodeId,
    pub capacitance: f64,
}

impl Capacitor {
    pub fn new(name: impl Into<String>, pos: NodeId, neg: NodeId, capacitance: f64) -> Self {
        Self {
            name: name.into(),
            node_pos: pos,
            node_neg: neg,
            capacitance,
        }
    }
}

/// Independent ideal voltage source: V_pos - V_neg = V
#[derive(Debug, Clone, PartialEq)]
pub struct VoltageSource {
    pub name: String,
    pub node_pos: NodeId,
    pub node_neg: NodeId,
    pub voltage: f64,
}

impl VoltageSource {
    pub fn new(name: impl Into<String>, pos: NodeId, neg: NodeId, voltage: f64) -> Self {
        Self {
            name: name.into(),
            node_pos: pos,
            node_neg: neg,
            voltage,
        }
    }
}

/// Independent ideal current source flowing from node_neg to node_pos
#[derive(Debug, Clone, PartialEq)]
pub struct CurrentSource {
    pub name: String,
    pub node_pos: NodeId,
    pub node_neg: NodeId,
    pub current: f64,
}

impl CurrentSource {
    pub fn new(name: impl Into<String>, pos: NodeId, neg: NodeId, current: f64) -> Self {
        Self {
            name: name.into(),
            node_pos: pos,
            node_neg: neg,
            current,
        }
    }
}
