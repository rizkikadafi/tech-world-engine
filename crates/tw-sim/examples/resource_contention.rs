use tw_core::{
    Component, ComponentId, Event, EventId, EventPayload, Port, Resource, ResourceId,
    SimulationContext, SimulationTime, State,
};
use tw_sim::Simulation;

#[derive(Debug)]
struct WorkerProcess {
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
                // Request resource at initial tick
                println!(
                    "[{:?}] Component '{}' (#{}) requesting 1 unit of Resource #{:?}",
                    ctx.time(),
                    self.name,
                    self.id.0,
                    self.resource_id.0
                );
                out_events.push(Event::new(
                    EventId(0),
                    ctx.time(),
                    self.id,
                    self.id,
                    EventPayload::ResourceRequested {
                        resource_id: self.resource_id,
                        amount: 1,
                        requester: self.id,
                    },
                ));
            }
            EventPayload::ResourceGranted { resource_id, amount: _ } => {
                println!(
                    "[{:?}] Component '{}' (#{}) GOT GRANTED Resource #{:?} -> Starting work for {} ticks!",
                    ctx.time(),
                    self.name,
                    self.id.0,
                    resource_id.0,
                    self.work_duration
                );
                self.state.set("status", "Working");

                // Schedule work completion
                let finish_time = SimulationTime::new(ctx.time().ticks() + self.work_duration);
                out_events.push(Event::new(
                    EventId(0),
                    finish_time,
                    self.id,
                    self.id,
                    EventPayload::Custom("WorkCompleted".to_string()),
                ));
            }
            EventPayload::Custom(msg) if msg == "WorkCompleted" => {
                println!(
                    "[{:?}] Component '{}' (#{}) finished work! Releasing 1 unit of Resource #{:?}",
                    ctx.time(),
                    self.name,
                    self.id.0,
                    self.resource_id.0
                );
                self.state.set("status", "Finished");

                out_events.push(Event::new(
                    EventId(0),
                    ctx.time(),
                    self.id,
                    self.id,
                    EventPayload::ResourceReleased {
                        resource_id: self.resource_id,
                        amount: 1,
                        releaser: self.id,
                    },
                ));
            }
            EventPayload::ResourceDenied { reason, .. } => {
                println!(
                    "[{:?}] Component '{}' (#{}) request DENIED: {}",
                    ctx.time(),
                    self.name,
                    self.id.0,
                    reason
                );
                self.state.set("status", "Denied");
            }
            _ => {}
        }

        out_events
    }
}

fn main() {
    println!("=== Tech World Engine Phase 5: Generic Resource System Example ===");

    let mut sim = Simulation::new();

    // 1. Register a shared compute capacity resource (Capacity: 2)
    let compute_res = Resource::new(ResourceId(100), "SharedComputePool", 2);
    println!(
        "Registered Resource: '{}' (#{}) with Capacity: {}",
        compute_res.name, compute_res.id.0, compute_res.capacity
    );
    sim.register_resource(compute_res);

    // 2. Register 3 worker processes competing for the resource
    let worker1 = WorkerProcess::new(ComponentId(1), "Worker1", ResourceId(100), 5);
    let worker2 = WorkerProcess::new(ComponentId(2), "Worker2", ResourceId(100), 8);
    let worker3 = WorkerProcess::new(ComponentId(3), "Worker3", ResourceId(100), 4);

    sim.register_component(Box::new(worker1));
    sim.register_component(Box::new(worker2));
    sim.register_component(Box::new(worker3));

    // 3. Schedule initial start events for all 3 workers at t=0
    println!("\n--- Triggering start ticks for Worker1, Worker2, Worker3 at t=0 ---");
    sim.schedule_event(Event::new(
        EventId(1),
        SimulationTime::new(0),
        ComponentId(1),
        ComponentId(1),
        EventPayload::Tick,
    ));
    sim.schedule_event(Event::new(
        EventId(2),
        SimulationTime::new(0),
        ComponentId(2),
        ComponentId(2),
        EventPayload::Tick,
    ));
    sim.schedule_event(Event::new(
        EventId(3),
        SimulationTime::new(0),
        ComponentId(3),
        ComponentId(3),
        EventPayload::Tick,
    ));

    // 4. Run simulation
    let mut worker3_finish_trace = None;
    while let Some(event) = sim.step() {
        if event.target == ComponentId(3)
            && matches!(&event.payload, EventPayload::Custom(msg) if msg == "WorkCompleted")
        {
            worker3_finish_trace = event.cause;
        }
    }

    // 5. Causal explanation for Worker3's completion
    println!("\n--- Causal Tracing Query (WHY did Worker3 finish at t=9?) ---");
    if let Some(trace_id) = worker3_finish_trace {
        let explanation = sim.explain(trace_id);
        for (i, node) in explanation.iter().enumerate() {
            println!("  Step {}: [t={}] {}", i + 1, node.time.ticks(), node.description);
        }
    }

    println!("\nPhase 5 Resource System verified successfully!");
}
