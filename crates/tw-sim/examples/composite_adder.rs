use tw_core::{
    Component, ComponentId, CompositeComponent, Event, EventId, EventLogObserver, EventPayload,
    Port, PortBinding, PortDirection, PortId, SimulationContext, SimulationTime, State, TraceId,
    TracingMode,
};
use tw_sim::{Simulation, SimulationEngine};

// Primitive XOR Gate
#[derive(Debug)]
struct XorGate {
    id: ComponentId,
    name: String,
    ports: Vec<Port>,
    state: State,
    a: u8,
    b: u8,
}

impl XorGate {
    fn new(id: ComponentId) -> Self {
        Self {
            id,
            name: "XorGate".to_string(),
            ports: vec![
                Port::new(PortId(101), "in_a", PortDirection::Input),
                Port::new(PortId(102), "in_b", PortDirection::Input),
                Port::new(PortId(103), "out_y", PortDirection::Output),
            ],
            state: State::new(),
            a: 0,
            b: 0,
        }
    }
}

impl Component for XorGate {
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
        if let EventPayload::PortSignal { port_id, ref value } = event.payload {
            let val: u8 = value.parse().unwrap_or(0);
            if port_id == PortId(101) {
                self.a = val;
            } else if port_id == PortId(102) {
                self.b = val;
            }
            let res = self.a ^ self.b;
            self.state.set("y", res.to_string());
            println!(
                "  [XOR Gate] Input updated (a={}, b={}) => Output Y={}",
                self.a, self.b, res
            );
            outputs.push(Event::new(
                EventId(event.id.0 + 10),
                event.time,
                self.id,
                self.id,
                EventPayload::PortSignal {
                    port_id: PortId(103),
                    value: res.to_string(),
                },
            ));
        }
        outputs
    }
    fn state(&self) -> &State {
        &self.state
    }
}

// Primitive AND Gate
#[derive(Debug)]
struct AndGate {
    id: ComponentId,
    name: String,
    ports: Vec<Port>,
    state: State,
    a: u8,
    b: u8,
}

impl AndGate {
    fn new(id: ComponentId) -> Self {
        Self {
            id,
            name: "AndGate".to_string(),
            ports: vec![
                Port::new(PortId(111), "in_a", PortDirection::Input),
                Port::new(PortId(112), "in_b", PortDirection::Input),
                Port::new(PortId(113), "out_y", PortDirection::Output),
            ],
            state: State::new(),
            a: 0,
            b: 0,
        }
    }
}

impl Component for AndGate {
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
        if let EventPayload::PortSignal { port_id, ref value } = event.payload {
            let val: u8 = value.parse().unwrap_or(0);
            if port_id == PortId(111) {
                self.a = val;
            } else if port_id == PortId(112) {
                self.b = val;
            }
            let res = self.a & self.b;
            self.state.set("y", res.to_string());
            println!(
                "  [AND Gate] Input updated (a={}, b={}) => Output Y={}",
                self.a, self.b, res
            );
            outputs.push(Event::new(
                EventId(event.id.0 + 20),
                event.time,
                self.id,
                self.id,
                EventPayload::PortSignal {
                    port_id: PortId(113),
                    value: res.to_string(),
                },
            ));
        }
        outputs
    }
    fn state(&self) -> &State {
        &self.state
    }
}

