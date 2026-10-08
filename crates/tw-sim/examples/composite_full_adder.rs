//! `CompositeComponent` with **internal connection** inside.
//!
//! FullAdder demonstration composed from two HalfAdder (composite) + one OR gate:
//!
//! ```text
//! FullAdder (#500)
//! ├─ port boundary: A, B, Cin (input) ─ Sum, Carry (output)
//! ├─ HalfAdder1 (#200)   HalfAdder2 (#201)   OR (#30)
//! │
//! │  bind_port (boundary input):  A→HA1.A, B→HA1.B, Cin→HA2.B
//! │  add_internal_connection:     HA1.Sum  → HA2.A      ← focus this example
//! │                               HA1.Carry→ OR.in_a    ←   (child → child,
//! │                               HA2.Carry→ OR.in_b    ←    sync in 1 tick)
//! │  bind_port (boundary output): Sum←HA2.Sum, Carry←OR.out
//! ```
//!
//! Three types of wiring in this engine:
//! 1. Boundary in  — `bind_port(PortBinding)` : external signal goes to the child.
//! 2. **Internal** — `add_internal_connection(Connection)` : child → child in the composite.
//! 3. Boundary out — `bind_port` + `sim.register_connection` top-level : output composite
//!    to consumers outside; without a top-level connection, PortSignal is discarded by the engine.

use std::cell::RefCell;
use std::rc::Rc;

use tw_core::{
    Component, ComponentId, CompositeComponent, Connection, Event, EventId, EventPayload, Port,
    PortBinding, PortDirection, PortId, SimulationContext, SimulationTime, State, TraceId,
    TracingMode,
};
use tw_sim::{Simulation, SimulationEngine};

const FULL_ADDER_ID: ComponentId = ComponentId(500);
const HA1_ID: ComponentId = ComponentId(200);
const HA2_ID: ComponentId = ComponentId(201);
const OR_ID: ComponentId = ComponentId(30);
const PROBE_ID: ComponentId = ComponentId(600);

#[derive(Debug, Clone, Copy)]
enum GateKind {
    Xor,
    And,
    Or,
}

impl GateKind {
    fn label(self) -> &'static str {
        match self {
            GateKind::Xor => "XOR",
            GateKind::And => "AND",
            GateKind::Or => "OR",
        }
    }

    fn base(self) -> u64 {
        match self {
            GateKind::Xor => 100,
            GateKind::And => 110,
            GateKind::Or => 120,
        }
    }

    fn out_port(self) -> PortId {
        PortId(self.base() + 3)
    }

    fn eval(self, a: u8, b: u8) -> u8 {
        match self {
            GateKind::Xor => a ^ b,
            GateKind::And => a & b,
            GateKind::Or => a | b,
        }
    }
}

#[derive(Debug)]
struct Gate {
    id: ComponentId,
    kind: GateKind,
    ports: Vec<Port>,
    state: State,
    a: u8,
    b: u8,
}

impl Gate {
    fn new(id: ComponentId, kind: GateKind) -> Self {
        let base = kind.base();
        Self {
            id,
            kind,
            ports: vec![
                Port::new(PortId(base + 1), "in_a", PortDirection::Input),
                Port::new(PortId(base + 2), "in_b", PortDirection::Input),
                Port::new(PortId(base + 3), "out_y", PortDirection::Output),
            ],
            state: State::new(),
            a: 0,
            b: 0,
        }
    }
}

impl Component for Gate {
    fn id(&self) -> ComponentId {
        self.id
    }

    fn name(&self) -> &str {
        self.kind.label()
    }

    fn ports(&self) -> &[Port] {
        &self.ports
    }

    fn handle_event(&mut self, event: &Event, _ctx: &mut SimulationContext) -> Vec<Event> {
        let mut outputs = Vec::new();
        if let EventPayload::PortSignal { port_id, ref value } = event.payload {
            let val: u8 = value.parse().unwrap_or(0);
            let base = self.kind.base();
            if port_id == PortId(base + 1) {
                self.a = val;
            } else if port_id == PortId(base + 2) {
                self.b = val;
            } else {
                return outputs;
            }

            let y = self.kind.eval(self.a, self.b);
            self.state.set("y", y.to_string());
            outputs.push(Event::new(
                EventId(event.id.0 + 10),
                event.time,
                self.id,
                self.id,
                EventPayload::PortSignal {
                    port_id: self.kind.out_port(),
                    value: y.to_string(),
                },
            ));
        }
        outputs
    }

    fn state(&self) -> &State {
        &self.state
    }
}

/// Top-level probe: receives the FullAdder boundary output result via
/// `sim.register_connection` and stores it for verification.
#[derive(Debug)]
struct Probe {
    id: ComponentId,
    name: String,
    ports: Vec<Port>,
    state: State,
    observed: Rc<RefCell<State>>,
}

