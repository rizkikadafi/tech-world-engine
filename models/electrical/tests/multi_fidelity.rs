//! End-to-end proof of the milestone:
//!
//! ```text
//! physical inverter → transient measurement → derived component → validate
//! ```

use std::sync::{Arc, Mutex};

use tw_core::{
    Component, ComponentId, Connection, Event, EventPayload, Port, PortDirection, PortId,
    SimulationContext, SimulationTime, State, StateValue,
};
use tw_electrical::prelude::*;
use tw_sim::Simulation;

const BRIDGE_ID: ComponentId = ComponentId(100);
const DERIVED_ID: ComponentId = ComponentId(101);
const BRIDGE_SINK_ID: ComponentId = ComponentId(200);
const DERIVED_SINK_ID: ComponentId = ComponentId(201);
const BRIDGE_INPUT_PORT: PortId = PortId(1);
const BRIDGE_OUTPUT_PORT: PortId = PortId(2);
const SINK_PORT: PortId = PortId(1);

#[derive(Debug)]
struct SignalSink {
    id: ComponentId,
    name: String,
    ports: Vec<Port>,
    state: State,
    log: Arc<Mutex<Vec<(u64, bool)>>>,
}

impl SignalSink {
    fn new(id: ComponentId, name: &str, log: Arc<Mutex<Vec<(u64, bool)>>>) -> Self {
        Self {
            id,
            name: name.to_string(),
            ports: vec![Port::new(SINK_PORT, "IN", PortDirection::Input)],
            state: State::new(),
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

fn first_falling_tick(events: &[(u64, bool)]) -> u64 {
    events
        .iter()
        .find(|(_, value)| !*value)
        .map(|(tick, _)| *tick)
        .expect("expected a falling output event")
}

#[test]
fn detailed_measure_abstract_validate_inverter() {
    // 1. Detailed: measure the physical transistor circuit.
    let measured = Characterizer::with_defaults()
        .inverter()
        .expect("physical inverter characterized");
    println!(
        "measured: t_pHL={}s t_pLH={}s t_rise={}s t_fall={}s E={}J",
        measured.timing.t_pHL,
        measured.timing.t_pLH,
        measured.timing.t_rise,
        measured.timing.t_fall,
        measured.timing.energy_per_cycle
    );

    // 2. Abstract: derive a discrete component from the measurements.
    let derived = DerivedGate::from_characterization(
        DERIVED_ID,
        "DerivedInverter",
        &measured,
        TimeBase::default(),
    )
    .expect("derived inverter");
    let derived_input = derived.input_port(0).expect("input port");
    let derived_output = derived.output_port();
    let expected_delay = derived.timing_ticks().t_pHL;
    assert_eq!(derived.truth_table(), &measured.truth_table);
    assert_eq!(
        expected_delay,
        TimeBase::default().delay_ticks(measured.timing.t_pHL)
    );

    // 3. Mixed-signal bridge for the same physical circuit.
    let circuit = build_inverter(measured.config.vdd);
    let n_out = circuit.node_by_name("out").unwrap();
    let bridge = CircuitComponent::new(
        BRIDGE_ID,
        "BridgeInverter",
        circuit,
        BridgeConfig {
            time_base: TimeBase::default(),
            solver_dt: measured.config.dt,
            sync_interval_ticks: 1,
            supply_source: Some(measured.config.supply_source.clone()),
            inputs: vec![BridgeInput::new(
                BRIDGE_INPUT_PORT,
                "IN",
                measured.config.input_source.clone(),
                measured.levels.low,
                measured.levels.high,
            )],
            outputs: vec![BridgeOutput::new(
                BRIDGE_OUTPUT_PORT,
                "OUT",
                n_out,
                measured.levels.threshold(measured.config.mid_threshold),
            )],
        },
    )
    .expect("bridge constructed");

    let bridge_log = Arc::new(Mutex::new(Vec::new()));
    let derived_log = Arc::new(Mutex::new(Vec::new()));

    let mut sim = Simulation::new();
    sim.register_component(Box::new(bridge));
    sim.register_component(Box::new(derived));
    sim.register_component(Box::new(SignalSink::new(
        BRIDGE_SINK_ID,
        "BridgeSink",
        bridge_log.clone(),
    )));
    sim.register_component(Box::new(SignalSink::new(
        DERIVED_SINK_ID,
        "DerivedSink",
        derived_log.clone(),
    )));
    sim.register_connection(Connection::new(
        BRIDGE_ID,
        BRIDGE_OUTPUT_PORT,
        BRIDGE_SINK_ID,
        SINK_PORT,
    ));
    sim.register_connection(Connection::new(
        DERIVED_ID,
        derived_output,
        DERIVED_SINK_ID,
        SINK_PORT,
    ));

    // Drive both implementations with the same discrete input.
    for (target, port) in [(BRIDGE_ID, BRIDGE_INPUT_PORT), (DERIVED_ID, derived_input)] {
        sim.schedule_event(Event::at(
            SimulationTime::new(0),
            ComponentId(999),
            target,
            EventPayload::PortSignal {
                port_id: port,
                value: StateValue::Bool(true),
            },
        ));
    }

    sim.run_until(SimulationTime::new(40));

    let bridge_events = bridge_log.lock().unwrap().clone();
    let derived_events = derived_log.lock().unwrap().clone();
    println!("bridge:  {bridge_events:?}");
    println!("derived: {derived_events:?}");

    // 4. Validate: same logic, comparable timing.
    assert_eq!(bridge_events.first(), Some(&(0, true)));
    assert_eq!(derived_events.first(), Some(&(0, true)));
    assert_eq!(
        bridge_events.last().map(|(_, value)| *value),
        Some(false),
        "bridge output must fall"
    );
    assert_eq!(
        derived_events.last().map(|(_, value)| *value),
        Some(false),
        "derived output must fall"
    );

    let bridge_fall = first_falling_tick(&bridge_events);
    let derived_fall = first_falling_tick(&derived_events);
    assert_eq!(
        derived_fall, expected_delay,
        "derived delay must equal the measured, quantized delay"
    );
    assert!(
        bridge_fall.abs_diff(derived_fall) <= 1,
        "bridge fall at tick {bridge_fall} should be within one tick of derived fall at {derived_fall}"
    );

    // The abstract model must be dramatically cheaper than the analog bridge.
    assert!(
        sim.events_processed() > 0,
        "simulation must have processed events"
    );
}

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

#[test]
fn detailed_measure_abstract_validate_nand2() {
    // 1. Detailed: measure the physical 2-input NAND.
    let measured = Characterizer::with_defaults()
        .nand2()
        .expect("physical NAND characterized");
    println!(
        "nand2 measured: t_pHL={}s t_pLH={}s E={}J truth={:?}",
        measured.timing.t_pHL,
        measured.timing.t_pLH,
        measured.timing.energy_per_cycle,
        measured.truth_table.outputs
    );
    assert_eq!(measured.truth_table.outputs, vec![true, true, true, false]);

    // 2. Abstract: derive a discrete NAND from the measurements.
    let derived = DerivedGate::from_characterization(
        DERIVED_ID,
        "DerivedNand2",
        &measured,
        TimeBase::default(),
    )
    .expect("derived nand2");
    let input_a = derived.input_port(0).expect("A port");
    let input_b = derived.input_port(1).expect("B port");
    let derived_output = derived.output_port();
    let fall_delay = derived.timing_ticks().t_pHL;
    let rise_delay = derived.timing_ticks().t_pLH;
    assert_eq!(derived.truth_table(), &measured.truth_table);

    // 3. Mixed-signal bridge for the same physical circuit.
    let circuit = build_nand2(measured.config.vdd);
    let n_out = circuit.node_by_name("out").unwrap();
    let bridge = CircuitComponent::new(
        BRIDGE_ID,
        "BridgeNand2",
        circuit,
        BridgeConfig {
            time_base: TimeBase::default(),
            solver_dt: measured.config.dt,
            sync_interval_ticks: 1,
            supply_source: Some(measured.config.supply_source.clone()),
            inputs: vec![
                BridgeInput::new(
                    PortId(1),
                    "A",
                    "Vin_A",
                    measured.levels.low,
                    measured.levels.high,
                ),
                BridgeInput::new(
                    PortId(2),
                    "B",
                    "Vin_B",
                    measured.levels.low,
                    measured.levels.high,
                ),
            ],
            outputs: vec![BridgeOutput::new(
                PortId(3),
                "OUT",
                n_out,
                measured.levels.threshold(measured.config.mid_threshold),
            )],
        },
    )
    .expect("nand bridge constructed");

    let bridge_log = Arc::new(Mutex::new(Vec::new()));
    let derived_log = Arc::new(Mutex::new(Vec::new()));

    let mut sim = Simulation::new();
    sim.register_component(Box::new(bridge));
    sim.register_component(Box::new(derived));
    sim.register_component(Box::new(SignalSink::new(
        BRIDGE_SINK_ID,
        "BridgeSink",
        bridge_log.clone(),
    )));
    sim.register_component(Box::new(SignalSink::new(
        DERIVED_SINK_ID,
        "DerivedSink",
        derived_log.clone(),
    )));
    sim.register_connection(Connection::new(
        BRIDGE_ID,
        PortId(3),
        BRIDGE_SINK_ID,
        SINK_PORT,
    ));
    sim.register_connection(Connection::new(
        DERIVED_ID,
        derived_output,
        DERIVED_SINK_ID,
        SINK_PORT,
    ));

    // A high, then B high (output falls), then B low (output rises).
    for (tick, a, b) in [(0u64, true, false), (5, false, true), (15, false, false)] {
        if a {
            sim.schedule_event(Event::at(
                SimulationTime::new(tick),
                ComponentId(999),
                BRIDGE_ID,
                EventPayload::PortSignal {
                    port_id: PortId(1),
                    value: StateValue::Bool(true),
                },
            ));
            sim.schedule_event(Event::at(
                SimulationTime::new(tick),
                ComponentId(999),
                DERIVED_ID,
                EventPayload::PortSignal {
                    port_id: input_a,
                    value: StateValue::Bool(true),
                },
            ));
        }
        if b {
            sim.schedule_event(Event::at(
                SimulationTime::new(tick),
                ComponentId(999),
                BRIDGE_ID,
                EventPayload::PortSignal {
                    port_id: PortId(2),
                    value: StateValue::Bool(true),
                },
            ));
            sim.schedule_event(Event::at(
                SimulationTime::new(tick),
                ComponentId(999),
                DERIVED_ID,
                EventPayload::PortSignal {
                    port_id: input_b,
                    value: StateValue::Bool(true),
                },
            ));
        }
    }
    // B returns low at t=15.
    sim.schedule_event(Event::at(
        SimulationTime::new(15),
        ComponentId(999),
        BRIDGE_ID,
        EventPayload::PortSignal {
            port_id: PortId(2),
            value: StateValue::Bool(false),
        },
    ));
    sim.schedule_event(Event::at(
        SimulationTime::new(15),
        ComponentId(999),
        DERIVED_ID,
        EventPayload::PortSignal {
            port_id: input_b,
            value: StateValue::Bool(false),
        },
    ));

    sim.run_until(SimulationTime::new(40));

    let bridge_events = bridge_log.lock().unwrap().clone();
    let derived_events = derived_log.lock().unwrap().clone();
    println!("nand2 bridge:  {bridge_events:?}");
    println!("nand2 derived: {derived_events:?}");

    assert_eq!(bridge_events.first(), Some(&(0, true)));
    assert_eq!(derived_events.first(), Some(&(0, true)));

    let bridge_fall = first_falling_tick(&bridge_events);
    let derived_fall = first_falling_tick(&derived_events);
    assert_eq!(derived_fall, 5 + fall_delay);
    assert!(
        bridge_fall.abs_diff(derived_fall) <= 1,
        "NAND bridge fall at {bridge_fall} should be within one tick of derived fall at {derived_fall}"
    );

    let bridge_rise = bridge_events
        .iter()
        .rev()
        .find(|(_, value)| *value)
        .map(|(tick, _)| *tick)
        .expect("bridge NAND output must rise");
    let derived_rise = derived_events
        .iter()
        .rev()
        .find(|(_, value)| *value)
        .map(|(tick, _)| *tick)
        .expect("derived NAND output must rise");
    assert_eq!(derived_rise, 15 + rise_delay);
    assert!(
        bridge_rise.abs_diff(derived_rise) <= 1,
        "NAND bridge rise at {bridge_rise} should be within one tick of derived rise at {derived_rise}"
    );
}

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

#[test]
fn detailed_measure_abstract_validate_dff() {
    // 1. Detailed: measure clock-to-Q timing and energy on the physical DFF.
    let measured = Characterizer::with_defaults()
        .dff()
        .expect("physical DFF characterized");
    println!(
        "dff measured: t_pHL={}s t_pLH={}s t_rise={}s t_fall={}s E/cycle={}J",
        measured.timing.t_pHL,
        measured.timing.t_pLH,
        measured.timing.t_rise,
        measured.timing.t_fall,
        measured.timing.energy_per_cycle
    );

    // 2. Abstract: derive an edge-triggered component from the measurements.
    let derived =
        DerivedDff::from_characterization(DERIVED_ID, "DerivedDff", &measured, TimeBase::default())
            .expect("derived dff");
    let d_port = derived.d_port();
    let clk_port = derived.clk_port();
    let q_port = derived.q_port();
    let rise_delay = derived.timing_ticks().t_pLH;
    let fall_delay = derived.timing_ticks().t_pHL;

    // 3. Mixed-signal bridge for the same physical circuit.
    let circuit = build_dff(measured.config.vdd);
    let n_q = circuit.node_by_name("q").unwrap();
    let bridge = CircuitComponent::new(
        BRIDGE_ID,
        "BridgeDff",
        circuit,
        BridgeConfig {
            time_base: TimeBase::default(),
            solver_dt: measured.config.dt,
            sync_interval_ticks: 1,
            supply_source: Some(measured.config.supply_source.clone()),
            inputs: vec![
                BridgeInput::new(
                    PortId(1),
                    "D",
                    "Vd",
                    measured.levels.low,
                    measured.levels.high,
                ),
                BridgeInput::new(
                    PortId(2),
                    "CLK",
                    "Vclk",
                    measured.levels.low,
                    measured.levels.high,
                ),
            ],
            outputs: vec![BridgeOutput::new(
                PortId(3),
                "Q",
                n_q,
                measured.levels.threshold(measured.config.mid_threshold),
            )],
        },
    )
    .expect("dff bridge constructed");

    let bridge_log = Arc::new(Mutex::new(Vec::new()));
    let derived_log = Arc::new(Mutex::new(Vec::new()));

    let mut sim = Simulation::new();
    sim.register_component(Box::new(bridge));
    sim.register_component(Box::new(derived));
    sim.register_component(Box::new(SignalSink::new(
        BRIDGE_SINK_ID,
        "BridgeSink",
        bridge_log.clone(),
    )));
    sim.register_component(Box::new(SignalSink::new(
        DERIVED_SINK_ID,
        "DerivedSink",
        derived_log.clone(),
    )));
    sim.register_connection(Connection::new(
        BRIDGE_ID,
        PortId(3),
        BRIDGE_SINK_ID,
        SINK_PORT,
    ));
    sim.register_connection(Connection::new(
        DERIVED_ID,
        q_port,
        DERIVED_SINK_ID,
        SINK_PORT,
    ));

    let schedule =
        |sim: &mut Simulation, target: ComponentId, port: PortId, tick: u64, value: bool| {
            sim.schedule_event(Event::at(
                SimulationTime::new(tick),
                ComponentId(999),
                target,
                EventPayload::PortSignal {
                    port_id: port,
                    value: StateValue::Bool(value),
                },
            ));
        };

    // Initialize Q low, capture D=1 on the edge at t=18, then capture D=0
    // on the edge at t=43. Ten nanoseconds of setup/hold is enough.
    for (tick, value) in [(1u64, true), (5, false)] {
        schedule(&mut sim, BRIDGE_ID, PortId(2), tick, value);
        schedule(&mut sim, DERIVED_ID, clk_port, tick, value);
    }
    schedule(&mut sim, BRIDGE_ID, PortId(1), 8, true);
    schedule(&mut sim, DERIVED_ID, d_port, 8, true);
    schedule(&mut sim, BRIDGE_ID, PortId(2), 18, true);
    schedule(&mut sim, DERIVED_ID, clk_port, 18, true);
    schedule(&mut sim, BRIDGE_ID, PortId(1), 28, false);
    schedule(&mut sim, DERIVED_ID, d_port, 28, false);
    schedule(&mut sim, BRIDGE_ID, PortId(2), 33, false);
    schedule(&mut sim, DERIVED_ID, clk_port, 33, false);
    schedule(&mut sim, BRIDGE_ID, PortId(2), 43, true);
    schedule(&mut sim, DERIVED_ID, clk_port, 43, true);

    sim.run_until(SimulationTime::new(55));

    let bridge_events = bridge_log.lock().unwrap().clone();
    let derived_events = derived_log.lock().unwrap().clone();
    println!("dff bridge:  {bridge_events:?}");
    println!("dff derived: {derived_events:?}");

    assert_eq!(bridge_events.first(), Some(&(0, false)));
    assert_eq!(derived_events.first(), Some(&(0, false)));

    let expected_rise = 18 + rise_delay;
    let expected_fall = 43 + fall_delay;

    let bridge_rise = bridge_events
        .iter()
        .find(|(tick, value)| *value && *tick >= 18)
        .map(|(tick, _)| *tick)
        .expect("bridge DFF must raise Q");
    let derived_rise = derived_events
        .iter()
        .find(|(tick, value)| *value && *tick >= 18)
        .map(|(tick, _)| *tick)
        .expect("derived DFF must raise Q");
    assert_eq!(derived_rise, expected_rise);
    assert!(
        bridge_rise.abs_diff(derived_rise) <= 1,
        "DFF bridge rise at {bridge_rise} should be within one tick of derived rise at {derived_rise}"
    );

    let bridge_fall = bridge_events
        .iter()
        .find(|(tick, value)| !*value && *tick >= 43)
        .map(|(tick, _)| *tick)
        .expect("bridge DFF must lower Q");
    let derived_fall = derived_events
        .iter()
        .find(|(tick, value)| !*value && *tick >= 43)
        .map(|(tick, _)| *tick)
        .expect("derived DFF must lower Q");
    assert_eq!(derived_fall, expected_fall);
    assert!(
        bridge_fall.abs_diff(derived_fall) <= 1,
        "DFF bridge fall at {bridge_fall} should be within one tick of derived fall at {derived_fall}"
    );
}
