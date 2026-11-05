//! Automated measurement of physical transistor circuits.
//!
//! The characterizer runs real transient simulations on a transistor netlist
//! and extracts the dynamic parameters that a derived (abstract) component
//! needs:
//!
//! * propagation delay `t_pHL` / `t_pLH`,
//! * rise time `t_r` and fall time `t_f`,
//! * energy drawn from the supply during each edge.
//!
//! Nothing here hardcodes gate logic: the truth table is obtained from `dc_op`
//! on the same physical circuit that produces the timing numbers.

use tw_core::PortId;

use crate::analysis::{dc_op, integrate_source_energy, transient, TransientResult};
use crate::circuit::Circuit;
use crate::digital::cmos_dff;
use crate::error::CircuitError;
use crate::gates::{cmos_inverter, cmos_nand, CmosGateConfig};
use crate::node::NodeId;
use crate::state::CircuitState;

/// Configuration for a characterization run.
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterizationConfig {
    /// Supply voltage (V).
    pub vdd: f64,
    /// Fixed transient timestep (s).
    pub dt: f64,
    /// Transient duration for each measured edge (s).
    pub duration: f64,
    /// Settling time allowed before a sequential measurement (s).
    ///
    /// Master-slave flip-flops need their internal latches to settle after the
    /// inputs change and before the measured clock edge.
    pub settle_duration: f64,
    /// Capacitive load attached to the output node (F). Zero disables it.
    pub load_capacitance: f64,
    /// Name of the supply voltage source in the generated circuit.
    pub supply_source: String,
    /// Name of the input voltage source in the generated circuit.
    pub input_source: String,
    /// Low threshold as a fraction of the measured output swing.
    pub low_threshold: f64,
    /// Mid threshold as a fraction of the measured output swing.
    pub mid_threshold: f64,
    /// High threshold as a fraction of the measured output swing.
    pub high_threshold: f64,
    /// Transistor parameters for the generated CMOS gates.
    pub gate_config: CmosGateConfig,
}

impl Default for CharacterizationConfig {
    fn default() -> Self {
        Self {
            vdd: 5.0,
            dt: 5e-11,
            duration: 20e-9,
            settle_duration: 10e-9,
            load_capacitance: 10e-12,
            supply_source: "Vdd".to_string(),
            input_source: "Vin".to_string(),
            low_threshold: 0.1,
            mid_threshold: 0.5,
            high_threshold: 0.9,
            gate_config: CmosGateConfig::default(),
        }
    }
}

impl CharacterizationConfig {
    /// Validates the configuration before a run.
    pub fn validate(&self) -> Result<(), CircuitError> {
        if !self.vdd.is_finite() || self.vdd <= 0.0 {
            return Err(CircuitError::InvalidElement(format!(
                "CharacterizationConfig vdd must be finite and positive, got {}",
                self.vdd
            )));
        }
        if !self.dt.is_finite() || self.dt <= 0.0 {
            return Err(CircuitError::InvalidElement(format!(
                "CharacterizationConfig dt must be finite and positive, got {}",
                self.dt
            )));
        }
        if !self.duration.is_finite() || self.duration < self.dt {
            return Err(CircuitError::InvalidElement(format!(
                "CharacterizationConfig duration ({}) must be finite and >= dt ({})",
                self.duration, self.dt
            )));
        }
        if !self.settle_duration.is_finite() || self.settle_duration < self.dt {
            return Err(CircuitError::InvalidElement(format!(
                "CharacterizationConfig settle_duration ({}) must be finite and >= dt ({})",
                self.settle_duration, self.dt
            )));
        }
        if !self.load_capacitance.is_finite() || self.load_capacitance < 0.0 {
            return Err(CircuitError::InvalidElement(format!(
                "CharacterizationConfig load_capacitance must be finite and non-negative, got {}",
                self.load_capacitance
            )));
        }
        if !(self.low_threshold > 0.0
            && self.low_threshold < self.mid_threshold
            && self.mid_threshold < self.high_threshold
            && self.high_threshold < 1.0)
        {
            return Err(CircuitError::InvalidElement(format!(
                "CharacterizationConfig thresholds must satisfy 0 < low < mid < high < 1, got {} / {} / {}",
                self.low_threshold, self.mid_threshold, self.high_threshold
            )));
        }
        Ok(())
    }
}