fn create_half_adder() -> CompositeComponent {
    let half_adder_id = ComponentId(100);
    let mut half_adder = CompositeComponent::new(half_adder_id, "HalfAdder");
    half_adder.add_port(Port::new(PortId(1), "A", PortDirection::Input));
    half_adder.add_port(Port::new(PortId(2), "B", PortDirection::Input));
    half_adder.add_port(Port::new(PortId(3), "Sum", PortDirection::Output));
    half_adder.add_port(Port::new(PortId(4), "Carry", PortDirection::Output));

    let xor_id = ComponentId(10);
    let and_id = ComponentId(11);
    half_adder.add_subcomponent(Box::new(XorGate::new(xor_id)));
    half_adder.add_subcomponent(Box::new(AndGate::new(and_id)));

    half_adder.bind_port(PortBinding::new(PortId(1), xor_id, PortId(101)));
    half_adder.bind_port(PortBinding::new(PortId(1), and_id, PortId(111)));
    half_adder.bind_port(PortBinding::new(PortId(2), xor_id, PortId(102)));
    half_adder.bind_port(PortBinding::new(PortId(2), and_id, PortId(112)));
    half_adder.bind_port(PortBinding::new(PortId(3), xor_id, PortId(103)));
    half_adder.bind_port(PortBinding::new(PortId(4), and_id, PortId(113)));

    half_adder
}

fn main() {
    println!("=== Tech World Engine: Dynamic TracingMode Demonstration ===");

    // 1. Run in Abstract / Black-Box Mode (Default)
    println!("\n--- [Run 1: TracingMode::Abstract (Black-Box / High Performance)] ---");
    let mut sim_abstract = Simulation::new();
    sim_abstract.set_tracing_mode(TracingMode::Abstract);
    sim_abstract.register_component(Box::new(create_half_adder()));
    sim_abstract.schedule_event(Event::new(
        EventId(1),
        SimulationTime::new(0),
        ComponentId(0),
        ComponentId(100),
        EventPayload::PortSignal { port_id: PortId(1), value: "1".to_string() },
    ));
    sim_abstract.schedule_event(Event::new(
        EventId(2),
        SimulationTime::new(0),
        ComponentId(0),
        ComponentId(100),
        EventPayload::PortSignal { port_id: PortId(2), value: "1".to_string() },
    ));
    let mut engine_abstract = SimulationEngine::new(sim_abstract);
    engine_abstract.run_ticks(5);

    let abstract_nodes = engine_abstract.simulation().causal_graph().nodes();
    println!("Total trace nodes in Abstract Mode: {}", abstract_nodes.len());

    // 2. Run in Hierarchical / Refined Mode
    println!("\n--- [Run 2: TracingMode::Hierarchical (Deep Refinement / Investigation)] ---");
    let mut sim_hierarchical = Simulation::new();
    sim_hierarchical.set_tracing_mode(TracingMode::Hierarchical);
    sim_hierarchical.register_component(Box::new(create_half_adder()));
    sim_hierarchical.register_observer(Box::new(EventLogObserver::new()));
    sim_hierarchical.schedule_event(Event::new(
        EventId(1),
        SimulationTime::new(0),
        ComponentId(0),
        ComponentId(100),
        EventPayload::PortSignal { port_id: PortId(1), value: "1".to_string() },
    ));
    sim_hierarchical.schedule_event(Event::new(
        EventId(2),
        SimulationTime::new(0),
        ComponentId(0),
        ComponentId(100),
        EventPayload::PortSignal { port_id: PortId(2), value: "1".to_string() },
    ));
    let mut engine_hierarchical = SimulationEngine::new(sim_hierarchical);
    engine_hierarchical.run_ticks(5);

    let hier_nodes = engine_hierarchical.simulation().causal_graph().nodes();
    println!("Total trace nodes in Hierarchical Mode: {}", hier_nodes.len());

    let max_trace_id = hier_nodes.keys().map(|t| t.0).max().unwrap_or(1);
    let target_trace = TraceId(max_trace_id);
    let causal_chain = engine_hierarchical.simulation().explain(target_trace);
    println!("\nHierarchical Causal Chain Explanation for Latest Trace {:?}:", target_trace);
    for (step_idx, node) in causal_chain.iter().enumerate() {
        println!(
            "  Step {}: [t={}] {}",
            step_idx + 1,
            node.time.ticks(),
            node.description
        );
    }

    println!("\nDynamic TracingMode demonstration finished successfully!");
}
