use tw_core::{ComponentId, StateValue};
use tw_scene::{Registry, Session, World};

#[test]
fn session_runs_counter_and_tracks_diffs() {
    let json = r#"{
      "name": "counter-test",
      "components": [
        {
          "id": 1,
          "kind": "Counter",
          "params": { "limit": 3, "interval": 5 }
        }
      ],
      "stimuli": [
        { "at": 0, "component": 1, "tick": true }
      ]
    }"#;

    let world = World::from_json_str(json).unwrap();
    let mut session = Session::from_world(world, Registry::builtin()).unwrap();

    assert_eq!(session.component_ids(), vec![ComponentId(1)]);
    assert_eq!(session.current_tick(), 0);

    // Initial state diff before stepping: empty because prev_states is seeded at mount
    let initial_diff = session.take_state_changes();
    assert_eq!(initial_diff.len(), 0);

    // Step to tick 10 (should process tick at 0 and tick at 5)
    let report = session.step(10);
    assert_eq!(report.tick, 10);
    assert!(report.events_executed >= 2);

    // State diff should now report count updated
    let diff = session.take_state_changes();
    assert_eq!(diff.len(), 1);
    assert_eq!(diff[0].key, "count");
    assert_eq!(diff[0].old, Some(StateValue::Int(0)));
    let count_val = diff[0].new.as_int().unwrap();
    assert!(count_val >= 2);

    // Subsequent take_state_changes without new events returns empty
    let empty_diff = session.take_state_changes();
    assert!(empty_diff.is_empty());
}

#[test]
fn session_wires_gates_and_traces_causality() {
    let json = r#"{
      "name": "gate-test",
      "components": [
        { "id": 1, "kind": "Gate", "params": { "gate": "AND" } }
      ]
    }"#;

    let world = World::from_json_str(json).unwrap();
    let mut session = Session::from_world(world, Registry::builtin()).unwrap();

    // Inject in_a = 1, in_b = 1
    session
        .inject_port_signal(ComponentId(1), "in_a", StateValue::Int(1))
        .unwrap();
    session
        .inject_port_signal(ComponentId(1), "in_b", StateValue::Int(1))
        .unwrap();

    let report = session.step(5);
    assert_eq!(report.events_executed, 2);

    // Verify traces for component 1 exist
    let traces = session.traces_for(ComponentId(1));
    assert_eq!(traces.len(), 2);

    let last_trace = session.last_trace_for(ComponentId(1)).unwrap();
    let explanation = session.explain(last_trace);
    assert!(!explanation.is_empty());
    assert_eq!(explanation.last().unwrap().subject, Some(ComponentId(1)));
}

#[test]
fn session_checkpoint_and_restore() {
    let json = r#"{
      "name": "checkpoint-test",
      "components": [
        { "id": 1, "kind": "Counter", "params": { "limit": 10, "interval": 1 } }
      ],
      "stimuli": [
        { "at": 0, "component": 1, "tick": true }
      ]
    }"#;

    let world = World::from_json_str(json).unwrap();
    let mut session = Session::from_world(world, Registry::builtin()).unwrap();

    session.step(5);
    let state_before = session.component_state(ComponentId(1)).unwrap().clone();

    let checkpoint = session.checkpoint();
    assert_eq!(checkpoint.component_states.len(), 1);

    session.step(5);
    let state_after = session.component_state(ComponentId(1)).unwrap().clone();
    assert_ne!(state_before, state_after);

    // Restore components from checkpoint
    session.restore(&checkpoint);
    let state_restored = session.component_state(ComponentId(1)).unwrap().clone();
    assert_eq!(state_before, state_restored);
}