/// Measured logic levels of a gate output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogicLevels {
    /// Output voltage for a logic low.
    pub low: f64,
    /// Output voltage for a logic high.
    pub high: f64,
}

impl LogicLevels {
    /// Full output swing (V).
    pub fn span(&self) -> f64 {
        self.high - self.low
    }

    /// Absolute voltage at a fraction of the measured swing.
    pub fn threshold(&self, fraction: f64) -> f64 {
        self.low + fraction * self.span()
    }
}

/// Dynamic metrics extracted from a physical circuit.
#[allow(non_snake_case)]
#[derive(Debug, Clone, PartialEq)]
pub struct TimingMetrics {
    /// High-to-low propagation delay, measured from the input step (s).
    pub t_pHL: f64,
    /// Low-to-high propagation delay, measured from the input step (s).
    pub t_pLH: f64,
    /// Output rise time, 10% → 90% of the measured swing (s).
    pub t_rise: f64,
    /// Output fall time, 90% → 10% of the measured swing (s).
    pub t_fall: f64,
    /// Supply energy drawn during the low-to-high output edge (J).
    pub energy_rise: f64,
    /// Supply energy drawn during the high-to-low output edge (J).
    pub energy_fall: f64,
    /// Energy drawn during one complete switching cycle (J).
    pub energy_per_cycle: f64,
}

impl TimingMetrics {
    /// Average switching power for a given cycle period (W).
    pub fn average_power(&self, period_seconds: f64) -> f64 {
        if period_seconds <= 0.0 || !period_seconds.is_finite() {
            return 0.0;
        }
        self.energy_per_cycle / period_seconds
    }
}

/// Truth table obtained by measuring the physical circuit at DC.
///
/// `outputs` is indexed by the binary input pattern, with the first input as
/// the most significant bit: for two inputs the order is `00, 01, 10, 11`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TruthTable {
    /// Number of inputs.
    pub inputs: usize,
    /// One output bit per input combination.
    pub outputs: Vec<bool>,
}

impl TruthTable {
    /// Creates a truth table, validating the number of entries.
    pub fn new(inputs: usize, outputs: Vec<bool>) -> Result<Self, CircuitError> {
        if inputs >= usize::BITS as usize {
            return Err(CircuitError::InvalidElement(format!(
                "Truth table input count {} is too large",
                inputs
            )));
        }
        let expected = 1usize << inputs;
        if outputs.len() != expected {
            return Err(CircuitError::InvalidElement(format!(
                "Truth table for {} inputs needs {} entries, got {}",
                inputs,
                expected,
                outputs.len()
            )));
        }
        Ok(Self { inputs, outputs })
    }

    /// Looks up the output for a concrete input vector.
    pub fn output_for(&self, inputs: &[bool]) -> Option<bool> {
        if inputs.len() != self.inputs {
            return None;
        }
        let mut index = 0usize;
        for &value in inputs {
            index = (index << 1) | usize::from(value);
        }
        self.outputs.get(index).copied()
    }

    /// Looks up the output by raw input pattern index.
    pub fn output_at(&self, index: usize) -> Option<bool> {
        self.outputs.get(index).copied()
    }

    /// Iterates over `(pattern, output)` pairs.
    pub fn rows(&self) -> impl Iterator<Item = (usize, bool)> + '_ {
        self.outputs.iter().copied().enumerate()
    }
}

