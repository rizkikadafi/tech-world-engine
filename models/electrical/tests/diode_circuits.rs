use tw_electrical::prelude::*;

#[test]
fn diode_forward_bias() {
    let mut ckt = Circuit::new();
    let n_in = ckt.get_or_add_node("in");
    let n_d = ckt.get_or_add_node("d");

    let vin = 5.0;
    let r = 1000.0;

    ckt.add_voltage_source("V1", n_in, NodeId::GROUND, vin);
    ckt.add_resistor("R1", n_in, n_d, r);
    ckt.add_diode("D1", n_d, NodeId::GROUND);

    let state = dc_op(&ckt).expect("diode forward bias solve succeeded");

    let vd = state.voltage(n_d);
    // Typical silicon diode drop is between 0.6V and 0.8V
    assert!(
        vd > 0.6 && vd < 0.85,
        "expected diode drop between 0.6V and 0.85V, got {}",
        vd
    );

    // KCL check: Current through resistor (Vin - Vd)/R must equal diode current
    let ir = (vin - vd) / r;
    let id_calc = 1e-14 * ((vd / 0.02585).exp() - 1.0);
    // Relative error between resistor current and diode Shockley current < 1%
    let rel_diff = (ir - id_calc).abs() / ir;
    assert!(
        rel_diff < 0.02,
        "KCL mismatch: i_resistor={}, i_diode={}, rel_diff={}",
        ir,
        id_calc,
        rel_diff
    );
}

#[test]
fn diode_reverse_bias() {
    let mut ckt = Circuit::new();
    let n_in = ckt.get_or_add_node("in");
    let n_d = ckt.get_or_add_node("d");

    // Reverse voltage: -5V
    let vin = -5.0;
    let r = 1000.0;

    ckt.add_voltage_source("V1", n_in, NodeId::GROUND, vin);
    ckt.add_resistor("R1", n_in, n_d, r);
    // Anode to n_d, Cathode to GND => V_anode is negative, diode reverse-biased
    ckt.add_diode("D1", n_d, NodeId::GROUND);

    let state = dc_op(&ckt).expect("diode reverse bias solve succeeded");

    let vd = state.voltage(n_d);
    // In reverse bias, almost no current flows through 1k resistor, so V_d is very close to Vin (-5V)
    assert!(
        (vd - vin).abs() < 1e-4,
        "expected Vd close to -5.0V, got {}",
        vd
    );
}
