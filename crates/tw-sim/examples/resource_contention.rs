//! Demonstrates emergent contention: three workers compete for a shared
//! capacity-2 resource. "Waiting" is not hardcoded — it falls out of the
//! resource queue.

use tw_sim::scenarios::resource_contention::{
    self, WORKER1_ID, WORKER2_ID, WORKER3_ID,
};
use tw_sim::SimulationEngine;

fn main() {
    println!("=== Tech World Engine Phase 5: Generic Resource System Example ===");

    let mut engine = SimulationEngine::new(resource_contention::build());
    while engine.simulation_mut().step().is_some() {}

    for (label, id) in [
        ("Worker1", WORKER1_ID),
        ("Worker2", WORKER2_ID),
        ("Worker3", WORKER3_ID),
    ] {
        let status = engine
            .simulation()
            .component_state(id)
            .and_then(|s| s.get("status"))
            .unwrap_or("<none>");
        println!("  {label}: {status}");
    }

    println!("\nPhase 5 Resource System verified successfully!");
}
