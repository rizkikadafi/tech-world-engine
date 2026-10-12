//! Tests for [`SimulationConfig`]: the event cap, tracing mode, and seed.

use tw_core::TracingMode;
use tw_sim::scenarios::counter;
use tw_sim::{Simulation, SimulationConfig, SimulationEngine};

#[test]
fn max_events_stops_the_run_loop() {
    // A counter that would tick 100 times, but the cap stops it early and
    // `run_ticks` must not spin forever.
    let mut sim = counter::build(100, 1);
    sim.configure(SimulationConfig::default().with_max_events(7));

    let mut engine = SimulationEngine::new(sim);
    engine.run_ticks(1000);

    assert_eq!(engine.simulation().events_processed(), 7);
}

#[test]
fn step_returns_none_after_cap() {
    let mut sim = counter::build(100, 1);
    sim.configure(SimulationConfig::default().with_max_events(3));

    while sim.step().is_some() {}

    assert_eq!(sim.events_processed(), 3);
    assert!(sim.step().is_none());
}

#[test]
fn config_round_trips() {
    let config = SimulationConfig::default()
        .with_tracing_mode(TracingMode::Off)
        .with_max_events(42)
        .with_seed(7);

    let sim = Simulation::with_config(config);

    assert_eq!(sim.config(), config);
    assert_eq!(sim.tracing_mode(), TracingMode::Off);
}

#[test]
fn default_config_has_no_cap() {
    let sim = Simulation::new();
    assert_eq!(sim.config().max_events, None);
    assert_eq!(sim.config().seed, 0);
    assert_eq!(sim.tracing_mode(), TracingMode::default());
}