/// Kind of physical circuit being characterized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GateKind {
    /// Single-input CMOS inverter.
    Inverter,
    /// Two-input CMOS NAND.
    Nand2,
    /// Positive-edge-triggered D flip-flop.
    Dff,
}

impl GateKind {
    /// Number of logic inputs.
    pub fn inputs(self) -> usize {
        match self {
            GateKind::Inverter => 1,
            GateKind::Nand2 => 2,
            GateKind::Dff => 2,
        }
    }

    /// Input port names, in truth-table bit order.
    pub fn input_names(self) -> &'static [&'static str] {
        match self {
            GateKind::Inverter => &["IN"],
            GateKind::Nand2 => &["A", "B"],
            GateKind::Dff => &["D", "CLK"],
        }
    }

    /// Primary output port name.
    pub fn output_name(self) -> &'static str {
        match self {
            GateKind::Inverter | GateKind::Nand2 => "OUT",
            GateKind::Dff => "Q",
        }
    }
}

/// Complete result of characterizing one physical gate.
#[derive(Debug, Clone, PartialEq)]
pub struct GateCharacterization {
    /// Kind of circuit that was measured.
    pub kind: GateKind,
    /// Truth table measured with `dc_op`.
    pub truth_table: TruthTable,
    /// Measured output logic levels.
    pub levels: LogicLevels,
    /// Measured dynamic metrics.
    pub timing: TimingMetrics,
    /// Configuration used for the measurement.
    pub config: CharacterizationConfig,
}

/// One discrete input bound to a voltage source in a [`CircuitTemplate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateInput {
    /// Port id used by the discrete/derived component.
    pub port_id: PortId,
    /// Human-readable port name.
    pub name: &'static str,
    /// Voltage source inside the circuit driven by this input.
    pub source_name: String,
}

/// A physical netlist together with the port mapping needed to expose it to the
/// discrete event engine.
///
/// The characterizer returns the same template it measured, so a mixed-signal
/// bridge built from it is guaranteed to wrap the exact circuit that produced
/// the timing numbers.
#[derive(Debug, Clone)]
pub struct CircuitTemplate {
    /// Transistor-level netlist.
    pub circuit: Circuit,
    /// Input bindings, in truth-table bit order.
    pub inputs: Vec<TemplateInput>,
    /// Node sampled as the primary output.
    pub output_node: NodeId,
    /// Discrete port id for the primary output.
    pub output_port: PortId,
    /// Name of the supply source, if the circuit has one.
    pub supply_source: Option<String>,
}

impl CircuitTemplate {
    /// Voltage-source names in the same order as [`Self::inputs`].
    pub fn input_sources(&self) -> Vec<String> {
        self.inputs
            .iter()
            .map(|input| input.source_name.clone())
            .collect()
    }
}

/// A characterization result plus the netlist that produced it.
#[derive(Debug, Clone)]
pub struct Measurement {
    /// Measured metrics and truth table.
    pub gate: GateCharacterization,
    /// Physical circuit and port mapping used for the measurement.
    pub template: CircuitTemplate,
}

/// Input values before and after a single logic step.
struct EdgeStimulus<'a> {
    initial: &'a [bool],
    final_values: &'a [bool],
}

/// Runs transient simulations on physical transistor circuits and extracts
/// timing and energy metrics.
#[derive(Debug, Clone)]
pub struct Characterizer {
    config: CharacterizationConfig,
}

impl Characterizer {
    /// Creates a characterizer after validating the configuration.
    pub fn new(config: CharacterizationConfig) -> Result<Self, CircuitError> {
        config.validate()?;
        Ok(Self { config })
    }

    /// Creates a characterizer with [`CharacterizationConfig::default`].
    pub fn with_defaults() -> Self {
        Self {
            config: CharacterizationConfig::default(),
        }
    }

    /// Configuration used by this characterizer.
    pub fn config(&self) -> &CharacterizationConfig {
        &self.config
    }

