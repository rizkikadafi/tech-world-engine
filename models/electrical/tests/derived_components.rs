use tw_core::{
    CausalGraph, Component, Event, EventPayload, PortId, SimulationContext, SimulationTime,
    StateValue, TracingMode,
};
use tw_electrical::prelude::*;

fn measured_inverter() -> GateCharacterization {
    Characterizer::with_defaults()
        .inverter()
        .expect("inverter characterized")
}

fn make_gate(measured: &GateCharacterization) -> DerivedGate {
    DerivedGate::from_characterization(
        tw_core::ComponentId(42),
        "DerivedInverter",
        measured,
        TimeBase::default(),
    )
    .expect("derived gate")
}

fn context<'a>(graph: &'a mut CausalGraph, time: u64) -> SimulationContext<'a> {
    SimulationContext::new(SimulationTime::new(time), graph, TracingMode::Off)
}

#[test]
fn time_base_quantizes_to_causal_ticks() {
    let time_base = TimeBase::default();
    assert_eq!(time_base.delay_ticks(0.0), 1);
    assert_eq!(time_base.delay_ticks(-1.0), 1);
    assert_eq!(time_base.delay_ticks(0.1e-9), 1);
    assert_eq!(time_base.delay_ticks(1.0e-9), 1);
    assert_eq!(time_base.delay_ticks(1.1e-9), 2);
    assert!((time_base.ticks_to_seconds(3) - 3e-9).abs() < 1e-21);
    assert!(TimeBase::new(0.0).validate().is_err());
}

#[test]
fn derived_inverter_uses_measured_truth_table_and_timing() {
    let measured = measured_inverter();
    let gate = make_gate(&measured);

    assert_eq!(gate.truth_table(), &measured.truth_table);
    assert!(gate.output(), "input low must produce output high");
    assert_eq!(gate.input_port(0), Some(PortId(1)));
    assert_eq!(gate.output_port(), PortId(2));

    let expected_phl = TimeBase::default().delay_ticks(measured.timing.t_pHL);
    let expected_plh = TimeBase::default().delay_ticks(measured.timing.t_pLH);
    assert_eq!(gate.timing_ticks().t_pHL, expected_phl);
    assert_eq!(gate.timing_ticks().t_pLH, expected_plh);

    let state = gate.state();
    assert_eq!(state.get_int("t_pHL_ticks"), Some(expected_phl as i64));
    assert_eq!(state.get_int("t_pLH_ticks"), Some(expected_plh as i64));
    assert!(state.get_float("t_pHL_seconds").unwrap() > 0.0);
    assert!(state.get_float("energy_per_cycle_joules").unwrap() > 0.0);
}

#[test]
fn derived_gate_delay_matches_measured_ticks() {
    let measured = measured_inverter();
    let mut gate = make_gate(&measured);
    let input_port = gate.input_port(0).unwrap();

    let mut graph = CausalGraph::new();
    let mut ctx = context(&mut graph, 10);
    let input_event = Event::at(
        SimulationTime::new(10),
        tw_core::ComponentId(1),
        gate.id(),
        EventPayload::PortSignal {
            port_id: input_port,
            value: StateValue::Bool(true),
        },
    );

    let scheduled = gate.handle_event(&input_event, &mut ctx);
    assert_eq!(scheduled.len(), 1);
    let expected_delay = gate.timing_ticks().t_pHL;
    assert_eq!(scheduled[0].time.ticks(), 10 + expected_delay);
    assert!(matches!(scheduled[0].payload, EventPayload::Custom(_)));
    assert!(gate.output(), "output must not change before delay");

    let mut commit_ctx = context(&mut graph, scheduled[0].time.ticks());
    let outputs = gate.handle_event(&scheduled[0].clone(), &mut commit_ctx);
    assert_eq!(outputs.len(), 1);
    match &outputs[0].payload {
        EventPayload::PortSignal { port_id, value } => {
            assert_eq!(*port_id, gate.output_port());
            assert_eq!(value.as_bool(), Some(false));
        }
        other => panic!("expected PortSignal, got {other:?}"),
    }
    assert!(!gate.output());
    assert_eq!(gate.state().get_bool("output"), Some(false));
}

