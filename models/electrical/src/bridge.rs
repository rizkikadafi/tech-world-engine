//! Mixed-signal bridge: expose an analog circuit as a discrete component.
//!
//! [`CircuitComponent`] implements [`tw_core::Component`], so it can be
//! registered directly in a `tw-sim` simulation. Discrete `PortSignal` events
//! drive voltage sources inside the circuit; the transient solver advances the
//! analog state; and sampled output nodes emit `PortSignal` events back to the
//! discrete world.

use std::collections::HashSet;

use tw_core::{
    Component, ComponentId, Event, EventPayload, Port, PortDirection, PortId, SimulationContext,
    SimulationTime, State, StateValue,
};

use crate::analysis::{dc_op, integrate_source_energy, transient};
use crate::circuit::Circuit;
use crate::error::CircuitError;
use crate::node::NodeId;
use crate::state::CircuitState;
use crate::time_base::TimeBase;

const ADVANCE_EVENT: &str = "circuit-component-advance";
const MOUNT_EVENT: &str = "circuit-component-emit-initial";

/// Settling window used when a DC operating point cannot be found.
///
/// Sequential circuits such as a master-slave DFF contain cross-coupled
/// latches: at DC the loop is bistable and Newton-Raphson may fail to choose a
/// state. A short transient from the zero state resolves it deterministically.
const STARTUP_SECONDS: f64 = 10e-9;

/// Maps a discrete input port to a voltage source inside the circuit.
#[derive(Debug, Clone, PartialEq)]
pub struct BridgeInput {
    /// Discrete port id.
    pub port_id: PortId,
    /// Human-readable port name.
    pub name: String,
    /// Name of the voltage source driven by this input.
    pub source_name: String,
    /// Source voltage representing logic low (V).
    pub low_voltage: f64,
    /// Source voltage representing logic high (V).
    pub high_voltage: f64,
}

impl BridgeInput {
    /// Creates an input binding.
    pub fn new(
        port_id: PortId,
        name: impl Into<String>,
        source_name: impl Into<String>,
        low_voltage: f64,
        high_voltage: f64,
    ) -> Self {
        Self {
            port_id,
            name: name.into(),
            source_name: source_name.into(),
            low_voltage,
            high_voltage,
        }
    }

    fn voltage_for(&self, value: bool) -> f64 {
        if value {
            self.high_voltage
        } else {
            self.low_voltage
        }
    }
}

/// Maps a circuit node to a discrete output port.
#[derive(Debug, Clone, PartialEq)]
pub struct BridgeOutput {
    /// Discrete port id.
    pub port_id: PortId,
    /// Human-readable port name.
    pub name: String,
    /// Circuit node sampled by this output.
    pub node: NodeId,
    /// Voltage above which the output is considered logic high (V).
    pub threshold: f64,
}

impl BridgeOutput {
    /// Creates an output binding.
    pub fn new(port_id: PortId, name: impl Into<String>, node: NodeId, threshold: f64) -> Self {
        Self {
            port_id,
            name: name.into(),
            node,
            threshold,
        }
    }
}

/// Configuration for a [`CircuitComponent`].
#[derive(Debug, Clone, PartialEq)]
pub struct BridgeConfig {
    /// Physical duration of one simulation tick.
    pub time_base: TimeBase,
    /// Transient timestep used inside each advance (s).
    pub solver_dt: f64,
    /// How often the analog circuit is advanced and sampled, in ticks.
    pub sync_interval_ticks: u64,
    /// Optional supply source whose energy is accumulated.
    pub supply_source: Option<String>,
    /// Discrete inputs wired to circuit voltage sources.
    pub inputs: Vec<BridgeInput>,
    /// Circuit nodes exposed as discrete outputs.
    pub outputs: Vec<BridgeOutput>,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            time_base: TimeBase::default(),
            solver_dt: 5e-11,
            sync_interval_ticks: 1,
            supply_source: None,
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }
}

impl BridgeConfig {
    /// Validates configuration values that do not depend on the circuit.
    pub fn validate(&self) -> Result<(), CircuitError> {
        self.time_base.validate()?;
        if !self.solver_dt.is_finite() || self.solver_dt <= 0.0 {
            return Err(CircuitError::InvalidElement(format!(
                "BridgeConfig solver_dt must be finite and positive, got {}",
                self.solver_dt
            )));
        }
        if self.sync_interval_ticks == 0 {
            return Err(CircuitError::InvalidElement(
                "BridgeConfig sync_interval_ticks must be at least 1".to_string(),
            ));
        }
        let mut seen = HashSet::new();
        for input in &self.inputs {
            if !seen.insert(input.port_id) {
                return Err(CircuitError::InvalidElement(format!(
                    "BridgeConfig declares duplicate port id {:?}",
                    input.port_id
                )));
            }
        }
        for output in &self.outputs {
            if !seen.insert(output.port_id) {
                return Err(CircuitError::InvalidElement(format!(
                    "BridgeConfig declares duplicate port id {:?}",
                    output.port_id
                )));
            }
        }
        Ok(())
    }
}

