use tw_num::error::NumericalError;
use tw_num::ode::{evaluate_step_lte, StepControlConfig, StepDecision};

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
    pub fn voltage_waveform_by_name(
        &self,
        circuit: &Circuit,
        name: &str,
    ) -> Option<Vec<(f64, f64)>> {
        let node = circuit.node_by_name(name)?;
        Some(self.voltage_waveform(node))
    }
}

impl Default for TransientResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Integrates the energy delivered by an ideal voltage source over a transient.
///
/// The MNA convention makes the branch current of a source *negative* when the
/// source is delivering power to the circuit (see `resistor_networks.rs`).
/// Therefore the delivered energy is:
///
/// ```text
/// E = ∫ V_source · (−I_source) dt
/// ```
///
/// The integral uses the trapezoidal rule over the recorded time points, which
/// is exact for constant currents and second-order accurate for smooth ones.
pub fn integrate_source_energy(
    result: &TransientResult,
    source_name: &str,
    voltage: f64,
) -> Result<f64, CircuitError> {
    if result.len() < 2 {
        return Ok(0.0);
    }

    let mut energy = 0.0;
    for i in 1..result.len() {
        let dt = result.time_points[i] - result.time_points[i - 1];
        let i_prev = source_current(result, i - 1, source_name).ok_or_else(|| {
            CircuitError::InvalidElement(format!(
                "Voltage source '{}' has no branch current in transient result",
                source_name
            ))
        })?;
        let i_curr = source_current(result, i, source_name).ok_or_else(|| {
            CircuitError::InvalidElement(format!(
                "Voltage source '{}' has no branch current in transient result",
                source_name
            ))
        })?;
        energy += voltage * -(i_prev + i_curr) * 0.5 * dt;
    }

    Ok(energy)
}

/// Returns the source current at `index`, falling back to the nearest sample
/// that carries a branch current.
///
/// A transient started from `None` records a synthetic t=0 state without branch
/// currents. Using the first solved sample for that interval keeps the
/// trapezoidal integral exact for constant currents.
fn source_current(result: &TransientResult, index: usize, source_name: &str) -> Option<f64> {
    if let Some(current) = result.states[index].current(source_name) {
        return Some(current);
    }
    for state in result.states.iter().skip(index + 1) {
        if let Some(current) = state.current(source_name) {
            return Some(current);
        }
    }
    for state in result.states.iter().take(index).rev() {
        if let Some(current) = state.current(source_name) {
            return Some(current);
        }
    }
    None
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
        current_state =
            MnaSystem::solve_circuit(circuit, Some((dt, &current_state)), Some(&current_state))?;
        result.push(t, current_state.clone());
    }

    Ok(result)
}

/// Runs an adaptive transient analysis from t=0 to t_stop.
///
/// Dynamically adjusts the time step based on Local Truncation Error (LTE)
/// and handles non-convergence step cuts (dt / 8), modeled after `DCtran`
/// in `ngspice-47/src/spicelib/analysis/dctran.c`.
pub fn transient_adaptive(
    circuit: &Circuit,
    t_stop: f64,
    step_config: Option<StepControlConfig>,
    initial_state: Option<CircuitState>,
) -> Result<TransientResult, CircuitError> {
    let config = step_config.unwrap_or_default();
    if t_stop <= 0.0 {
        return Err(CircuitError::InvalidElement(format!(
            "t_stop must be positive, got {}",
            t_stop
        )));
    }

    let mut t = 0.0;
    let mut dt = config.dt_init.min(t_stop);
    let mut current_state = initial_state.unwrap_or_default();
    let mut prev_state: Option<CircuitState> = None;
    let mut dt_old: Option<f64> = None;
    let mut first_step = true;
    let mut result = TransientResult::new();

    // Record t=0
    result.push(0.0, current_state.clone());

    let nodes = circuit.non_ground_nodes();

    while t < t_stop {
        if t + dt > t_stop {
            dt = t_stop - t;
        }

        let t_cand = t + dt;

        // Attempt solve at candidate step
        match MnaSystem::solve_circuit(circuit, Some((dt, &current_state)), Some(&current_state)) {
            Err(_) => {
                // Non-convergence: cut timestep by 8 (ngspice DCtran convention)
                dt /= 8.0;
                if dt < config.dt_min {
                    return Err(CircuitError::Numerical(NumericalError::NoConvergence {
                        iterations: 0,
                        reason: format!(
                            "Timestep too small ({} < {}) at t={}",
                            dt, config.dt_min, t
                        ),
                    }));
                }
                continue;
            }
            Ok(candidate_state) => {
                if first_step {
                    first_step = false;
                    t = t_cand;
                    dt_old = Some(dt);
                    prev_state = Some(current_state);
                    current_state = candidate_state;
                    result.push(t, current_state.clone());
                    continue;
                }

                // Evaluate LTE across all non-ground nodes
                let cand_vec: Vec<f64> =
                    nodes.iter().map(|&n| candidate_state.voltage(n)).collect();
                let curr_vec: Vec<f64> = nodes.iter().map(|&n| current_state.voltage(n)).collect();
                let prev_vec: Option<Vec<f64>> = prev_state
                    .as_ref()
                    .map(|ps| nodes.iter().map(|&n| ps.voltage(n)).collect());

                let decision = evaluate_step_lte(
                    &cand_vec,
                    &curr_vec,
                    prev_vec.as_deref(),
                    dt,
                    dt_old,
                    &config,
                );

                match decision {
                    StepDecision::Reject { retry_dt } => {
                        dt = retry_dt;
                        if dt < config.dt_min {
                            return Err(CircuitError::Numerical(NumericalError::NoConvergence {
                                iterations: 0,
                                reason: format!(
                                    "Timestep too small in LTE rejection ({} < {}) at t={}",
                                    dt, config.dt_min, t
                                ),
                            }));
                        }
                        continue;
                    }
                    StepDecision::Accept { next_dt } => {
                        t = t_cand;
                        dt_old = Some(dt);
                        prev_state = Some(current_state);
                        current_state = candidate_state;
                        result.push(t, current_state.clone());
                        dt = next_dt;
                    }
                }
            }
        }
    }

    Ok(result)
}
