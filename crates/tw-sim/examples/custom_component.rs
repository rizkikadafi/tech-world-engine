//! Build your own components with the public engine API.
//!
//! This example implements two components from scratch — no scenario module,
//! no domain-specific engine support:
//!
//! ```text
//! Accumulator --(sum)--> Recorder
//! ```
//!
//! It shows the full extension path: ports, connections, typed state, event
//! scheduling, lifecycle hooks, and wiring validation. Run it with:
//!
//! ```bash
//! cargo run -p tw-sim --example custom_component
//! ```

use tw_core::{
    Component, ComponentId, Connection, Event, EventPayload, Port, PortDirection, PortId,
    SimulationContext, SimulationTime, State, StateValue,
};
use tw_sim::{Simulation, SimulationEngine};

const ACCUMULATOR_ID: ComponentId = ComponentId(1);
const RECORDER_ID: ComponentId = ComponentId(2);

const ADD_IN: PortId = PortId(1);
const SUM_OUT: PortId = PortId(2);
const VALUE_IN: PortId = PortId(1);

/// Adds every integer it receives and emits the running total.
#[derive(Debug)]
struct Accumulator {
    id: ComponentId,
    name: String,
    ports: Vec<Port>,
    state: State,
    total: i64,
    updates: i64,
}

impl Accumulator {
    fn new() -> Self {
        let mut state = State::new();
        state.set("total", 0);
        state.set("updates", 0);

        Self {
            id: ACCUMULATOR_ID,
            name: "Accumulator".to_string(),
            ports: vec![
                Port::new(ADD_IN, "add", PortDirection::Input),
                Port::new(SUM_OUT, "sum", PortDirection::Output),
            ],
            state,
            total: 0,
            updates: 0,
        }
    }
}

impl Component for Accumulator {
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

    fn on_mount(&mut self, ctx: &mut SimulationContext) -> Vec<Event> {
        self.state.set("mounted_at", ctx.time().ticks() as i64);
        Vec::new()
    }

    fn on_unmount(&mut self, ctx: &mut SimulationContext) -> Vec<Event> {
        self.state.set("unmounted_at", ctx.time().ticks() as i64);
        Vec::new()
    }

    fn handle_event(&mut self, event: &Event, _ctx: &mut SimulationContext) -> Vec<Event> {
        let EventPayload::PortSignal { port_id, value } = &event.payload else {
            return Vec::new();
        };
        if *port_id != ADD_IN {
            return Vec::new();
        }

        let Some(delta) = value.as_int() else {
            return Vec::new();
        };

        self.total += delta;
        self.updates += 1;
        self.state.set("total", self.total);
        self.state.set("updates", self.updates);

        // The engine assigns the real event id; `Event::at` uses a placeholder.
        vec![Event::at(
            event.time,
            self.id,
            self.id,
            EventPayload::PortSignal {
                port_id: SUM_OUT,
                value: StateValue::from(self.total),
            },
        )]
    }
}

/// Records the last value it receives and how many values arrived.
#[derive(Debug)]
struct Recorder {
    id: ComponentId,
    name: String,
    ports: Vec<Port>,
    state: State,
}

impl Recorder {
    fn new() -> Self {
        let mut state = State::new();
        state.set("count", 0);

        Self {
            id: RECORDER_ID,
            name: "Recorder".to_string(),
            ports: vec![Port::new(VALUE_IN, "value", PortDirection::Input)],
            state,
        }
    }
}

impl Component for Recorder {
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
        if let EventPayload::PortSignal { port_id, value } = &event.payload {
            if *port_id == VALUE_IN {
                if let Some(value) = value.as_int() {
                    let count = self.state.get_int("count").unwrap_or(0) + 1;
                    self.state.set("last_value", value);
                    self.state.set("count", count);
                }
            }
        }
        Vec::new()
    }
}

fn main() {
    let mut sim = Simulation::new();
    sim.register_component(Box::new(Accumulator::new()));
    sim.register_component(Box::new(Recorder::new()));
    sim.register_connection(Connection::new(
        ACCUMULATOR_ID,
        SUM_OUT,
        RECORDER_ID,
        VALUE_IN,
    ));

    // Catch port/id typos before running. The engine does not validate wiring
    // at registration time, and unmatched PortSignals are silently dropped.
    sim.validate_wiring().expect("wiring is valid");

    for (at, delta) in [(1u64, 3i64), (2, 4), (3, 5)] {
        sim.schedule_event(Event::at(
            SimulationTime::new(at),
            ComponentId(0),
            ACCUMULATOR_ID,
            EventPayload::PortSignal {
                port_id: ADD_IN,
                value: StateValue::from(delta),
            },
        ));
    }

    let mut engine = SimulationEngine::new(sim);
    engine.run_ticks(5);

    let accumulator = engine
        .simulation()
        .component_state(ACCUMULATOR_ID)
        .expect("accumulator registered");
    let recorder = engine
        .simulation()
        .component_state(RECORDER_ID)
        .expect("recorder registered");

    println!("=== Custom component example ===");
    println!(
        "Accumulator: total={} updates={} mounted_at={:?}",
        accumulator.get_int("total").unwrap_or_default(),
        accumulator.get_int("updates").unwrap_or_default(),
        accumulator.get_int("mounted_at"),
    );
    println!(
        "Recorder:    last_value={:?} count={}",
        recorder.get_int("last_value"),
        recorder.get_int("count").unwrap_or_default(),
    );

    engine.simulation_mut().shutdown();
    let accumulator = engine
        .simulation()
        .component_state(ACCUMULATOR_ID)
        .expect("accumulator registered");
    println!(
        "Accumulator unmounted_at={:?}",
        accumulator.get_int("unmounted_at")
    );
}
