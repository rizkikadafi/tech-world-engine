//! Full adder truth table + hierarchical causal tracing, built from the shared
//! digital scenario (two HalfAdder composites + OR gate, wired with internal
//! connections). Proves emergent composition: no AND/OR/XOR/FullAdder lives in
//! the engine.

use tw_core::{Component, TraceId, TracingMode};
use tw_sim::scenarios::digital;

fn main() {
    println!("=== Tech World Engine: CompositeComponent Internal Connection ===");

    let demo = digital::create_full_adder();
    println!(
        "\nInternal connections inside '{}' (#{}):",
        demo.name(),
        demo.id().0
    );
    for conn in demo.internal_connections() {
        println!(
            "  Subcomponent #{} port {}  -->  Subcomponent #{} port {}",
            conn.source_component.0,
            conn.source_port.0,
            conn.target_component.0,
            conn.target_port.0
        );
    }
    drop(demo);

    println!("\n--- Full Adder Truth Table (TracingMode::Abstract) ---");
    println!(" A B Cin | Sum Carry | expected | status");
    for i in 0..8u8 {
        let (a, b, cin) = ((i >> 2) & 1, (i >> 1) & 1, i & 1);
        let engine = digital::run_full_adder(a, b, cin, TracingMode::Abstract);
        let got = digital::full_adder_result(&engine).expect("probe output");
        let expected = digital::expected_full_adder(a, b, cin);
        let ok = got == expected;

        println!(
            " {}  {}  {}  |   {}     {}    |    {} {}    | {}",
            a,
            b,
            cin,
            got.0,
            got.1,
            expected.0,
            expected.1,
            if ok { "OK" } else { "FAIL" }
        );
        assert!(
            ok,
            "FullAdder wrong for A={a} B={b} Cin={cin}: got {got:?}, expected {expected:?}"
        );
    }
    println!("Semua 8 kombinasi benar.");

    println!("\n--- Causal Tracing (TracingMode::Hierarchical), A=1 B=1 Cin=1 ---");
    let engine = digital::run_full_adder(1, 1, 1, TracingMode::Hierarchical);
    let nodes = engine.simulation().causal_graph().nodes();

    let internal: Vec<_> = nodes
        .values()
        .filter(|n| n.description.contains("routed Internal Connection"))
        .collect();
    let external_in = nodes
        .values()
        .filter(|n| n.description.contains("routed External Input"))
        .count();
    let boundary_out = nodes
        .values()
        .filter(|n| n.description.contains("bound Subcomponent"))
        .count();

    println!("Total trace nodes: {}", nodes.len());
    println!("  - routed External Input (boundary in)     : {external_in}");
    println!("  - routed Internal Connection (child→child): {}", internal.len());
    println!("  - bound Subcomponent (boundary out)       : {boundary_out}");
    assert!(!internal.is_empty(), "internal connection not triggered!");

    let last_internal = internal.iter().map(|n| n.id.0).max().unwrap();
    println!("\nCausal chain from trace #{last_internal}:");
    for (step, node) in engine
        .simulation()
        .explain(TraceId(last_internal))
        .iter()
        .enumerate()
    {
        println!(
            "  Step {}: [t={}] {}",
            step + 1,
            node.time.ticks(),
            node.description
        );
    }

    println!("\nInternal connection example finished successfully!");
}