impl Probe {
    fn new(id: ComponentId, observed: Rc<RefCell<State>>) -> Self {
        Self {
            id,
            name: "Probe".to_string(),
            ports: vec![
                Port::new(PortId(1), "in_sum", PortDirection::Input),
                Port::new(PortId(2), "in_carry", PortDirection::Input),
            ],
            state: State::new(),
            observed,
        }
    }
}

impl Component for Probe {
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
        if let EventPayload::PortSignal { port_id, ref value } = event.payload {
            let mut observed = self.observed.borrow_mut();
            if port_id == PortId(1) {
                observed.set("sum", value.clone());
                self.state.set("sum", value.clone());
            } else if port_id == PortId(2) {
                observed.set("carry", value.clone());
                self.state.set("carry", value.clone());
            }
        }
        Vec::new()
    }

    fn state(&self) -> &State {
        &self.state
    }
}

/// Half Adder: XOR and AND gates operate in parallel.
///
/// Input fan-out (A/B to both gates) is handled simply via multiple `bind_port` calls—
/// no internal connection is needed since the two child components are not interconnected.
fn create_half_adder(
    name: &str,
    id: ComponentId,
    xor_id: ComponentId,
    and_id: ComponentId,
) -> CompositeComponent {
    let mut ha = CompositeComponent::new(id, name);
    ha.add_port(Port::new(PortId(1), "A", PortDirection::Input));
    ha.add_port(Port::new(PortId(2), "B", PortDirection::Input));
    ha.add_port(Port::new(PortId(3), "Sum", PortDirection::Output));
    ha.add_port(Port::new(PortId(4), "Carry", PortDirection::Output));

    ha.add_subcomponent(Box::new(Gate::new(xor_id, GateKind::Xor)));
    ha.add_subcomponent(Box::new(Gate::new(and_id, GateKind::And)));

    // Boundary input → child
    ha.bind_port(PortBinding::new(PortId(1), xor_id, PortId(101)));
    ha.bind_port(PortBinding::new(PortId(1), and_id, PortId(111)));
    ha.bind_port(PortBinding::new(PortId(2), xor_id, PortId(102)));
    ha.bind_port(PortBinding::new(PortId(2), and_id, PortId(112)));

    // Child output → boundary output
    ha.bind_port(PortBinding::new(PortId(3), xor_id, PortId(103)));
    ha.bind_port(PortBinding::new(PortId(4), and_id, PortId(113)));

    ha
}

fn create_full_adder() -> CompositeComponent {
    let mut fa = CompositeComponent::new(FULL_ADDER_ID, "FullAdder");
    fa.add_port(Port::new(PortId(1), "A", PortDirection::Input));
    fa.add_port(Port::new(PortId(2), "B", PortDirection::Input));
    fa.add_port(Port::new(PortId(3), "Cin", PortDirection::Input));
    fa.add_port(Port::new(PortId(4), "Sum", PortDirection::Output));
    fa.add_port(Port::new(PortId(5), "Carry", PortDirection::Output));

    // ID anak HARUS unik di dalam satu composite: subcomponents disimpan di
    // HashMap keyed by ComponentId, id sama = child pertama ditimpa.
    fa.add_subcomponent(Box::new(create_half_adder(
        "HalfAdder1",
        HA1_ID,
        ComponentId(10),
        ComponentId(11),
    )));
    fa.add_subcomponent(Box::new(create_half_adder(
        "HalfAdder2",
        HA2_ID,
        ComponentId(12),
        ComponentId(13),
    )));
    fa.add_subcomponent(Box::new(Gate::new(OR_ID, GateKind::Or)));

    // --- 1. Boundary input: sinyal eksternal masuk ke child ---
    fa.bind_port(PortBinding::new(PortId(1), HA1_ID, PortId(1))); // A  → HA1.A
    fa.bind_port(PortBinding::new(PortId(2), HA1_ID, PortId(2))); // B  → HA1.B
    fa.bind_port(PortBinding::new(PortId(3), HA2_ID, PortId(2))); // Cin → HA2.B

    // --- 2. INTERNAL CONNECTIONS: child → child di dalam composite ---
    //     Diproses sinkron/rekursif dalam satu tick oleh resolve_sub_outputs.
    fa.add_internal_connection(Connection::new(HA1_ID, PortId(3), HA2_ID, PortId(1)));
    fa.add_internal_connection(Connection::new(HA1_ID, PortId(4), OR_ID, PortId(121)));
    fa.add_internal_connection(Connection::new(HA2_ID, PortId(4), OR_ID, PortId(122)));

    // --- 3. Boundary output: child output → port eksternal composite ---
    fa.bind_port(PortBinding::new(PortId(4), HA2_ID, PortId(3))); // HA2.Sum → Sum
    fa.bind_port(PortBinding::new(PortId(5), OR_ID, PortId(123))); // OR.out  → Carry

    fa
}

