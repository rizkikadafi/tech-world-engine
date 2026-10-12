//! Generic resource contention scenario.
//!
//! Three worker components compete for a shared resource of capacity 2. The
//! "waiting" behavior is not hardcoded: it emerges from the resource queue in
//! the engine. Workers have different service durations so a worker can be
//! queued behind others and finish later.

use tw_core::{
    Component, ComponentId, Event, EventPayload, Port, Resource, ResourceId, SimulationContext,
    SimulationTime, State,
};

use crate::simulation::Simulation;

pub const RESOURCE_ID: ResourceId = ResourceId(100);
pub const WORKER1_ID: ComponentId = ComponentId(1);
pub const WORKER2_ID: ComponentId = ComponentId(2);
pub const WORKER3_ID: ComponentId = ComponentId(3);

#[derive(Debug)]
pub struct WorkerProcess {
    id: ComponentId,
    name: String,
    resource_id: ResourceId,
    work_duration: u64,
    ports: Vec<Port>,
    state: State,
}

impl WorkerProcess {
    pub fn new(
        id: ComponentId,
        name: impl Into<String>,
        resource_id: ResourceId,
        work_duration: u64,
    ) -> Self {
        let mut state = State::new();
        state.set("status", "Idle");
        Self {
            id,
            name: name.into(),
            resource_id,
            work_duration,
            ports: Vec::new(),
            state,
        }
    }
}

impl Component for WorkerProcess {
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

    fn handle_event(&mut self, event: &Event, ctx: &mut SimulationContext) -> Vec<Event> {
        let mut out_events = Vec::new();

        match &event.payload {
            EventPayload::Tick => {
                out_events.push(Event::at(
                    ctx.time(),
                    self.id,
                    self.id,
                    EventPayload::ResourceRequested {
                        resource_id: self.resource_id,
                        amount: 1.into(),
                        requester: self.id,
                    },
                ));
            }
            EventPayload::ResourceGranted { .. } => {
                self.state.set("status", "Working");
                let finish = SimulationTime::new(ctx.time().ticks() + self.work_duration);
                out_events.push(Event::at(
                    finish,
                    self.id,
                    self.id,
                    EventPayload::Custom("WorkCompleted".to_string()),
                ));
            }
            EventPayload::Custom(msg) if msg == "WorkCompleted" => {
                self.state.set("status", "Finished");
                out_events.push(Event::at(
                    ctx.time(),
                    self.id,
                    self.id,
                    EventPayload::ResourceReleased {
                        resource_id: self.resource_id,
                        amount: 1.into(),
                        releaser: self.id,
                    },
                ));
            }
            EventPayload::ResourceDenied { .. } => {
                self.state.set("status", "Denied");
            }
            _ => {}
        }

        out_events
    }
}

/// Build the contention scenario: capacity 2 resource, three workers.
pub fn build() -> Simulation {
    let mut sim = Simulation::new();
    sim.register_resource(Resource::new(RESOURCE_ID, "SharedComputePool", 2));
    sim.register_component(Box::new(WorkerProcess::new(
        WORKER1_ID,
        "Worker1",
        RESOURCE_ID,
        5,
    )));
    sim.register_component(Box::new(WorkerProcess::new(
        WORKER2_ID,
        "Worker2",
        RESOURCE_ID,
        8,
    )));
    sim.register_component(Box::new(WorkerProcess::new(
        WORKER3_ID,
        "Worker3",
        RESOURCE_ID,
        4,
    )));

    for id in [WORKER1_ID, WORKER2_ID, WORKER3_ID] {
        sim.schedule_event(Event::at(
            SimulationTime::new(0),
            id,
            id,
            EventPayload::Tick,
        ));
    }

    sim
}