#[test]
fn derived_gate_cancels_stale_output_event() {
    let measured = measured_inverter();
    let mut gate = make_gate(&measured);
    let input_port = gate.input_port(0).unwrap();

    let mut graph = CausalGraph::new();
    let mut ctx = context(&mut graph, 0);
    let rising_input = Event::at(
        SimulationTime::new(0),
        tw_core::ComponentId(1),
        gate.id(),
        EventPayload::PortSignal {
            port_id: input_port,
            value: StateValue::Bool(true),
        },
    );
    let pending = gate.handle_event(&rising_input, &mut ctx);
    assert_eq!(pending.len(), 1);

    // Input returns low before the pending falling edge commits.
    let mut second_ctx = context(&mut graph, 1);
    let falling_input = Event::at(
        SimulationTime::new(1),
        tw_core::ComponentId(1),
        gate.id(),
        EventPayload::PortSignal {
            port_id: input_port,
            value: StateValue::Bool(false),
        },
    );
    assert!(gate
        .handle_event(&falling_input, &mut second_ctx)
        .is_empty());

    // The stale commit event must be ignored.
    let mut stale_ctx = context(&mut graph, pending[0].time.ticks());
    assert!(gate
        .handle_event(&pending[0].clone(), &mut stale_ctx)
        .is_empty());
    assert!(gate.output());
    assert_eq!(gate.state().get_bool("pending"), Some(false));
}

#[test]
fn derived_gate_emits_initial_output_on_mount() {
    let measured = measured_inverter();
    let mut gate = make_gate(&measured);

    let mut graph = CausalGraph::new();
    let mut ctx = context(&mut graph, 7);
    let mount_events = gate.on_mount(&mut ctx);
    assert_eq!(mount_events.len(), 1);
    assert_eq!(mount_events[0].time.ticks(), 7);
    assert!(matches!(mount_events[0].payload, EventPayload::Custom(_)));

    let mut emit_ctx = context(&mut graph, 7);
    let events = gate.handle_event(&mount_events[0].clone(), &mut emit_ctx);
    assert_eq!(events.len(), 1);
    match &events[0].payload {
        EventPayload::PortSignal { port_id, value } => {
            assert_eq!(*port_id, gate.output_port());
            assert_eq!(value.as_bool(), Some(true));
        }
        other => panic!("expected PortSignal, got {other:?}"),
    }
}

#[test]
fn derived_gate_rejects_unsupported_characterization() {
    let measured = measured_inverter();
    let mut wrong = measured.clone();
    wrong.truth_table.outputs = vec![true];

    let error = DerivedGate::from_characterization(
        tw_core::ComponentId(1),
        "bad",
        &wrong,
        TimeBase::default(),
    )
    .unwrap_err();
    assert!(matches!(error, DerivedError::InvalidTruthTable { .. }));
}

#[test]
fn derived_nand2_uses_measured_truth_table_and_ports() {
    let measured = Characterizer::with_defaults()
        .nand2()
        .expect("nand2 characterized");
    let gate = DerivedGate::from_characterization(
        tw_core::ComponentId(43),
        "DerivedNand2",
        &measured,
        TimeBase::default(),
    )
    .expect("derived nand2");

    assert_eq!(gate.truth_table(), &measured.truth_table);
    assert_eq!(gate.truth_table().outputs, vec![true, true, true, false]);
    assert!(gate.output(), "inputs 00 must produce output high");
    assert_eq!(gate.input_port(0), Some(PortId(1)));
    assert_eq!(gate.input_port(1), Some(PortId(2)));
    assert_eq!(gate.output_port(), PortId(3));

    let expected_phl = TimeBase::default().delay_ticks(measured.timing.t_pHL);
    let expected_plh = TimeBase::default().delay_ticks(measured.timing.t_pLH);
    assert_eq!(gate.timing_ticks().t_pHL, expected_phl);
    assert_eq!(gate.timing_ticks().t_pLH, expected_plh);
}

