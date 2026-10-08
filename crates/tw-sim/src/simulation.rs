use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use tw_core::{
    CausalGraph, Component, ComponentId, Connection, Event, EventId, EventPayload, Observer,
    Resource, ResourceId, ResourceRequestResult, SimulationContext, SimulationTime, TraceId,
    TraceNode, TracingMode,
};

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
        other
            .0
            .time
            .cmp(&self.0.time)
            .then_with(|| other.0.id.cmp(&self.0.id))
    }
}

#[derive(Default)]
pub struct Simulation {
    clock: SimulationTime,
    event_queue: BinaryHeap<ScheduledEvent>,
    components: HashMap<ComponentId, Box<dyn Component>>,
    resources: HashMap<ResourceId, Resource>,
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

    pub fn register_resource(&mut self, resource: Resource) {
        self.resources.insert(resource.id, resource);
    }

    pub fn get_resource(&self, id: ResourceId) -> Option<&Resource> {
        self.resources.get(&id)
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
                .unwrap_or("Engine/Resource");
            let desc = format!(
                "Event #{:?} [{:?}] executed on Component '{}' (#{})",
                event.id.0, event.payload, target_name, event.target.0
            );
            let current_trace_id = self
                .causal_graph
                .record_event(event.id, event.time, desc, event.cause);

            // 2. Notify all registered observers
            for observer in &mut self.observers {
                observer.on_event(&event);
            }

            // Prepare exec event with updated cause
            let mut exec_event = event.clone();
            exec_event.cause = Some(current_trace_id);

            // 3. Handle Resource lifecycle events in engine loop
            match &exec_event.payload {
                EventPayload::ResourceRequested {
                    resource_id,
                    amount,
                    requester,
                } => {
                    let resource_id = *resource_id;
                    let amount = *amount;
                    let requester = *requester;

                    if let Some(res) = self.resources.get_mut(&resource_id) {
                        let res_name = res.name.clone();
                        match res.request(requester, amount, exec_event.cause) {
                            ResourceRequestResult::Granted => {
                                self.next_event_id += 1;
                                let mut grant_event = Event::new(
                                    EventId(self.next_event_id),
                                    self.clock,
                                    requester,
                                    requester,
                                    EventPayload::ResourceGranted {
                                        resource_id,
                                        amount,
                                    },
                                );
                                grant_event.cause = exec_event.cause;
                                self.event_queue.push(ScheduledEvent(grant_event));
                            }
                            ResourceRequestResult::Queued { queue_position } => {
                                self.causal_graph.record_event(
                                    exec_event.id,
                                    self.clock,
                                    format!(
                                        "Resource '{}' (#{}) capacity exhausted. Request for {} units queued at position #{}",
                                        res_name, resource_id.0, amount, queue_position
                                    ),
                                    exec_event.cause,
                                );
                            }
                            ResourceRequestResult::Invalid { reason } => {
                                self.next_event_id += 1;
                                let mut deny_event = Event::new(
                                    EventId(self.next_event_id),
                                    self.clock,
                                    requester,
                                    requester,
                                    EventPayload::ResourceDenied {
                                        resource_id,
                                        amount,
                                        reason,
                                    },
                                );
                                deny_event.cause = exec_event.cause;
                                self.event_queue.push(ScheduledEvent(deny_event));
                            }
                        }
                    } else {
                        self.next_event_id += 1;
                        let mut deny_event = Event::new(
                            EventId(self.next_event_id),
                            self.clock,
                            requester,
                            requester,
                            EventPayload::ResourceDenied {
                                resource_id,
                                amount,
                                reason: format!("Resource #{} not found", resource_id.0),
                            },
                        );
                        deny_event.cause = exec_event.cause;
                        self.event_queue.push(ScheduledEvent(deny_event));
                    }
                }
                EventPayload::ResourceReleased {
                    resource_id,
                    amount,
                    releaser: _,
                } => {
                    let resource_id = *resource_id;
                    let amount = *amount;

                    if let Some(res) = self.resources.get_mut(&resource_id) {
                        let granted_list = res.release(amount);
                        for granted in granted_list {
                            self.next_event_id += 1;
                            let mut grant_event = Event::new(
                                EventId(self.next_event_id),
                                self.clock,
                                granted.requester,
                                granted.requester,
                                EventPayload::ResourceGranted {
                                    resource_id: res.id,
                                    amount: granted.amount,
                                },
                            );
                            // Link cause of unblocking directly to the release event!
                            grant_event.cause = Some(current_trace_id);
                            self.event_queue.push(ScheduledEvent(grant_event));
                        }
                    }
                }
                _ => {}
            }

            // 4. Dispatch to target component with SimulationContext
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
