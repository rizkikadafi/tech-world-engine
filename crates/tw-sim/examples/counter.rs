//! Demonstrates a generic counter component driven entirely by the event loop.
//! The engine has no idea what a "counter" is.

use tw_sim::scenarios::counter::{self, COUNTER_ID};
use tw_sim::SimulationEngine;

fn main() {
    println!("=== Tech World Engine: Counter Example ===");

    let mut engine = SimulationEngine::new(counter::build(3, 5));
    engine.run_ticks(20);

    let count = engine
        .simulation()
        .component_state(COUNTER_ID)
        .and_then(|s| s.get_int("count"))
        .unwrap_or(0);

    println!(
        "Simulation finished at t={} ticks",
        engine.simulation().current_time().ticks()
    );
    println!("Counter value: {count}");
}