    /// Characterizes a physical circuit and returns both the measured metrics
    /// and the netlist that produced them.
    pub fn measure(&self, kind: GateKind) -> Result<Measurement, CircuitError> {
        match kind {
            GateKind::Inverter => self.measure_inverter(),
            GateKind::Nand2 => self.measure_nand2(),
            GateKind::Dff => self.measure_dff(),
        }
    }

    /// Characterizes a physical CMOS inverter.
    pub fn inverter(&self) -> Result<GateCharacterization, CircuitError> {
        Ok(self.measure_inverter()?.gate)
    }

    /// Characterizes a physical 2-input CMOS NAND.
    ///
    /// The falling edge is measured with `A` held high while `B` rises, and the
    /// rising edge with both inputs high while `B` falls. This isolates a single
    /// input step, which is the quantity a derived gate needs.
    pub fn nand2(&self) -> Result<GateCharacterization, CircuitError> {
        Ok(self.measure_nand2()?.gate)
    }

    /// Characterizes a physical positive-edge-triggered D flip-flop.
    ///
    /// The clock-to-Q delay is measured for both output edges, together with
    /// the supply energy of each edge. The returned truth table is the
    /// *next-state* table: after a rising clock edge `Q` equals `D`, so the
    /// `CLK` column is a don't-care.
    pub fn dff(&self) -> Result<GateCharacterization, CircuitError> {
        Ok(self.measure_dff()?.gate)
    }

    fn measure_inverter(&self) -> Result<Measurement, CircuitError> {
        let template = self.build_inverter()?;
        let input_sources = template.input_sources();
        let gate = self.characterize(
            GateKind::Inverter,
            template.circuit.clone(),
            template.output_node,
            &input_sources,
            // Falling output: IN 0 -> Vdd.
            EdgeStimulus {
                initial: &[false],
                final_values: &[true],
            },
            // Rising output: IN Vdd -> 0.
            EdgeStimulus {
                initial: &[true],
                final_values: &[false],
            },
        )?;
        Ok(Measurement { gate, template })
    }

    fn measure_nand2(&self) -> Result<Measurement, CircuitError> {
        let template = self.build_nand2()?;
        let input_sources = template.input_sources();
        let gate = self.characterize(
            GateKind::Nand2,
            template.circuit.clone(),
            template.output_node,
            &input_sources,
            // Falling output: A=1, B 0 -> 1 (both high).
            EdgeStimulus {
                initial: &[true, false],
                final_values: &[true, true],
            },
            // Rising output: A=1, B 1 -> 0.
            EdgeStimulus {
                initial: &[true, true],
                final_values: &[true, false],
            },
        )?;
        Ok(Measurement { gate, template })
    }

