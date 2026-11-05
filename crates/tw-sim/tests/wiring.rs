//! Conformance tests for `Simulation::validate_wiring`.

use tw_core::{
    Component, ComponentId, Connection, Event, Port, PortDirection, PortId, SimulationContext,
    State,
};
use tw_sim::{Simulation, WiringError};

const SOURCE: ComponentId = ComponentId(1);
const SINK: ComponentId = ComponentId(2);
const SOURCE_OUT: PortId = PortId(10);
const SINK_IN: PortId = PortId(20);

#[derive(Debug)]
struct Node {
    id: ComponentId,
    name: &'static str,
    ports: Vec<Port>,
    state: State,
}

impl Node {
    fn new(id: ComponentId, name: &'static str, ports: Vec<Port>) -> Self {
        Self {
            id,
            name,
            ports,
            state: State::new(),
        }
    }
}

impl Component for Node {
    fn id(&self) -> ComponentId {
        self.id
    }

    fn name(&self) -> &str {
        self.name
    }

    fn ports(&self) -> &[Port] {
        &self.ports
    }

    fn state(&self) -> &State {
        &self.state
    }

    fn handle_event(&mut self, _event: &Event, _ctx: &mut SimulationContext) -> Vec<Event> {
        Vec::new()
    }
}

fn simulation_with_source_and_sink() -> Simulation {
    let mut sim = Simulation::new();
    sim.register_component(Box::new(Node::new(
        SOURCE,
        "source",
        vec![Port::new(SOURCE_OUT, "out", PortDirection::Output)],
    )));
    sim.register_component(Box::new(Node::new(
        SINK,
        "sink",
        vec![Port::new(SINK_IN, "in", PortDirection::Input)],
    )));
    sim
}

#[test]
fn valid_wiring_passes() {
    let mut sim = simulation_with_source_and_sink();
    sim.register_connection(Connection::new(SOURCE, SOURCE_OUT, SINK, SINK_IN));
    assert_eq!(sim.validate_wiring(), Ok(()));
}

#[test]
fn empty_wiring_passes() {
    assert_eq!(Simulation::new().validate_wiring(), Ok(()));
}

#[test]
fn unknown_source_component_is_reported() {
    let mut sim = simulation_with_source_and_sink();
    let connection = Connection::new(ComponentId(99), SOURCE_OUT, SINK, SINK_IN);
    sim.register_connection(connection.clone());

    assert_eq!(
        sim.validate_wiring(),
        Err(vec![WiringError::UnknownSourceComponent { connection }])
    );
}

#[test]
fn unknown_target_component_is_reported() {
    let mut sim = simulation_with_source_and_sink();
    let connection = Connection::new(SOURCE, SOURCE_OUT, ComponentId(99), SINK_IN);
    sim.register_connection(connection.clone());

    assert_eq!(
        sim.validate_wiring(),
        Err(vec![WiringError::UnknownTargetComponent { connection }])
    );
}

#[test]
fn unknown_source_port_is_reported() {
    let mut sim = simulation_with_source_and_sink();
    sim.register_connection(Connection::new(SOURCE, PortId(999), SINK, SINK_IN));

    assert_eq!(
        sim.validate_wiring(),
        Err(vec![WiringError::UnknownSourcePort {
            component: SOURCE,
            port: PortId(999),
        }])
    );
}

#[test]
fn unknown_target_port_is_reported() {
    let mut sim = simulation_with_source_and_sink();
    sim.register_connection(Connection::new(SOURCE, SOURCE_OUT, SINK, PortId(999)));

    assert_eq!(
        sim.validate_wiring(),
        Err(vec![WiringError::UnknownTargetPort {
            component: SINK,
            port: PortId(999),
        }])
    );
}

#[test]
fn input_cannot_be_used_as_source() {
    let mut sim = simulation_with_source_and_sink();
    sim.register_connection(Connection::new(SINK, SINK_IN, SINK, SINK_IN));

    assert_eq!(
        sim.validate_wiring(),
        Err(vec![WiringError::SourceNotOutput {
            component: SINK,
            port: SINK_IN,
            direction: PortDirection::Input,
        }])
    );
}

#[test]
fn output_cannot_be_used_as_target() {
    let mut sim = simulation_with_source_and_sink();
    sim.register_connection(Connection::new(SOURCE, SOURCE_OUT, SOURCE, SOURCE_OUT));

    assert_eq!(
        sim.validate_wiring(),
        Err(vec![WiringError::TargetNotInput {
            component: SOURCE,
            port: SOURCE_OUT,
            direction: PortDirection::Output,
        }])
    );
}

#[test]
fn duplicate_connection_is_reported_once() {
    let mut sim = simulation_with_source_and_sink();
    let connection = Connection::new(SOURCE, SOURCE_OUT, SINK, SINK_IN);
    sim.register_connection(connection.clone());
    sim.register_connection(connection.clone());

    assert_eq!(
        sim.validate_wiring(),
        Err(vec![WiringError::DuplicateConnection { connection }])
    );
}

#[test]
fn bidirectional_ports_are_valid_on_both_ends() {
    let mut sim = Simulation::new();
    sim.register_component(Box::new(Node::new(
        ComponentId(3),
        "bidirectional",
        vec![Port::new(PortId(30), "io", PortDirection::Bidirectional)],
    )));
    sim.register_connection(Connection::new(
        ComponentId(3),
        PortId(30),
        ComponentId(3),
        PortId(30),
    ));

    assert_eq!(sim.validate_wiring(), Ok(()));
}

#[test]
fn every_problem_is_reported() {
    let mut sim = simulation_with_source_and_sink();
    sim.register_connection(Connection::new(ComponentId(99), SOURCE_OUT, SINK, SINK_IN));
    sim.register_connection(Connection::new(SOURCE, PortId(999), SINK, PortId(999)));

    let errors = sim.validate_wiring().expect_err("wiring is invalid");
    assert_eq!(errors.len(), 3, "each bad endpoint is reported");
}
