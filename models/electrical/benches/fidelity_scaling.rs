//! Cost of one NAND switching event at each fidelity.
//!
//! ```text
//! transistor edge  vs  bridge edge  vs  N derived-gate edges
//! ```
//!
//! The NAND2 measurement is performed once, outside the timed region, so the
//! benchmark compares switching cost rather than characterization cost.
//!
//! Run with:
//! ```bash
//! cargo bench -p tw-electrical --bench fidelity_scaling -- --quick
//! ```

use std::hint::black_box;
use std::sync::OnceLock;

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use tw_core::{ComponentId, Event, EventPayload, PortId, SimulationTime, StateValue};
use tw_electrical::prelude::*;
use tw_sim::Simulation;

const NAND_INPUT_A: PortId = PortId(1);
const NAND_INPUT_B: PortId = PortId(2);

fn nand_measurement() -> &'static Measurement {
    static MEASUREMENT: OnceLock<Measurement> = OnceLock::new();
    MEASUREMENT.get_or_init(|| {
        Characterizer::with_defaults()
            .measure(GateKind::Nand2)
            .expect("physical NAND2 characterized")
    })
}

fn nand_horizon_ticks() -> u64 {
    // Same simulated window as the transistor edge measurement.
    TimeBase::default().delay_ticks(nand_measurement().gate.config.duration)
}

fn schedule_nand_inputs(sim: &mut Simulation, target: ComponentId) {
    for port_id in [NAND_INPUT_A, NAND_INPUT_B] {
        sim.schedule_event(Event::at(
            SimulationTime::new(0),
            ComponentId(999_999),
            target,
            EventPayload::PortSignal {
                port_id,
                value: StateValue::Bool(true),
            },
        ));
    }
}

fn build_derived_sim(size: usize, factory: &mut FidelityFactory) -> Simulation {
    let mut sim = Simulation::new();
    for index in 0..size {
        let spec = ComponentSpec::new(
            ComponentId(index as u64 + 1),
            "derived-nand",
            GateKind::Nand2,
        );
        let component = factory
            .select_component(&spec, Fidelity::Derived)
            .expect("derived NAND2");
        sim.register_component(component);
    }
    // Drain mount events outside the timed region.
    sim.run_until(SimulationTime::new(0));
    sim
}

fn transistor_nand_edge(c: &mut Criterion) {
    let measurement = nand_measurement();
    let mut circuit = measurement.template.circuit.clone();
    let sources = measurement.template.input_sources();

    // Initial state: both NAND inputs low.
    for source in &sources {
        circuit
            .set_voltage_source(source, 0.0)
            .expect("input source");
    }
    let initial = dc_op(&circuit).expect("NAND2 initial operating point");

    // One falling edge: both inputs rise.
    for source in &sources {
        circuit
            .set_voltage_source(source, measurement.gate.config.vdd)
            .expect("input source");
    }

    let duration = measurement.gate.config.duration;
    let dt = measurement.gate.config.dt;

    c.bench_function("transistor_nand_edge", |b| {
        b.iter_batched(
            || initial.clone(),
            |state| {
                let result = transient(&circuit, duration, dt, Some(state)).expect("NAND2 edge");
                black_box(result.final_state().cloned());
            },
            BatchSize::SmallInput,
        );
    });
}

fn bridge_nand_edge(c: &mut Criterion) {
    let mut factory = FidelityFactory::with_defaults();
    let spec = ComponentSpec::new(ComponentId(1), "bridge-nand", GateKind::Nand2);
    let horizon = nand_horizon_ticks();

    c.bench_function("bridge_nand_edge", |b| {
        b.iter_batched(
            || {
                let mut sim = Simulation::new();
                let component = factory
                    .select_component(&spec, Fidelity::Bridge)
                    .expect("bridge NAND2");
                sim.register_component(component);
                sim.run_until(SimulationTime::new(0));
                sim
            },
            |mut sim| {
                schedule_nand_inputs(&mut sim, spec.id);
                sim.run_until(SimulationTime::new(horizon));
                black_box(sim.events_processed());
            },
            BatchSize::PerIteration,
        );
    });
}

fn derived_nand_switching(c: &mut Criterion) {
    let mut factory = FidelityFactory::with_defaults();
    let horizon = nand_horizon_ticks();
    let mut group = c.benchmark_group("derived_nand_switching");
    group.sample_size(10);

    for &size in &[1usize, 10, 100, 1_000, 10_000, 100_000] {
        // Two input events and one output event per gate.
        group.throughput(Throughput::Elements((size * 3) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter_batched(
                || build_derived_sim(size, &mut factory),
                |mut sim| {
                    for index in 0..size {
                        schedule_nand_inputs(&mut sim, ComponentId(index as u64 + 1));
                    }
                    sim.run_until(SimulationTime::new(horizon));
                    black_box(sim.events_processed());
                },
                BatchSize::LargeInput,
            );
        });
    }

    group.finish();
}

fn derived_nand_construction(c: &mut Criterion) {
    let mut factory = FidelityFactory::with_defaults();
    let spec = ComponentSpec::new(ComponentId(1), "derived-nand", GateKind::Nand2);

    // Prime the characterization cache before timing construction.
    factory
        .select_component(&spec, Fidelity::Derived)
        .expect("derived NAND2");

    let mut group = c.benchmark_group("derived_nand_construction");
    group.sample_size(10);

    for &size in &[1usize, 10, 100, 1_000, 10_000, 100_000] {
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter_batched(
                || (),
                |_| {
                    for _ in 0..size {
                        black_box(
                            factory
                                .select_component(&spec, Fidelity::Derived)
                                .expect("derived NAND2"),
                        );
                    }
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    transistor_nand_edge,
    bridge_nand_edge,
    derived_nand_switching,
    derived_nand_construction,
);
criterion_main!(benches);
