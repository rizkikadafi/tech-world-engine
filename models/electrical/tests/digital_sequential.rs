use tw_electrical::prelude::*;

const DT: f64 = 1e-10; // 0.1 ns
const PHASE: f64 = 5e-9; // 5 ns

fn node(ckt: &Circuit, name: &str) -> NodeId {
    ckt.node_by_name(name)
        .unwrap_or_else(|| panic!("node '{name}' not found"))
}

fn build_d_latch(vd: f64, ven: f64) -> Circuit {
    let config = CmosGateConfig::default();
    let mut ckt = Circuit::new();
    let n_vdd = ckt.get_or_add_node("vdd");
    let n_d = ckt.get_or_add_node("d");
    let n_en = ckt.get_or_add_node("en");
    let n_q = ckt.get_or_add_node("q");
    let n_q_bar = ckt.get_or_add_node("q_bar");

    ckt.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, 5.0);
    ckt.add_voltage_source("Vd", n_d, NodeId::GROUND, vd);
    ckt.add_voltage_source("Ven", n_en, NodeId::GROUND, ven);

    cmos_d_latch(config)
        .instantiate(
            &mut ckt,
            "LATCH1",
            &[
                ("D", n_d),
                ("EN", n_en),
                ("Q", n_q),
                ("Q_BAR", n_q_bar),
                ("VDD", n_vdd),
                ("GND", NodeId::GROUND),
            ],
        )
        .expect("d latch instantiated");

    ckt
}

fn build_dff(vd: f64, vclk: f64) -> Circuit {
    let config = CmosGateConfig::default();
    let mut ckt = Circuit::new();
    let n_vdd = ckt.get_or_add_node("vdd");
    let n_d = ckt.get_or_add_node("d");
    let n_clk = ckt.get_or_add_node("clk");
    let n_q = ckt.get_or_add_node("q");
    let n_q_bar = ckt.get_or_add_node("q_bar");

    ckt.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, 5.0);
    ckt.add_voltage_source("Vd", n_d, NodeId::GROUND, vd);
    ckt.add_voltage_source("Vclk", n_clk, NodeId::GROUND, vclk);

    cmos_dff(config)
        .instantiate(
            &mut ckt,
            "DFF1",
            &[
                ("D", n_d),
                ("CLK", n_clk),
                ("Q", n_q),
                ("Q_BAR", n_q_bar),
                ("VDD", n_vdd),
                ("GND", NodeId::GROUND),
            ],
        )
        .expect("d flip-flop instantiated");

    ckt
}

fn run_phase(ckt: &Circuit, previous: Option<&CircuitState>) -> CircuitState {
    transient(ckt, PHASE, DT, previous.cloned())
        .expect("transient phase converged")
        .final_state()
        .expect("transient produced a final state")
        .clone()
}

#[test]
fn d_latch_transparent_and_hold() {
    // Transparent with EN=1: Q follows D=0.
    let ckt = build_d_latch(0.0, 5.0);
    let state = run_phase(&ckt, None);
    assert!(
        state.voltage(node(&ckt, "q")) < 0.5,
        "transparent latch should drive Q low, got {:.3}V",
        state.voltage(node(&ckt, "q"))
    );

    // Transparent with EN=1: Q follows D=1.
    let ckt = build_d_latch(5.0, 5.0);
    let state = run_phase(&ckt, Some(&state));
    assert!(
        state.voltage(node(&ckt, "q")) > 4.5,
        "transparent latch should drive Q high, got {:.3}V",
        state.voltage(node(&ckt, "q"))
    );

    // Hold with EN=0: D falls, Q must stay high.
    let ckt = build_d_latch(0.0, 0.0);
    let state = run_phase(&ckt, Some(&state));
    assert!(
        state.voltage(node(&ckt, "q")) > 4.5,
        "latch should hold HIGH while EN=0, got {:.3}V",
        state.voltage(node(&ckt, "q"))
    );
    assert!(
        state.voltage(node(&ckt, "q_bar")) < 0.5,
        "Q_BAR should remain LOW while holding, got {:.3}V",
        state.voltage(node(&ckt, "q_bar"))
    );

    // Transparent with EN=1: Q follows D=0 again.
    let ckt = build_d_latch(0.0, 5.0);
    let state = run_phase(&ckt, Some(&state));
    assert!(
        state.voltage(node(&ckt, "q")) < 0.5,
        "transparent latch should drive Q low again, got {:.3}V",
        state.voltage(node(&ckt, "q"))
    );

    // Hold with EN=0: D rises, Q must stay low.
    let ckt = build_d_latch(5.0, 0.0);
    let state = run_phase(&ckt, Some(&state));
    assert!(
        state.voltage(node(&ckt, "q")) < 0.5,
        "latch should hold LOW while EN=0, got {:.3}V",
        state.voltage(node(&ckt, "q"))
    );
}

