//! Tech World Engine CLI.
//!
//! The CLI is only an interface: all simulation wiring lives in
//! `tw_sim::scenarios`, never here.

use std::process::ExitCode;

use tw_core::ComponentId;
use tw_sim::scenarios::{counter, digital, disk_service, resource_contention};
use tw_sim::SimulationEngine;

const USAGE: &str = "\
Tech World Engine CLI

Usage:
  tw-cli run <scenario>      Run a scenario and print its result
  tw-cli inspect <scenario>  Run a scenario and print component state
  tw-cli list                List available scenarios
  tw-cli help                Show this help

Scenarios:
  counter                Generic counter that ticks to a limit
  resource-contention    Three workers competing for a capacity-2 resource
  disk-service           Capacity-1 disk with service time (emergent latency)
  full-adder             Full adder (composite components) truth table
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => run(args.get(1).map(String::as_str)),
        Some("inspect") => inspect(args.get(1).map(String::as_str)),
        Some("list") => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        None | Some("help") | Some("--help") | Some("-h") => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("error: unknown command '{other}'\n");
            eprint!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}

fn read_state(engine: &SimulationEngine, id: ComponentId, key: &str) -> Option<String> {
    engine
        .simulation()
        .component_state(id)?
        .get(key)
        .map(|v| v.to_string())
}

fn run(scenario: Option<&str>) -> ExitCode {
    match scenario {
        Some("counter") => {
            let mut engine = SimulationEngine::new(counter::build(5, 5));
            engine.run_ticks(100);
            println!(
                "counter: final count = {}",
                read_state(&engine, counter::COUNTER_ID, "count").unwrap_or_default()
            );
            ExitCode::SUCCESS
        }
        Some("resource-contention") => {
            let mut engine = SimulationEngine::new(resource_contention::build());
            while engine.simulation_mut().step().is_some() {}
            for (label, id) in [
                ("Worker1", resource_contention::WORKER1_ID),
                ("Worker2", resource_contention::WORKER2_ID),
                ("Worker3", resource_contention::WORKER3_ID),
            ] {
                println!(
                    "{label}: {}",
                    read_state(&engine, id, "status").unwrap_or_default()
                );
            }
            ExitCode::SUCCESS
        }
        Some("disk-service") => {
            let mut engine = SimulationEngine::new(disk_service::build());
            while engine.simulation_mut().step().is_some() {}
            println!("job | arrived | granted | latency");
            for (id, latency) in disk_service::latencies(&engine) {
                println!(
                    "Job{} | {} | {} | {}",
                    id.0,
                    read_state(&engine, id, "arrived_at").unwrap_or_default(),
                    read_state(&engine, id, "granted_at").unwrap_or_default(),
                    latency
                );
            }
            ExitCode::SUCCESS
        }
        Some("full-adder") => {
            println!(" A B Cin | Sum Carry | expected | status");
            let mut ok = true;
            for i in 0..8u8 {
                let (a, b, cin) = ((i >> 2) & 1, (i >> 1) & 1, i & 1);
                let engine = digital::run_full_adder(a, b, cin, tw_core::TracingMode::Abstract);
                let got = match digital::full_adder_result(&engine) {
                    Some(v) => v,
                    None => {
                        eprintln!("error: no output for A={a} B={b} Cin={cin}");
                        return ExitCode::FAILURE;
                    }
                };
                let expected = digital::expected_full_adder(a, b, cin);
                let pass = got == expected;
                ok &= pass;
                println!(
                    " {a}  {b}  {cin}  |   {}     {}    |    {} {}    | {}",
                    got.0,
                    got.1,
                    expected.0,
                    expected.1,
                    if pass { "OK" } else { "FAIL" }
                );
            }
            if ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        other => {
            eprintln!("error: unknown scenario {other:?}\n");
            eprint!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}

fn inspect(scenario: Option<&str>) -> ExitCode {
    match scenario {
        Some("counter") => {
            let mut engine = SimulationEngine::new(counter::build(5, 5));
            engine.run_ticks(100);
            println!("components: {}", engine.simulation().component_count());
            println!(
                "counter.count = {}",
                read_state(&engine, counter::COUNTER_ID, "count").unwrap_or_default()
            );
            ExitCode::SUCCESS
        }
        Some("resource-contention") => {
            let mut engine = SimulationEngine::new(resource_contention::build());
            while engine.simulation_mut().step().is_some() {}
            println!("components: {}", engine.simulation().component_count());
            for (label, id) in [
                ("Worker1", resource_contention::WORKER1_ID),
                ("Worker2", resource_contention::WORKER2_ID),
                ("Worker3", resource_contention::WORKER3_ID),
            ] {
                println!(
                    "{label}.status = {}",
                    read_state(&engine, id, "status").unwrap_or_default()
                );
            }
            ExitCode::SUCCESS
        }
        Some("disk-service") => {
            let mut engine = SimulationEngine::new(disk_service::build());
            while engine.simulation_mut().step().is_some() {}
            println!("components: {}", engine.simulation().component_count());
            for (label, id) in [
                ("Job1", disk_service::JOB1_ID),
                ("Job2", disk_service::JOB2_ID),
                ("Job3", disk_service::JOB3_ID),
            ] {
                println!(
                    "{label}.status = {}",
                    read_state(&engine, id, "status").unwrap_or_default()
                );
            }
            ExitCode::SUCCESS
        }
        Some("full-adder") => {
            let engine = digital::run_full_adder(1, 1, 1, tw_core::TracingMode::Abstract);
            println!("components: {}", engine.simulation().component_count());
            println!("probe.sum   = {}", read_state(&engine, digital::PROBE_ID, "sum").unwrap_or_default());
            println!("probe.carry = {}", read_state(&engine, digital::PROBE_ID, "carry").unwrap_or_default());
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("error: unknown scenario {other:?}\n");
            eprint!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}