    fn measure_dff(&self) -> Result<Measurement, CircuitError> {
        let template = self.build_dff()?;
        let mut circuit = template.circuit.clone();
        let n_q = template.output_node;
        let input_sources = template.input_sources();
        let d_source = input_sources[0].clone();
        let clk_source = input_sources[1].clone();

        // Initialize the slave latch with a rising edge while D=0.
        self.drive_dff(&mut circuit, &d_source, &clk_source, false, false)?;
        let mut state = self.settle(&circuit, None)?;
        circuit.set_voltage_source(&clk_source, self.config.vdd)?;
        state = self.settle(&circuit, Some(state))?;

        // Prepare Q=1: the master samples D=1 while CLK is low.
        self.drive_dff(&mut circuit, &d_source, &clk_source, true, false)?;
        state = self.settle(&circuit, Some(state))?;

        // Rising clock edge with D=1: Q rises.
        circuit.set_voltage_source(&clk_source, self.config.vdd)?;
        let rise = transient(&circuit, self.config.duration, self.config.dt, Some(state))?;
        let rise_final = rise.final_state().cloned();

        // Prepare Q=0: the master samples D=0 while CLK is low.
        self.drive_dff(&mut circuit, &d_source, &clk_source, false, false)?;
        let state = self.settle(&circuit, rise_final)?;

        // Rising clock edge with D=0: Q falls.
        circuit.set_voltage_source(&clk_source, self.config.vdd)?;
        let fall = transient(&circuit, self.config.duration, self.config.dt, Some(state))?;

        let rise_wave = rise.voltage_waveform(n_q);
        let fall_wave = fall.voltage_waveform(n_q);

        let high = rise_wave
            .iter()
            .map(|&(_, voltage)| voltage)
            .fold(f64::NEG_INFINITY, f64::max);
        let low = fall_wave
            .iter()
            .map(|&(_, voltage)| voltage)
            .fold(f64::INFINITY, f64::min);
        let levels = LogicLevels { low, high };
        let mid = levels.threshold(self.config.mid_threshold);

        // Next-state table: Q after a rising edge equals the sampled D.
        let next_high = high > mid;
        let next_low = low > mid;
        let truth_table = TruthTable::new(2, vec![next_low, next_low, next_high, next_high])?;

        let low_th = levels.threshold(self.config.low_threshold);
        let high_th = levels.threshold(self.config.high_threshold);

        let fall_high = crossing_time(&fall_wave, high_th, false)
            .ok_or_else(|| missing_crossing("falling", self.config.high_threshold))?;
        let fall_mid = crossing_time(&fall_wave, mid, false)
            .ok_or_else(|| missing_crossing("falling", self.config.mid_threshold))?;
        let fall_low = crossing_time(&fall_wave, low_th, false)
            .ok_or_else(|| missing_crossing("falling", self.config.low_threshold))?;

        let rise_low = crossing_time(&rise_wave, low_th, true)
            .ok_or_else(|| missing_crossing("rising", self.config.low_threshold))?;
        let rise_mid = crossing_time(&rise_wave, mid, true)
            .ok_or_else(|| missing_crossing("rising", self.config.mid_threshold))?;
        let rise_high = crossing_time(&rise_wave, high_th, true)
            .ok_or_else(|| missing_crossing("rising", self.config.high_threshold))?;

        let t_phl = fall_mid;
        let t_plh = rise_mid;
        let t_fall = fall_low - fall_high;
        let t_rise = rise_high - rise_low;

        if t_fall <= 0.0 || t_rise <= 0.0 {
            return Err(CircuitError::InvalidElement(format!(
                "Measured non-positive DFF edge times: t_fall={}, t_rise={}",
                t_fall, t_rise
            )));
        }

        let supply_voltage = circuit
            .voltage_source(&self.config.supply_source)
            .map(|src| src.voltage)
            .unwrap_or(self.config.vdd);
        let energy_rise =
            integrate_source_energy(&rise, &self.config.supply_source, supply_voltage)?;
        let energy_fall =
            integrate_source_energy(&fall, &self.config.supply_source, supply_voltage)?;

        let timing = TimingMetrics {
            t_pHL: t_phl,
            t_pLH: t_plh,
            t_rise,
            t_fall,
            energy_rise,
            energy_fall,
            energy_per_cycle: energy_rise + energy_fall,
        };

        let gate = GateCharacterization {
            kind: GateKind::Dff,
            truth_table,
            levels,
            timing,
            config: self.config.clone(),
        };
        Ok(Measurement { gate, template })
    }

    fn drive_dff(
        &self,
        circuit: &mut Circuit,
        d_source: &str,
        clk_source: &str,
        d: bool,
        clk: bool,
    ) -> Result<(), CircuitError> {
        let voltage = |value: bool| if value { self.config.vdd } else { 0.0 };
        circuit.set_voltage_source(d_source, voltage(d))?;
        circuit.set_voltage_source(clk_source, voltage(clk))?;
        Ok(())
    }

