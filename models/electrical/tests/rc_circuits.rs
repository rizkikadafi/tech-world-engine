use tw_electrical::prelude::*;

#[test]
fn rc_step_charging_automated() {
    let mut ckt = Circuit::new();
    let n_in = ckt.get_or_add_node("in");
    let n_out = ckt.get_or_add_node("out");

    let vdd = 5.0;
    let r = 1000.0;
    let c = 1e-6; // 1 uF
    let tau = r * c; // 1 ms
    let dt = 1e-5; // 10 us (tau / 100)
    let t_stop = 5.0 * tau; // 5 ms

    ckt.add_voltage_source("V1", n_in, NodeId::GROUND, vdd);
    ckt.add_resistor("R1", n_in, n_out, r);
    ckt.add_capacitor("C1", n_out, NodeId::GROUND, c);

    // Initial state: 0V everywhere
    let result = transient(&ckt, t_stop, dt, None).expect("transient analysis succeeded");

    // Check waveforms at several checkpoints: t = 0.5 tau, 1.0 tau, 2.0 tau, 5.0 tau
    let waveforms = result.voltage_waveform(n_out);

    let check_times = [0.5 * tau, 1.0 * tau, 2.0 * tau, 3.0 * tau, 5.0 * tau];
    for &t_target in &check_times {
        // Find closest point
        let (t_sim, v_sim) = waveforms
            .iter()
            .min_by(|a, b| {
                (a.0 - t_target)
                    .abs()
                    .partial_cmp(&(b.0 - t_target).abs())
                    .unwrap()
            })
            .copied()
            .unwrap();

        let v_analytic = vdd * (1.0 - (-t_sim / tau).exp());

        // Error with dt = tau / 100 on Backward Euler is < 1%
        let rel_error = (v_sim - v_analytic).abs() / vdd;
        assert!(
            rel_error < 0.01,
            "at t={:?}, got v_sim={}, v_analytic={}, rel_error={}",
            t_sim,
            v_sim,
            v_analytic,
            rel_error
        );
    }

    // At 5 tau, capacitor should be essentially fully charged (~99.3%)
    let final_state = result.final_state().unwrap();
    let v_final = final_state.voltage(n_out);
    assert!((v_final - vdd).abs() < 0.04);
}

#[test]
fn rc_discharging_automated() {
    let mut ckt = Circuit::new();
    let n_out = ckt.get_or_add_node("out");

    let r = 2000.0;
    let c = 0.5e-6; // 0.5 uF
    let tau = r * c; // 1 ms
    let dt = 1e-5;
    let t_stop = 4.0 * tau;

    // Resistor to GND
    ckt.add_resistor("R1", n_out, NodeId::GROUND, r);
    // Capacitor to GND
    ckt.add_capacitor("C1", n_out, NodeId::GROUND, c);

    // Pre-charged to 10V
    let mut init = CircuitState::new();
    init.set_voltage(n_out, 10.0);

    let result = transient(&ckt, t_stop, dt, Some(init)).expect("transient discharge succeeded");
    let waveforms = result.voltage_waveform(n_out);

    // V(t) = 10 * exp(-t / tau)
    for &t_target in &[0.5 * tau, 1.0 * tau, 2.0 * tau] {
        let (t_sim, v_sim) = waveforms
            .iter()
            .min_by(|a, b| {
                (a.0 - t_target)
                    .abs()
                    .partial_cmp(&(b.0 - t_target).abs())
                    .unwrap()
            })
            .copied()
            .unwrap();

        let v_analytic = 10.0 * (-t_sim / tau).exp();
        let rel_error = (v_sim - v_analytic).abs() / 10.0;
        assert!(
            rel_error < 0.01,
            "discharge at t={:?}, got v_sim={}, v_analytic={}",
            t_sim,
            v_sim,
            v_analytic
        );
    }
}
