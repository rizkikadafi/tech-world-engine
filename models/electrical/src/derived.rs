//! Derived discrete components generated from physical measurements.
//!
//! A [`DerivedGate`] implements [`tw_core::Component`] using the truth table and
//! timing measured by the [`Characterizer`](crate::characterization::Characterizer).
//! It is intentionally *not* a behavioral gate definition: the logic function is
//! whatever the transistor circuit did at DC, and the delay is the measured
//! propagation delay quantized to the simulation [`TimeBase`].

use tw_core::{
    Component, ComponentId, Event, EventPayload, Port, PortDirection, PortId, SimulationContext,
    SimulationTime, State, StateValue,
};

use crate::characterization::{
    GateCharacterization, GateKind, GenericCircuitCharacterization, LogicLevels, TimingMetrics,
    TruthTable,
};
use crate::error::CircuitError;
use crate::time_base::TimeBase;

const OUTPUT_EVENT_PREFIX: &str = "derived-gate-output:";
const MOUNT_EVENT: &str = "derived-gate-emit-initial";

/// Errors produced while deriving a component from a characterization.
#[derive(Debug, thiserror::Error)]
pub enum DerivedError {
    /// The supplied time base cannot convert seconds to ticks.
    #[error("invalid time base: {0}")]
    InvalidTimeBase(String),
    /// The gate kind has no combinational derived implementation yet.
    #[error("gate kind {0:?} is not supported by DerivedGate")]
    UnsupportedKind(GateKind),
    /// The measured truth table does not match the gate's input count.
    #[error("truth table for {inputs} inputs needs {expected} entries, got {actual}")]
    InvalidTruthTable {
        /// Number of inputs advertised by the gate kind.
        inputs: usize,
        /// Required number of entries.
        expected: usize,
        /// Actual number of entries.
        actual: usize,
    },
    /// A circuit-level error escaped characterization.
    #[error("circuit error: {0}")]
    Circuit(#[from] CircuitError),
}

/// Propagation delays quantized to whole simulation ticks.
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimingTicks {
    /// High-to-low delay in ticks.
    pub t_pHL: u64,
    /// Low-to-high delay in ticks.
    pub t_pLH: u64,
}

impl TimingTicks {
    /// Quantizes measured seconds into ticks, always producing a causal delay.
    pub fn from_metrics(metrics: &TimingMetrics, time_base: TimeBase) -> Self {
        Self {
            t_pHL: time_base.delay_ticks(metrics.t_pHL),
            t_pLH: time_base.delay_ticks(metrics.t_pLH),
        }
    }

