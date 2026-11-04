use std::sync::{Arc, Mutex};

use tw_core::{
    Component, ComponentId, Connection, Event, EventPayload, Port, PortDirection, PortId,
    SimulationContext, SimulationTime, State, StateValue,
};
use tw_electrical::prelude::*;
use tw_sim::Simulation;

const BRIDGE_ID: ComponentId = ComponentId(100);
const SINK_ID: ComponentId = ComponentId(200);
const INPUT_PORT: PortId = PortId(1);
const OUTPUT_PORT: PortId = PortId(2);
const SINK_PORT: PortId = PortId(1);

type SignalLog = Arc<Mutex<Vec<(u64, bool)>>>;

#[derive(Debug)]
struct SignalSink {
    id: ComponentId,
    name: String,
    ports: Vec<Port>,
    state: State,
    log: SignalLog,
}

impl SignalSink {
    fn new(id: ComponentId, name: &str, log: SignalLog) -> Self {
        let mut state = State::new();
        state.set("events", 0u64);
        Self {
            id,
            name: name.to_string(),
            ports: vec![Port::new(SINK_PORT, "IN", PortDirection::Input)],
            state,
            log,
        }
    }
}

impl Component for SignalSink {
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
        if let EventPayload::PortSignal { port_id, value } = &event.payload {
            if *port_id == SINK_PORT {
                if let Some(value) = value.as_bool() {
                    if let Ok(mut log) = self.log.lock() {
                        log.push((event.time.ticks(), value));
                    }
                    let count = self.state.get_int("events").unwrap_or(0) + 1;
                    self.state.set("events", count as u64);
                    self.state.set("last", value);
                }
            }
        }
        Vec::new()
    }
}

fn build_inverter(vdd: f64) -> Circuit {
    let mut circuit = Circuit::new();
    let n_vdd = circuit.get_or_add_node("vdd");
    let n_in = circuit.get_or_add_node("in");
    let n_out = circuit.get_or_add_node("out");

    circuit.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, vdd);
    circuit.add_voltage_source("Vin", n_in, NodeId::GROUND, 0.0);
    cmos_inverter(CmosGateConfig::default())
        .instantiate(
            &mut circuit,
            "INV1",
            &[
                ("IN", n_in),
                ("OUT", n_out),
                ("VDD", n_vdd),
                ("GND", NodeId::GROUND),
            ],
        )
        .expect("inverter instantiated");
    circuit.add_capacitor("CL", n_out, NodeId::GROUND, 10e-12);
    circuit
}

fn bridge_config(node: NodeId) -> BridgeConfig {
    BridgeConfig {
        time_base: TimeBase::default(),
        solver_dt: 5e-11,
        sync_interval_ticks: 1,
        supply_source: Some("Vdd".to_string()),
        inputs: vec![BridgeInput::new(INPUT_PORT, "IN", "Vin", 0.0, 5.0)],
        outputs: vec![BridgeOutput::new(OUTPUT_PORT, "OUT", node, 2.5)],
    }
}

fn build_simulation() -> (Simulation, SignalLog) {
    let circuit = build_inverter(5.0);
    let n_out = circuit.node_by_name("out").unwrap();
    let bridge = CircuitComponent::new(BRIDGE_ID, "BridgeInverter", circuit, bridge_config(n_out))
        .expect("bridge constructed");

    let log = Arc::new(Mutex::new(Vec::new()));
    let sink = SignalSink::new(SINK_ID, "Sink", log.clone());

    let mut sim = Simulation::new();
    sim.register_component(Box::new(bridge));
    sim.register_component(Box::new(sink));
    sim.register_connection(Connection::new(BRIDGE_ID, OUTPUT_PORT, SINK_ID, SINK_PORT));
    (sim, log)
}

fn schedule_input(sim: &mut Simulation, tick: u64, value: bool) {
    sim.schedule_event(Event::at(
        SimulationTime::new(tick),
        ComponentId(999),
        BRIDGE_ID,
        EventPayload::PortSignal {
            port_id: INPUT_PORT,
            value: StateValue::Bool(value),
        },
    ));
}

#[test]
fn bridge_inverter_matches_standalone_transient() {
    let measured = Characterizer::with_defaults()
        .inverter()
        .expect("inverter characterized");
    let expected_tick = TimeBase::default().delay_ticks(measured.timing.t_pHL);

    let (mut sim, log) = build_simulation();
    schedule_input(&mut sim, 0, true);
    sim.run_until(SimulationTime::new(40));

    let events = log.lock().unwrap().clone();
    println!("bridge events: {events:?}");

    assert_eq!(events.first(), Some(&(0, true)), "initial output high");
    let fall_tick = events
        .iter()
        .find(|(_, value)| !*value)
        .map(|(tick, _)| *tick)
        .expect("bridge must produce a falling output");
    assert!(
        fall_tick.abs_diff(expected_tick) <= 1,
        "falling tick {fall_tick} should be within one tick of {expected_tick}"
    );
}

