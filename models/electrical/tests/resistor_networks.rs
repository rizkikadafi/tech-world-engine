use tw_electrical::prelude::*;

#[test]
fn simple_resistor_ohms_law() {
    let mut ckt = Circuit::new();
    let n1 = ckt.get_or_add_node("1");

    // V1 = 10V between node 1 and GND
    ckt.add_voltage_source("V1", n1, NodeId::GROUND, 10.0);
    // R1 = 50 Ohm between node 1 and GND
    ckt.add_resistor("R1", n1, NodeId::GROUND, 50.0);

    let state = dc_op(&ckt).expect("DC operating point solve failed");

    // Node 1 should be 10V
    assert!((state.voltage(n1) - 10.0).abs() < 1e-12);

    // Current through R1 is V/R = 10 / 50 = 0.2 A.
    // By MNA convention, current through V1 leaves positive terminal into node 1,
    // so current is -0.2 A (source delivers 0.2 A).
    let i_v1 = state.current("V1").expect("V1 branch current exists");
    assert!((i_v1 - (-0.2)).abs() < 1e-12, "got {}", i_v1);
}

#[test]
fn voltage_divider_automated() {
    let mut ckt = Circuit::new();
    let n_in = ckt.get_or_add_node("in");
    let n_mid = ckt.get_or_add_node("mid");

    let vdd = 5.0;
    let r1 = 2000.0;
    let r2 = 3000.0;

    ckt.add_voltage_source("Vdd", n_in, NodeId::GROUND, vdd);
    ckt.add_resistor("R1", n_in, n_mid, r1);
    ckt.add_resistor("R2", n_mid, NodeId::GROUND, r2);

    let state = dc_op(&ckt).unwrap();

    let v_expected = vdd * r2 / (r1 + r2); // 5 * 3/5 = 3.0 V
    let v_mid = state.voltage(n_mid);
    assert!(
        (v_mid - v_expected).abs() < 1e-12,
        "got {}, expected {}",
        v_mid,
        v_expected
    );
    assert!((state.voltage(n_in) - vdd).abs() < 1e-12);
}

#[test]
fn three_resistor_ladder() {
    let mut ckt = Circuit::new();
    let n1 = ckt.get_or_add_node("n1");
    let n2 = ckt.get_or_add_node("n2");
    let n3 = ckt.get_or_add_node("n3");

    // 12V -- R1(1k) -- n2 -- R2(2k) -- n3 -- R3(3k) -- GND
    ckt.add_voltage_source("V1", n1, NodeId::GROUND, 12.0);
    ckt.add_resistor("R1", n1, n2, 1000.0);
    ckt.add_resistor("R2", n2, n3, 2000.0);
    ckt.add_resistor("R3", n3, NodeId::GROUND, 3000.0);

    let state = dc_op(&ckt).unwrap();

    let total_r = 1000.0 + 2000.0 + 3000.0; // 6000 Ohm
    let i_total = 12.0 / total_r; // 2 mA

    let exp_n3 = i_total * 3000.0; // 6.0 V
    let exp_n2 = i_total * (2000.0 + 3000.0); // 10.0 V

    assert!((state.voltage(n1) - 12.0).abs() < 1e-12);
    assert!((state.voltage(n2) - exp_n2).abs() < 1e-12);
    assert!((state.voltage(n3) - exp_n3).abs() < 1e-12);
}

#[test]
fn wheatstone_bridge() {
    // Wheatstone bridge:
    // Vin connected to node A.
    // Branch 1: A -> R1 -> B -> R2 -> GND
    // Branch 2: A -> R3 -> C -> R4 -> GND
    // Bridge resistor: B -> R_bridge -> C
    let mut ckt = Circuit::new();
    let n_a = ckt.get_or_add_node("A");
    let n_b = ckt.get_or_add_node("B");
    let n_c = ckt.get_or_add_node("C");

    ckt.add_voltage_source("V_in", n_a, NodeId::GROUND, 10.0);

    // Balanced bridge: R1/R2 == R3/R4
    // R1 = 100, R2 = 200 => V_B = 10 * 200/300 = 6.666...
    // R3 = 500, R4 = 1000 => V_C = 10 * 1000/1500 = 6.666...
    // Bridge resistor R5 = 50 between B and C.
    ckt.add_resistor("R1", n_a, n_b, 100.0);
    ckt.add_resistor("R2", n_b, NodeId::GROUND, 200.0);
    ckt.add_resistor("R3", n_a, n_c, 500.0);
    ckt.add_resistor("R4", n_c, NodeId::GROUND, 1000.0);
    ckt.add_resistor("R5", n_b, n_c, 50.0);

    let state = dc_op(&ckt).unwrap();

    let vb = state.voltage(n_b);
    let vc = state.voltage(n_c);

    // Because bridge is balanced, V_B == V_C and no current flows through R5!
    assert!((vb - vc).abs() < 1e-12, "vb={}, vc={}", vb, vc);
    let expected_v = 10.0 * 200.0 / 300.0;
    assert!((vb - expected_v).abs() < 1e-12);
}

#[test]
fn current_source_parallel_resistor() {
    let mut ckt = Circuit::new();
    let n1 = ckt.get_or_add_node("1");

    // 2 Ampere current source into node 1 from GND
    ckt.add_current_source("I1", n1, NodeId::GROUND, 2.0);
    // 50 Ohm resistor to GND
    ckt.add_resistor("R1", n1, NodeId::GROUND, 50.0);

    let state = dc_op(&ckt).unwrap();

    // V = I * R = 2.0 * 50 = 100 V
    assert!((state.voltage(n1) - 100.0).abs() < 1e-12);
}
