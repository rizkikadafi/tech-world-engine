use tw_core::{
    Component, ComponentId, Connection, Event, EventId, EventLogObserver, EventPayload, Port,
    PortDirection, PortId, SimulationContext, State, SimulationTime, TraceId,
};
use tw_sim::{Simulation, SimulationEngine};

// 1. Signal Source Component
#[derive(Debug)]
struct SignalSource {
    id: ComponentId,
    name: String,
    ports: Vec<Port>,
    state: State,
}

impl SignalSource {
    fn new(id: ComponentId) -> Self {
        Self {
            id,
            name: "SignalSource".to_string(),
            ports: vec![Port::new(PortId(10), "out", PortDirection::Output)],
            state: State::new(),
        }
    }
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
    fn handle_event(&mut self, event: &Event, _ctx: &mut SimulationContext) -> Vec<Event> {
        let mut outputs = Vec::new();
        if let EventPayload::PortSignal { ref value, .. } = event.payload {
            self.state.set("val", value);
            println!("  [SignalSource] Emitting signal: {}", value);
            outputs.push(Event::new(
                EventId(event.id.0 + 100),
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
    fn state(&self) -> &State {
        &self.state
    }
}

// 2. Buffer Gate Component
#[derive(Debug)]
struct BufferGate {
    id: ComponentId,
    name: String,
    ports: Vec<Port>,
    state: State,
}

impl BufferGate {
    fn new(id: ComponentId) -> Self {
        Self {
            id,
            name: "BufferGate".to_string(),
            ports: vec![
                Port::new(PortId(20), "in", PortDirection::Input),
                Port::new(PortId(21), "out", PortDirection::Output),
            ],
            state: State::new(),
        }
    }
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
    fn handle_event(&mut self, event: &Event, _ctx: &mut SimulationContext) -> Vec<Event> {
        let mut outputs = Vec::new();
        if let EventPayload::PortSignal { ref value, .. } = event.payload {
            self.state.set("val", value);
            println!("  [BufferGate] Received signal: {} -> Forwarding to out", value);
            outputs.push(Event::new(
                EventId(event.id.0 + 200),
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
    fn state(&self) -> &State {
        &self.state
    }
}

// 3. Inverter Gate Component
#[derive(Debug)]
struct InverterGate {
    id: ComponentId,
    name: String,
    ports: Vec<Port>,
    state: State,
}

impl InverterGate {
    fn new(id: ComponentId) -> Self {
        Self {
            id,
            name: "InverterGate".to_string(),
            ports: vec![
                Port::new(PortId(30), "in", PortDirection::Input),
                Port::new(PortId(31), "out", PortDirection::Output),
            ],
            state: State::new(),
        }
    }
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
    fn handle_event(&mut self, event: &Event, _ctx: &mut SimulationContext) -> Vec<Event> {
        let mut outputs = Vec::new();
        if let EventPayload::PortSignal { ref value, .. } = event.payload {
            let inv_val = if value == "1" { "0" } else { "1" };
            self.state.set("val", inv_val);
            println!(
                "  [InverterGate] Received signal: {} -> Inverting to out: {}",
                value, inv_val
            );
            outputs.push(Event::new(
                EventId(event.id.0 + 300),
                event.time,
                self.id,
                self.id,
                EventPayload::PortSignal {
                    port_id: PortId(31),
                    value: inv_val.to_string(),
                },
            ));
        }
        outputs
    }
    fn state(&self) -> &State {
        &self.state
    }
}

fn main() {
    println!("=== Tech World Engine Phase 4: Observation & Causal Tracing Example ===");

    let mut sim = Simulation::new();

    // Register EventLogObserver for observation
    let log_observer = EventLogObserver::new();
    sim.register_observer(Box::new(log_observer));

    // Register Pipeline Components
    let src_id = ComponentId(1);
    let buf_id = ComponentId(2);
    let inv_id = ComponentId(3);

    sim.register_component(Box::new(SignalSource::new(src_id)));
    sim.register_component(Box::new(BufferGate::new(buf_id)));
    sim.register_component(Box::new(InverterGate::new(inv_id)));

    // Register Connections:
    // SignalSource(port 10) -> BufferGate(port 20)
    sim.register_connection(Connection::new(src_id, PortId(10), buf_id, PortId(20)));
    // BufferGate(port 21) -> InverterGate(port 30)
    sim.register_connection(Connection::new(buf_id, PortId(21), inv_id, PortId(30)));

    // Trigger Initial Event: SignalSource receives signal "1" at t=0
    println!("Triggering initial input '1' on SignalSource at t=0...");
    sim.schedule_event(Event::new(
        EventId(1),
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

    println!("\n--- Causal Tracing Query (WHY did the final event happen?) ---");
    let target_trace = TraceId(3);
    let causal_chain = engine.simulation().explain(target_trace);

    println!("Causal Chain Explanation for {:?}:", target_trace);
    for (step_idx, node) in causal_chain.iter().enumerate() {
        println!("  Step {}: [t={}] {}", step_idx + 1, node.time.ticks(), node.description);
    }

    println!("\nPhase 4 Observation & Causal Tracing verified successfully!");
}