#[test]
fn bridge_accumulates_supply_energy() {
    let measured = Characterizer::with_defaults()
        .inverter()
        .expect("inverter characterized");

    let (mut sim, log) = build_simulation();
    schedule_input(&mut sim, 0, true); // output falls
    schedule_input(&mut sim, 10, false); // output rises, draws supply charge
    sim.run_until(SimulationTime::new(40));

    let state = sim.component_state(BRIDGE_ID).expect("bridge state");
    let energy = state.get_float("energy_joules").expect("energy key");
    println!(
        "bridge energy: {energy}, measured rise: {}",
        measured.timing.energy_rise
    );

    assert!(
        energy > 1e-10,
        "expected measurable switching energy, got {energy}"
    );
    assert!(
        energy < 1e-9,
        "energy should stay in the expected 10 pF range, got {energy}"
    );

    let events = log.lock().unwrap().clone();
    assert!(events.contains(&(0, true)));
    assert!(events.iter().any(|(_, value)| !*value));
    assert_eq!(events.last().map(|(_, value)| *value), Some(true));
}

#[test]
fn bridge_exposes_initial_state_and_ports() {
    let circuit = build_inverter(5.0);
    let n_out = circuit.node_by_name("out").unwrap();
    let bridge = CircuitComponent::new(BRIDGE_ID, "BridgeInverter", circuit, bridge_config(n_out))
        .expect("bridge constructed");

    assert_eq!(bridge.state().get_bool("output_0"), Some(true));
    let v_out = bridge.state().get_float("output_voltage_0").unwrap();
    assert!(
        v_out > 4.9,
        "initial output should be near Vdd, got {v_out}"
    );
    assert_eq!(bridge.energy_joules(), 0.0);
    assert_eq!(bridge.ports().len(), 2);
    assert_eq!(bridge.ports()[0].direction, PortDirection::Input);
    assert_eq!(bridge.ports()[1].direction, PortDirection::Output);
}

#[test]
fn bridge_rejects_invalid_configuration() {
    let circuit = build_inverter(5.0);
    let n_out = circuit.node_by_name("out").unwrap();

    let mut config = bridge_config(n_out);
    config.solver_dt = 0.0;
    assert!(CircuitComponent::new(BRIDGE_ID, "bad", build_inverter(5.0), config).is_err());

    let mut config = bridge_config(n_out);
    config.sync_interval_ticks = 0;
    assert!(CircuitComponent::new(BRIDGE_ID, "bad", build_inverter(5.0), config).is_err());

    let mut config = bridge_config(n_out);
    config.inputs[0].source_name = "Missing".to_string();
    assert!(CircuitComponent::new(BRIDGE_ID, "bad", build_inverter(5.0), config).is_err());

    let mut config = bridge_config(n_out);
    config.supply_source = Some("Missing".to_string());
    assert!(CircuitComponent::new(BRIDGE_ID, "bad", build_inverter(5.0), config).is_err());

    let mut config = bridge_config(n_out);
    config.outputs[0].node = NodeId(99);
    assert!(CircuitComponent::new(BRIDGE_ID, "bad", build_inverter(5.0), config).is_err());
}

const NAND_INPUT_A: PortId = PortId(1);
const NAND_INPUT_B: PortId = PortId(2);
const NAND_OUTPUT: PortId = PortId(3);

fn build_nand2(vdd: f64) -> Circuit {
    let mut circuit = Circuit::new();
    let n_vdd = circuit.get_or_add_node("vdd");
    let n_a = circuit.get_or_add_node("a");
    let n_b = circuit.get_or_add_node("b");
    let n_out = circuit.get_or_add_node("out");

    circuit.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, vdd);
    circuit.add_voltage_source("Vin_A", n_a, NodeId::GROUND, 0.0);
    circuit.add_voltage_source("Vin_B", n_b, NodeId::GROUND, 0.0);
    cmos_nand(CmosGateConfig::default())
        .instantiate(
            &mut circuit,
            "NAND1",
            &[
                ("A", n_a),
                ("B", n_b),
                ("OUT", n_out),
                ("VDD", n_vdd),
                ("GND", NodeId::GROUND),
            ],
        )
        .expect("nand2 instantiated");
    circuit.add_capacitor("CL", n_out, NodeId::GROUND, 10e-12);
    circuit
}

