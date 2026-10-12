//! Disk service-time scenario.
//!
//! A single-capacity resource ("Disk") takes `service_time` ticks to complete
//! each granted request. The engine releases the capacity automatically when the
//! service completes, so requests that arrive while the disk is busy simply wait
//! in the resource queue. The resulting latency is **emergent**: no component
//! contains a rule like "if the disk is busy, slow down".

use tw_core::{
    Component, ComponentId, Event, EventPayload, Port, Resource, ResourceId, SimulationContext,
    SimulationTime, State,
};

use crate::simulation::Simulation;

pub const DISK_ID: ResourceId = ResourceId(200);
pub const JOB1_ID: ComponentId = ComponentId(1);
pub const JOB2_ID: ComponentId = ComponentId(2);
pub const JOB3_ID: ComponentId = ComponentId(3);

/// How many ticks the disk is busy per granted request.
pub const SERVICE_TIME: u64 = 3;

#[derive(Debug)]
pub struct DiskJob {
    id: ComponentId,
    name: String,
    resource_id: ResourceId,
    ports: Vec<Port>,
    state: State,
}

impl DiskJob {
    pub fn new(id: ComponentId, name: impl Into<String>, resource_id: ResourceId) -> Self {
        let mut state = State::new();
        state.set("status", "WaitingToStart");
        Self {
            id,
            name: name.into(),
            resource_id,
            ports: Vec::new(),
            state,
        }
    }
}

impl Component for DiskJob {
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
                self.state.set("status", "Requested");
                self.state.set("arrived_at", ctx.time().ticks() as i64);
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
                self.state.set("status", "Granted");
                self.state.set("granted_at", ctx.time().ticks() as i64);
            }
            EventPayload::ResourceReleased { .. } => {
                // The engine emits this automatically once service completes.
                self.state.set("status", "Finished");
                self.state.set("released_at", ctx.time().ticks() as i64);
            }
            EventPayload::ResourceDenied { .. } => {
                self.state.set("status", "Denied");
            }
            _ => {}
        }

        out_events
    }
}

/// The per-job start delays, producing staggered arrival times.
pub const JOBS: [(ComponentId, u64); 3] = [(JOB1_ID, 0), (JOB2_ID, 1), (JOB3_ID, 2)];

/// Build the disk scenario: one capacity-1 disk with a service time, three jobs.
pub fn build() -> Simulation {
    let mut sim = Simulation::new();
    sim.register_resource(
        Resource::new(DISK_ID, "Disk", 1).with_service_time(SERVICE_TIME),
    );

    for (id, start_tick) in JOBS {
        sim.register_component(Box::new(DiskJob::new(id, format!("Job{}", id.0), DISK_ID)));
        sim.schedule_event(Event::at(
            SimulationTime::new(start_tick),
            id,
            id,
            EventPayload::Tick,
        ));
    }

    sim
}

/// The latency each job experienced: time between arriving and being granted.
pub fn latencies(engine: &crate::SimulationEngine) -> Vec<(ComponentId, i64)> {
    JOBS.iter()
        .filter_map(|(id, _)| {
            let state = engine.simulation().component_state(*id)?;
            let arrived = state.get_int("arrived_at")?;
            let granted = state.get_int("granted_at")?;
            Some((*id, granted - arrived))
        })
        .collect()
}

/// The tick at which each job was granted (`None` if it never was).
pub fn granted_at(engine: &crate::SimulationEngine, id: ComponentId) -> Option<i64> {
    engine
        .simulation()
        .component_state(id)
        .and_then(|state| state.get_int("granted_at"))
}

/// The tick at which each job finished service (`None` if it never did).
pub fn released_at(engine: &crate::SimulationEngine, id: ComponentId) -> Option<i64> {
    engine
        .simulation()
        .component_state(id)
        .and_then(|state| state.get_int("released_at"))
}
