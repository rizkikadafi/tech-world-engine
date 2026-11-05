//! Optional large-scale stress test for the derived fidelity.
//!
//! The criterion benchmark stops at 100k components because each `DerivedGate`
//! carries a `State` map and the simulation stores every component. This test
//! lets you push to 1M manually:
//!
//! ```bash
//! cargo test --release -p tw-electrical --test fidelity_scale -- --ignored --nocapture
//! ```
//!
//! Override the component count with `TW_SCALE_COMPONENTS`, for example:
//!
//! ```bash
//! TW_SCALE_COMPONENTS=100000 cargo test --release -p tw-electrical --test fidelity_scale -- --ignored --nocapture
//! ```

use std::time::{Duration, Instant};

use tw_core::{ComponentId, Event, EventPayload, PortId, SimulationTime, StateValue};
use tw_electrical::prelude::*;
use tw_sim::Simulation;

const NAND_INPUT_A: PortId = PortId(1);
const NAND_INPUT_B: PortId = PortId(2);

fn process_rss_bytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmRSS:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib * 1024)
}

fn format_duration(duration: Duration) -> String {
    if duration.as_secs() > 0 {
        format!("{:.2} s", duration.as_secs_f64())
    } else {
        format!("{:.2} ms", duration.as_secs_f64() * 1e3)
    }
}

#[test]
#[ignore = "1M components needs a release build and significant RAM"]
fn million_derived_nand_switching() {
    let components: usize = std::env::var("TW_SCALE_COMPONENTS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1_000_000);

    let mut factory = FidelityFactory::with_defaults();
    let characterize_start = Instant::now();
    let measurement = factory
        .measurement(GateKind::Nand2)
        .expect("physical NAND2 characterized")
        .clone();
    let characterize_time = characterize_start.elapsed();

    let horizon = TimeBase::default().delay_ticks(measurement.gate.config.duration);

    let build_start = Instant::now();
    let mut sim = Simulation::new();
    for index in 0..components {
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
    sim.run_until(SimulationTime::new(0));
    let build_time = build_start.elapsed();
    let rss_after_build = process_rss_bytes();

    let run_start = Instant::now();
    for index in 0..components {
        let target = ComponentId(index as u64 + 1);
        for port_id in [NAND_INPUT_A, NAND_INPUT_B] {
            sim.schedule_event(Event::at(
                SimulationTime::new(0),
                ComponentId(u64::MAX),
                target,
                EventPayload::PortSignal {
                    port_id,
                    value: StateValue::Bool(true),
                },
            ));
        }
    }
    sim.run_until(SimulationTime::new(horizon));
    let run_time = run_start.elapsed();

    println!("components:          {components}");
    println!(
        "characterization:    {}",
        format_duration(characterize_time)
    );
    println!("build + mount:       {}", format_duration(build_time));
    println!("switch + run:        {}", format_duration(run_time));
    println!("events processed:    {}", sim.events_processed());
    println!("simulated horizon:   {horizon} ticks");
    if let Some(bytes) = rss_after_build {
        println!(
            "RSS after build:     {:.1} MiB",
            bytes as f64 / (1024.0 * 1024.0)
        );
    }
    println!(
        "simulated ns/second: {:.1}",
        (components as f64 * horizon as f64) / run_time.as_secs_f64()
    );

    assert_eq!(sim.component_count(), components);
    assert!(
        sim.events_processed() >= (components * 3) as u64,
        "every gate should process its two inputs and one output"
    );
}