    /// Delay for an output transition in the given direction.
    pub fn delay_for(&self, rising: bool) -> u64 {
        if rising {
            self.t_pLH
        } else {
            self.t_pHL
        }
    }
}

/// A discrete gate whose logic and timing were measured from a physical circuit.
#[derive(Debug)]
pub struct DerivedGate {
    id: ComponentId,
    name: String,
    ports: Vec<Port>,
    state: State,
    truth_table: TruthTable,
    levels: LogicLevels,
    metrics: TimingMetrics,
    time_base: TimeBase,
    timing: TimingTicks,
    input_ports: Vec<PortId>,
    output_port: PortId,
    inputs: Vec<bool>,
    output: bool,
    pending: bool,
    pending_output: bool,
    generation: u64,
}

impl DerivedGate {
    /// Builds a discrete component from a measured characterization.
    pub fn from_characterization(
        id: ComponentId,
        name: impl Into<String>,
        characterization: &GateCharacterization,
        time_base: TimeBase,
    ) -> Result<Self, DerivedError> {
        time_base
            .validate()
            .map_err(|err| DerivedError::InvalidTimeBase(err.to_string()))?;

        let input_count = characterization.kind.inputs();
        match characterization.kind {
            GateKind::Inverter | GateKind::Nand2 => {}
            other => return Err(DerivedError::UnsupportedKind(other)),
        }

        let expected = 1usize << input_count;
        if characterization.truth_table.inputs != input_count
            || characterization.truth_table.outputs.len() != expected
        {
            return Err(DerivedError::InvalidTruthTable {
                inputs: input_count,
                expected,
                actual: characterization.truth_table.outputs.len(),
            });
        }

        let mut ports = Vec::with_capacity(input_count + 1);
        let mut input_ports = Vec::with_capacity(input_count);
        for (index, port_name) in characterization
            .kind
            .input_names()
            .iter()
            .take(input_count)
            .enumerate()
        {
            let port_id = PortId(index as u64 + 1);
            ports.push(Port::new(port_id, *port_name, PortDirection::Input));
            input_ports.push(port_id);
        }

        let output_port = PortId(input_count as u64 + 1);
        ports.push(Port::new(
            output_port,
            characterization.kind.output_name(),
            PortDirection::Output,
        ));

        let inputs = vec![false; input_count];
        let output = characterization
            .truth_table
            .output_for(&inputs)
            .unwrap_or(false);
        let timing = TimingTicks::from_metrics(&characterization.timing, time_base);

        let mut state = State::new();
        state.set("output", output);
        state.set("pending", false);
        state.set("generation", 0u64);
        state.set("delay_ticks", 0u64);
        state.set("t_pHL_ticks", timing.t_pHL);
        state.set("t_pLH_ticks", timing.t_pLH);
        state.set("t_pHL_seconds", characterization.timing.t_pHL);
        state.set("t_pLH_seconds", characterization.timing.t_pLH);
        state.set("t_rise_seconds", characterization.timing.t_rise);
        state.set("t_fall_seconds", characterization.timing.t_fall);
        state.set(
            "energy_per_cycle_joules",
            characterization.timing.energy_per_cycle,
        );
        state.set("logic_low_volts", characterization.levels.low);
        state.set("logic_high_volts", characterization.levels.high);
        for (index, value) in inputs.iter().enumerate() {
            state.set(format!("input_{index}"), *value);
        }

        Ok(Self {
            id,
            name: name.into(),
            ports,
            state,
            truth_table: characterization.truth_table.clone(),
            levels: characterization.levels,
            metrics: characterization.timing.clone(),
            time_base,
            timing,
            input_ports,
            output_port,
            inputs,
            output,
            pending: false,
            pending_output: output,
            generation: 0,
        })
    }

    /// Measured truth table backing this component.
    pub fn truth_table(&self) -> &TruthTable {
        &self.truth_table
    }

    /// Quantized timing used by the component.
    pub fn timing_ticks(&self) -> TimingTicks {
        self.timing
    }

    /// Raw measured metrics.
    pub fn metrics(&self) -> &TimingMetrics {
        &self.metrics
    }

    /// Measured logic levels.
    pub fn levels(&self) -> LogicLevels {
        self.levels
    }

    /// Time base used for quantization.
    pub fn time_base(&self) -> TimeBase {
        self.time_base
    }

    /// Port id of an input by index.
    pub fn input_port(&self, index: usize) -> Option<PortId> {
        self.input_ports.get(index).copied()
    }

    /// Port id of the output.
    pub fn output_port(&self) -> PortId {
        self.output_port
    }

    /// Current committed output value.
    pub fn output(&self) -> bool {
        self.output
    }

    fn commit_pending(&mut self, event_time: SimulationTime) -> Vec<Event> {
        self.output = self.pending_output;
        self.pending = false;
        self.state.set("output", self.output);
        self.state.set("pending", false);
        vec![Event::at(
            event_time,
            self.id,
            self.id,
            EventPayload::PortSignal {
                port_id: self.output_port,
                value: StateValue::Bool(self.output),
            },
        )]
    }
}

impl Component for DerivedGate {
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
                let Some(index) = self.input_ports.iter().position(|id| id == port_id) else {
                    return Vec::new();
                };
                let Some(new_value) = value.as_bool() else {
                    return Vec::new();
                };
                if self.inputs[index] == new_value {
                    return Vec::new();
                }

                self.inputs[index] = new_value;
                self.state.set(format!("input_{index}"), new_value);

                let desired = match self.truth_table.output_for(&self.inputs) {
                    Some(value) => value,
                    None => return Vec::new(),
                };

                if desired == self.output {
                    // The input returned before the pending edge committed:
                    // cancel the stale event (inertial delay).
                    if self.pending {
                        self.generation = self.generation.wrapping_add(1);
                        self.pending = false;
                        self.state.set("generation", self.generation);
                        self.state.set("pending", false);
                    }
                    return Vec::new();
                }

