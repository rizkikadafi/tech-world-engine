//! Demonstrates emergent latency: a single-capacity disk with a service time.
//! Requests that arrive while the disk is busy simply queue, so later jobs wait
//! longer — no "if busy, slow down" rule anywhere.

use tw_sim::scenarios::disk_service::{self, JOB1_ID, JOB2_ID, JOB3_ID};
use tw_sim::SimulationEngine;

fn main() {
    println!("=== Tech World Engine: Disk Service-Time (Emergent Latency) Example ===");

    let mut engine = SimulationEngine::new(disk_service::build());
    while engine.simulation_mut().step().is_some() {}

    println!("  job  | arrived | granted | latency");
    for (label, id) in [
        ("Job1", JOB1_ID),
        ("Job2", JOB2_ID),
        ("Job3", JOB3_ID),
    ] {
        let state = engine.simulation().component_state(id);
        let arrived = state.and_then(|s| s.get_int("arrived_at")).unwrap_or(-1);
        let granted = state.and_then(|s| s.get_int("granted_at")).unwrap_or(-1);
        println!(
            "  {label} |    {arrived}    |    {granted}    |   {}",
            granted - arrived
        );
    }

    println!("\nLatency grows from queue position alone (service_time = {}).", disk_service::SERVICE_TIME);
}
