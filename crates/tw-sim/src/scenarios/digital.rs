//! Digital composition scenario.
//!
//! A `FullAdder` (`CompositeComponent`) built from two `HalfAdder` composites
//! plus an `OR` gate, wired with **internal connections** (child → child) and
//! port bindings. Used to prove emergent construction: the engine has no notion
//! of AND/OR/XOR/HalfAdder/FullAdder — they are all user-defined components.
//!
//! Wiring summary (see also the original example for the diagram):
//! ```text
//! FullAdder (#500)
//! ├─ boundary in : A → HA1.A, B → HA1.B, Cin → HA2.B
//! ├─ internal    : HA1.Sum → HA2.A, HA1.Carry → OR.in_a, HA2.Carry → OR.in_b
//! └─ boundary out: HA2.Sum → Sum, OR.out → Carry
//! ```

use std::collections::HashMap;

use tw_core::{
    Component, ComponentId, CompositeComponent, Connection, Event, EventPayload, Port,
    PortBinding, PortDirection, PortId, SimulationContext, SimulationTime, State, StateValue,
    TraceId, TracingMode,
};

use crate::engine::SimulationEngine;
use crate::simulation::Simulation;

pub const FULL_ADDER_ID: ComponentId = ComponentId(500);
pub const PROBE_ID: ComponentId = ComponentId(600);
const HA1_ID: ComponentId = ComponentId(200);
const HA2_ID: ComponentId = ComponentId(201);
const OR_ID: ComponentId = ComponentId(30);

#[derive(Debug, Clone, Copy)]
pub enum GateKind {
    Xor,
    And,
    Or,
}

impl GateKind {
    pub fn label(self) -> &'static str {
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

    pub fn eval(self, a: u8, b: u8) -> u8 {
        match self {
            GateKind::Xor => a ^ b,
            GateKind::And => a & b,
            GateKind::Or => a | b,
        }
    }
}

#[derive(Debug)]
pub struct Gate {
    id: ComponentId,
    kind: GateKind,
    ports: Vec<Port>,
    state: State,
    a: u8,
    b: u8,
}

