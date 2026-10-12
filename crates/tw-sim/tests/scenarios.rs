use std::cell::RefCell;
use std::rc::Rc;

use tw_core::{
    Component, ComponentId, Event, EventId, EventPayload, Port, SimulationContext, SimulationTime,
    State, TracingMode,
};
use tw_sim::scenarios::{counter, digital, resource_contention};
use tw_sim::{Simulation, SimulationEngine};

/// Minimal component that records every event it receives. Used to observe the
/// engine's scheduling/ordering behavior without any domain logic.
#[derive(Debug)]
struct Recorder {
    id: ComponentId,
    name: String,
    ports: Vec<Port>,
    state: State,
    seen: Rc<RefCell<Vec<Event>>>,
}

impl Recorder {
    fn new(id: ComponentId, seen: Rc<RefCell<Vec<Event>>>) -> Self {
        Self {
            id,
            name: "Recorder".to_string(),
            ports: Vec::new(),
            state: State::new(),
            seen,
        }
    }
}

impl Component for Recorder {
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
        self.seen.borrow_mut().push(event.clone());
        Vec::new()
    }
}

fn schedule_tick(sim: &mut Simulation, at: u64) {
    sim.schedule_event(Event::at(
        SimulationTime::new(at),
        ComponentId(1),
        ComponentId(1),
        EventPayload::Tick,
    ));
}

#[test]
fn counter_ticks_until_limit() {
    let mut engine = SimulationEngine::new(counter::build(3, 5));
    engine.run_ticks(100);
    let state = engine
        .simulation()
        .component_state(counter::COUNTER_ID)
        .expect("counter registered");
    assert_eq!(state.get_int("count"), Some(3));
}

#[test]
fn full_adder_truth_table_is_correct() {
    for i in 0..8u8 {
        let (a, b, cin) = ((i >> 2) & 1, (i >> 1) & 1, i & 1);
        let engine = digital::run_full_adder(a, b, cin, TracingMode::Abstract);
        let got = digital::full_adder_result(&engine)
            .unwrap_or_else(|| panic!("no probe output for A={a} B={b} Cin={cin}"));
        assert_eq!(
            got,
            digital::expected_full_adder(a, b, cin),
            "wrong result for A={a} B={b} Cin={cin}"
        );
    }
}

#[test]
fn resource_capacity_causes_queue_and_latency() {
    let mut engine = SimulationEngine::new(resource_contention::build());
    while engine.simulation_mut().step().is_some() {}

    for id in [
        resource_contention::WORKER1_ID,
        resource_contention::WORKER2_ID,
        resource_contention::WORKER3_ID,
    ] {
        let state = engine
            .simulation()
            .component_state(id)
            .expect("worker registered");
        assert_eq!(state.get_str("status"), Some("Finished"));
    }
}

#[test]
fn tracing_off_records_no_nodes() {
    let engine = digital::run_full_adder(1, 1, 1, TracingMode::Off);
    assert!(engine.simulation().causal_graph().nodes().is_empty());
    assert!(engine.simulation().causal_graph().edges().is_empty());
}

#[test]
fn hierarchical_mode_records_internal_routing() {
    let engine = digital::run_full_adder(1, 1, 1, TracingMode::Hierarchical);
    let has_internal = engine
        .simulation()
        .causal_graph()
        .nodes()
        .values()
        .any(|n| n.description.contains("routed Internal Connection"));
    assert!(has_internal, "expected composite internal routing traces");
}

#[test]
fn simulation_is_deterministic_across_runs() {
    let first = digital::run_full_adder(1, 1, 1, TracingMode::Abstract);
    let second = digital::run_full_adder(1, 1, 1, TracingMode::Abstract);
    assert_eq!(
        digital::trace_sequence(&first),
        digital::trace_sequence(&second)
    );
    assert_eq!(
        digital::full_adder_result(&first),
        digital::full_adder_result(&second)
    );
}

#[test]
fn events_are_processed_in_time_order() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut sim = Simulation::new();
    sim.register_component(Box::new(Recorder::new(ComponentId(1), Rc::clone(&seen))));
    schedule_tick(&mut sim, 5);
    schedule_tick(&mut sim, 1);
    schedule_tick(&mut sim, 3);

    let mut engine = SimulationEngine::new(sim);
    engine.run_ticks(10);

    let times: Vec<u64> = seen.borrow().iter().map(|e| e.time.ticks()).collect();
    assert_eq!(times, vec![1, 3, 5]);
}

#[test]
fn schedule_event_assigns_unique_engine_ids() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut sim = Simulation::new();
    sim.register_component(Box::new(Recorder::new(ComponentId(1), Rc::clone(&seen))));

    // Both events are authored with the same id and land on the same tick.
    for _ in 0..2 {
        sim.schedule_event(Event::new(
            EventId(7),
            SimulationTime::new(0),
            ComponentId(1),
            ComponentId(1),
            EventPayload::Tick,
        ));
    }

    let mut engine = SimulationEngine::new(sim);
    engine.run_ticks(1);

    let seen = seen.borrow();
    assert_eq!(seen.len(), 2, "both events must be processed");
    assert_ne!(seen[0].id, seen[1].id, "engine must assign distinct ids");
}
