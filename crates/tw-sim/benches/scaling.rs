//! Scaling benchmark: how the event loop behaves as the number of components
//! and events grows. Each component executes exactly one `Tick`, so the measured
//! work is roughly one scheduled event per component.

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use tw_core::{
    Component, ComponentId, Event, EventPayload, Port, SimulationContext, SimulationTime, State,
};
use tw_sim::{Simulation, SimulationEngine};

#[derive(Debug)]
struct TickComponent {
    id: ComponentId,
    ports: Vec<Port>,
    state: State,
}

impl TickComponent {
    fn new(id: u64) -> Self {
        Self {
            id: ComponentId(id),
            ports: Vec::new(),
            state: State::new(),
        }
    }
}

impl Component for TickComponent {
    fn id(&self) -> ComponentId {
        self.id
    }

    fn name(&self) -> &str {
        "tick"
    }

    fn ports(&self) -> &[Port] {
        &self.ports
    }

    fn state(&self) -> &State {
        &self.state
    }

    fn handle_event(&mut self, event: &Event, _ctx: &mut SimulationContext) -> Vec<Event> {
        if let EventPayload::Tick = &event.payload {
            let ticks = self.state.get_int("ticks").unwrap_or(0) + 1;
            self.state.set("ticks", ticks);
        }
        Vec::new()
    }
}

fn build_engine(components: usize) -> SimulationEngine {
    let mut sim = Simulation::new();
    for i in 0..components {
        let id = ComponentId(i as u64 + 1);
        sim.register_component(Box::new(TickComponent::new(i as u64 + 1)));
        sim.schedule_event(Event::at(
            SimulationTime::new(0),
            id,
            id,
            EventPayload::Tick,
        ));
    }
    SimulationEngine::new(sim)
}

fn event_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_throughput");
    for &size in &[10usize, 100, 1_000, 10_000] {
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter_batched(
                || build_engine(size),
                |mut engine| {
                    while engine.simulation_mut().step().is_some() {}
                },
                BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

criterion_group!(benches, event_throughput);
criterion_main!(benches);