/// Build a fresh simulation for a single input combination, run it, and return the engine and probe results.
fn run_full_adder(
    a: u8,
    b: u8,
    cin: u8,
    mode: TracingMode,
) -> (SimulationEngine, Rc<RefCell<State>>) {
    let mut sim = Simulation::new();
    sim.set_tracing_mode(mode);
    sim.register_component(Box::new(create_full_adder()));

    let observed = Rc::new(RefCell::new(State::new()));
    sim.register_component(Box::new(Probe::new(PROBE_ID, Rc::clone(&observed))));

    // Top-level connections: boundary output composite → probe.
    // Without this, the composite output PortSignal is discarded by the engine.
    sim.register_connection(Connection::new(
        FULL_ADDER_ID,
        PortId(4),
        PROBE_ID,
        PortId(1),
    ));
    sim.register_connection(Connection::new(
        FULL_ADDER_ID,
        PortId(5),
        PROBE_ID,
        PortId(2),
    ));

    // External input is targeted at the composite, not the child.
    // ID 1000+ to avoid collisions with engine-generated EventIds.
    for (idx, (port_id, value)) in [(PortId(1), a), (PortId(2), b), (PortId(3), cin)]
        .into_iter()
        .enumerate()
    {
        sim.schedule_event(Event::new(
            EventId(1000 + idx as u64),
            SimulationTime::new(0),
            ComponentId(0),
            FULL_ADDER_ID,
            EventPayload::PortSignal {
                port_id,
                value: value.to_string(),
            },
        ));
    }

    let mut engine = SimulationEngine::new(sim);
    engine.run_ticks(5);
    (engine, observed)
}

fn main() {
    println!("=== Tech World Engine: CompositeComponent Internal Connection ===");

    let demo = create_full_adder();
    println!(
        "\nInternal connections inside '{}' (#{}):",
        demo.name(),
        demo.id().0
    );
    for conn in demo.internal_connections() {
        println!(
            "  Subcomponent #{} port {}  -->  Subcomponent #{} port {}",
            conn.source_component.0,
            conn.source_port.0,
            conn.target_component.0,
            conn.target_port.0
        );
    }
    drop(demo);

    println!("\n--- Full Adder Truth Table (TracingMode::Abstract) ---");
    println!(" A B Cin | Sum Carry | expected | status");
    for i in 0..8u8 {
        let (a, b, cin) = ((i >> 2) & 1, (i >> 1) & 1, i & 1);
        let (_engine, observed) = run_full_adder(a, b, cin, TracingMode::Abstract);

        let obs = observed.borrow();
        let sum: Option<u8> = obs.get("sum").and_then(|v| v.parse().ok());
        let carry: Option<u8> = obs.get("carry").and_then(|v| v.parse().ok());
        drop(obs);

        let (sum, carry) = match (sum, carry) {
            (Some(s), Some(c)) => (s, c),
            _ => panic!(
                "probe tidak menerima Sum/Carry untuk A={} B={} Cin={}",
                a, b, cin
            ),
        };

        let expected_sum = a ^ b ^ cin;
        let expected_carry = ((a + b + cin) >= 2) as u8;
        let ok = sum == expected_sum && carry == expected_carry;

        println!(
            " {}  {}  {}  |   {}     {}    |    {} {}    | {}",
            a,
            b,
            cin,
            sum,
            carry,
            expected_sum,
            expected_carry,
            if ok { "OK" } else { "FAIL" }
        );
        assert!(
            ok,
            "FullAdder salah untuk A={} B={} Cin={}: dapat Sum={} Carry={}, expect Sum={} Carry={}",
            a, b, cin, sum, carry, expected_sum, expected_carry
        );
    }
    println!("Semua 8 kombinasi benar.");

    println!("\n--- Causal Tracing (TracingMode::Hierarchical), A=1 B=1 Cin=1 ---");
    let (engine, _observed) = run_full_adder(1, 1, 1, TracingMode::Hierarchical);
    let sim = engine.simulation();

    let nodes = sim.causal_graph().nodes();
    let internal: Vec<_> = nodes
        .values()
        .filter(|n| n.description.contains("routed Internal Connection"))
        .collect();
    let external_in = nodes
        .values()
        .filter(|n| n.description.contains("routed External Input"))
        .count();
    let boundary_out = nodes
        .values()
        .filter(|n| n.description.contains("bound Subcomponent"))
        .count();

    println!("Total trace nodes: {}", nodes.len());
    println!(
        "  - routed External Input (boundary in)   : {}",
        external_in
    );
    println!(
        "  - routed Internal Connection (child→child): {}",
        internal.len()
    );
    println!(
        "  - bound Subcomponent (boundary out)     : {}",
        boundary_out
    );
    assert!(!internal.is_empty(), "internal connection not trigered!");

    let last_internal = internal.iter().map(|n| n.id.0).max().unwrap();
    println!(
        "\nCausal chain from trace #{} (involve internal connection):",
        last_internal
    );
    for (step, node) in sim.explain(TraceId(last_internal)).iter().enumerate() {
        println!(
            "  Step {}: [t={}] {}",
            step + 1,
            node.time.ticks(),
            node.description
        );
    }

    println!("\nInternal connection example finished successfully!");
}