    fn settle(
        &self,
        circuit: &Circuit,
        initial: Option<CircuitState>,
    ) -> Result<CircuitState, CircuitError> {
        let result = transient(
            circuit,
            self.config.settle_duration,
            self.config.dt,
            initial,
        )?;
        result.final_state().cloned().ok_or_else(|| {
            CircuitError::InvalidElement("DFF settling transient produced no state".to_string())
        })
    }

    /// Characterizes a physical circuit by measuring its DC truth table and the
    /// transient response of one falling and one rising output edge.
    fn characterize(
        &self,
        kind: GateKind,
        mut circuit: Circuit,
        n_out: NodeId,
        input_sources: &[String],
        fall: EdgeStimulus<'_>,
        rise: EdgeStimulus<'_>,
    ) -> Result<GateCharacterization, CircuitError> {
        debug_assert_eq!(input_sources.len(), kind.inputs());
        debug_assert_eq!(fall.initial.len(), input_sources.len());
        debug_assert_eq!(fall.final_values.len(), input_sources.len());
        debug_assert_eq!(rise.initial.len(), input_sources.len());
        debug_assert_eq!(rise.final_values.len(), input_sources.len());

        // --- DC truth table and measured logic levels -------------------
        let combinations = 1usize << input_sources.len();
        let mut output_voltages = Vec::with_capacity(combinations);
        let mut low = f64::INFINITY;
        let mut high = f64::NEG_INFINITY;
        for pattern in 0..combinations {
            let values = decode_pattern(pattern, input_sources.len());
            self.set_inputs(&mut circuit, input_sources, &values)?;
            let state = dc_op(&circuit)?;
            let voltage = state.voltage(n_out);
            low = low.min(voltage);
            high = high.max(voltage);
            output_voltages.push(voltage);
        }

        let levels = LogicLevels { low, high };
        let mid = levels.threshold(self.config.mid_threshold);
        let truth_table = TruthTable::new(
            input_sources.len(),
            output_voltages.iter().map(|&v| v > mid).collect(),
        )?;

        // --- Falling and rising output edges ----------------------------
        let fall =
            self.measure_edge(&mut circuit, input_sources, fall.initial, fall.final_values)?;
        let rise =
            self.measure_edge(&mut circuit, input_sources, rise.initial, rise.final_values)?;

        let fall_wave = fall.voltage_waveform(n_out);
        let rise_wave = rise.voltage_waveform(n_out);

        let low_th = levels.threshold(self.config.low_threshold);
        let high_th = levels.threshold(self.config.high_threshold);

        let fall_high = crossing_time(&fall_wave, high_th, false)
            .ok_or_else(|| missing_crossing("falling", self.config.high_threshold))?;
        let fall_mid = crossing_time(&fall_wave, mid, false)
            .ok_or_else(|| missing_crossing("falling", self.config.mid_threshold))?;
        let fall_low = crossing_time(&fall_wave, low_th, false)
            .ok_or_else(|| missing_crossing("falling", self.config.low_threshold))?;

        let rise_low = crossing_time(&rise_wave, low_th, true)
            .ok_or_else(|| missing_crossing("rising", self.config.low_threshold))?;
        let rise_mid = crossing_time(&rise_wave, mid, true)
            .ok_or_else(|| missing_crossing("rising", self.config.mid_threshold))?;
        let rise_high = crossing_time(&rise_wave, high_th, true)
            .ok_or_else(|| missing_crossing("rising", self.config.high_threshold))?;

        let t_phl = fall_mid;
        let t_plh = rise_mid;
        let t_fall = fall_low - fall_high;
        let t_rise = rise_high - rise_low;

        if t_fall <= 0.0 || t_rise <= 0.0 {
            return Err(CircuitError::InvalidElement(format!(
                "Measured non-positive edge times: t_fall={}, t_rise={}",
                t_fall, t_rise
            )));
        }

        // --- Energy -----------------------------------------------------
        let supply_voltage = circuit
            .voltage_source(&self.config.supply_source)
            .map(|src| src.voltage)
            .unwrap_or(self.config.vdd);
        let energy_rise =
            integrate_source_energy(&rise, &self.config.supply_source, supply_voltage)?;
        let energy_fall =
            integrate_source_energy(&fall, &self.config.supply_source, supply_voltage)?;

        let timing = TimingMetrics {
            t_pHL: t_phl,
            t_pLH: t_plh,
            t_rise,
            t_fall,
            energy_rise,
            energy_fall,
            energy_per_cycle: energy_rise + energy_fall,
        };

        Ok(GateCharacterization {
            kind,
            truth_table,
            levels,
            timing,
            config: self.config.clone(),
        })
    }

