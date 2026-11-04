//! Multi-fidelity walkthrough:
//!
//! ```text
//! transistor circuit → measure → derived component → run both in tw-sim
//! ```
//!
//! Demonstrates the inverter, 2-input NAND, and positive-edge DFF.
//!
//! Run with:
//! ```bash
//! cargo run -p tw-electrical --example multi_fidelity
//! ```

use std::sync::{Arc, Mutex};

use tw_core::{
    Component, ComponentId, Connection, Event, EventPayload, Port, PortDirection, PortId,
    SimulationContext, SimulationTime, State, StateValue,
};
use tw_electrical::prelude::*;
use tw_sim::Simulation;

const SINK_PORT: PortId = PortId(1);
const VDD: f64 = 5.0;

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
                    self.log.lock().unwrap().push((event.time.ticks(), value));
                }
            }
        }
        Vec::new()
    }
}

fn schedule(sim: &mut Simulation, target: ComponentId, port: PortId, tick: u64, value: bool) {
    sim.schedule_event(Event::at(
        SimulationTime::new(tick),
        ComponentId(999),
        target,
        EventPayload::PortSignal {
            port_id: port,
            value: StateValue::Bool(value),
        },
    ));
}

fn print_measurement(label: &str, measured: &GateCharacterization) {
    println!("=== physical {label} ===");
    println!(
        "logic levels: low={:.3} V high={:.3} V",
        measured.levels.low, measured.levels.high
    );
    println!(
        "delay: t_pHL={:.3} ns t_pLH={:.3} ns",
        measured.timing.t_pHL * 1e9,
        measured.timing.t_pLH * 1e9
    );
    println!(
        "edges: t_rise={:.3} ns t_fall={:.3} ns",
        measured.timing.t_rise * 1e9,
        measured.timing.t_fall * 1e9
    );
    println!(
        "energy: rise={:.3} pJ fall={:.3} pJ cycle={:.3} pJ",
        measured.timing.energy_rise * 1e12,
        measured.timing.energy_fall * 1e12,
        measured.timing.energy_per_cycle * 1e12
    );
    println!("truth table: {:?}", measured.truth_table.outputs);
}

fn build_inverter() -> Circuit {
    let mut circuit = Circuit::new();
    let n_vdd = circuit.get_or_add_node("vdd");
    let n_in = circuit.get_or_add_node("in");
    let n_out = circuit.get_or_add_node("out");

    circuit.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, VDD);
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

fn build_nand2() -> Circuit {
    let mut circuit = Circuit::new();
    let n_vdd = circuit.get_or_add_node("vdd");
    let n_a = circuit.get_or_add_node("a");
    let n_b = circuit.get_or_add_node("b");
    let n_out = circuit.get_or_add_node("out");

    circuit.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, VDD);
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

fn build_dff() -> Circuit {
    let mut circuit = Circuit::new();
    let n_vdd = circuit.get_or_add_node("vdd");
    let n_d = circuit.get_or_add_node("d");
    let n_clk = circuit.get_or_add_node("clk");
    let n_q = circuit.get_or_add_node("q");
    let n_q_bar = circuit.get_or_add_node("q_bar");

    circuit.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, VDD);
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