                self.generation = self.generation.wrapping_add(1);
                self.pending = true;
                self.pending_output = desired;
                let delay = self.timing.delay_for(desired);
                self.state.set("generation", self.generation);
                self.state.set("pending", true);
                self.state.set("delay_ticks", delay);

                let at = SimulationTime::new(event.time.ticks() + delay);
                vec![Event::at(
                    at,
                    self.id,
                    self.id,
                    EventPayload::Custom(format!("{}{}", OUTPUT_EVENT_PREFIX, self.generation)),
                )]
            }
            EventPayload::Custom(payload) if payload == MOUNT_EVENT => {
                vec![Event::at(
                    event.time,
                    self.id,
                    self.id,
                    EventPayload::PortSignal {
                        port_id: self.output_port,
                        value: StateValue::Bool(self.output),
                    },
                )]
            }
            EventPayload::Custom(payload) => {
                let Some(generation) = payload
                    .strip_prefix(OUTPUT_EVENT_PREFIX)
                    .and_then(|raw| raw.parse::<u64>().ok())
                else {
                    return Vec::new();
                };
                if !self.pending || generation != self.generation {
                    return Vec::new();
                }
                self.commit_pending(event.time)
            }
            _ => Vec::new(),
        }
    }

    fn on_mount(&mut self, ctx: &mut SimulationContext) -> Vec<Event> {
        // Returned mount events are scheduled directly by the engine and do not
        // pass through `PortSignal` connection routing. Bounce through a
        // self-targeted `Custom` event so `handle_event` can emit the signal
        // through the normal routing path.
        vec![Event::at(
            ctx.time(),
            self.id,
            self.id,
            EventPayload::Custom(MOUNT_EVENT.to_string()),
        )]
    }
}

const DFF_OUTPUT_EVENT_PREFIX: &str = "derived-dff-output:";
const DFF_MOUNT_EVENT: &str = "derived-dff-emit-initial";

/// A positive-edge-triggered D flip-flop derived from a physical DFF.
///
/// `Q` only changes on a rising `CLK` edge and takes the value of `D` sampled
/// at that edge. The delay is the measured clock-to-Q delay, quantized to
/// ticks, and a pending edge is cancelled if a newer clock edge supersedes it.
#[derive(Debug)]
pub struct DerivedDff {
    id: ComponentId,
    name: String,
    ports: Vec<Port>,
    state: State,
    truth_table: TruthTable,
    levels: LogicLevels,
    metrics: TimingMetrics,
    time_base: TimeBase,
    timing: TimingTicks,
    d_port: PortId,
    clk_port: PortId,
    q_port: PortId,
    d: bool,
    clk: bool,
    q: bool,
    pending: bool,
    pending_q: bool,
    generation: u64,
}

impl DerivedDff {
    /// Builds a discrete DFF from a measured physical characterization.
    pub fn from_characterization(
        id: ComponentId,
        name: impl Into<String>,
        characterization: &GateCharacterization,
        time_base: TimeBase,
    ) -> Result<Self, DerivedError> {
        time_base
            .validate()
            .map_err(|err| DerivedError::InvalidTimeBase(err.to_string()))?;

        if characterization.kind != GateKind::Dff {
            return Err(DerivedError::UnsupportedKind(characterization.kind));
        }

        let expected = 1usize << GateKind::Dff.inputs();
        if characterization.truth_table.inputs != GateKind::Dff.inputs()
            || characterization.truth_table.outputs.len() != expected
        {
            return Err(DerivedError::InvalidTruthTable {
                inputs: GateKind::Dff.inputs(),
                expected,
                actual: characterization.truth_table.outputs.len(),
            });
        }

        let d_port = PortId(1);
        let clk_port = PortId(2);
        let q_port = PortId(3);
        let ports = vec![
            Port::new(d_port, "D", PortDirection::Input),
            Port::new(clk_port, "CLK", PortDirection::Input),
            Port::new(q_port, "Q", PortDirection::Output),
        ];

        let d = false;
        let clk = false;
        let q = characterization
            .truth_table
            .output_for(&[d, false])
            .unwrap_or(false);
        let timing = TimingTicks::from_metrics(&characterization.timing, time_base);

        let mut state = State::new();
        state.set("q", q);
        state.set("d", d);
        state.set("clk", clk);
        state.set("pending", false);
        state.set("generation", 0u64);
        state.set("delay_ticks", 0u64);
        state.set("t_pHL_ticks", timing.t_pHL);
        state.set("t_pLH_ticks", timing.t_pLH);
        state.set("t_pHL_seconds", characterization.timing.t_pHL);
        state.set("t_pLH_seconds", characterization.timing.t_pLH);
        state.set("t_rise_seconds", characterization.timing.t_rise);
        state.set("t_fall_seconds", characterization.timing.t_fall);
        state.set(
            "energy_per_cycle_joules",
            characterization.timing.energy_per_cycle,
        );
        state.set("logic_low_volts", characterization.levels.low);
        state.set("logic_high_volts", characterization.levels.high);

        Ok(Self {
            id,
            name: name.into(),
            ports,
            state,
            truth_table: characterization.truth_table.clone(),
            levels: characterization.levels,
            metrics: characterization.timing.clone(),
            time_base,
            timing,
            d_port,
            clk_port,
            q_port,
            d,
            clk,
            q,
            pending: false,
            pending_q: q,
            generation: 0,
        })
    }

