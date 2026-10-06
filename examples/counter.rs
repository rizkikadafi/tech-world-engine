use tw_core::{
    Component, ComponentId, Event, EventId, EventPayload, State, SimulationTime,
};
use tw_sim::{Simulation, SimulationEngine};

#[derive(Debug)]
struct CounterComponent {
    id: ComponentId,
    name: String,
    state: State,
    count: u64,
}

impl CounterComponent {
    fn new(id: ComponentId, name: impl Into<String>) -> Self {
        let mut state = State::new();
        state.set("count", "0");
        Self {
            id,
            name: name.into(),
            state,
            count: 0,
        }
    }
}

impl Component for CounterComponent {
    fn id(&self) -> ComponentId {
        self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn handle_event(&mut self, event: &Event) -> Vec<Event> {
        let mut next_events = Vec::new();
        if let EventPayload::Tick = event.payload {
            self.count += 1;
            self.state.set("count", self.count.to_string());
            println!(
                "[Time {}] Component '{}' received tick event. New count: {}",
                event.time.ticks(),
                self.name,
                self.count
            );

            // Schedule next tick after 5 ticks if count < 3
            if self.count < 3 {
                next_events.push(Event::new(
                    EventId(event.id.0 + 1),
                    SimulationTime::new(event.time.ticks() + 5),
                    self.id,
                    self.id,
                    EventPayload::Tick,
                ));
            }
        }
        next_events
    }

    fn state(&self) -> &State {
        &self.state
    }
}

fn main() {
    println!("=== Tech World Engine: Counter Example ===");
    let mut sim = Simulation::new();

    let counter_id = ComponentId(1);
    let counter = CounterComponent::new(counter_id, "GenericCounter");
    sim.register_component(Box::new(counter));

    // Schedule initial event at t=0
    sim.schedule_event(Event::new(
        EventId(100),
        SimulationTime::new(0),
        counter_id,
        counter_id,
        EventPayload::Tick,
    ));

    let mut engine = SimulationEngine::new(sim);
    engine.run_ticks(20);

    println!("Simulation finished at time: {} ticks", engine.simulation().current_time().ticks());
}
