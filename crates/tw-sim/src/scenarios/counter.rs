//! Generic counter scenario: a component that increments its own state on each
//! `Tick` event. The engine knows nothing about what a "counter" is.

use tw_core::{
    Component, ComponentId, Event, EventPayload, Port, SimulationContext, SimulationTime, State,
};

use crate::simulation::Simulation;

pub const COUNTER_ID: ComponentId = ComponentId(1);

#[derive(Debug)]
pub struct Counter {
    id: ComponentId,
    name: String,
    ports: Vec<Port>,
    state: State,
    count: u64,
    limit: u64,
    interval: u64,
}

impl Counter {
    pub fn new(id: ComponentId, name: impl Into<String>, limit: u64, interval: u64) -> Self {
        let mut state = State::new();
        state.set("count", "0");
        Self {
            id,
            name: name.into(),
            ports: Vec::new(),
            state,
            count: 0,
            limit,
            interval,
        }
    }
}

impl Component for Counter {
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

    fn handle_event(&mut self, event: &Event, _ctx: &mut SimulationContext) -> Vec<Event> {
        let mut next_events = Vec::new();
        if let EventPayload::Tick = event.payload {
            self.count += 1;
            self.state.set("count", self.count.to_string());

            if self.count < self.limit {
                let at = SimulationTime::new(event.time.ticks() + self.interval);
                next_events.push(Event::at(at, self.id, self.id, EventPayload::Tick));
            }
        }
        next_events
    }
}

/// Build a counter simulation that ticks `limit` times, `interval` ticks apart.
pub fn build(limit: u64, interval: u64) -> Simulation {
    let mut sim = Simulation::new();
    sim.register_component(Box::new(Counter::new(
        COUNTER_ID,
        "GenericCounter",
        limit,
        interval,
    )));
    sim.schedule_event(Event::at(
        SimulationTime::new(0),
        COUNTER_ID,
        COUNTER_ID,
        EventPayload::Tick,
    ));
    sim
}