    /// Measured next-state truth table (`Q = D` after a rising edge).
    pub fn truth_table(&self) -> &TruthTable {
        &self.truth_table
    }

    /// Quantized clock-to-Q delays.
    pub fn timing_ticks(&self) -> TimingTicks {
        self.timing
    }

    /// Raw measured metrics.
    pub fn metrics(&self) -> &TimingMetrics {
        &self.metrics
    }

    /// Measured logic levels.
    pub fn levels(&self) -> LogicLevels {
        self.levels
    }

    /// Time base used for quantization.
    pub fn time_base(&self) -> TimeBase {
        self.time_base
    }

    /// Data input port.
    pub fn d_port(&self) -> PortId {
        self.d_port
    }

    /// Clock input port.
    pub fn clk_port(&self) -> PortId {
        self.clk_port
    }

    /// Output port.
    pub fn q_port(&self) -> PortId {
        self.q_port
    }

    /// Current committed output.
    pub fn output(&self) -> bool {
        self.q
    }

    fn next_state(&self) -> bool {
        self.truth_table
            .output_for(&[self.d, true])
            .unwrap_or(false)
    }

    fn cancel_pending(&mut self) {
        if self.pending {
            self.generation = self.generation.wrapping_add(1);
            self.pending = false;
            self.state.set("generation", self.generation);
            self.state.set("pending", false);
        }
    }

    fn commit_pending(&mut self, event_time: SimulationTime) -> Vec<Event> {
        self.q = self.pending_q;
        self.pending = false;
        self.state.set("q", self.q);
        self.state.set("pending", false);
        vec![Event::at(
            event_time,
            self.id,
            self.id,
            EventPayload::PortSignal {
                port_id: self.q_port,
                value: StateValue::Bool(self.q),
            },
        )]
    }
}

impl Component for DerivedDff {
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
                let Some(new_value) = value.as_bool() else {
                    return Vec::new();
                };

                if *port_id == self.d_port {
                    if self.d != new_value {
                        self.d = new_value;
                        self.state.set("d", new_value);
                    }
                    return Vec::new();
                }

                if *port_id != self.clk_port {
                    return Vec::new();
                }

                let rising_edge = !self.clk && new_value;
                self.clk = new_value;
                self.state.set("clk", new_value);
                if !rising_edge {
                    return Vec::new();
                }

                let desired = self.next_state();
                if desired == self.q {
                    self.cancel_pending();
                    return Vec::new();
                }

                self.generation = self.generation.wrapping_add(1);
                self.pending = true;
                self.pending_q = desired;
                let delay = self.timing.delay_for(desired);
                self.state.set("generation", self.generation);
                self.state.set("pending", true);
                self.state.set("delay_ticks", delay);

