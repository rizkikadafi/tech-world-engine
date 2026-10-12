use tw_core::{Event, EventId, EventPayload, SimulationTime, Snapshot, State};
use tw_core::{ComponentId, TracingMode};

#[test]
fn simulation_time_advances_by_ticks() {
    let mut t = SimulationTime::new(5);
    assert_eq!(t.ticks(), 5);
    t.advance(7);
    assert_eq!(t.ticks(), 12);
    assert!(SimulationTime::ZERO < t);
}

#[test]
fn state_stores_and_reads_values() {
    let mut state = State::new();
    assert_eq!(state.get("missing"), None);
    state.set("count", "42");
    assert_eq!(state.get("count"), Some("42"));
    state.set("count", "43");
    assert_eq!(state.get("count"), Some("43"));
}

#[test]
fn snapshot_captures_time_and_state() {
    let mut state = State::new();
    state.set("voltage", "3.3");
    let snapshot = Snapshot::new(SimulationTime::new(10), state.clone());
    assert_eq!(snapshot.time.ticks(), 10);
    assert_eq!(snapshot.state.get("voltage"), Some("3.3"));
}

#[test]
fn event_at_uses_placeholder_id() {
    let event = Event::at(
        SimulationTime::new(0),
        ComponentId(1),
        ComponentId(2),
        EventPayload::Tick,
    );
    assert_eq!(event.id, EventId(0));
    assert!(event.cause.is_none());
}

#[test]
fn tracing_mode_helpers() {
    assert!(!TracingMode::Off.is_enabled());
    assert!(TracingMode::Abstract.is_enabled());
    assert!(TracingMode::Hierarchical.is_enabled());
    assert!(TracingMode::Hierarchical.is_hierarchical());
    assert!(!TracingMode::Abstract.is_hierarchical());
    assert_eq!(TracingMode::default(), TracingMode::Abstract);
}