impl Gate {
    pub fn new(id: ComponentId, kind: GateKind) -> Self {
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

    fn state(&self) -> &State {
        &self.state
    }

    fn handle_event(&mut self, event: &Event, _ctx: &mut SimulationContext) -> Vec<Event> {
        let mut outputs = Vec::new();
        if let EventPayload::PortSignal { port_id, ref value } = event.payload {
            let val = value.as_int().unwrap_or(0) as u8;
            let base = self.kind.base();
            if port_id == PortId(base + 1) {
                self.a = val;
            } else if port_id == PortId(base + 2) {
                self.b = val;
            } else {
                return outputs;
            }

            let y = self.kind.eval(self.a, self.b);
            self.state.set("y", y);
            outputs.push(Event::at(
                event.time,
                self.id,
                self.id,
                EventPayload::PortSignal {
                    port_id: self.kind.out_port(),
                    value: StateValue::from(y),
                },
            ));
        }
        outputs
    }
}

/// Observes the FullAdder boundary outputs. Stores into its own `State`, which
/// the caller can read back via [`Simulation::component_state`].
#[derive(Debug)]
pub struct Probe {
    id: ComponentId,
    name: String,
    ports: Vec<Port>,
    state: State,
}

impl Probe {
    pub fn new(id: ComponentId) -> Self {
        Self {
            id,
            name: "Probe".to_string(),
            ports: vec![
                Port::new(PortId(1), "in_sum", PortDirection::Input),
                Port::new(PortId(2), "in_carry", PortDirection::Input),
            ],
            state: State::new(),
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

    fn state(&self) -> &State {
        &self.state
    }

    fn handle_event(&mut self, event: &Event, _ctx: &mut SimulationContext) -> Vec<Event> {
        if let EventPayload::PortSignal { port_id, ref value } = event.payload {
            if port_id == PortId(1) {
                self.state.set("sum", value.clone());
            } else if port_id == PortId(2) {
                self.state.set("carry", value.clone());
            }
        }
        Vec::new()
    }
}

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

    ha.bind_port(PortBinding::new(PortId(1), xor_id, PortId(101)));
    ha.bind_port(PortBinding::new(PortId(1), and_id, PortId(111)));
    ha.bind_port(PortBinding::new(PortId(2), xor_id, PortId(102)));
    ha.bind_port(PortBinding::new(PortId(2), and_id, PortId(112)));
    ha.bind_port(PortBinding::new(PortId(3), xor_id, PortId(103)));
    ha.bind_port(PortBinding::new(PortId(4), and_id, PortId(113)));

    ha
}

/// Build the `FullAdder` composite.
pub fn create_full_adder() -> CompositeComponent {
    let mut fa = CompositeComponent::new(FULL_ADDER_ID, "FullAdder");
    fa.add_port(Port::new(PortId(1), "A", PortDirection::Input));
    fa.add_port(Port::new(PortId(2), "B", PortDirection::Input));
    fa.add_port(Port::new(PortId(3), "Cin", PortDirection::Input));
    fa.add_port(Port::new(PortId(4), "Sum", PortDirection::Output));
    fa.add_port(Port::new(PortId(5), "Carry", PortDirection::Output));

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

    fa.bind_port(PortBinding::new(PortId(1), HA1_ID, PortId(1)));
    fa.bind_port(PortBinding::new(PortId(2), HA1_ID, PortId(2)));
    fa.bind_port(PortBinding::new(PortId(3), HA2_ID, PortId(2)));

    fa.add_internal_connection(Connection::new(HA1_ID, PortId(3), HA2_ID, PortId(1)));
    fa.add_internal_connection(Connection::new(HA1_ID, PortId(4), OR_ID, PortId(121)));
    fa.add_internal_connection(Connection::new(HA2_ID, PortId(4), OR_ID, PortId(122)));

    fa.bind_port(PortBinding::new(PortId(4), HA2_ID, PortId(3)));
    fa.bind_port(PortBinding::new(PortId(5), OR_ID, PortId(123)));

    fa
}

/// Build a full-adder simulation, drive it with the given inputs for one tick,
/// and return the engine so callers can inspect trace/probe state.
pub fn run_full_adder(a: u8, b: u8, cin: u8, mode: TracingMode) -> SimulationEngine {
    let mut sim = Simulation::new();
    sim.set_tracing_mode(mode);
    sim.register_component(Box::new(create_full_adder()));
    sim.register_component(Box::new(Probe::new(PROBE_ID)));

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

    for (port_id, value) in [(PortId(1), a), (PortId(2), b), (PortId(3), cin)] {
        sim.schedule_event(Event::at(
            SimulationTime::new(0),
            ComponentId(0),
            FULL_ADDER_ID,
            EventPayload::PortSignal {
                port_id,
                value: StateValue::from(value),
            },
        ));
    }

    let mut engine = SimulationEngine::new(sim);
    engine.run_ticks(5);
    engine
}

/// Read the observed `(Sum, Carry)` from the probe, if present.
pub fn full_adder_result(engine: &SimulationEngine) -> Option<(u8, u8)> {
    let state = engine.simulation().component_state(PROBE_ID)?;
    let sum = state.get("sum")?.as_int()? as u8;
    let carry = state.get("carry")?.as_int()? as u8;
    Some((sum, carry))
}

/// Expected `(Sum, Carry)` for the given inputs.
pub fn expected_full_adder(a: u8, b: u8, cin: u8) -> (u8, u8) {
    (a ^ b ^ cin, ((a + b + cin) >= 2) as u8)
}

/// Collect causal trace node ids sorted ascending (creation order), together
/// with their descriptions. Useful for determinism checks.
pub fn trace_sequence(engine: &SimulationEngine) -> Vec<(TraceId, String)> {
    let nodes: &HashMap<TraceId, tw_core::TraceNode> = engine.simulation().causal_graph().nodes();
    let mut seq: Vec<(TraceId, String)> = nodes
        .iter()
        .map(|(id, node)| (*id, node.description.clone()))
        .collect();
    seq.sort_by_key(|(id, _)| id.0);
    seq
}