fn build_nand_simulation() -> (Simulation, SignalLog) {
    let circuit = build_nand2(5.0);
    let n_out = circuit.node_by_name("out").unwrap();
    let bridge = CircuitComponent::new(
        BRIDGE_ID,
        "BridgeNand2",
        circuit,
        BridgeConfig {
            time_base: TimeBase::default(),
            solver_dt: 5e-11,
            sync_interval_ticks: 1,
            supply_source: Some("Vdd".to_string()),
            inputs: vec![
                BridgeInput::new(NAND_INPUT_A, "A", "Vin_A", 0.0, 5.0),
                BridgeInput::new(NAND_INPUT_B, "B", "Vin_B", 0.0, 5.0),
            ],
            outputs: vec![BridgeOutput::new(NAND_OUTPUT, "OUT", n_out, 2.5)],
        },
    )
    .expect("nand bridge constructed");

    let log = Arc::new(Mutex::new(Vec::new()));
    let sink = SignalSink::new(SINK_ID, "Sink", log.clone());

    let mut sim = Simulation::new();
    sim.register_component(Box::new(bridge));
    sim.register_component(Box::new(sink));
    sim.register_connection(Connection::new(BRIDGE_ID, NAND_OUTPUT, SINK_ID, SINK_PORT));
    (sim, log)
}

fn schedule_port_input(sim: &mut Simulation, tick: u64, port: PortId, value: bool) {
    sim.schedule_event(Event::at(
        SimulationTime::new(tick),
        ComponentId(999),
        BRIDGE_ID,
        EventPayload::PortSignal {
            port_id: port,
            value: StateValue::Bool(value),
        },
    ));
}

#[test]
fn bridge_nand2_falls_only_when_both_inputs_are_high() {
    let measured = Characterizer::with_defaults()
        .nand2()
        .expect("nand2 characterized");
    let fall_delay = TimeBase::default().delay_ticks(measured.timing.t_pHL);
    let rise_delay = TimeBase::default().delay_ticks(measured.timing.t_pLH);

    let (mut sim, log) = build_nand_simulation();
    schedule_port_input(&mut sim, 0, NAND_INPUT_A, true);
    schedule_port_input(&mut sim, 5, NAND_INPUT_B, true);
    schedule_port_input(&mut sim, 15, NAND_INPUT_B, false);
    sim.run_until(SimulationTime::new(40));

    let events = log.lock().unwrap().clone();
    println!("nand bridge events: {events:?}");

    assert_eq!(events.first(), Some(&(0, true)), "initial output high");

    let fall_tick = events
        .iter()
        .find(|(_, value)| !*value)
        .map(|(tick, _)| *tick)
        .expect("NAND output must fall when both inputs go high");
    let expected_fall = 5 + fall_delay;
    assert!(
        fall_tick.abs_diff(expected_fall) <= 1,
        "NAND fall at {fall_tick} should be within one tick of {expected_fall}"
    );

    let rise_tick = events
        .iter()
        .rev()
        .find(|(_, value)| *value)
        .map(|(tick, _)| *tick)
        .expect("NAND output must rise when an input returns low");
    let expected_rise = 15 + rise_delay;
    assert!(
        rise_tick.abs_diff(expected_rise) <= 1,
        "NAND rise at {rise_tick} should be within one tick of {expected_rise}"
    );
    assert_eq!(events.last().map(|(_, value)| *value), Some(true));

    let state = sim.component_state(BRIDGE_ID).expect("bridge state");
    let energy = state.get_float("energy_joules").expect("energy key");
    assert!(
        energy > 1e-10,
        "expected measurable NAND switching energy, got {energy}"
    );
}

#[test]
fn bridge_nand2_does_not_switch_for_single_high_input() {
    let (mut sim, log) = build_nand_simulation();
    schedule_port_input(&mut sim, 0, NAND_INPUT_A, true);
    sim.run_until(SimulationTime::new(20));

    let events = log.lock().unwrap().clone();
    println!("single-high nand bridge events: {events:?}");
    assert_eq!(
        events,
        vec![(0, true)],
        "A high with B low must leave the NAND output high"
    );
}

const DFF_INPUT_D: PortId = PortId(1);
const DFF_INPUT_CLK: PortId = PortId(2);
const DFF_OUTPUT_Q: PortId = PortId(3);

