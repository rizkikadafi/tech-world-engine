//! Construction-time fidelity selection:
//!
//! ```text
//! ComponentSpec + FidelityFactory
//!   ├── Fidelity::Transistor → raw netlist
//!   ├── Fidelity::Bridge     → CircuitComponent (mixed-signal)
//!   └── Fidelity::Derived    → DerivedGate / DerivedDff (discrete)
//! ```
//!
//! The same logical component exposes identical ports at every discrete
//! fidelity, and `refine` moves it one step toward more detail.
//!
//! Run with:
//! ```bash
//! cargo run -p tw-electrical --example fidelity_selection
//! ```

use tw_core::ComponentId;
use tw_electrical::prelude::*;

fn describe(kind: GateKind) -> ComponentSpec {
    ComponentSpec::new(
        ComponentId(kind as u64 + 1),
        format!("{}_model", kind.output_name()),
        kind,
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut factory = FidelityFactory::with_defaults();

    println!("== fidelity selection ==");
    for kind in [GateKind::Inverter, GateKind::Nand2, GateKind::Dff] {
        let spec = describe(kind);
        let characterization = factory.characterization(kind)?;
        println!(
            "\n{:?}: t_pHL={:.3} ns, t_pLH={:.3} ns, E/cycle={:.3} pJ",
            kind,
            characterization.timing.t_pHL * 1e9,
            characterization.timing.t_pLH * 1e9,
            characterization.timing.energy_per_cycle * 1e12,
        );

        let derived = factory.select_component(&spec, Fidelity::Derived)?;
        let bridge = factory.select_component(&spec, Fidelity::Bridge)?;
        let transistor = factory.select(&spec, Fidelity::Transistor)?;

        println!("  ports (derived): {:?}", derived.ports());
        assert_eq!(
            derived.ports(),
            bridge.ports(),
            "fidelities must expose identical ports"
        );

        match &transistor {
            Model::Transistor(circuit) => println!(
                "  transistor netlist: {} nodes, nonlinear={}",
                circuit.node_count(),
                circuit.is_nonlinear()
            ),
            other => println!("  unexpected model: {:?}", other.fidelity()),
        }
    }

    println!("\n== refinement ==");
    let inverter = describe(GateKind::Inverter);
    let mut model = factory.select(&inverter, Fidelity::Derived)?;
    loop {
        println!("current fidelity: {}", model.fidelity().label());
        match factory.refine(&inverter, model.fidelity()) {
            Ok(next) => model = next,
            Err(FidelityError::AlreadyMostDetailed(_)) => {
                println!("no more detail available");
                break;
            }
            Err(error) => return Err(error.into()),
        }
    }

    println!(
        "\ncached characterizations: {} (each circuit measured at most once)",
        factory.cached_count(),
    );

    Ok(())
}