#[test]
fn derived_nand2_only_falls_when_both_inputs_are_high() {
    let measured = Characterizer::with_defaults()
        .nand2()
        .expect("nand2 characterized");
    let mut gate = DerivedGate::from_characterization(
        tw_core::ComponentId(44),
        "DerivedNand2",
        &measured,
        TimeBase::default(),
    )
    .expect("derived nand2");

    let input_a = gate.input_port(0).unwrap();
    let input_b = gate.input_port(1).unwrap();
    let mut graph = CausalGraph::new();

    // A rises while B is low: NAND output must stay high, no event.
    let mut ctx = context(&mut graph, 0);
    let a_high = Event::at(
        SimulationTime::new(0),
        tw_core::ComponentId(1),
        gate.id(),
        EventPayload::PortSignal {
            port_id: input_a,
            value: StateValue::Bool(true),
        },
    );
    assert!(gate.handle_event(&a_high, &mut ctx).is_empty());
    assert!(gate.output());

    // B rises: both inputs high, output falls after t_pHL.
    let mut ctx = context(&mut graph, 5);
    let b_high = Event::at(
        SimulationTime::new(5),
        tw_core::ComponentId(1),
        gate.id(),
        EventPayload::PortSignal {
            port_id: input_b,
            value: StateValue::Bool(true),
        },
    );
    let scheduled = gate.handle_event(&b_high, &mut ctx);
    assert_eq!(scheduled.len(), 1);
    let expected_fall = 5 + gate.timing_ticks().t_pHL;
    assert_eq!(scheduled[0].time.ticks(), expected_fall);
    assert!(gate.output(), "output must not change before delay");

    let mut commit_ctx = context(&mut graph, expected_fall);
    let outputs = gate.handle_event(&scheduled[0].clone(), &mut commit_ctx);
    assert_eq!(outputs.len(), 1);
    match &outputs[0].payload {
        EventPayload::PortSignal { port_id, value } => {
            assert_eq!(*port_id, gate.output_port());
            assert_eq!(value.as_bool(), Some(false));
        }
        other => panic!("expected PortSignal, got {other:?}"),
    }
    assert!(!gate.output());

    // A falls: output rises after t_pLH even though B is still high.
    let mut ctx = context(&mut graph, expected_fall + 1);
    let a_low = Event::at(
        SimulationTime::new(expected_fall + 1),
        tw_core::ComponentId(1),
        gate.id(),
        EventPayload::PortSignal {
            port_id: input_a,
            value: StateValue::Bool(false),
        },
    );
    let scheduled = gate.handle_event(&a_low, &mut ctx);
    assert_eq!(scheduled.len(), 1);
    assert_eq!(
        scheduled[0].time.ticks(),
        expected_fall + 1 + gate.timing_ticks().t_pLH
    );

    let mut commit_ctx = context(&mut graph, scheduled[0].time.ticks());
    let outputs = gate.handle_event(&scheduled[0].clone(), &mut commit_ctx);
    assert_eq!(outputs.len(), 1);
    assert_eq!(
        outputs[0].payload,
        EventPayload::PortSignal {
            port_id: gate.output_port(),
            value: StateValue::Bool(true),
        }
    );
    assert!(gate.output());
}

use std::sync::OnceLock;

fn measured_dff() -> &'static GateCharacterization {
    static MEASURED: OnceLock<GateCharacterization> = OnceLock::new();
    MEASURED.get_or_init(|| {
        Characterizer::with_defaults()
            .dff()
            .expect("dff characterized")
    })
}

fn make_dff() -> DerivedDff {
    DerivedDff::from_characterization(
        tw_core::ComponentId(45),
        "DerivedDff",
        measured_dff(),
        TimeBase::default(),
    )
    .expect("derived dff")
}

#[test]
fn derived_dff_exposes_measured_metrics_and_ports() {
    let measured = measured_dff();
    let dff = make_dff();

    assert_eq!(dff.truth_table(), &measured.truth_table);
    assert_eq!(dff.truth_table().outputs, vec![false, false, true, true]);
    assert!(!dff.output(), "DFF must start with Q low");
    assert_eq!(dff.d_port(), PortId(1));
    assert_eq!(dff.clk_port(), PortId(2));
    assert_eq!(dff.q_port(), PortId(3));

    let expected_phl = TimeBase::default().delay_ticks(measured.timing.t_pHL);
    let expected_plh = TimeBase::default().delay_ticks(measured.timing.t_pLH);
    assert_eq!(dff.timing_ticks().t_pHL, expected_phl);
    assert_eq!(dff.timing_ticks().t_pLH, expected_plh);
    assert!(dff.state().get_float("energy_per_cycle_joules").unwrap() > 0.0);
}