#[test]
fn dff_captures_on_rising_edge_and_holds() {
    // Power-up: CLK=0, D=0. The master latch captures 0. The slave latch is
    // disabled and may start in an undefined state, so we initialize it with
    // a rising edge before checking Q.
    let ckt = build_dff(0.0, 0.0);
    let state = run_phase(&ckt, None);

    // First rising edge: slave becomes transparent and adopts master Q=0.
    let ckt = build_dff(0.0, 5.0);
    let state = run_phase(&ckt, Some(&state));
    assert!(
        state.voltage(node(&ckt, "q")) < 0.5,
        "DFF should initialize Q low after the first clock edge, got {:.3}V",
        state.voltage(node(&ckt, "q"))
    );

    // Falling edge with D=1: master captures 1, slave holds 0.
    let ckt = build_dff(5.0, 0.0);
    let state = run_phase(&ckt, Some(&state));
    assert!(
        state.voltage(node(&ckt, "q")) < 0.5,
        "Q must not change before the clock edge, got {:.3}V",
        state.voltage(node(&ckt, "q"))
    );

    // Rising edge: slave propagates the captured 1.
    let ckt = build_dff(5.0, 5.0);
    let state = run_phase(&ckt, Some(&state));
    assert!(
        state.voltage(node(&ckt, "q")) > 4.5,
        "Q should become HIGH on the rising edge, got {:.3}V",
        state.voltage(node(&ckt, "q"))
    );
    assert!(
        state.voltage(node(&ckt, "q_bar")) < 0.5,
        "Q_BAR should become LOW on the rising edge, got {:.3}V",
        state.voltage(node(&ckt, "q_bar"))
    );

    // D falls while CLK is high: slave is transparent, Q should stay high.
    let ckt = build_dff(0.0, 5.0);
    let state = run_phase(&ckt, Some(&state));
    assert!(
        state.voltage(node(&ckt, "q")) > 4.5,
        "Q should hold HIGH while CLK is high, got {:.3}V",
        state.voltage(node(&ckt, "q"))
    );

    // Falling edge: master captures 0, slave holds the old 1.
    let ckt = build_dff(0.0, 0.0);
    let state = run_phase(&ckt, Some(&state));
    assert!(
        state.voltage(node(&ckt, "q")) > 4.5,
        "Q should hold HIGH after the falling edge, got {:.3}V",
        state.voltage(node(&ckt, "q"))
    );

    // Next rising edge: slave propagates the captured 0.
    let ckt = build_dff(0.0, 5.0);
    let state = run_phase(&ckt, Some(&state));
    assert!(
        state.voltage(node(&ckt, "q")) < 0.5,
        "Q should become LOW on the next rising edge, got {:.3}V",
        state.voltage(node(&ckt, "q"))
    );
    assert!(
        state.voltage(node(&ckt, "q_bar")) > 4.5,
        "Q_BAR should become HIGH on the next rising edge, got {:.3}V",
        state.voltage(node(&ckt, "q_bar"))
    );
}
