use tw_electrical::prelude::*;

#[test]
fn nmos_resistive_inverter() {
    let mut ckt = Circuit::new();
    let n_vdd = ckt.get_or_add_node("vdd");
    let n_in = ckt.get_or_add_node("in");
    let n_out = ckt.get_or_add_node("out");

    let vdd = 5.0;
    ckt.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, vdd);
    ckt.add_resistor("Rd", n_vdd, n_out, 10_000.0); // 10 kOhm pull-up

    // NMOS: Drain = n_out, Gate = n_in, Source = GND
    ckt.add_nmos("M1", n_out, n_in, NodeId::GROUND, 1.0, 1e-3);

    // Case 1: Vin = 0V (Cutoff: NMOS OFF)
    ckt.add_voltage_source("Vin", n_in, NodeId::GROUND, 0.0);
    let state_off = dc_op(&ckt).expect("NMOS off state solve succeeded");
    assert!(
        (state_off.voltage(n_out) - vdd).abs() < 1e-3,
        "Vout should be pulled up to 5V when Vin=0V, got {}",
        state_off.voltage(n_out)
    );

    // Case 2: Vin = 5V (NMOS ON)
    // Replace Vin with 5V
    let mut ckt_on = Circuit::new();
    let n_vdd = ckt_on.get_or_add_node("vdd");
    let n_in = ckt_on.get_or_add_node("in");
    let n_out = ckt_on.get_or_add_node("out");

    ckt_on.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, 5.0);
    ckt_on.add_voltage_source("Vin", n_in, NodeId::GROUND, 5.0);
    ckt_on.add_resistor("Rd", n_vdd, n_out, 10_000.0);
    ckt_on.add_nmos("M1", n_out, n_in, NodeId::GROUND, 1.0, 1e-3);

    let state_on = dc_op(&ckt_on).expect("NMOS on state solve succeeded");
    let v_out_low = state_on.voltage(n_out);
    assert!(
        v_out_low < 0.2,
        "Vout should be pulled low (< 0.2V) when Vin=5V, got {}",
        v_out_low
    );
}

#[test]
fn cmos_inverter_logic_levels() {
    // CMOS Inverter:
    // PMOS: Source = VDD, Drain = Out, Gate = In
    // NMOS: Drain = Out, Source = GND, Gate = In

    // Test A: Logic LOW Input (Vin = 0V) -> Output must be Logic HIGH (5.0V)
    {
        let mut ckt = Circuit::new();
        let n_vdd = ckt.get_or_add_node("vdd");
        let n_in = ckt.get_or_add_node("in");
        let n_out = ckt.get_or_add_node("out");

        ckt.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, 5.0);
        ckt.add_voltage_source("Vin", n_in, NodeId::GROUND, 0.0);

        ckt.add_pmos("Mp", n_out, n_in, n_vdd, 0.8, 1e-3);
        ckt.add_nmos("Mn", n_out, n_in, NodeId::GROUND, 0.8, 1e-3);

        let state = dc_op(&ckt).expect("CMOS inverter low input solve succeeded");
        let v_out = state.voltage(n_out);
        assert!(
            (v_out - 5.0).abs() < 0.05,
            "Logic LOW input (0V) should produce Logic HIGH output (~5.0V), got {}",
            v_out
        );
    }

    // Test B: Logic HIGH Input (Vin = 5V) -> Output must be Logic LOW (0.0V)
    {
        let mut ckt = Circuit::new();
        let n_vdd = ckt.get_or_add_node("vdd");
        let n_in = ckt.get_or_add_node("in");
        let n_out = ckt.get_or_add_node("out");

        ckt.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, 5.0);
        ckt.add_voltage_source("Vin", n_in, NodeId::GROUND, 5.0);

        ckt.add_pmos("Mp", n_out, n_in, n_vdd, 0.8, 1e-3);
        ckt.add_nmos("Mn", n_out, n_in, NodeId::GROUND, 0.8, 1e-3);

        let state = dc_op(&ckt).expect("CMOS inverter high input solve succeeded");
        let v_out = state.voltage(n_out);
        assert!(
            v_out < 0.05,
            "Logic HIGH input (5V) should produce Logic LOW output (~0.0V), got {}",
            v_out
        );
    }

    // Test C: Midpoint Switching Threshold (Vin = 2.5V) -> Output ~ 2.5V
    {
        let mut ckt = Circuit::new();
        let n_vdd = ckt.get_or_add_node("vdd");
        let n_in = ckt.get_or_add_node("in");
        let n_out = ckt.get_or_add_node("out");

        ckt.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, 5.0);
        ckt.add_voltage_source("Vin", n_in, NodeId::GROUND, 2.5);

        ckt.add_pmos("Mp", n_out, n_in, n_vdd, 0.8, 1e-3);
        ckt.add_nmos("Mn", n_out, n_in, NodeId::GROUND, 0.8, 1e-3);

        let state = dc_op(&ckt).expect("CMOS inverter midpoint solve succeeded");
        let v_out = state.voltage(n_out);
        assert!(
            (v_out - 2.5).abs() < 0.15,
            "Midpoint input (2.5V) should produce ~2.5V output, got {}",
            v_out
        );
    }
}

#[test]
fn cmos_inverter_transient_discharge() {
    let mut ckt = Circuit::new();
    let n_vdd = ckt.get_or_add_node("vdd");
    let n_in = ckt.get_or_add_node("in");
    let n_out = ckt.get_or_add_node("out");

    ckt.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, 5.0);
    // Step input: Vin = 5.0V (switched HIGH)
    ckt.add_voltage_source("Vin", n_in, NodeId::GROUND, 5.0);

    ckt.add_pmos("Mp", n_out, n_in, n_vdd, 0.8, 1e-3);
    ckt.add_nmos("Mn", n_out, n_in, NodeId::GROUND, 0.8, 1e-3);

    // 10 pF load capacitor
    let c_load = 10e-12;
    ckt.add_capacitor("Cload", n_out, NodeId::GROUND, c_load);

    // Initial state: capacitor charged to 5.0V
    let mut init = CircuitState::new();
    init.set_voltage(n_out, 5.0);
    init.set_voltage(n_vdd, 5.0);
    init.set_voltage(n_in, 5.0);

    let dt = 1e-9; // 1 ns step
    let t_stop = 50e-9; // 50 ns
    let res = transient(&ckt, t_stop, dt, Some(init)).expect("transient solve succeeded");

    let waveform = res.voltage_waveform(n_out);
    assert!(!waveform.is_empty());

    // Check that voltage starts at 5.0V and discharges towards 0V
    let v_start = waveform.first().unwrap().1;
    let v_end = waveform.last().unwrap().1;

    assert!((v_start - 5.0).abs() < 1e-3);
    assert!(v_end < 0.1, "expected Vout discharged < 0.1V, got {}", v_end);

    // Monotonic discharge
    for i in 1..waveform.len() {
        assert!(
            waveform[i].1 <= waveform[i - 1].1 + 1e-6,
            "waveform should decrease monotonically: at {}: {} vs {}",
            waveform[i].0,
            waveform[i].1,
            waveform[i - 1].1
        );
    }
}