/// A `tw_core::Component` backed by an analog circuit.
#[derive(Debug)]
pub struct CircuitComponent {
    id: ComponentId,
    name: String,
    ports: Vec<Port>,
    state: State,
    circuit: Circuit,
    config: BridgeConfig,
    analog_state: CircuitState,
    input_values: Vec<bool>,
    output_values: Vec<bool>,
    last_advance_tick: u64,
    energy_joules: f64,
    ticks_advanced: u64,
}

impl CircuitComponent {
    /// Wraps a circuit in a discrete component.
    ///
    /// Input sources are initialized to their low voltage and a DC operating
    /// point is solved to establish the initial analog state. If the circuit is
    /// bistable and the DC solve does not converge (for example a master-slave
    /// flip-flop), a short startup transient settles the analog state instead.
    pub fn new(
        id: ComponentId,
        name: impl Into<String>,
        mut circuit: Circuit,
        config: BridgeConfig,
    ) -> Result<Self, CircuitError> {
        let name = name.into();
        config.validate()?;

        for input in &config.inputs {
            if circuit.voltage_source(&input.source_name).is_none() {
                return Err(CircuitError::InvalidElement(format!(
                    "BridgeInput '{}' references missing voltage source '{}'",
                    input.name, input.source_name
                )));
            }
        }
        if let Some(source_name) = &config.supply_source {
            if circuit.voltage_source(source_name).is_none() {
                return Err(CircuitError::InvalidElement(format!(
                    "BridgeConfig supply_source references missing voltage source '{}'",
                    source_name
                )));
            }
        }
        for output in &config.outputs {
            if output.node.id() >= circuit.node_count() {
                return Err(CircuitError::InvalidElement(format!(
                    "BridgeOutput '{}' references unknown node {:?}",
                    output.name, output.node
                )));
            }
        }

        for input in &config.inputs {
            circuit.set_voltage_source(&input.source_name, input.low_voltage)?;
        }
        let analog_state = match dc_op(&circuit) {
            Ok(state) => state,
            Err(err) => {
                log::warn!(
                    "CircuitComponent '{}' DC operating point failed ({}); \
                     falling back to a {:.1} ns startup transient",
                    name,
                    err,
                    STARTUP_SECONDS * 1e9
                );
                let dt = config.solver_dt.min(STARTUP_SECONDS);
                let result = transient(&circuit, STARTUP_SECONDS, dt, None)?;
                result.final_state().cloned().ok_or_else(|| {
                    CircuitError::InvalidElement("startup transient produced no state".to_string())
                })?
            }
        };

        let mut ports = Vec::with_capacity(config.inputs.len() + config.outputs.len());
        for input in &config.inputs {
            ports.push(Port::new(
                input.port_id,
                input.name.clone(),
                PortDirection::Input,
            ));
        }
        for output in &config.outputs {
            ports.push(Port::new(
                output.port_id,
                output.name.clone(),
                PortDirection::Output,
            ));
        }

        let input_values = vec![false; config.inputs.len()];
        let output_values: Vec<bool> = config
            .outputs
            .iter()
            .map(|output| analog_state.voltage(output.node) > output.threshold)
            .collect();

        let mut state = State::new();
        for (index, value) in input_values.iter().enumerate() {
            state.set(format!("input_{index}"), *value);
        }
        for (index, value) in output_values.iter().enumerate() {
            state.set(format!("output_{index}"), *value);
            state.set(
                format!("output_voltage_{index}"),
                analog_state.voltage(config.outputs[index].node),
            );
        }
        state.set("energy_joules", 0.0);
        state.set("ticks_advanced", 0u64);
        state.set("last_error", "");

        Ok(Self {
            id,
            name,
            ports,
            state,
            circuit,
            config,
            analog_state,
            input_values,
            output_values,
            last_advance_tick: 0,
            energy_joules: 0.0,
            ticks_advanced: 0,
        })
    }

    /// The wrapped circuit.
    pub fn circuit(&self) -> &Circuit {
        &self.circuit
    }

    /// Bridge configuration.
    pub fn config(&self) -> &BridgeConfig {
        &self.config
    }

    /// Latest analog state.
    pub fn analog_state(&self) -> &CircuitState {
        &self.analog_state
    }

    /// Energy drawn from the supply so far (J).
    pub fn energy_joules(&self) -> f64 {
        self.energy_joules
    }

