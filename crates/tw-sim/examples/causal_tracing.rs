//! Observation (event log) + causal tracing through a signal pipeline:
//! `SignalSource -> BufferGate -> InverterGate`. This is a distinct demo (not a
//! scenario) so it keeps its own components, but it uses [`Event::at`] so it
//! never invents event ids.

use tw_core::{
    Component, ComponentId, Connection, Event, EventLogObserver, EventPayload, Port,
    PortDirection, PortId, SimulationContext, SimulationTime, State, TraceId,
};
use tw_sim::{Simulation, SimulationEngine};

#[derive(Debug)]
struct SignalSource {
    id: ComponentId,
    name: String,
    ports: Vec<Port>,
    state: State,
}

impl Component for SignalSource {
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
        let mut outputs = Vec::new();
        if let EventPayload::PortSignal { ref value, .. } = event.payload {
            self.state.set("val", value);
            outputs.push(Event::at(
                event.time,
                self.id,
                self.id,
                EventPayload::PortSignal {
                    port_id: PortId(10),
                    value: value.clone(),
                },
            ));
        }
        outputs
    }
}

#[derive(Debug)]
struct BufferGate {
    id: ComponentId,
    name: String,
    ports: Vec<Port>,
    state: State,
}

impl Component for BufferGate {
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
        let mut outputs = Vec::new();
        if let EventPayload::PortSignal { ref value, .. } = event.payload {
            self.state.set("val", value);
            outputs.push(Event::at(
                event.time,
                self.id,
                self.id,
                EventPayload::PortSignal {
                    port_id: PortId(21),
                    value: value.clone(),
                },
            ));
        }
        outputs
    }
}

#[derive(Debug)]
struct InverterGate {
    id: ComponentId,
    name: String,
    ports: Vec<Port>,
    state: State,
}

impl Component for InverterGate {
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
        let mut outputs = Vec::new();
        if let EventPayload::PortSignal { ref value, .. } = event.payload {
            let inv = if value == "1" { "0" } else { "1" };
            self.state.set("val", inv);
            outputs.push(Event::at(
                event.time,
                self.id,
                self.id,
                EventPayload::PortSignal {
                    port_id: PortId(31),
                    value: inv.to_string(),
                },
            ));
        }
        outputs
    }
}

fn main() {
    println!("=== Tech World Engine Phase 4: Observation & Causal Tracing Example ===");

    let mut sim = Simulation::new();
    sim.register_observer(Box::new(EventLogObserver::new()));

    let src_id = ComponentId(1);
    let buf_id = ComponentId(2);
    let inv_id = ComponentId(3);

    sim.register_component(Box::new(SignalSource {
        id: src_id,
        name: "SignalSource".to_string(),
        ports: vec![Port::new(PortId(10), "out", PortDirection::Output)],
        state: State::new(),
    }));
    sim.register_component(Box::new(BufferGate {
        id: buf_id,
        name: "BufferGate".to_string(),
        ports: vec![
            Port::new(PortId(20), "in", PortDirection::Input),
            Port::new(PortId(21), "out", PortDirection::Output),
        ],
        state: State::new(),
    }));
    sim.register_component(Box::new(InverterGate {
        id: inv_id,
        name: "InverterGate".to_string(),
        ports: vec![
            Port::new(PortId(30), "in", PortDirection::Input),
            Port::new(PortId(31), "out", PortDirection::Output),
        ],
        state: State::new(),
    }));

    sim.register_connection(Connection::new(src_id, PortId(10), buf_id, PortId(20)));
    sim.register_connection(Connection::new(buf_id, PortId(21), inv_id, PortId(30)));

    println!("Triggering initial input '1' on SignalSource at t=0...");
    sim.schedule_event(Event::at(
        SimulationTime::new(0),
        ComponentId(0),
        src_id,
        EventPayload::PortSignal {
            port_id: PortId(10),
            value: "1".to_string(),
        },
    ));

    let mut engine = SimulationEngine::new(sim);
    engine.run_ticks(10);

    let final_val = engine
        .simulation()
        .component_state(inv_id)
        .and_then(|s| s.get("val"))
        .unwrap_or("<none>");
    println!("Inverter final value: {final_val}");

    println!("\n--- Causal Tracing Query (WHY did the final event happen?) ---");
    let nodes = engine.simulation().causal_graph().nodes();
    let max = nodes.keys().map(|t| t.0).max().unwrap_or(1);
    for (step, node) in engine.simulation().explain(TraceId(max)).iter().enumerate() {
        println!(
            "  Step {}: [t={}] {}",
            step + 1,
            node.time.ticks(),
            node.description
        );
    }

    println!("\nPhase 4 Observation & Causal Tracing verified successfully!");
}