    fn measure_edge(
        &self,
        circuit: &mut Circuit,
        input_sources: &[String],
        initial: &[bool],
        final_values: &[bool],
    ) -> Result<TransientResult, CircuitError> {
        self.set_inputs(circuit, input_sources, initial)?;
        let initial_state = dc_op(circuit)?;
        self.set_inputs(circuit, input_sources, final_values)?;
        transient(
            circuit,
            self.config.duration,
            self.config.dt,
            Some(initial_state),
        )
    }

    fn set_inputs(
        &self,
        circuit: &mut Circuit,
        input_sources: &[String],
        values: &[bool],
    ) -> Result<(), CircuitError> {
        for (source, &value) in input_sources.iter().zip(values) {
            let voltage = if value { self.config.vdd } else { 0.0 };
            circuit.set_voltage_source(source, voltage)?;
        }
        Ok(())
    }

    fn build_inverter(&self) -> Result<CircuitTemplate, CircuitError> {
        let mut circuit = Circuit::new();
        let n_vdd = circuit.get_or_add_node("vdd");
        let n_in = circuit.get_or_add_node("in");
        let n_out = circuit.get_or_add_node("out");

        circuit.add_voltage_source(
            self.config.supply_source.clone(),
            n_vdd,
            NodeId::GROUND,
            self.config.vdd,
        );
        circuit.add_voltage_source(self.config.input_source.clone(), n_in, NodeId::GROUND, 0.0);

        cmos_inverter(self.config.gate_config).instantiate(
            &mut circuit,
            "INV1",
            &[
                ("IN", n_in),
                ("OUT", n_out),
                ("VDD", n_vdd),
                ("GND", NodeId::GROUND),
            ],
        )?;

        if self.config.load_capacitance > 0.0 {
            circuit.add_capacitor("CL", n_out, NodeId::GROUND, self.config.load_capacitance);
        }

        Ok(CircuitTemplate {
            circuit,
            inputs: vec![TemplateInput {
                port_id: PortId(1),
                name: "IN",
                source_name: self.config.input_source.clone(),
            }],
            output_node: n_out,
            output_port: PortId(2),
            supply_source: Some(self.config.supply_source.clone()),
        })
    }

    fn build_nand2(&self) -> Result<CircuitTemplate, CircuitError> {
        let mut circuit = Circuit::new();
        let n_vdd = circuit.get_or_add_node("vdd");
        let n_a = circuit.get_or_add_node("a");
        let n_b = circuit.get_or_add_node("b");
        let n_out = circuit.get_or_add_node("out");

        let source_a = format!("{}_A", self.config.input_source);
        let source_b = format!("{}_B", self.config.input_source);

        circuit.add_voltage_source(
            self.config.supply_source.clone(),
            n_vdd,
            NodeId::GROUND,
            self.config.vdd,
        );
        circuit.add_voltage_source(source_a.clone(), n_a, NodeId::GROUND, 0.0);
        circuit.add_voltage_source(source_b.clone(), n_b, NodeId::GROUND, 0.0);

        cmos_nand(self.config.gate_config).instantiate(
            &mut circuit,
            "NAND1",
            &[
                ("A", n_a),
                ("B", n_b),
                ("OUT", n_out),
                ("VDD", n_vdd),
                ("GND", NodeId::GROUND),
            ],
        )?;

        if self.config.load_capacitance > 0.0 {
            circuit.add_capacitor("CL", n_out, NodeId::GROUND, self.config.load_capacitance);
        }

        Ok(CircuitTemplate {
            circuit,
            inputs: vec![
                TemplateInput {
                    port_id: PortId(1),
                    name: "A",
                    source_name: source_a,
                },
                TemplateInput {
                    port_id: PortId(2),
                    name: "B",
                    source_name: source_b,
                },
            ],
            output_node: n_out,
            output_port: PortId(3),
            supply_source: Some(self.config.supply_source.clone()),
        })
    }

