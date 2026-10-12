use crate::config::SimulationConfig;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use tw_core::{
    Amount, CausalGraph, Component, ComponentId, Connection, Event, EventId, EventPayload, Observer,
    Resource, ResourceId, ResourceRequestResult, ResourceSnapshot, SimulationContext,
    SimulationTime, Snapshot, State, TraceId, TraceNode, TracingMode,
};

/// A captured point-in-time view of engine-owned state.
///
/// `component_states` holds each component's public [`State`] for inspection or
/// replay; `resources` holds restorable resource state. Component *internal*
/// fields are not captured generically — a component that needs full restore
/// support should keep its meaningful state in [`State`].
#[derive(Debug, Clone)]
pub struct Checkpoint {
    pub time: SimulationTime,
    pub component_states: Vec<(ComponentId, State)>,
    pub resources: Vec<ResourceSnapshot>,
}

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
    config: SimulationConfig,
    next_event_id: u64,
    processed_events: u64,
}

impl Simulation {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a simulation with explicit [`SimulationConfig`].
    pub fn with_config(config: SimulationConfig) -> Self {
        Self {
            config,
            ..Self::default()
        }
    }

    /// Replace the engine configuration (tracing mode, event cap, seed).
    pub fn configure(&mut self, config: SimulationConfig) {
        self.config = config;
    }

    /// Current engine configuration.
    pub fn config(&self) -> SimulationConfig {
        self.config
    }

    /// Number of events executed so far, for progress and benchmarking.
    pub fn events_processed(&self) -> u64 {
        self.processed_events
    }

    pub fn current_time(&self) -> SimulationTime {
        self.clock
    }

    pub fn set_tracing_mode(&mut self, mode: TracingMode) {
        self.config.tracing_mode = mode;
    }

    pub fn tracing_mode(&self) -> TracingMode {
        self.config.tracing_mode
    }

