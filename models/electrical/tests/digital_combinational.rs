use tw_electrical::prelude::*;

fn logic_high(v: f64) -> bool {
    v > 4.5
}

fn logic_low(v: f64) -> bool {
    v < 0.5
}

fn solve_xor(va: f64, vb: f64) -> f64 {
    let config = CmosGateConfig::default();
    let mut ckt = Circuit::new();
    let n_vdd = ckt.get_or_add_node("vdd");
    let n_a = ckt.get_or_add_node("a");
    let n_b = ckt.get_or_add_node("b");
    let n_out = ckt.get_or_add_node("out");

    ckt.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, 5.0);
    ckt.add_voltage_source("Va", n_a, NodeId::GROUND, va);
    ckt.add_voltage_source("Vb", n_b, NodeId::GROUND, vb);

    cmos_xor(config)
        .instantiate(
            &mut ckt,
            "XOR1",
            &[
                ("A", n_a),
                ("B", n_b),
                ("OUT", n_out),
                ("VDD", n_vdd),
                ("GND", NodeId::GROUND),
            ],
        )
        .expect("xor instantiated");

    dc_op(&ckt).expect("xor dc solve").voltage(n_out)
}

fn solve_half_adder(va: f64, vb: f64) -> (f64, f64) {
    let config = CmosGateConfig::default();
    let mut ckt = Circuit::new();
    let n_vdd = ckt.get_or_add_node("vdd");
    let n_a = ckt.get_or_add_node("a");
    let n_b = ckt.get_or_add_node("b");
    let n_sum = ckt.get_or_add_node("sum");
    let n_carry = ckt.get_or_add_node("carry");

    ckt.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, 5.0);
    ckt.add_voltage_source("Va", n_a, NodeId::GROUND, va);
    ckt.add_voltage_source("Vb", n_b, NodeId::GROUND, vb);

    cmos_half_adder(config)
        .instantiate(
            &mut ckt,
            "HA1",
            &[
                ("A", n_a),
                ("B", n_b),
                ("SUM", n_sum),
                ("CARRY", n_carry),
                ("VDD", n_vdd),
                ("GND", NodeId::GROUND),
            ],
        )
        .expect("half adder instantiated");

    let state = dc_op(&ckt).expect("half adder dc solve");
    (state.voltage(n_sum), state.voltage(n_carry))
}

fn solve_full_adder(va: f64, vb: f64, vcin: f64) -> (f64, f64) {
    let config = CmosGateConfig::default();
    let mut ckt = Circuit::new();
    let n_vdd = ckt.get_or_add_node("vdd");
    let n_a = ckt.get_or_add_node("a");
    let n_b = ckt.get_or_add_node("b");
    let n_cin = ckt.get_or_add_node("cin");
    let n_sum = ckt.get_or_add_node("sum");
    let n_cout = ckt.get_or_add_node("cout");

    ckt.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, 5.0);
    ckt.add_voltage_source("Va", n_a, NodeId::GROUND, va);
    ckt.add_voltage_source("Vb", n_b, NodeId::GROUND, vb);
    ckt.add_voltage_source("Vcin", n_cin, NodeId::GROUND, vcin);

    cmos_full_adder(config)
        .instantiate(
            &mut ckt,
            "FA1",
            &[
                ("A", n_a),
                ("B", n_b),
                ("CIN", n_cin),
                ("SUM", n_sum),
                ("COUT", n_cout),
                ("VDD", n_vdd),
                ("GND", NodeId::GROUND),
            ],
        )
        .expect("full adder instantiated");

    let state = dc_op(&ckt).expect("full adder dc solve");
    (state.voltage(n_sum), state.voltage(n_cout))
}

#[test]
fn cmos_xor_truth_table() {
    let cases = [
        (0.0, 0.0, false),
        (0.0, 5.0, true),
        (5.0, 0.0, true),
        (5.0, 5.0, false),
    ];

    for (va, vb, expected_high) in cases {
        let v_out = solve_xor(va, vb);
        if expected_high {
            assert!(
                logic_high(v_out),
                "XOR({va}, {vb}) should be HIGH, got {v_out:.3}V"
            );
        } else {
            assert!(
                logic_low(v_out),
                "XOR({va}, {vb}) should be LOW, got {v_out:.3}V"
            );
        }
    }
}

#[test]
fn cmos_half_adder_truth_table() {
    let cases = [
        (0.0, 0.0, 0.0, 0.0),
        (0.0, 5.0, 5.0, 0.0),
        (5.0, 0.0, 5.0, 0.0),
        (5.0, 5.0, 0.0, 5.0),
    ];

    for (va, vb, expected_sum, expected_carry) in cases {
        let (v_sum, v_carry) = solve_half_adder(va, vb);
        assert!(
            (v_sum - expected_sum).abs() < 0.5,
            "Half adder SUM({va}, {vb}) = {v_sum:.3}V, expected {expected_sum:.1}V"
        );
        assert!(
            (v_carry - expected_carry).abs() < 0.5,
            "Half adder CARRY({va}, {vb}) = {v_carry:.3}V, expected {expected_carry:.1}V"
        );
    }
}

#[test]
fn cmos_full_adder_truth_table() {
    // (A, B, CIN) -> (SUM, COUT)
    let cases = [
        (0.0, 0.0, 0.0, 0.0, 0.0),
        (0.0, 0.0, 5.0, 5.0, 0.0),
        (0.0, 5.0, 0.0, 5.0, 0.0),
        (0.0, 5.0, 5.0, 0.0, 5.0),
        (5.0, 0.0, 0.0, 5.0, 0.0),
        (5.0, 0.0, 5.0, 0.0, 5.0),
        (5.0, 5.0, 0.0, 0.0, 5.0),
        (5.0, 5.0, 5.0, 5.0, 5.0),
    ];

    for (va, vb, vcin, expected_sum, expected_cout) in cases {
        let (v_sum, v_cout) = solve_full_adder(va, vb, vcin);
        assert!(
            (v_sum - expected_sum).abs() < 0.5,
            "Full adder SUM({va}, {vb}, {vcin}) = {v_sum:.3}V, expected {expected_sum:.1}V"
        );
        assert!(
            (v_cout - expected_cout).abs() < 0.5,
            "Full adder COUT({va}, {vb}, {vcin}) = {v_cout:.3}V, expected {expected_cout:.1}V"
        );
    }
}
