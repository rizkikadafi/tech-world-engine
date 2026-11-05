//! Tech World Engine CLI.
//!
//! Thin command-line interface: all simulation wiring lives in `tw-scene` and
//! `tw_sim::scenarios`, never here.

use std::path::Path;
use std::process::ExitCode;

use tw_core::ComponentId;
use tw_scene::{Registry, Session};
use tw_sim::scenarios::{counter, digital, disk_service, resource_contention};
use tw_sim::SimulationEngine;

const USAGE: &str = "\
Tech World Engine CLI

Usage:
  tw-cli run <file.twscene | scenario> [--ticks N]
  tw-cli inspect <file.twscene | scenario> [--component ID] [--ticks N]
  tw-cli trace <file.twscene> --component ID [--ticks N]
  tw-cli list-kinds
  tw-cli list
  tw-cli help

Builtin Scenarios:
  counter                Generic counter that ticks to a limit
  resource-contention    Three workers competing for a capacity-2 resource
  disk-service           Capacity-1 disk with service time (emergent latency)
  full-adder             Full adder (composite components) truth table
";

fn main() -> ExitCode {
    env_logger::init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => run(&args[1..]),
        Some("inspect") => inspect(&args[1..]),
        Some("trace") => trace(&args[1..]),
        Some("list-kinds") => list_kinds(),
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

fn parse_flag_u64(args: &[String], flag: &str) -> Option<u64> {
    for i in 0..args.len() {
        if args[i] == flag {
            return args.get(i + 1).and_then(|s| s.parse().ok());
        }
    }
    None
}

fn is_scene_file(target: &str) -> bool {
    target.ends_with(".twscene") || Path::new(target).exists()
}

fn list_kinds() -> ExitCode {
    let reg = Registry::builtin();
    println!("Available Component Kinds in Registry:");
    for kind in reg.kinds() {
        println!("  - {kind}");
    }
    ExitCode::SUCCESS
}

fn run(args: &[String]) -> ExitCode {
    let target = match args.first() {
        Some(t) => t.as_str(),
        None => {
            eprintln!("error: missing scenario or scene file path\n");
            eprint!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };

    if is_scene_file(target) {
        let ticks = parse_flag_u64(args, "--ticks").unwrap_or(100);
        let mut session = match Session::load(target) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error loading scene '{target}': {e}");
                return ExitCode::FAILURE;
            }
        };

        println!("Running scene '{}' for {} ticks...", session.world().name, ticks);
        let report = session.step(ticks);
        println!(
            "Finished at tick {}, executed {} events.",
            report.tick, report.events_executed
        );

        println!("\nFinal Component States:");
        for id in session.component_ids() {
            let name = session.component_name(id).unwrap_or("Component");
            println!("  #{}: '{}'", id.0, name);
            if let Some(state) = session.component_state(id) {
                for (k, v) in state.iter() {
                    println!("    {} = {}", k, v);
                }
            }
        }
        return ExitCode::SUCCESS;
    }

    match target {
        "counter" => {
            let mut engine = build_engine(counter::build(5, 5));
            engine.run_ticks(100);
            println!(
                "counter: final count = {}",
                read_state(&engine, counter::COUNTER_ID, "count").unwrap_or_default()
            );
            ExitCode::SUCCESS
        }
        "resource-contention" => {
            let mut engine = build_engine(resource_contention::build());
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
        "disk-service" => {
            let mut engine = build_engine(disk_service::build());
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
        "full-adder" => {
            println!(" A B Cin | Sum Carry | expected | status");
            let mut ok = true;
            for i in 0..8u8 {
                let (a, b, cin) = ((i >> 2) & 1, (i >> 1) & 1, i & 1);
                let mut engine = digital::run_full_adder(a, b, cin, tw_core::TracingMode::Abstract);
                apply_env_config(&mut engine);
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
            eprintln!("error: unknown scenario '{other}'\n");
            eprint!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}

fn inspect(args: &[String]) -> ExitCode {
    let target = match args.first() {
        Some(t) => t.as_str(),
        None => {
            eprintln!("error: missing scenario or scene file path\n");
            eprint!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };

    if is_scene_file(target) {
        let ticks = parse_flag_u64(args, "--ticks").unwrap_or(0);
        let mut session = match Session::load(target) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error loading scene '{target}': {e}");
                return ExitCode::FAILURE;
            }
        };

        if ticks > 0 {
            session.step(ticks);
        }

        let target_comp = parse_flag_u64(args, "--component").map(ComponentId);

        println!("Scene: '{}' (at tick {})", session.world().name, session.current_tick());
        println!("Components ({}):", session.component_ids().len());

        for id in session.component_ids() {
            if let Some(filter) = target_comp {
                if id != filter {
                    continue;
                }
            }

            let name = session.component_name(id).unwrap_or("Component");
            println!("\n  Component #{}: '{}'", id.0, name);

            if let Some(ports) = session.component_ports(id) {
                print!("    Ports: ");
                if ports.is_empty() {
                    println!("(none)");
                } else {
                    let port_descs: Vec<String> = ports
                        .iter()
                        .map(|p| format!("{}:{:?}", p.name, p.direction))
                        .collect();
                    println!("{}", port_descs.join(", "));
                }
            }

            if let Some(state) = session.component_state(id) {
                println!("    State Variables:");
                for (k, v) in state.iter() {
                    println!("      {} = {}", k, v);
                }
            }
        }
        return ExitCode::SUCCESS;
    }

    match target {
        "counter" => {
            let mut engine = build_engine(counter::build(5, 5));
            engine.run_ticks(100);
            println!("components: {}", engine.simulation().component_count());
            println!(
                "counter.count = {}",
                read_state(&engine, counter::COUNTER_ID, "count").unwrap_or_default()
            );
            ExitCode::SUCCESS
        }
        "resource-contention" => {
            let mut engine = build_engine(resource_contention::build());
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
        "disk-service" => {
            let mut engine = build_engine(disk_service::build());
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
        "full-adder" => {
            let mut engine = digital::run_full_adder(1, 1, 1, tw_core::TracingMode::Abstract);
            apply_env_config(&mut engine);
            println!("components: {}", engine.simulation().component_count());
            println!(
                "probe.sum   = {}",
                read_state(&engine, digital::PROBE_ID, "sum").unwrap_or_default()
            );
            println!(
                "probe.carry = {}",
                read_state(&engine, digital::PROBE_ID, "carry").unwrap_or_default()
            );
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("error: unknown scenario '{other}'\n");
            eprint!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}

fn trace(args: &[String]) -> ExitCode {
    let target = match args.first() {
        Some(t) => t.as_str(),
        None => {
            eprintln!("error: missing scene file path for trace\n");
            eprint!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };

    let comp_id = match parse_flag_u64(args, "--component") {
        Some(id) => ComponentId(id),
        None => {
            eprintln!("error: --component <ID> is required for trace\n");
            return ExitCode::FAILURE;
        }
    };

    let ticks = parse_flag_u64(args, "--ticks").unwrap_or(100);
    let mut session = match Session::load(target) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error loading scene '{target}': {e}");
            return ExitCode::FAILURE;
        }
    };

    session.step(ticks);

    let traces = session.traces_for(comp_id);
    if traces.is_empty() {
        println!("No traces recorded for component #{}.", comp_id.0);
        return ExitCode::SUCCESS;
    }

    println!(
        "Component #{} has {} trace events. Most recent causal chain:",
        comp_id.0,
        traces.len()
    );

    let last_trace = session.last_trace_for(comp_id).unwrap();
    let explanation = session.explain(last_trace);

    for (step_idx, node) in explanation.iter().enumerate() {
        println!(
            "  [{}] t={} Trace #{} (Event #{}): {}",
            step_idx + 1,
            node.time.ticks(),
            node.id.0,
            node.event_id.0,
            node.description
        );
    }

    ExitCode::SUCCESS
}

fn read_state(engine: &SimulationEngine, id: ComponentId, key: &str) -> Option<String> {
    engine
        .simulation()
        .component_state(id)?
        .get(key)
        .map(|v| v.to_string())
}

fn build_engine(simulation: tw_sim::Simulation) -> SimulationEngine {
    let mut engine = SimulationEngine::new(simulation);
    apply_env_config(&mut engine);
    engine
}

fn apply_env_config(engine: &mut SimulationEngine) {
    let mut config = engine.simulation().config();
    if let Some(n) = env_u64("TW_MAX_EVENTS") {
        config.max_events = Some(n);
    }
    if let Some(n) = env_u64("TW_SEED") {
        config.seed = n;
    }
    engine.simulation_mut().configure(config);
}

fn env_u64(key: &str) -> Option<u64> {
    std::env::var(key).ok()?.trim().parse().ok()
}