    pub fn register_component(&mut self, mut component: Box<dyn Component>) {
        let mount_events = {
            let mut ctx =
                SimulationContext::new(self.clock, &mut self.causal_graph, self.config.tracing_mode);
            component.on_mount(&mut ctx)
        };
        self.components.insert(component.id(), component);
        for event in mount_events {
            self.schedule_event(event);
        }
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

    /// Call [`Component::on_unmount`] on every component and schedule any events
    /// they return.
    pub fn shutdown(&mut self) {
        let component_ids: Vec<ComponentId> = self.components.keys().copied().collect();
        for id in component_ids {
            let events = {
                let mut ctx =
                    SimulationContext::new(self.clock, &mut self.causal_graph, self.config.tracing_mode);
                match self.components.get_mut(&id) {
                    Some(component) => component.on_unmount(&mut ctx),
                    None => Vec::new(),
                }
            };
            for event in events {
                self.schedule_event(event);
            }
        }
    }

    pub fn causal_graph(&self) -> &CausalGraph {
        &self.causal_graph
    }

    /// Borrow a registered component by id (top-level components only).
    pub fn component(&self, id: ComponentId) -> Option<&dyn Component> {
        self.components.get(&id).map(|c| c.as_ref())
    }

    /// Borrow the state of a registered component by id.
    pub fn component_state(&self, id: ComponentId) -> Option<&State> {
        self.components.get(&id).map(|c| c.state())
    }

    /// Number of registered top-level components.
    pub fn component_count(&self) -> usize {
        self.components.len()
    }

    pub fn explain(&self, target: TraceId) -> Vec<&TraceNode> {
        self.causal_graph.explain(target)
    }

    /// Capture a checkpoint of engine-owned state.
    ///
    /// Every registered observer receives one [`Snapshot`] per component (with
    /// the current simulation time), and resource state is captured for a later
    /// [`Simulation::restore_resources`].
    pub fn checkpoint(&mut self) -> Checkpoint {
        let time = self.clock;

        let mut component_states = Vec::with_capacity(self.components.len());
        for component in self.components.values() {
            let state = component.state().clone();
            for observer in &mut self.observers {
                observer.on_snapshot(&Snapshot::new(time, state.clone()));
            }
            component_states.push((component.id(), state));
        }

        let resources = self.resources.values().map(Resource::snapshot).collect();

        Checkpoint {
            time,
            component_states,
            resources,
        }
    }

    /// Restore engine-owned resource state from a checkpoint.
    ///
    /// The simulation clock is intentionally left untouched: time is advanced
    /// by the event loop, not by restore.
    pub fn restore_resources(&mut self, checkpoint: &Checkpoint) {
        for snapshot in &checkpoint.resources {
            if let Some(resource) = self.resources.get_mut(&snapshot.id) {
                resource.restore(snapshot);
            }
        }
    }

    /// Assign the next engine-authoritative event id.
    fn next_id(&mut self) -> EventId {
        self.next_event_id += 1;
        EventId(self.next_event_id)
    }

    /// Schedule an event for execution.
    ///
    /// The engine overwrites `event.id` with a fresh, authoritative id, so
    /// component-authored ids (which may collide) never affect ordering.
    pub fn schedule_event(&mut self, mut event: Event) {
        event.id = self.next_id();
        self.event_queue.push(ScheduledEvent(event));
    }

    pub fn step(&mut self) -> Option<Event> {
        if let Some(max) = self.config.max_events {
            if self.processed_events >= max {
                log::warn!(
                    "max_events limit ({max}) reached at t={:?}; stopping run loop",
                    self.clock
                );
                return None;
            }
        }

        let ScheduledEvent(event) = self.event_queue.pop()?;
        self.processed_events += 1;
        self.clock = event.time;

        log::trace!(
            "t={:?} executing #{:?} {:?} -> #{}",
            event.time,
            event.id.0,
            event.payload,
            event.target.0
        );

        // 1. Record event execution in the causal graph (unless tracing is off).
        let current_trace_id = if self.config.tracing_mode.is_enabled() {
            let target_name = self
                .components
                .get(&event.target)
                .map(|c| c.name())
                .unwrap_or("Engine/Resource");
            let desc = format!(
                "Event #{:?} [{:?}] executed on Component '{}' (#{})",
                event.id.0, event.payload, target_name, event.target.0
            );
            Some(
                self.causal_graph
                    .record_event(event.id, event.time, desc, event.cause),
            )
        } else {
            event.cause
        };

        // 2. Notify all registered observers (observation is independent of tracing).
        for observer in &mut self.observers {
            observer.on_event(&event);
        }

        // Prepare exec event with updated cause.
        let mut exec_event = event.clone();
        exec_event.cause = current_trace_id;

        // 3. Handle Resource lifecycle events in the engine loop.
        match &exec_event.payload {
            EventPayload::ResourceRequested {
                resource_id,
                amount,
                requester,
            } => {
                let resource_id = *resource_id;
                let amount = *amount;
                let requester = *requester;

                let known = self
                    .resources
                    .get(&resource_id)
                    .map(|res| (res.name.clone(), res.service_time));

                match known {
                    Some((res_name, service_time)) => {
                        let result = self
                            .resources
                            .get_mut(&resource_id)
                            .expect("resource checked above")
                            .request(requester, amount, exec_event.cause);

                        match result {
                            ResourceRequestResult::Granted => {
                                self.grant_and_schedule(
                                    resource_id,
                                    requester,
                                    amount,
                                    service_time,
                                    exec_event.cause,
                                );
                            }
                            ResourceRequestResult::Queued { queue_position } => {
                                if self.config.tracing_mode.is_enabled() {
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
                            }
                            ResourceRequestResult::Invalid { reason } => {
                                self.push_internal(
                                    requester,
                                    requester,
                                    EventPayload::ResourceDenied {
                                        resource_id,
                                        amount,
                                        reason: reason.to_string(),
                                    },
                                    exec_event.cause,
                                );
                            }
                        }
                    }
                    None => {
                        self.push_internal(
                            requester,
                            requester,
                            EventPayload::ResourceDenied {
                                resource_id,
                                amount,
                                reason: format!("Resource #{} not found", resource_id.0),
                            },
                            exec_event.cause,
                        );
                    }
                }
            }
            EventPayload::ResourceReleased {
                resource_id,
                amount,
                releaser: _,
            } => {
                let resource_id = *resource_id;
                let amount = *amount;
                let service_time = self
                    .resources
                    .get(&resource_id)
                    .map(|res| res.service_time)
                    .unwrap_or(0);

                let granted_list = if let Some(res) = self.resources.get_mut(&resource_id) {
                    res.release(amount)
                } else {
                    Vec::new()
                };

                for granted in granted_list {
                    // Link the unblocking directly to the release event.
                    self.grant_and_schedule(
                        resource_id,
                        granted.requester,
                        granted.amount,
                        service_time,
                        current_trace_id,
                    );
                }
            }
            _ => {}
        }

        // 4. Dispatch to the target component.
        if let Some(component) = self.components.get_mut(&event.target) {
            let mut ctx =
                SimulationContext::new(self.clock, &mut self.causal_graph, self.config.tracing_mode);
            let new_events = component.handle_event(&exec_event, &mut ctx);

            for new_event in new_events {
                let cause = new_event.cause.or(current_trace_id);

                // Route PortSignal outputs through registered connections.
                if let EventPayload::PortSignal { port_id, ref value } = new_event.payload {
                    let source = new_event.source;
                    let targets: Vec<_> = self
                        .connections
                        .iter()
                        .filter(|c| c.source_component == source && c.source_port == port_id)
                        .map(|c| (c.target_component, c.target_port))
                        .collect();

                    for (target_comp, target_port) in targets {
                        let id = self.next_id();
                        let mut fwd = Event::new(
                            id,
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
                    let id = self.next_id();
                    let mut e = new_event;
                    e.id = id;
                    e.cause = cause;
                    self.event_queue.push(ScheduledEvent(e));
                }
            }
        }

        Some(event)
    }

    /// Push an engine-generated event, assigning it a fresh authoritative id.
    fn push_internal(
        &mut self,
        source: ComponentId,
        target: ComponentId,
        payload: EventPayload,
        cause: Option<TraceId>,
    ) {
        let id = self.next_id();
        let mut event = Event::new(id, self.clock, source, target, payload);
        event.cause = cause;
        self.event_queue.push(ScheduledEvent(event));
    }

    /// Emit a `ResourceGranted` and, when the resource has a service time,
    /// schedule the automatic release that frees the capacity later.
    ///
    /// This is the mechanism that turns contention into latency: because a
    /// granted allocation holds capacity for `service_time`, any request queued
    /// behind it waits that long before it can be granted — with no
    /// domain-specific "slow down" rule anywhere.
    fn grant_and_schedule(
        &mut self,
        resource_id: ResourceId,
        requester: ComponentId,
        amount: Amount,
        service_time: u64,
        cause: Option<TraceId>,
    ) {
        self.push_internal(
            requester,
            requester,
            EventPayload::ResourceGranted { resource_id, amount },
            cause,
        );

        if service_time > 0 {
            let id = self.next_id();
            let completes_at = SimulationTime::new(self.clock.ticks() + service_time);
            let mut event = Event::new(
                id,
                completes_at,
                requester,
                requester,
                EventPayload::ResourceReleased {
                    resource_id,
                    amount,
                    releaser: requester,
                },
            );
            event.cause = cause;
            self.event_queue.push(ScheduledEvent(event));
        }
    }

    pub fn run_until(&mut self, until: SimulationTime) {
        while let Some(ScheduledEvent(next_event)) = self.event_queue.peek() {
            if next_event.time > until {
                break;
            }
            if self.step().is_none() {
                break;
            }
        }
    }
}
