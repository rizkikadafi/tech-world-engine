//! Conformance tests for user-defined components built on the public API.

use tw_core::{
    Component, ComponentId, Connection, Event, EventPayload, Port, PortDirection, PortId,
    SimulationContext, SimulationTime, State, StateValue,
};
use tw_sim::{Simulation, SimulationEngine, WiringError};

const DOUBLER_ID: ComponentId = ComponentId(1);
const SINK_ID: ComponentId = ComponentId(2);

const INPUT: PortId = PortId(1);
const OUTPUT: PortId = PortId(2);
const SINK_INPUT: PortId = PortId(1);

#[derive(Debug)]
struct Doubler {
    id: ComponentId,
    name: &'static str,
    ports: Vec<Port>,
    state: State,
}

impl Doubler {
    fn new() -> Self {
        Self {
            id: DOUBLER_ID,
            name: "doubler",
            ports: vec![
                Port::new(INPUT, "in", PortDirection::Input),
                Port::new(OUTPUT, "out", PortDirection::Output),
            ],
            state: State::new(),
        }
    }
}

impl Component for Doubler {
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

    fn handle_event(&mut self, event: &Event, _ctx: &mut SimulationContext) -> Vec<Event> {
        let EventPayload::PortSignal { port_id, value } = &event.payload else {
            return Vec::new();
        };
        if *port_id != INPUT {
            return Vec::new();
        }

        let Some(value) = value.as_int() else {
            return Vec::new();
        };
        let doubled = value * 2;
        self.state.set("last_input", value);
        self.state.set("last_output", doubled);

        vec![Event::at(
            event.time,
            self.id,
            self.id,
            EventPayload::PortSignal {
                port_id: OUTPUT,
                value: StateValue::from(doubled),
            },
        )]
    }
}

#[derive(Debug)]
struct Sink {
    id: ComponentId,
    name: &'static str,
    ports: Vec<Port>,
    state: State,
}

impl Sink {
    fn new() -> Self {
        let mut state = State::new();
        state.set("count", 0);
        Self {
            id: SINK_ID,
            name: "sink",
            ports: vec![Port::new(SINK_INPUT, "in", PortDirection::Input)],
            state,
        }
    }
}

impl Component for Sink {
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

    fn handle_event(&mut self, event: &Event, _ctx: &mut SimulationContext) -> Vec<Event> {
        if let EventPayload::PortSignal { port_id, value } = &event.payload {
            if *port_id == SINK_INPUT {
                if let Some(value) = value.as_int() {
                    let count = self.state.get_int("count").unwrap_or(0) + 1;
                    self.state.set("last", value);
                    self.state.set("count", count);
                }
            }
        }
        Vec::new()
    }
}

fn build() -> Simulation {
    let mut sim = Simulation::new();
    sim.register_component(Box::new(Doubler::new()));
    sim.register_component(Box::new(Sink::new()));
    sim.register_connection(Connection::new(DOUBLER_ID, OUTPUT, SINK_ID, SINK_INPUT));

    for (at, value) in [(0u64, 2i64), (1, 3), (2, 4)] {
        sim.schedule_event(Event::at(
            SimulationTime::new(at),
            ComponentId(0),
            DOUBLER_ID,
            EventPayload::PortSignal {
                port_id: INPUT,
                value: StateValue::from(value),
            },
        ));
    }

    sim
}

#[test]
fn user_defined_components_route_port_signals() {
    let sim = build();
    assert_eq!(sim.validate_wiring(), Ok(()));

    let mut engine = SimulationEngine::new(sim);
    engine.run_ticks(5);

    let doubler = engine
        .simulation()
        .component_state(DOUBLER_ID)
        .expect("doubler registered");
    assert_eq!(doubler.get_int("last_input"), Some(4));
    assert_eq!(doubler.get_int("last_output"), Some(8));

    let sink = engine
        .simulation()
        .component_state(SINK_ID)
        .expect("sink registered");
    assert_eq!(sink.get_int("last"), Some(8));
    assert_eq!(sink.get_int("count"), Some(3));
}

#[test]
fn custom_components_run_deterministically() {
    let run = || {
        let mut engine = SimulationEngine::new(build());
        engine.run_ticks(5);
        let state = engine
            .simulation()
            .component_state(SINK_ID)
            .expect("sink registered");
        (
            state.get_int("last"),
            state.get_int("count"),
            engine.simulation().events_processed(),
        )
    };

    assert_eq!(run(), run());
}

#[test]
fn lifecycle_hooks_run_for_user_components() {
    #[derive(Debug)]
    struct Probe {
        id: ComponentId,
        ports: Vec<Port>,
        state: State,
    }

    impl Component for Probe {
        fn id(&self) -> ComponentId {
            self.id
        }

        fn name(&self) -> &str {
            "probe"
        }

        fn ports(&self) -> &[Port] {
            &self.ports
        }

        fn state(&self) -> &State {
            &self.state
        }

        fn on_mount(&mut self, ctx: &mut SimulationContext) -> Vec<Event> {
            self.state.set("mounted_at", ctx.time().ticks() as i64);
            Vec::new()
        }

        fn on_unmount(&mut self, ctx: &mut SimulationContext) -> Vec<Event> {
            self.state.set("unmounted_at", ctx.time().ticks() as i64);
            Vec::new()
        }

        fn handle_event(&mut self, _event: &Event, _ctx: &mut SimulationContext) -> Vec<Event> {
            Vec::new()
        }
    }

    let mut sim = Simulation::new();
    sim.register_component(Box::new(Probe {
        id: ComponentId(7),
        ports: Vec::new(),
        state: State::new(),
    }));
    sim.run_until(SimulationTime::new(0));

    let state = sim
        .component_state(ComponentId(7))
        .expect("probe registered");
    assert_eq!(state.get_int("mounted_at"), Some(0));

    sim.shutdown();
    let state = sim
        .component_state(ComponentId(7))
        .expect("probe registered");
    assert_eq!(state.get_int("unmounted_at"), Some(0));
}

#[test]
fn wiring_typos_are_reported_before_running() {
    let mut sim = Simulation::new();
    sim.register_component(Box::new(Doubler::new()));
    sim.register_component(Box::new(Sink::new()));
    sim.register_connection(Connection::new(
        DOUBLER_ID,
        PortId(999),
        SINK_ID,
        SINK_INPUT,
    ));

    assert_eq!(
        sim.validate_wiring(),
        Err(vec![WiringError::UnknownSourcePort {
            component: DOUBLER_ID,
            port: PortId(999),
        }])
    );
}
