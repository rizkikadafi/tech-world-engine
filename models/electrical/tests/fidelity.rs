//! Fidelity selection and refinement across transistor, bridge, and derived
//! representations.
//!
//! The key invariant is that all discrete fidelities expose the *same* ports for
//! a given [`GateKind`], so a caller can swap representations without rewiring.

use std::sync::{Arc, Mutex};

use tw_core::{
    Component, ComponentId, Connection, Event, EventPayload, Port, PortDirection, PortId,
    SimulationContext, SimulationTime, State, StateValue,
};
use tw_electrical::prelude::*;
use tw_sim::Simulation;

const SINK_PORT: PortId = PortId(1);
const SOURCE_ID: ComponentId = ComponentId(901);

#[derive(Debug)]
struct SignalSink {
    id: ComponentId,
    name: String,
    ports: Vec<Port>,
    state: State,
    log: Arc<Mutex<Vec<(u64, bool)>>>,
}

impl SignalSink {
    fn new(id: ComponentId, log: Arc<Mutex<Vec<(u64, bool)>>>) -> Self {
        Self {
            id,
            name: "sink".to_string(),
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

fn spec(id: u64, kind: GateKind) -> ComponentSpec {
    ComponentSpec::new(ComponentId(id), format!("{kind:?}"), kind)
}

#[test]
fn fidelity_progression_is_monotonic() {
    assert_eq!(
        Fidelity::ALL,
        [Fidelity::Derived, Fidelity::Bridge, Fidelity::Transistor,]
    );
    assert_eq!(Fidelity::Derived.more_detailed(), Some(Fidelity::Bridge));
    assert_eq!(Fidelity::Bridge.more_detailed(), Some(Fidelity::Transistor));
    assert_eq!(Fidelity::Transistor.more_detailed(), None);
    assert_eq!(Fidelity::Transistor.less_detailed(), Some(Fidelity::Bridge));
    assert_eq!(Fidelity::Bridge.less_detailed(), Some(Fidelity::Derived));
    assert_eq!(Fidelity::Derived.less_detailed(), None);
    assert!(!Fidelity::Transistor.is_component());
    assert!(Fidelity::Bridge.is_component());
    assert!(Fidelity::Derived.is_component());
}

#[test]
fn select_across_fidelities_keeps_identical_ports() {
    let mut factory = FidelityFactory::with_defaults();

    for (index, kind) in [GateKind::Inverter, GateKind::Nand2]
        .into_iter()
        .enumerate()
    {
        let spec = spec(index as u64 + 1, kind);
        let derived = factory
            .select_component(&spec, Fidelity::Derived)
            .expect("derived component");
        let bridge = factory
            .select_component(&spec, Fidelity::Bridge)
            .expect("bridge component");

        assert_eq!(
            derived.ports(),
            bridge.ports(),
            "{kind:?} must expose identical ports across fidelities"
        );
        assert_eq!(derived.id(), bridge.id());
        assert_eq!(derived.name(), bridge.name());
        assert_eq!(derived.ports().len(), kind.inputs() + 1);
        assert_eq!(
            derived
                .ports()
                .iter()
                .filter(|port| port.direction == PortDirection::Input)
                .count(),
            kind.inputs()
        );
    }
}

#[test]
fn select_transistor_returns_same_netlist_as_measurement() {
    let mut factory = FidelityFactory::with_defaults();
    let spec = spec(1, GateKind::Inverter);

    let model = factory
        .select(&spec, Fidelity::Transistor)
        .expect("transistor model");
    let Model::Transistor(circuit) = model else {
        panic!("expected transistor model");
    };

    let template = factory
        .template(GateKind::Inverter)
        .expect("template")
        .clone();

    assert_eq!(circuit.node_count(), template.circuit.node_count());
    assert_eq!(
        circuit.node_by_name("out"),
        Some(template.output_node),
        "selected netlist must expose the measured output node"
    );
    assert_eq!(
        circuit
            .voltage_source(&template.inputs[0].source_name)
            .map(|source| source.voltage),
        Some(0.0)
    );
    assert!(circuit.is_nonlinear());
}

#[test]
fn select_component_rejects_transistor() {
    let mut factory = FidelityFactory::with_defaults();
    let spec = spec(1, GateKind::Inverter);

    let error = factory
        .select_component(&spec, Fidelity::Transistor)
        .expect_err("transistor fidelity is not a component");
    assert!(matches!(
        error,
        FidelityError::NotAComponent(Fidelity::Transistor)
    ));
}

#[test]
fn characterization_is_cached() {
    let mut factory = FidelityFactory::with_defaults();
    assert!(!factory.is_cached(GateKind::Inverter));
    assert_eq!(factory.cached_count(), 0);

    let first = factory
        .characterization(GateKind::Inverter)
        .expect("first measurement") as *const GateCharacterization;
    assert!(factory.is_cached(GateKind::Inverter));
    assert_eq!(factory.cached_count(), 1);

    let second = factory
        .characterization(GateKind::Inverter)
        .expect("cached measurement") as *const GateCharacterization;
    assert_eq!(first, second, "second lookup must reuse the cached value");

    factory
        .select_component(&spec(1, GateKind::Inverter), Fidelity::Derived)
        .expect("derived component");
    assert_eq!(
        factory.cached_count(),
        1,
        "building a model must not re-characterize"
    );
}

#[test]
fn refine_moves_derived_to_bridge_to_transistor() {
    let mut factory = FidelityFactory::with_defaults();
    let spec = spec(1, GateKind::Inverter);

    let bridge = factory
        .refine(&spec, Fidelity::Derived)
        .expect("refine derived -> bridge");
    assert_eq!(bridge.fidelity(), Fidelity::Bridge);
    assert!(bridge.as_component().is_some());

    let transistor = factory
        .refine(&spec, Fidelity::Bridge)
        .expect("refine bridge -> transistor");
    assert_eq!(transistor.fidelity(), Fidelity::Transistor);
    assert!(matches!(transistor, Model::Transistor(_)));

    let error = factory
        .refine(&spec, Fidelity::Transistor)
        .expect_err("transistor is already most detailed");
    assert!(matches!(
        error,
        FidelityError::AlreadyMostDetailed(Fidelity::Transistor)
    ));
}

#[test]
fn derived_and_bridge_inverter_agree_end_to_end() {
    let mut factory = FidelityFactory::with_defaults();
    let derived_spec = spec(10, GateKind::Inverter);
    let bridge_spec = spec(11, GateKind::Inverter);

    let derived = factory
        .select_component(&derived_spec, Fidelity::Derived)
        .expect("derived inverter");
    let bridge = factory
        .select_component(&bridge_spec, Fidelity::Bridge)
        .expect("bridge inverter");

    let derived_log = Arc::new(Mutex::new(Vec::new()));
    let bridge_log = Arc::new(Mutex::new(Vec::new()));

    let mut sim = Simulation::new();
    sim.register_component(derived);
    sim.register_component(bridge);
    sim.register_component(Box::new(SignalSink::new(
        ComponentId(910),
        derived_log.clone(),
    )));
    sim.register_component(Box::new(SignalSink::new(
        ComponentId(911),
        bridge_log.clone(),
    )));

    sim.register_connection(Connection::new(
        derived_spec.id,
        PortId(2),
        ComponentId(910),
        SINK_PORT,
    ));
    sim.register_connection(Connection::new(
        bridge_spec.id,
        PortId(2),
        ComponentId(911),
        SINK_PORT,
    ));

    for target in [derived_spec.id, bridge_spec.id] {
        sim.schedule_event(Event::at(
            SimulationTime::new(0),
            SOURCE_ID,
            target,
            EventPayload::PortSignal {
                port_id: PortId(1),
                value: StateValue::Bool(true),
            },
        ));
    }

    sim.run_until(SimulationTime::new(40));

    let derived_events = derived_log.lock().unwrap().clone();
    let bridge_events = bridge_log.lock().unwrap().clone();
    assert_eq!(derived_events.first(), Some(&(0, true)));
    assert_eq!(bridge_events.first(), Some(&(0, true)));

    let derived_fall = derived_events
        .iter()
        .find(|(_, value)| !*value)
        .map(|(tick, _)| *tick)
        .expect("derived inverter must fall");
    let bridge_fall = bridge_events
        .iter()
        .find(|(_, value)| !*value)
        .map(|(tick, _)| *tick)
        .expect("bridge inverter must fall");

    assert!(
        bridge_fall.abs_diff(derived_fall) <= 1,
        "bridge fall at {bridge_fall} should be within one tick of derived fall at {derived_fall}"
    );
}

#[test]
fn dff_selection_refinement_and_cache() {
    let mut factory = FidelityFactory::with_defaults();
    let dff = spec(20, GateKind::Dff);

    let derived = factory
        .select_component(&dff, Fidelity::Derived)
        .expect("derived dff");
    let bridge = factory
        .select_component(&dff, Fidelity::Bridge)
        .expect("bridge dff");

    assert_eq!(derived.ports(), bridge.ports());
    assert_eq!(
        derived
            .ports()
            .iter()
            .map(|port| (port.id, port.name.as_str(), port.direction))
            .collect::<Vec<_>>(),
        vec![
            (PortId(1), "D", PortDirection::Input),
            (PortId(2), "CLK", PortDirection::Input),
            (PortId(3), "Q", PortDirection::Output),
        ]
    );

    // Both discrete models came from a single characterization.
    assert!(factory.is_cached(GateKind::Dff));
    assert_eq!(factory.cached_count(), 1);

    let refined = factory
        .refine(&dff, Fidelity::Bridge)
        .expect("refine dff bridge -> transistor");
    assert_eq!(refined.fidelity(), Fidelity::Transistor);

    let error = factory
        .refine(&dff, Fidelity::Transistor)
        .expect_err("dff transistor is already most detailed");
    assert!(matches!(
        error,
        FidelityError::AlreadyMostDetailed(Fidelity::Transistor)
    ));
}
