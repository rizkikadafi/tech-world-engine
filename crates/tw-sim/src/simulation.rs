use tw_core::{
    CausalGraph, Component, ComponentId, Connection, Event, EventId, EventPayload, Observer,
    SimulationContext, SimulationTime, TraceId, TraceNode, TracingMode,
};
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;

#[derive(Debug)]
struct ScheduledEvent(Event);

impl PartialEq for ScheduledEvent {
    fn eq(&self, other: &Self) -> bool {
        self.0.time == other.0.time && self.0.id == other.0.id
    }
}

impl Eq for ScheduledEvent {}

impl PartialOrd for ScheduledEvent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse for min-heap (earliest time first)
        other.0.time.cmp(&self.0.time).then_with(|| other.0.id.cmp(&self.0.id))
    }
}

#[derive(Default)]
pub struct Simulation {
    clock: SimulationTime,
    event_queue: BinaryHeap<ScheduledEvent>,
    components: HashMap<ComponentId, Box<dyn Component>>,
    connections: Vec<Connection>,
    observers: Vec<Box<dyn Observer>>,
    causal_graph: CausalGraph,
    tracing_mode: TracingMode,
    next_event_id: u64,
}

impl Simulation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current_time(&self) -> SimulationTime {
        self.clock
    }

    pub fn set_tracing_mode(&mut self, mode: TracingMode) {
        self.tracing_mode = mode;
    }

    pub fn tracing_mode(&self) -> TracingMode {
        self.tracing_mode
    }

    pub fn register_component(&mut self, component: Box<dyn Component>) {
        self.components.insert(component.id(), component);
    }

    pub fn register_connection(&mut self, connection: Connection) {
        self.connections.push(connection);
    }

    pub fn register_observer(&mut self, observer: Box<dyn Observer>) {
        self.observers.push(observer);
    }

    pub fn causal_graph(&self) -> &CausalGraph {
        &self.causal_graph
    }

    pub fn explain(&self, target: TraceId) -> Vec<&TraceNode> {
        self.causal_graph.explain(target)
    }

    pub fn schedule_event(&mut self, event: Event) {
        self.event_queue.push(ScheduledEvent(event));
    }

    pub fn step(&mut self) -> Option<Event> {
        if let Some(ScheduledEvent(event)) = self.event_queue.pop() {
            self.clock = event.time;

            // 1. Record event execution in Causal Graph
            let target_name = self
                .components
                .get(&event.target)
                .map(|c| c.name())
                .unwrap_or("Unknown");
            let desc = format!(
                "Event #{:?} [{:?}] executed on Component '{}' (#{})",
                event.id.0, event.payload, target_name, event.target.0
            );
            let current_trace_id = self.causal_graph.record_event(
                event.id,
                event.time,
                desc,
                event.cause,
            );

            // 2. Notify all registered observers
            for observer in &mut self.observers {
                observer.on_event(&event);
            }

            // Prepare exec event with updated cause
            let mut exec_event = event.clone();
            exec_event.cause = Some(current_trace_id);

            // 3. Dispatch to target component with SimulationContext
            if let Some(component) = self.components.get_mut(&event.target) {
                let mut ctx = SimulationContext::new(
                    self.clock,
                    &mut self.causal_graph,
                    self.tracing_mode,
                );
                let new_events = component.handle_event(&exec_event, &mut ctx);

                for mut new_event in new_events {
                    // Attach causality link to generated events if missing
                    if new_event.cause.is_none() {
                        new_event.cause = Some(current_trace_id);
                    }

                    // Route PortSignal outputs through registered connections
                    if let EventPayload::PortSignal { port_id, ref value } = new_event.payload {
                        let source = new_event.source;
                        let cause = new_event.cause;

                        let targets: Vec<_> = self
                            .connections
                            .iter()
                            .filter(|c| c.source_component == source && c.source_port == port_id)
                            .map(|c| (c.target_component, c.target_port))
                            .collect();

                        for (target_comp, target_port) in targets {
                            self.next_event_id += 1;
                            let mut fwd = Event::new(
                                EventId(self.next_event_id),
                                new_event.time,
                                source,
                                target_comp,
                                EventPayload::PortSignal {
                                    port_id: target_port,
                                    value: value.clone(),
                                },
                            );
                            fwd.cause = cause;
                            self.event_queue.push(ScheduledEvent(fwd));
                        }
                    } else {
                        self.event_queue.push(ScheduledEvent(new_event));
                    }
                }
            }
            Some(event)
        } else {
            None
        }
    }

    pub fn run_until(&mut self, until: SimulationTime) {
        while let Some(ScheduledEvent(next_event)) = self.event_queue.peek() {
            if next_event.time > until {
                break;
            }
            self.step();
        }
    }
}