fn demo_inverter(time_base: TimeBase) {
    let measured = Characterizer::with_defaults()
        .inverter()
        .expect("physical inverter characterized");
    print_measurement("inverter", &measured);

    let derived =
        DerivedGate::from_characterization(ComponentId(2), "DerivedInverter", &measured, time_base)
            .expect("derived inverter");
    let derived_input = derived.input_port(0).unwrap();
    let derived_output = derived.output_port();
    let expected_fall_tick = derived.timing_ticks().t_pHL;
    println!("\n=== derived inverter ===");
    println!("truth table: {:?}", derived.truth_table().outputs);
    println!(
        "quantized delay: t_pHL={} ticks t_pLH={} ticks ({:.1} ns/tick)",
        derived.timing_ticks().t_pHL,
        derived.timing_ticks().t_pLH,
        time_base.seconds_per_tick * 1e9
    );

    let circuit = build_inverter();
    let n_out = circuit.node_by_name("out").unwrap();
    let bridge = CircuitComponent::new(
        ComponentId(1),
        "BridgeInverter",
        circuit,
        BridgeConfig {
            time_base,
            solver_dt: 5e-11,
            sync_interval_ticks: 1,
            supply_source: Some("Vdd".to_string()),
            inputs: vec![BridgeInput::new(PortId(1), "IN", "Vin", 0.0, VDD)],
            outputs: vec![BridgeOutput::new(PortId(2), "OUT", n_out, 2.5)],
        },
    )
    .expect("bridge constructed");

    let bridge_log: SignalLog = Arc::new(Mutex::new(Vec::new()));
    let derived_log: SignalLog = Arc::new(Mutex::new(Vec::new()));

    let mut sim = Simulation::new();
    sim.register_component(Box::new(bridge));
    sim.register_component(Box::new(derived));
    sim.register_component(Box::new(SignalSink::new(
        ComponentId(3),
        "BridgeSink",
        bridge_log.clone(),
    )));
    sim.register_component(Box::new(SignalSink::new(
        ComponentId(4),
        "DerivedSink",
        derived_log.clone(),
    )));
    sim.register_connection(Connection::new(
        ComponentId(1),
        PortId(2),
        ComponentId(3),
        SINK_PORT,
    ));
    sim.register_connection(Connection::new(
        ComponentId(2),
        derived_output,
        ComponentId(4),
        SINK_PORT,
    ));

    schedule(&mut sim, ComponentId(1), PortId(1), 0, true);
    schedule(&mut sim, ComponentId(2), derived_input, 0, true);

    sim.run_until(SimulationTime::new(40));

    println!("\n=== inverter: same discrete input applied (0 → 1) ===");
    println!(
        "bridge  output transitions: {:?}",
        bridge_log.lock().unwrap()
    );
    println!(
        "derived output transitions: {:?}",
        derived_log.lock().unwrap()
    );
    println!("derived expected fall tick: {expected_fall_tick}");
}

fn demo_nand2(time_base: TimeBase) {
    let measured = Characterizer::with_defaults()
        .nand2()
        .expect("physical NAND characterized");
    print_measurement("NAND2", &measured);

    let derived =
        DerivedGate::from_characterization(ComponentId(12), "DerivedNand2", &measured, time_base)
            .expect("derived nand2");
    let derived_a = derived.input_port(0).unwrap();
    let derived_b = derived.input_port(1).unwrap();
    let derived_output = derived.output_port();
    println!("\n=== derived NAND2 ===");
    println!("truth table: {:?}", derived.truth_table().outputs);
    println!(
        "quantized delay: t_pHL={} ticks t_pLH={} ticks",
        derived.timing_ticks().t_pHL,
        derived.timing_ticks().t_pLH
    );

    let circuit = build_nand2();
    let n_out = circuit.node_by_name("out").unwrap();
    let bridge = CircuitComponent::new(
        ComponentId(11),
        "BridgeNand2",
        circuit,
        BridgeConfig {
            time_base,
            solver_dt: 5e-11,
            sync_interval_ticks: 1,
            supply_source: Some("Vdd".to_string()),
            inputs: vec![
                BridgeInput::new(PortId(1), "A", "Vin_A", 0.0, VDD),
                BridgeInput::new(PortId(2), "B", "Vin_B", 0.0, VDD),
            ],
            outputs: vec![BridgeOutput::new(PortId(3), "OUT", n_out, 2.5)],
        },
    )
    .expect("nand bridge constructed");

    let bridge_log: SignalLog = Arc::new(Mutex::new(Vec::new()));
    let derived_log: SignalLog = Arc::new(Mutex::new(Vec::new()));

    let mut sim = Simulation::new();
    sim.register_component(Box::new(bridge));
    sim.register_component(Box::new(derived));
    sim.register_component(Box::new(SignalSink::new(
        ComponentId(13),
        "BridgeSink",
        bridge_log.clone(),
    )));
    sim.register_component(Box::new(SignalSink::new(
        ComponentId(14),
        "DerivedSink",
        derived_log.clone(),
    )));
    sim.register_connection(Connection::new(
        ComponentId(11),
        PortId(3),
        ComponentId(13),
        SINK_PORT,
    ));
    sim.register_connection(Connection::new(
        ComponentId(12),
        derived_output,
        ComponentId(14),
        SINK_PORT,
    ));

    // A high, then B high (output falls), then B low (output rises).
    for (tick, port, derived_port, value) in [
        (0u64, PortId(1), derived_a, true),
        (5, PortId(2), derived_b, true),
        (15, PortId(2), derived_b, false),
    ] {
        schedule(&mut sim, ComponentId(11), port, tick, value);
        schedule(&mut sim, ComponentId(12), derived_port, tick, value);
    }

    sim.run_until(SimulationTime::new(40));

    println!("\n=== NAND2: A high, B pulses high then low ===");
    println!(
        "bridge  output transitions: {:?}",
        bridge_log.lock().unwrap()
    );
    println!(
        "derived output transitions: {:?}",
        derived_log.lock().unwrap()
    );
}