                let at = SimulationTime::new(event.time.ticks() + delay);
                vec![Event::at(
                    at,
                    self.id,
                    self.id,
                    EventPayload::Custom(format!("{}{}", DFF_OUTPUT_EVENT_PREFIX, self.generation)),
                )]
            }
            EventPayload::Custom(payload) if payload == DFF_MOUNT_EVENT => {
                vec![Event::at(
                    event.time,
                    self.id,
                    self.id,
                    EventPayload::PortSignal {
                        port_id: self.q_port,
                        value: StateValue::Bool(self.q),
                    },
                )]
            }
            EventPayload::Custom(payload) => {
                let Some(generation) = payload
                    .strip_prefix(DFF_OUTPUT_EVENT_PREFIX)
                    .and_then(|raw| raw.parse::<u64>().ok())
                else {
                    return Vec::new();
                };
                if !self.pending || generation != self.generation {
                    return Vec::new();
                }
                self.commit_pending(event.time)
            }
            _ => Vec::new(),
        }
    }

    fn on_mount(&mut self, ctx: &mut SimulationContext) -> Vec<Event> {
        vec![Event::at(
            ctx.time(),
            self.id,
            self.id,
            EventPayload::Custom(DFF_MOUNT_EVENT.to_string()),
        )]
    }
}

// ---------------------------------------------------------------------------
// DerivedCustomComponent (Multi-input, multi-output arbitrary circuit)
// ---------------------------------------------------------------------------

const CUSTOM_OUTPUT_EVENT_PREFIX: &str = "derived-custom-output:";
const CUSTOM_MOUNT_EVENT: &str = "derived-custom-emit-initial";

/// A discrete component whose multi-input, multi-output logic and timing were measured
/// from an arbitrary physical transistor circuit.
#[derive(Debug)]
pub struct DerivedCustomComponent {
    id: ComponentId,
    name: String,
    ports: Vec<Port>,
    state: State,
    characterization: GenericCircuitCharacterization,
    time_base: TimeBase,
    input_ports: Vec<PortId>,
    output_ports: Vec<PortId>,
    inputs: Vec<bool>,
    outputs: Vec<bool>,
    pending: Vec<bool>,
    pending_outputs: Vec<bool>,
    generations: Vec<u64>,
    timings: Vec<TimingTicks>,
}

impl DerivedCustomComponent {
    /// Builds a discrete component from an arbitrary measured characterization.
    pub fn from_characterization(
        id: ComponentId,
        name: impl Into<String>,
        characterization: GenericCircuitCharacterization,
        time_base: TimeBase,
    ) -> Result<Self, DerivedError> {
        time_base
            .validate()
            .map_err(|err| DerivedError::InvalidTimeBase(err.to_string()))?;

        let in_count = characterization.input_names.len();
        let out_count = characterization.output_names.len();

        let mut ports = Vec::with_capacity(in_count + out_count);
        let mut input_ports = Vec::with_capacity(in_count);
        let mut output_ports = Vec::with_capacity(out_count);

        let mut port_id_counter = 1u64;

        for in_name in &characterization.input_names {
            let port_id = PortId(port_id_counter);
            port_id_counter += 1;
            ports.push(Port::new(port_id, in_name.clone(), PortDirection::Input));
            input_ports.push(port_id);
        }

        for out_name in &characterization.output_names {
            let port_id = PortId(port_id_counter);
            port_id_counter += 1;
            ports.push(Port::new(port_id, out_name.clone(), PortDirection::Output));
            output_ports.push(port_id);
        }

        let inputs = vec![false; in_count];
        let mut outputs = Vec::with_capacity(out_count);
        let mut timings = Vec::with_capacity(out_count);

        let mut state = State::new();

        for in_name in &characterization.input_names {
            state.set(in_name.as_str(), false);
        }

        for out_name in &characterization.output_names {
            let single = characterization.outputs.get(out_name).ok_or_else(|| {
                DerivedError::Circuit(CircuitError::InvalidElement(format!(
                    "Missing characterization for output '{}'",
                    out_name
                )))
            })?;
            let val = single.truth_table.output_for(&inputs).unwrap_or(false);
            let timing = TimingTicks::from_metrics(&single.timing, time_base);
            outputs.push(val);
            timings.push(timing);
            state.set(out_name.as_str(), val);
            state.set(format!("{}_delay_t_pHL", out_name), timing.t_pHL);
            state.set(format!("{}_delay_t_pLH", out_name), timing.t_pLH);
        }

        let pending = vec![false; out_count];
        let pending_outputs = outputs.clone();
        let generations = vec![0u64; out_count];

        Ok(Self {
            id,
            name: name.into(),
            ports,
            state,
            characterization,
            time_base,
            input_ports,
            output_ports,
            inputs,
            outputs,
            pending,
            pending_outputs,
            generations,
            timings,
        })
    }

