use crate::circuit::Circuit;
use crate::error::CircuitError;
use crate::mna::MnaSystem;
use crate::node::NodeId;
use crate::state::CircuitState;

/// Result of a transient simulation containing states at each time point.
#[derive(Debug, Clone)]
pub struct TransientResult {
    pub time_points: Vec<f64>,
    pub states: Vec<CircuitState>,
}

impl TransientResult {
    pub fn new() -> Self {
        Self {
            time_points: Vec::new(),
            states: Vec::new(),
        }
    }

    pub fn push(&mut self, t: f64, state: CircuitState) {
        self.time_points.push(t);
        self.states.push(state);
    }

    pub fn len(&self) -> usize {
        self.time_points.len()
    }

    pub fn is_empty(&self) -> bool {
        self.time_points.is_empty()
    }

    pub fn final_state(&self) -> Option<&CircuitState> {
        self.states.last()
    }

    /// Extracts the voltage waveform `(t, V)` for a given node.
    pub fn voltage_waveform(&self, node: NodeId) -> Vec<(f64, f64)> {
        self.time_points
            .iter()
            .zip(self.states.iter())
            .map(|(&t, state)| (t, state.voltage(node)))
            .collect()
    }

    /// Extracts the voltage waveform `(t, V)` for a node by name.
    pub fn voltage_waveform_by_name(&self, circuit: &Circuit, name: &str) -> Option<Vec<(f64, f64)>> {
        let node = circuit.node_by_name(name)?;
        Some(self.voltage_waveform(node))
    }
}

impl Default for TransientResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Solves the DC Operating Point (capacitors treated as open circuits).
/// Automatically uses Newton-Raphson if nonlinear components (Diodes, MOSFETs) are present.
pub fn dc_op(circuit: &Circuit) -> Result<CircuitState, CircuitError> {
    MnaSystem::solve_circuit(circuit, None, None)
}

/// Runs a transient analysis from t=0 to t_stop with fixed time step dt.
/// If `initial_state` is None, an initial state with zero voltages is used.
/// Automatically handles nonlinear devices across time steps.
pub fn transient(
    circuit: &Circuit,
    t_stop: f64,
    dt: f64,
    initial_state: Option<CircuitState>,
) -> Result<TransientResult, CircuitError> {
    if dt <= 0.0 {
        return Err(CircuitError::InvalidElement(format!(
            "Timestep dt must be positive, got {}",
            dt
        )));
    }
    if t_stop < dt {
        return Err(CircuitError::InvalidElement(format!(
            "t_stop ({}) must be >= dt ({})",
            t_stop, dt
        )));
    }

    let mut current_state = initial_state.unwrap_or_default();
    let mut result = TransientResult::new();

    // Record t=0
    result.push(0.0, current_state.clone());

    let steps = (t_stop / dt).round() as usize;
    for step in 1..=steps {
        let t = step as f64 * dt;
        current_state = MnaSystem::solve_circuit(
            circuit,
            Some((dt, &current_state)),
            Some(&current_state),
        )?;
        result.push(t, current_state.clone());
    }

    Ok(result)
}
