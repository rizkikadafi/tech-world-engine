use tw_core::{
    Component, ComponentId, Event, EventPayload, Port, SimulationContext, SimulationTime, State,
};
use tw_sim::Simulation;

#[derive(Debug)]
struct LifecycleProbe {
    id: ComponentId,
    ports: Vec<Port>,
    state: State,
}

impl LifecycleProbe {
    fn new(id: u64) -> Self {
        Self {
            id: ComponentId(id),
            ports: Vec::new(),
            state: State::new(),
        }
    }
}

impl Component for LifecycleProbe {
    fn id(&self) -> ComponentId {
        self.id
    }

    fn name(&self) -> &str {
        "probe"
    }

    fn ports(&self) -> &[Port] {
        &self.ports
    }

    fn state(&self) -> &State {
        &self.state
    }

    fn on_mount(&mut self, ctx: &mut SimulationContext) -> Vec<Event> {
        self.state.set("mounted_at", ctx.time().ticks() as i64);
        vec![Event::at(
            ctx.time(),
            self.id,
            self.id,
            EventPayload::Custom("mounted".to_string()),
        )]
    }

    fn on_unmount(&mut self, ctx: &mut SimulationContext) -> Vec<Event> {
        self.state.set("unmounted_at", ctx.time().ticks() as i64);
        vec![Event::at(
            ctx.time(),
            self.id,
            self.id,
            EventPayload::Custom("unmounted".to_string()),
        )]
    }

    fn handle_event(&mut self, event: &Event, _ctx: &mut SimulationContext) -> Vec<Event> {
        if let EventPayload::Custom(msg) = &event.payload {
            match msg.as_str() {
                "mounted" => self.state.set("saw_mount_event", true),
                "unmounted" => self.state.set("saw_unmount_event", true),
                _ => {}
            }
        }
        Vec::new()
    }
}

#[test]
fn on_mount_runs_on_registration_and_schedules_events() {
    let mut sim = Simulation::new();
    sim.register_component(Box::new(LifecycleProbe::new(1)));

    // The mount hook returns an event; it should be scheduled automatically.
    sim.run_until(SimulationTime::new(0));

    let state = sim
        .component_state(ComponentId(1))
        .expect("component registered");
    assert_eq!(state.get_int("mounted_at"), Some(0));
    assert_eq!(state.get_bool("saw_mount_event"), Some(true));
}

#[test]
fn shutdown_runs_on_unmount_and_schedules_events() {
    let mut sim = Simulation::new();
    sim.register_component(Box::new(LifecycleProbe::new(1)));

    sim.shutdown();
    sim.run_until(SimulationTime::new(0));

    let state = sim
        .component_state(ComponentId(1))
        .expect("component registered");
    assert_eq!(state.get_int("unmounted_at"), Some(0));
    assert_eq!(state.get_bool("saw_unmount_event"), Some(true));
}