    /// Characterization data backing this component.
    pub fn characterization(&self) -> &GenericCircuitCharacterization {
        &self.characterization
    }

    /// Time base used for quantization.
    pub fn time_base(&self) -> TimeBase {
        self.time_base
    }
}

impl Component for DerivedCustomComponent {
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
                let Some(in_idx) = self.input_ports.iter().position(|id| id == port_id) else {
                    return Vec::new();
                };
                let Some(new_val) = value.as_bool() else {
                    return Vec::new();
                };
                if self.inputs[in_idx] == new_val {
                    return Vec::new();
                }

                self.inputs[in_idx] = new_val;
                let in_name = &self.characterization.input_names[in_idx];
                self.state.set(in_name.as_str(), new_val);

                let mut scheduled = Vec::new();

                for (out_idx, out_name) in self.characterization.output_names.iter().enumerate() {
                    let single = match self.characterization.outputs.get(out_name) {
                        Some(s) => s,
                        None => continue,
                    };
                    let desired = match single.truth_table.output_for(&self.inputs) {
                        Some(d) => d,
                        None => continue,
                    };

                    if desired == self.outputs[out_idx] {
                        if self.pending[out_idx] {
                            self.generations[out_idx] =
                                self.generations[out_idx].wrapping_add(1);
                            self.pending[out_idx] = false;
                        }
                    } else {
                        self.generations[out_idx] = self.generations[out_idx].wrapping_add(1);
                        self.pending[out_idx] = true;
                        self.pending_outputs[out_idx] = desired;
                        let delay = self.timings[out_idx].delay_for(desired);

                        let at = SimulationTime::new(event.time.ticks() + delay);
                        let gen = self.generations[out_idx];
                        scheduled.push(Event::at(
                            at,
                            self.id,
                            self.id,
                            EventPayload::Custom(format!(
                                "{}{}:{}:{}",
                                CUSTOM_OUTPUT_EVENT_PREFIX, self.id.0, out_idx, gen
                            )),
                        ));
                    }
                }

                scheduled
            }
            EventPayload::Custom(payload) if payload == CUSTOM_MOUNT_EVENT => {
                let mut events = Vec::with_capacity(self.output_ports.len());
                for (out_idx, &port_id) in self.output_ports.iter().enumerate() {
                    events.push(Event::at(
                        event.time,
                        self.id,
                        self.id,
                        EventPayload::PortSignal {
                            port_id,
                            value: StateValue::Bool(self.outputs[out_idx]),
                        },
                    ));
                }
                events
            }
            EventPayload::Custom(payload) => {
                let Some(rest) = payload.strip_prefix(CUSTOM_OUTPUT_EVENT_PREFIX) else {
                    return Vec::new();
                };
                let parts: Vec<&str> = rest.split(':').collect();
                if parts.len() != 3 {
                    return Vec::new();
                }
                let Ok(comp_id) = parts[0].parse::<u64>() else {
                    return Vec::new();
                };
                let Ok(out_idx) = parts[1].parse::<usize>() else {
                    return Vec::new();
                };
                let Ok(gen) = parts[2].parse::<u64>() else {
                    return Vec::new();
                };

                if comp_id != self.id.0 || out_idx >= self.outputs.len() {
                    return Vec::new();
                }
                if !self.pending[out_idx] || gen != self.generations[out_idx] {
                    return Vec::new();
                }

                self.outputs[out_idx] = self.pending_outputs[out_idx];
                self.pending[out_idx] = false;
                let out_name = &self.characterization.output_names[out_idx];
                self.state.set(out_name.as_str(), self.outputs[out_idx]);

                vec![Event::at(
                    event.time,
                    self.id,
                    self.id,
                    EventPayload::PortSignal {
                        port_id: self.output_ports[out_idx],
                        value: StateValue::Bool(self.outputs[out_idx]),
                    },
                )]
            }
            _ => Vec::new(),
        }
    }

    fn on_mount(&mut self, ctx: &mut SimulationContext) -> Vec<Event> {
        vec![Event::at(
            ctx.time(),
            self.id,
            self.id,
            EventPayload::Custom(CUSTOM_MOUNT_EVENT.to_string()),
        )]
    }
}
