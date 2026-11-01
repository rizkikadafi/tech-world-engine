use tw_electrical::prelude::*;

#[test]
fn sr_latch_bistable_memory_retention() {
    // Cross-coupled CMOS NOR SR Latch:
    // NOR 1: Inputs (R, Q_bar) -> Output Q
    // NOR 2: Inputs (S, Q)     -> Output Q_bar
    //
    // Sequence of operations:
    // 1. SET (S = 5V, R = 0V)       => Q charges to 5V, Q_bar to 0V.
    // 2. HOLD (S = 0V, R = 0V)      => Q remains 5V (retained memory!), Q_bar remains 0V.
    // 3. RESET (S = 0V, R = 5V)     => Q drops to 0V, Q_bar charges to 5V.
    // 4. HOLD (S = 0V, R = 0V)      => Q remains 0V (retained memory!), Q_bar remains 5V.

    let build_latch_circuit = |vs: f64, vr: f64| -> (Circuit, NodeId, NodeId) {
        let mut ckt = Circuit::new();
        let n_vdd = ckt.get_or_add_node("vdd");
        let n_s = ckt.get_or_add_node("s");
        let n_r = ckt.get_or_add_node("r");
        let n_q = ckt.get_or_add_node("q");
        let n_q_bar = ckt.get_or_add_node("q_bar");

        ckt.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, 5.0);
        ckt.add_voltage_source("Vs", n_s, NodeId::GROUND, vs);
        ckt.add_voltage_source("Vr", n_r, NodeId::GROUND, vr);

        // NOR 1: inputs (R, Q_bar) -> Q
        ckt.add_cmos_nor("NOR_Q", n_r, n_q_bar, n_q, n_vdd, NodeId::GROUND)
            .expect("nor Q instantiated");

        // NOR 2: inputs (S, Q) -> Q_bar
        ckt.add_cmos_nor("NOR_QBAR", n_s, n_q, n_q_bar, n_vdd, NodeId::GROUND)
            .expect("nor Q_bar instantiated");

        // Parasitic / load capacitance on storage nodes Q and Q_bar (1 pF)
        ckt.add_capacitor("C_q", n_q, NodeId::GROUND, 1e-12);
        ckt.add_capacitor("C_qbar", n_q_bar, NodeId::GROUND, 1e-12);

        (ckt, n_q, n_q_bar)
    };

    let dt = 1e-10; // 0.1 ns step
    let t_duration = 5e-9; // 5 ns per phase

    // --- Phase 1: SET (S = 5V, R = 0V) ---
    let (ckt_set, q_node, qbar_node) = build_latch_circuit(5.0, 0.0);
    let res_set = transient(&ckt_set, t_duration, dt, None).expect("SET phase succeeded");
    let state_after_set = res_set.final_state().unwrap();

    let v_q_set = state_after_set.voltage(q_node);
    let v_qbar_set = state_after_set.voltage(qbar_node);

    assert!(
        v_q_set > 4.5,
        "After SET: Q should be HIGH (> 4.5V), got {:.3}V",
        v_q_set
    );
    assert!(
        v_qbar_set < 0.5,
        "After SET: Q_bar should be LOW (< 0.5V), got {:.3}V",
        v_qbar_set
    );

    // --- Phase 2: HOLD after SET (S = 0V, R = 0V) ---
    let (ckt_hold1, q_node, qbar_node) = build_latch_circuit(0.0, 0.0);
    let res_hold1 = transient(&ckt_hold1, t_duration, dt, Some(state_after_set.clone()))
        .expect("HOLD 1 phase succeeded");
    let state_after_hold1 = res_hold1.final_state().unwrap();

    let v_q_hold1 = state_after_hold1.voltage(q_node);
    let v_qbar_hold1 = state_after_hold1.voltage(qbar_node);

    assert!(
        v_q_hold1 > 4.5,
        "HOLD after SET: Q must remain stored as HIGH (> 4.5V), got {:.3}V",
        v_q_hold1
    );
    assert!(
        v_qbar_hold1 < 0.5,
        "HOLD after SET: Q_bar must remain stored as LOW (< 0.5V), got {:.3}V",
        v_qbar_hold1
    );

    // --- Phase 3: RESET (S = 0V, R = 5V) ---
    let (ckt_reset, q_node, qbar_node) = build_latch_circuit(0.0, 5.0);
    let res_reset = transient(&ckt_reset, t_duration, dt, Some(state_after_hold1.clone()))
        .expect("RESET phase succeeded");
    let state_after_reset = res_reset.final_state().unwrap();

    let v_q_reset = state_after_reset.voltage(q_node);
    let v_qbar_reset = state_after_reset.voltage(qbar_node);

    assert!(
        v_q_reset < 0.5,
        "After RESET: Q should be LOW (< 0.5V), got {:.3}V",
        v_q_reset
    );
    assert!(
        v_qbar_reset > 4.5,
        "After RESET: Q_bar should be HIGH (> 4.5V), got {:.3}V",
        v_qbar_reset
    );

    // --- Phase 4: HOLD after RESET (S = 0V, R = 0V) ---
    let (ckt_hold2, q_node, qbar_node) = build_latch_circuit(0.0, 0.0);
    let res_hold2 = transient(&ckt_hold2, t_duration, dt, Some(state_after_reset.clone()))
        .expect("HOLD 2 phase succeeded");
    let state_after_hold2 = res_hold2.final_state().unwrap();

    let v_q_hold2 = state_after_hold2.voltage(q_node);
    let v_qbar_hold2 = state_after_hold2.voltage(qbar_node);

    assert!(
        v_q_hold2 < 0.5,
        "HOLD after RESET: Q must remain stored as LOW (< 0.5V), got {:.3}V",
        v_q_hold2
    );
    assert!(
        v_qbar_hold2 > 4.5,
        "HOLD after RESET: Q_bar must remain stored as HIGH (> 4.5V), got {:.3}V",
        v_qbar_hold2
    );
}
