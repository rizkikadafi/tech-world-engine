//! Side-by-side comparison of the three `TracingMode`s and a causal chain dump.
//! Same simulation, different tracing cost: `Off` records nothing, `Abstract`
//! records one node per executed event, `Hierarchical` also records composite
//! internal routing.

use tw_core::{TraceId, TracingMode};
use tw_sim::scenarios::digital;

fn main() {
    println!("=== Tech World Engine: TracingMode Demonstration ===");

    for (label, mode) in [
        ("Abstract (black-box)", TracingMode::Abstract),
        ("Hierarchical (refined)", TracingMode::Hierarchical),
        ("Off (no tracing)", TracingMode::Off),
    ] {
        let engine = digital::run_full_adder(1, 1, 1, mode);
        println!(
            "{label:24} -> {} trace nodes",
            engine.simulation().causal_graph().nodes().len()
        );
    }

    println!("\n--- Causal chain (Hierarchical), A=1 B=1 Cin=1 ---");
    let engine = digital::run_full_adder(1, 1, 1, TracingMode::Hierarchical);
    let nodes = engine.simulation().causal_graph().nodes();
    let max = nodes.keys().map(|t| t.0).max().unwrap_or(1);
    for (i, node) in engine
        .simulation()
        .explain(TraceId(max))
        .iter()
        .enumerate()
    {
        println!(
            "  Step {}: [t={}] {}",
            i + 1,
            node.time.ticks(),
            node.description
        );
    }

    println!("\nTracingMode demonstration finished successfully!");
}