    fn advance_to(&mut self, target_tick: u64, now: SimulationTime) -> Vec<Event> {
        if target_tick <= self.last_advance_tick {
            return Vec::new();
        }

        let elapsed_ticks = target_tick - self.last_advance_tick;
        let elapsed_seconds = self.config.time_base.ticks_to_seconds(elapsed_ticks);
        let dt = self.config.solver_dt.min(elapsed_seconds);

        match transient(
            &self.circuit,
            elapsed_seconds,
            dt,
            Some(self.analog_state.clone()),
        ) {
            Ok(result) => {
                if let Some(source_name) = &self.config.supply_source {
                    if let Some(source) = self.circuit.voltage_source(source_name) {
                        match integrate_source_energy(&result, source_name, source.voltage) {
                            Ok(energy) => {
                                self.energy_joules += energy;
                                self.state.set("energy_joules", self.energy_joules);
                            }
                            Err(err) => {
                                log::warn!(
                                    "CircuitComponent '{}' energy integration failed at t={}: {}",
                                    self.name,
                                    now.ticks(),
                                    err
                                );
                                self.state.set("last_error", err.to_string());
                            }
                        }
                    }
                }

                if let Some(final_state) = result.final_state() {
                    self.analog_state = final_state.clone();
                }
                self.last_advance_tick = target_tick;
                self.ticks_advanced += elapsed_ticks;
                self.state.set("ticks_advanced", self.ticks_advanced);
                self.state.set("last_error", "");
                self.sample_outputs(now)
            }
            Err(err) => {
                log::warn!(
                    "CircuitComponent '{}' transient failed at t={}: {}",
                    self.name,
                    now.ticks(),
                    err
                );
                self.state.set("last_error", err.to_string());
                self.last_advance_tick = target_tick;
                self.ticks_advanced += elapsed_ticks;
                self.state.set("ticks_advanced", self.ticks_advanced);
                Vec::new()
            }
        }
    }

    fn sample_outputs(&mut self, now: SimulationTime) -> Vec<Event> {
        let mut events = Vec::new();
        for (index, output) in self.config.outputs.iter().enumerate() {
            let voltage = self.analog_state.voltage(output.node);
            self.state.set(format!("output_voltage_{index}"), voltage);
            let value = voltage > output.threshold;
            if value == self.output_values[index] {
                continue;
            }
            self.output_values[index] = value;
            self.state.set(format!("output_{index}"), value);
            events.push(Event::at(
                now,
                self.id,
                self.id,
                EventPayload::PortSignal {
                    port_id: output.port_id,
                    value: StateValue::Bool(value),
                },
            ));
        }
        events
    }
}

impl Component for CircuitComponent {
    fn id(&self) -> ComponentId {
        self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn ports(&self) -> &[Port] {
        &self.ports
    }

    fn state(&self) -> &State {
        &self.state
    }

    fn handle_event(&mut self, event: &Event, _ctx: &mut SimulationContext) -> Vec<Event> {
        match &event.payload {
            EventPayload::PortSignal { port_id, value } => {
                let Some(index) = self
                    .config
                    .inputs
                    .iter()
                    .position(|input| input.port_id == *port_id)
                else {
                    return Vec::new();
                };
                let Some(new_value) = value.as_bool() else {
                    return Vec::new();
                };
                if self.input_values[index] == new_value {
                    return Vec::new();
                }

                // Integrate the elapsed interval with the old input values
                // before applying the new one, so no physical time is skipped.
                let mut events = self.advance_to(event.time.ticks(), event.time);

                self.input_values[index] = new_value;
                self.state.set(format!("input_{index}"), new_value);
                let voltage = self.config.inputs[index].voltage_for(new_value);
                if let Err(err) = self
                    .circuit
                    .set_voltage_source(&self.config.inputs[index].source_name, voltage)
                {
                    log::warn!(
                        "CircuitComponent '{}' failed to drive source '{}': {}",
                        self.name,
                        self.config.inputs[index].source_name,
                        err
                    );
                    self.state.set("last_error", err.to_string());
                }

                events.extend(self.sample_outputs(event.time));
                events
            }
            EventPayload::Custom(payload) if payload == MOUNT_EVENT => self
                .config
                .outputs
                .iter()
                .enumerate()
                .map(|(index, output)| {
                    Event::at(
                        event.time,
                        self.id,
                        self.id,
                        EventPayload::PortSignal {
                            port_id: output.port_id,
                            value: StateValue::Bool(self.output_values[index]),
                        },
                    )
                })
                .collect(),
            EventPayload::Custom(payload) if payload == ADVANCE_EVENT => {
                let mut events = self.advance_to(event.time.ticks(), event.time);
                let next =
                    SimulationTime::new(event.time.ticks() + self.config.sync_interval_ticks);
                events.push(Event::at(
                    next,
                    self.id,
                    self.id,
                    EventPayload::Custom(ADVANCE_EVENT.to_string()),
                ));
                events
            }
            _ => Vec::new(),
        }
    }

    fn on_mount(&mut self, ctx: &mut SimulationContext) -> Vec<Event> {
        self.last_advance_tick = ctx.time().ticks();

        // Mount events bypass `PortSignal` routing, so emit the initial values
        // through a self-targeted `Custom` event handled by `handle_event`.
        vec![
            Event::at(
                ctx.time(),
                self.id,
                self.id,
                EventPayload::Custom(MOUNT_EVENT.to_string()),
            ),
            Event::at(
                SimulationTime::new(ctx.time().ticks() + self.config.sync_interval_ticks),
                self.id,
                self.id,
                EventPayload::Custom(ADVANCE_EVENT.to_string()),
            ),
        ]
    }
}