fn build_dff(vdd: f64) -> Circuit {
    let mut circuit = Circuit::new();
    let n_vdd = circuit.get_or_add_node("vdd");
    let n_d = circuit.get_or_add_node("d");
    let n_clk = circuit.get_or_add_node("clk");
    let n_q = circuit.get_or_add_node("q");
    let n_q_bar = circuit.get_or_add_node("q_bar");

    circuit.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, vdd);
    circuit.add_voltage_source("Vd", n_d, NodeId::GROUND, 0.0);
    circuit.add_voltage_source("Vclk", n_clk, NodeId::GROUND, 0.0);
    cmos_dff(CmosGateConfig::default())
        .instantiate(
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
        )
        .expect("dff instantiated");
    circuit.add_capacitor("CL_Q", n_q, NodeId::GROUND, 10e-12);
    circuit
}

fn build_dff_simulation() -> (Simulation, SignalLog) {
    let circuit = build_dff(5.0);
    let n_q = circuit.node_by_name("q").unwrap();
    let bridge = CircuitComponent::new(
        BRIDGE_ID,
        "BridgeDff",
        circuit,
        BridgeConfig {
            time_base: TimeBase::default(),
            solver_dt: 5e-11,
            sync_interval_ticks: 1,
            supply_source: Some("Vdd".to_string()),
            inputs: vec![
                BridgeInput::new(DFF_INPUT_D, "D", "Vd", 0.0, 5.0),
                BridgeInput::new(DFF_INPUT_CLK, "CLK", "Vclk", 0.0, 5.0),
            ],
            outputs: vec![BridgeOutput::new(DFF_OUTPUT_Q, "Q", n_q, 2.5)],
        },
    )
    .expect("dff bridge constructed");

    let log = Arc::new(Mutex::new(Vec::new()));
    let sink = SignalSink::new(SINK_ID, "Sink", log.clone());

    let mut sim = Simulation::new();
    sim.register_component(Box::new(bridge));
    sim.register_component(Box::new(sink));
    sim.register_connection(Connection::new(BRIDGE_ID, DFF_OUTPUT_Q, SINK_ID, SINK_PORT));
    (sim, log)
}

#[test]
fn bridge_dff_captures_data_on_rising_clock_edge() {
    let measured = Characterizer::with_defaults()
        .dff()
        .expect("dff characterized");
    let rise_delay = TimeBase::default().delay_ticks(measured.timing.t_pLH);
    let fall_delay = TimeBase::default().delay_ticks(measured.timing.t_pHL);

    let (mut sim, log) = build_dff_simulation();

    // Initialize Q low with a clock pulse while D=0.
    schedule_port_input(&mut sim, 1, DFF_INPUT_CLK, true);
    schedule_port_input(&mut sim, 5, DFF_INPUT_CLK, false);

    // D high well before the rising edge at t=25.
    schedule_port_input(&mut sim, 10, DFF_INPUT_D, true);
    schedule_port_input(&mut sim, 25, DFF_INPUT_CLK, true);

    // D low after the edge, then a new rising edge at t=55 captures 0.
    schedule_port_input(&mut sim, 35, DFF_INPUT_D, false);
    schedule_port_input(&mut sim, 40, DFF_INPUT_CLK, false);
    schedule_port_input(&mut sim, 55, DFF_INPUT_CLK, true);

    sim.run_until(SimulationTime::new(70));

    let events = log.lock().unwrap().clone();
    println!("dff bridge events: {events:?}");

    let expected_rise = 25 + rise_delay;
    let rise_tick = events
        .iter()
        .find(|(tick, value)| *value && *tick >= 25)
        .map(|(tick, _)| *tick)
        .expect("DFF bridge must raise Q after the first rising edge");
    assert!(
        rise_tick.abs_diff(expected_rise) <= 1,
        "DFF Q rise at {rise_tick} should be within one tick of {expected_rise}"
    );

    let expected_fall = 55 + fall_delay;
    let fall_tick = events
        .iter()
        .find(|(tick, value)| !*value && *tick >= 55)
        .map(|(tick, _)| *tick)
        .expect("DFF bridge must lower Q after the second rising edge");
    assert!(
        fall_tick.abs_diff(expected_fall) <= 1,
        "DFF Q fall at {fall_tick} should be within one tick of {expected_fall}"
    );
    assert_eq!(events.last().map(|(_, value)| *value), Some(false));

    // D changes while the clock is low must not move Q.
    assert!(
        !events
            .iter()
            .any(|(tick, value)| *value && (10..25).contains(tick)),
        "Q must not rise before the clock edge: {events:?}"
    );

    let state = sim.component_state(BRIDGE_ID).expect("bridge state");
    let energy = state.get_float("energy_joules").expect("energy key");
    assert!(
        energy > 1e-10,
        "expected measurable DFF switching energy, got {energy}"
    );
}
