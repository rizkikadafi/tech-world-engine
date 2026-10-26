use tw_num::linear::{Matrix, Vector};
use tw_num::linear::dense::solve;

use crate::circuit::Circuit;
use crate::error::CircuitError;
use crate::node::NodeId;
use crate::state::CircuitState;

/// Internal MNA system representation.
pub struct MnaSystem {
    dim: usize,
    num_nodes: usize,
    num_vsources: usize,
    matrix: Matrix,
    rhs: Vector,
}

impl MnaSystem {
    /// Builds the MNA system for DC operating point analysis.
    /// Capacitors are treated as open circuits (omitted).
    pub fn build_dc(circuit: &Circuit) -> Result<Self, CircuitError> {
        Self::build(circuit, None)
    }

    /// Builds the MNA system for a transient time step using Backward Euler.
    /// `dt`: time step size.
    /// `prev_state`: circuit state from the previous time step.
    pub fn build_transient(
        circuit: &Circuit,
        dt: f64,
        prev_state: &CircuitState,
    ) -> Result<Self, CircuitError> {
        Self::build(circuit, Some((dt, prev_state)))
    }

    fn build(
        circuit: &Circuit,
        transient_info: Option<(f64, &CircuitState)>,
    ) -> Result<Self, CircuitError> {
        let non_ground_nodes = circuit.non_ground_nodes();
        let num_nodes = non_ground_nodes.len();
        let num_vsources = circuit.voltage_sources.len();
        let dim = num_nodes + num_vsources;

        if dim == 0 {
            return Err(CircuitError::InvalidElement(
                "Circuit has no non-ground nodes or voltage sources to solve".to_string(),
            ));
        }

        let mut matrix = Matrix::zeros(dim, dim)?;
        let mut rhs = Vector::zeros(dim);

        // Helper to map NodeId to matrix row/col index (0..num_nodes)
        let node_idx = |node: NodeId| -> Option<usize> {
            if node.is_ground() {
                None
            } else {
                Some(node.id() - 1)
            }
        };

        // 1. Stamp Resistors
        for r in &circuit.resistors {
            let g = r.conductance();
            let p = node_idx(r.node_pos);
            let n = node_idx(r.node_neg);

            if let Some(i) = p {
                matrix[(i, i)] += g;
            }
            if let Some(j) = n {
                matrix[(j, j)] += g;
            }
            if let (Some(i), Some(j)) = (p, n) {
                matrix[(i, j)] -= g;
                matrix[(j, i)] -= g;
            }
        }

        // 2. Stamp Capacitors (Companion Model for Backward Euler if in transient mode)
        if let Some((dt, prev_state)) = transient_info {
            for c in &circuit.capacitors {
                let g_eq = c.capacitance / dt;
                let p = node_idx(c.node_pos);
                let n = node_idx(c.node_neg);

                if let Some(i) = p {
                    matrix[(i, i)] += g_eq;
                }
                if let Some(j) = n {
                    matrix[(j, j)] += g_eq;
                }
                if let (Some(i), Some(j)) = (p, n) {
                    matrix[(i, j)] -= g_eq;
                    matrix[(j, i)] -= g_eq;
                }

                // Equivalent current source: I_eq = G_eq * (V_pos(t_n) - V_neg(t_n))
                let v_prev = prev_state.voltage_diff(c.node_pos, c.node_neg);
                let i_eq = g_eq * v_prev;

                // Current enters pos node, leaves neg node
                if let Some(i) = p {
                    rhs[i] += i_eq;
                }
                if let Some(j) = n {
                    rhs[j] -= i_eq;
                }
            }
        }

        // 3. Stamp Independent Current Sources
        // Current flows from node_neg to node_pos (enters pos, leaves neg)
        for isrc in &circuit.current_sources {
            let p = node_idx(isrc.node_pos);
            let n = node_idx(isrc.node_neg);

            if let Some(i) = p {
                rhs[i] += isrc.current;
            }
            if let Some(j) = n {
                rhs[j] -= isrc.current;
            }
        }

        // 4. Stamp Independent Voltage Sources
        for (m, vsrc) in circuit.voltage_sources.iter().enumerate() {
            let branch_row = num_nodes + m;
            let p = node_idx(vsrc.node_pos);
            let n = node_idx(vsrc.node_neg);

            if let Some(i) = p {
                matrix[(i, branch_row)] += 1.0;
                matrix[(branch_row, i)] += 1.0;
            }
            if let Some(j) = n {
                matrix[(j, branch_row)] -= 1.0;
                matrix[(branch_row, j)] -= 1.0;
            }

            rhs[branch_row] += vsrc.voltage;
        }

        Ok(Self {
            dim,
            num_nodes,
            num_vsources,
            matrix,
            rhs,
        })
    }

    /// Solves the MNA system and maps the numerical solution back to a `CircuitState`.
    pub fn solve(&self, circuit: &Circuit) -> Result<CircuitState, CircuitError> {
        let x = solve(&self.matrix, &self.rhs)?;

        let mut state = CircuitState::new();

        // Map node voltages
        let non_ground_nodes = circuit.non_ground_nodes();
        for (idx, node) in non_ground_nodes.into_iter().enumerate() {
            state.set_voltage(node, x[idx]);
        }

        // Map voltage source branch currents
        for (m, vsrc) in circuit.voltage_sources.iter().enumerate() {
            let branch_idx = self.num_nodes + m;
            state.set_branch_current(&vsrc.name, x[branch_idx]);
        }

        Ok(state)
    }

    pub fn dimension(&self) -> usize {
        self.dim
    }

    pub fn num_nodes(&self) -> usize {
        self.num_nodes
    }

    pub fn num_vsources(&self) -> usize {
        self.num_vsources
    }

    pub fn matrix(&self) -> &Matrix {
        &self.matrix
    }

    pub fn rhs(&self) -> &Vector {
        &self.rhs
    }
}