#[test]
fn derived_dff_captures_data_only_on_rising_clock_edge() {
    let mut dff = make_dff();
    let d_port = dff.d_port();
    let clk_port = dff.clk_port();
    let q_port = dff.q_port();
    let mut graph = CausalGraph::new();

    // D rises while CLK is low: no output event yet.
    let mut ctx = context(&mut graph, 0);
    let d_high = Event::at(
        SimulationTime::new(0),
        tw_core::ComponentId(1),
        dff.id(),
        EventPayload::PortSignal {
            port_id: d_port,
            value: StateValue::Bool(true),
        },
    );
    assert!(dff.handle_event(&d_high, &mut ctx).is_empty());
    assert!(!dff.output());

    // Rising clock edge schedules Q high after t_pLH.
    let mut ctx = context(&mut graph, 1);
    let clk_high = Event::at(
        SimulationTime::new(1),
        tw_core::ComponentId(1),
        dff.id(),
        EventPayload::PortSignal {
            port_id: clk_port,
            value: StateValue::Bool(true),
        },
    );
    let scheduled = dff.handle_event(&clk_high, &mut ctx);
    assert_eq!(scheduled.len(), 1);
    let expected_rise = 1 + dff.timing_ticks().t_pLH;
    assert_eq!(scheduled[0].time.ticks(), expected_rise);
    assert!(!dff.output(), "Q must not change before the delay");

    let mut commit_ctx = context(&mut graph, expected_rise);
    let outputs = dff.handle_event(&scheduled[0].clone(), &mut commit_ctx);
    assert_eq!(outputs.len(), 1);
    assert_eq!(
        outputs[0].payload,
        EventPayload::PortSignal {
            port_id: q_port,
            value: StateValue::Bool(true),
        }
    );
    assert!(dff.output());

    // D falls while CLK is high: master is disabled, Q must hold.
    let mut ctx = context(&mut graph, expected_rise + 1);
    let d_low = Event::at(
        SimulationTime::new(expected_rise + 1),
        tw_core::ComponentId(1),
        dff.id(),
        EventPayload::PortSignal {
            port_id: d_port,
            value: StateValue::Bool(false),
        },
    );
    assert!(dff.handle_event(&d_low, &mut ctx).is_empty());
    assert!(dff.output());

    // Falling clock edge does not change Q.
    let mut ctx = context(&mut graph, expected_rise + 2);
    let clk_low = Event::at(
        SimulationTime::new(expected_rise + 2),
        tw_core::ComponentId(1),
        dff.id(),
        EventPayload::PortSignal {
            port_id: clk_port,
            value: StateValue::Bool(false),
        },
    );
    assert!(dff.handle_event(&clk_low, &mut ctx).is_empty());
    assert!(dff.output());

    // Next rising edge captures D=0, so Q falls after t_pHL.
    let mut ctx = context(&mut graph, expected_rise + 3);
    let clk_high_again = Event::at(
        SimulationTime::new(expected_rise + 3),
        tw_core::ComponentId(1),
        dff.id(),
        EventPayload::PortSignal {
            port_id: clk_port,
            value: StateValue::Bool(true),
        },
    );
    let scheduled = dff.handle_event(&clk_high_again, &mut ctx);
    assert_eq!(scheduled.len(), 1);
    let expected_fall = expected_rise + 3 + dff.timing_ticks().t_pHL;
    assert_eq!(scheduled[0].time.ticks(), expected_fall);

    let mut commit_ctx = context(&mut graph, expected_fall);
    let outputs = dff.handle_event(&scheduled[0].clone(), &mut commit_ctx);
    assert_eq!(outputs.len(), 1);
    assert_eq!(
        outputs[0].payload,
        EventPayload::PortSignal {
            port_id: q_port,
            value: StateValue::Bool(false),
        }
    );
    assert!(!dff.output());
}

#[test]
fn derived_dff_cancels_superseded_clock_edge() {
    let mut dff = make_dff();
    let d_port = dff.d_port();
    let clk_port = dff.clk_port();
    let dff_id = dff.id();
    let mut graph = CausalGraph::new();

    let event = |time: u64, port: PortId, value: bool| {
        Event::at(
            SimulationTime::new(time),
            tw_core::ComponentId(1),
            dff_id,
            EventPayload::PortSignal {
                port_id: port,
                value: StateValue::Bool(value),
            },
        )
    };

    // D=1, first rising edge schedules Q high.
    let mut ctx = context(&mut graph, 0);
    assert!(dff
        .handle_event(&event(0, d_port, true), &mut ctx)
        .is_empty());
    let mut ctx = context(&mut graph, 1);
    let pending = dff.handle_event(&event(1, clk_port, true), &mut ctx);
    assert_eq!(pending.len(), 1);

    // Clock falls, D returns low, then a new rising edge desires Q low,
    // which is already the committed value: the pending edge is cancelled.
    let mut ctx = context(&mut graph, 2);
    assert!(dff
        .handle_event(&event(2, clk_port, false), &mut ctx)
        .is_empty());
    let mut ctx = context(&mut graph, 3);
    assert!(dff
        .handle_event(&event(3, d_port, false), &mut ctx)
        .is_empty());
    let mut ctx = context(&mut graph, 4);
    assert!(dff
        .handle_event(&event(4, clk_port, true), &mut ctx)
        .is_empty());

    // The stale commit event must be ignored.
    let mut ctx = context(&mut graph, pending[0].time.ticks());
    assert!(dff.handle_event(&pending[0].clone(), &mut ctx).is_empty());
    assert!(!dff.output());
    assert_eq!(dff.state().get_bool("pending"), Some(false));
}

#[test]
fn derived_dff_rejects_non_dff_characterization() {
    let measured = measured_inverter();
    let error = DerivedDff::from_characterization(
        tw_core::ComponentId(1),
        "bad",
        &measured,
        TimeBase::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        DerivedError::UnsupportedKind(GateKind::Inverter)
    ));
}