    fn build_dff(&self) -> Result<CircuitTemplate, CircuitError> {
        let mut circuit = Circuit::new();
        let n_vdd = circuit.get_or_add_node("vdd");
        let n_d = circuit.get_or_add_node("d");
        let n_clk = circuit.get_or_add_node("clk");
        let n_q = circuit.get_or_add_node("q");
        let n_q_bar = circuit.get_or_add_node("q_bar");

        let source_d = format!("{}_D", self.config.input_source);
        let source_clk = format!("{}_CLK", self.config.input_source);

        circuit.add_voltage_source(
            self.config.supply_source.clone(),
            n_vdd,
            NodeId::GROUND,
            self.config.vdd,
        );
        circuit.add_voltage_source(source_d.clone(), n_d, NodeId::GROUND, 0.0);
        circuit.add_voltage_source(source_clk.clone(), n_clk, NodeId::GROUND, 0.0);

        cmos_dff(self.config.gate_config).instantiate(
            &mut circuit,
            "DFF1",
            &[
                ("D", n_d),
                ("CLK", n_clk),
                ("Q", n_q),
                ("Q_BAR", n_q_bar),
                ("VDD", n_vdd),
                ("GND", NodeId::GROUND),
            ],
        )?;

        if self.config.load_capacitance > 0.0 {
            circuit.add_capacitor("CL_Q", n_q, NodeId::GROUND, self.config.load_capacitance);
        }

        Ok(CircuitTemplate {
            circuit,
            inputs: vec![
                TemplateInput {
                    port_id: PortId(1),
                    name: "D",
                    source_name: source_d,
                },
                TemplateInput {
                    port_id: PortId(2),
                    name: "CLK",
                    source_name: source_clk,
                },
            ],
            output_node: n_q,
            output_port: PortId(3),
            supply_source: Some(self.config.supply_source.clone()),
        })
    }
}

/// Expands a truth-table pattern into input bits, first input as MSB.
fn decode_pattern(pattern: usize, inputs: usize) -> Vec<bool> {
    (0..inputs)
        .map(|bit| (pattern >> (inputs - 1 - bit)) & 1 == 1)
        .collect()
}

fn missing_crossing(edge: &str, fraction: f64) -> CircuitError {
    CircuitError::InvalidElement(format!(
        "Output never crossed the {}% threshold during the {} edge; \
         increase CharacterizationConfig::duration or reduce the load",
        fraction * 100.0,
        edge
    ))
}

/// Finds the first time the waveform crosses `level` in the given direction.
///
/// Linear interpolation between adjacent samples keeps the measured delay
/// independent of the timestep (up to the integrator's own accuracy).
fn crossing_time(waveform: &[(f64, f64)], level: f64, rising: bool) -> Option<f64> {
    for window in waveform.windows(2) {
        let (t0, v0) = window[0];
        let (t1, v1) = window[1];
        let crossed = if rising {
            v0 <= level && v1 >= level
        } else {
            v0 >= level && v1 <= level
        };
        if !crossed {
            continue;
        }
        let delta = v1 - v0;
        if delta.abs() < f64::EPSILON {
            return Some(t0);
        }
        let fraction = (level - v0) / delta;
        return Some(t0 + fraction * (t1 - t0));
    }
    None
}