fn demo_dff(time_base: TimeBase) {
    let measured = Characterizer::with_defaults()
        .dff()
        .expect("physical DFF characterized");
    print_measurement("DFF (CLK→Q)", &measured);

    let derived =
        DerivedDff::from_characterization(ComponentId(22), "DerivedDff", &measured, time_base)
            .expect("derived dff");
    let derived_d = derived.d_port();
    let derived_clk = derived.clk_port();
    let derived_q = derived.q_port();
    println!("\n=== derived DFF ===");
    println!("next-state table: {:?}", derived.truth_table().outputs);
    println!(
        "quantized delay: t_pHL={} ticks t_pLH={} ticks",
        derived.timing_ticks().t_pHL,
        derived.timing_ticks().t_pLH
    );

    let circuit = build_dff();
    let n_q = circuit.node_by_name("q").unwrap();
    let bridge = CircuitComponent::new(
        ComponentId(21),
        "BridgeDff",
        circuit,
        BridgeConfig {
            time_base,
            solver_dt: 5e-11,
            sync_interval_ticks: 1,
            supply_source: Some("Vdd".to_string()),
            inputs: vec![
                BridgeInput::new(PortId(1), "D", "Vd", 0.0, VDD),
                BridgeInput::new(PortId(2), "CLK", "Vclk", 0.0, VDD),
            ],
            outputs: vec![BridgeOutput::new(PortId(3), "Q", n_q, 2.5)],
        },
    )
    .expect("dff bridge constructed");

    let bridge_log: SignalLog = Arc::new(Mutex::new(Vec::new()));
    let derived_log: SignalLog = Arc::new(Mutex::new(Vec::new()));

    let mut sim = Simulation::new();
    sim.register_component(Box::new(bridge));
    sim.register_component(Box::new(derived));
    sim.register_component(Box::new(SignalSink::new(
        ComponentId(23),
        "BridgeSink",
        bridge_log.clone(),
    )));
    sim.register_component(Box::new(SignalSink::new(
        ComponentId(24),
        "DerivedSink",
        derived_log.clone(),
    )));
    sim.register_connection(Connection::new(
        ComponentId(21),
        PortId(3),
        ComponentId(23),
        SINK_PORT,
    ));
    sim.register_connection(Connection::new(
        ComponentId(22),
        derived_q,
        ComponentId(24),
        SINK_PORT,
    ));

    // Initialize Q low, capture D=1 at t=18, then capture D=0 at t=43.
    for (tick, port, derived_port, value) in [
        (1u64, PortId(2), derived_clk, true),
        (5, PortId(2), derived_clk, false),
        (8, PortId(1), derived_d, true),
        (18, PortId(2), derived_clk, true),
        (28, PortId(1), derived_d, false),
        (33, PortId(2), derived_clk, false),
        (43, PortId(2), derived_clk, true),
    ] {
        schedule(&mut sim, ComponentId(21), port, tick, value);
        schedule(&mut sim, ComponentId(22), derived_port, tick, value);
    }

    sim.run_until(SimulationTime::new(55));

    println!("\n=== DFF: capture 1, then capture 0 on rising edges ===");
    println!(
        "bridge  output transitions: {:?}",
        bridge_log.lock().unwrap()
    );
    println!(
        "derived output transitions: {:?}",
        derived_log.lock().unwrap()
    );
}

fn main() {
    let time_base = TimeBase::default();
    println!(
        "time base: {:.1} ns/tick\n",
        time_base.seconds_per_tick * 1e9
    );

    demo_inverter(time_base);
    println!();
    demo_nand2(time_base);
    println!();
    demo_dff(time_base);
}
