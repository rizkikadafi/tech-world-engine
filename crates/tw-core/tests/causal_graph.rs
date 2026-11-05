use tw_core::{CausalGraph, EventId, SimulationTime, TraceId};

#[test]
fn explain_returns_linear_chain_root_first() {
    let mut graph = CausalGraph::new();
    let a = graph.record_event(EventId(1), SimulationTime::new(0), "A", None);
    let b = graph.record_event(EventId(2), SimulationTime::new(1), "B", Some(a));
    let c = graph.record_event(EventId(3), SimulationTime::new(2), "C", Some(b));

    let path = graph.explain(c);
    let descriptions: Vec<&str> = path.iter().map(|n| n.description.as_str()).collect();
    assert_eq!(descriptions, vec!["A", "B", "C"]);
}

#[test]
fn record_event_creates_causal_edge() {
    let mut graph = CausalGraph::new();
    let a = graph.record_event(EventId(1), SimulationTime::new(0), "A", None);
    graph.record_event(EventId(2), SimulationTime::new(1), "B", Some(a));

    assert_eq!(graph.nodes().len(), 2);
    assert_eq!(graph.edges().len(), 1);
    assert_eq!(graph.edges()[0].cause, a);
}

#[test]
fn explain_deduplicates_diamond_and_orders_causes_first() {
    let mut graph = CausalGraph::new();
    let a = graph.record_event(EventId(1), SimulationTime::new(0), "A", None);
    let b = graph.record_event(EventId(2), SimulationTime::new(1), "B", Some(a));
    let c = graph.record_event(EventId(3), SimulationTime::new(1), "C", Some(a));
    let d = graph.record_event(EventId(4), SimulationTime::new(2), "D", Some(b));

    // D is also caused by C (diamond): D <- B <- A and D <- C <- A.
    graph.add_edge(tw_core::CausalEdge {
        cause: c,
        effect: d,
        relation: "caused".to_string(),
    });

    let path = graph.explain(d);
    let descriptions: Vec<&str> = path.iter().map(|n| n.description.as_str()).collect();

    // Each node appears exactly once, causes before effects.
    assert_eq!(descriptions, vec!["A", "B", "C", "D"]);
}

#[test]
fn explain_unknown_trace_is_empty() {
    let graph = CausalGraph::new();
    assert!(graph.explain(TraceId(999)).is_empty());
}

#[test]
fn nodes_are_numbered_from_one_and_increment() {
    let mut graph = CausalGraph::new();
    let a = graph.record_event(EventId(1), SimulationTime::new(0), "A", None);
    let b = graph.record_event(EventId(2), SimulationTime::new(0), "B", None);
    assert_eq!(a, TraceId(1));
    assert_eq!(b, TraceId(2));
}

#[test]
fn traces_by_component_and_serialization() {
    use tw_core::{ComponentId, TraceNode};

    let mut graph = CausalGraph::new();
    let comp1 = ComponentId(10);
    let comp2 = ComponentId(20);

    let t1 = graph.record_event_with_subject(
        EventId(1),
        SimulationTime::new(0),
        "tick comp1",
        None,
        Some(comp1),
    );
    let t2 = graph.record_event_with_subject(
        EventId(2),
        SimulationTime::new(1),
        "tick comp2",
        Some(t1),
        Some(comp2),
    );
    let t3 = graph.record_event_with_subject(
        EventId(3),
        SimulationTime::new(2),
        "step comp1",
        Some(t2),
        Some(comp1),
    );

    assert_eq!(graph.traces_for(comp1), &[t1, t3]);
    assert_eq!(graph.last_trace_for(comp1), Some(t3));
    assert_eq!(graph.traces_for(comp2), &[t2]);
    assert_eq!(graph.last_trace_for(comp2), Some(t2));
    assert_eq!(graph.traces_for(ComponentId(99)), &[]);
    assert_eq!(graph.last_trace_for(ComponentId(99)), None);

    // Serialization roundtrip
    let node = graph.nodes().get(&t1).unwrap();
    let json = serde_json::to_string(node).unwrap();
    let deserialized: TraceNode = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, *node);
    assert_eq!(deserialized.subject, Some(comp1));
}
