use tw_electrical::analysis::transient_adaptive;
use tw_electrical::circuit::Circuit;
use tw_electrical::gates::{cmos_inverter, CmosGateConfig};
use tw_electrical::mna::MnaSystem;
use tw_electrical::node::NodeId;
use tw_num::ode::StepControlConfig;

#[test]
fn test_adaptive_rc_charging_step_count_and_accuracy() {
    let mut ckt = Circuit::new();
    let n_in = ckt.get_or_add_node("in");
    let n_out = ckt.get_or_add_node("out");

    let r_val = 1000.0; // 1 kOhm
    let c_val = 1e-6; // 1 uF
    let v_dd = 5.0; // 5 V
    let tau = r_val * c_val; // 1 ms

    ckt.add_voltage_source("V1", n_in, NodeId::GROUND, v_dd);
    ckt.add_resistor("R1", n_in, n_out, r_val);
    ckt.add_capacitor("C1", n_out, NodeId::GROUND, c_val);

    let t_stop = 5.0 * tau; // 5 ms (approx 99.3% charged)

    let config = StepControlConfig {
        dt_init: 1e-5, // start at 10 us
        dt_max: 5e-4,  // max 0.5 ms
        dt_min: 1e-12,
        trtol: 7.0,
        reltol: 1e-3,
        abstol: 1e-4,
        max_growth_factor: 2.0,
        min_shrink_factor: 0.1,
    };

    let result = transient_adaptive(&ckt, t_stop, Some(config), None).unwrap();

    // Verify step efficiency: adaptive should take far fewer than 5000 steps
    println!("Adaptive RC steps taken: {}", result.len());
    assert!(
        result.len() < 200,
        "Adaptive transient took too many steps: {}",
        result.len()
    );

    // Verify that time steps expanded as voltage leveled off
    let final_state = result.final_state().unwrap();
    let v_final = final_state.voltage(n_out);
    let expected_final = v_dd * (1.0 - (-5.0f64).exp()); // ~4.9663 V

    let rel_err = (v_final - expected_final).abs() / expected_final;
    assert!(
        rel_err < 0.02,
        "Final voltage {} differs from analytic {} by {}",
        v_final,
        expected_final,
        rel_err
    );

    // Verify monotonic charging
    let waveform = result.voltage_waveform(n_out);
    for window in waveform.windows(2) {
        let (t1, v1) = window[0];
        let (t2, v2) = window[1];
        assert!(t2 > t1, "Time must advance strictly forward");
        assert!(v2 >= v1 - 1e-9, "RC charging voltage must be monotonic");
    }
}

#[test]
fn test_adaptive_cmos_inverter_discharge() {
    let mut ckt = Circuit::new();
    let n_in = ckt.get_or_add_node("in");
    let n_out = ckt.get_or_add_node("out");
    let n_vdd = ckt.get_or_add_node("vdd");

    ckt.add_voltage_source("VDD", n_vdd, NodeId::GROUND, 5.0);
    // Input is switched HIGH (5V) -> pull-down NMOS will discharge n_out
    ckt.add_voltage_source("VIN", n_in, NodeId::GROUND, 5.0);

    let gate_cfg = CmosGateConfig {
        load_capacitance: 10e-12, // 10 pF
        ..Default::default()
    };
    cmos_inverter(gate_cfg)
        .instantiate(
            &mut ckt,
            "INV1",
            &[
                ("IN", n_in),
                ("OUT", n_out),
                ("VDD", n_vdd),
                ("GND", NodeId::GROUND),
            ],
        )
        .unwrap();

    // Initial state: output charged to 5V
    let mut init_state = tw_electrical::state::CircuitState::new();
    init_state.set_voltage(n_out, 5.0);
    init_state.set_voltage(n_vdd, 5.0);
    init_state.set_voltage(n_in, 5.0);

    let config = StepControlConfig {
        dt_init: 1e-12,
        dt_max: 1e-10,
        dt_min: 1e-16,
        abstol: 1e-3,
        ..Default::default()
    };

    let result = transient_adaptive(&ckt, 5e-9, Some(config), Some(init_state)).unwrap();

    // Verify discharged output is LOW (< 0.1 V)
    let final_v = result.final_state().unwrap().voltage(n_out);
    assert!(
        final_v < 0.1,
        "CMOS inverter output should discharge to LOW, got {}",
        final_v
    );
}

#[test]
fn test_sparse_dense_equivalence_on_mna_system() {
    let mut ckt = Circuit::new();
    let n_in = ckt.get_or_add_node("in");
    let n_mid = ckt.get_or_add_node("mid");
    let n_out = ckt.get_or_add_node("out");

    ckt.add_voltage_source("V1", n_in, NodeId::GROUND, 10.0);
    ckt.add_resistor("R1", n_in, n_mid, 1000.0);
    ckt.add_resistor("R2", n_mid, n_out, 2000.0);
    ckt.add_resistor("R3", n_out, NodeId::GROUND, 3000.0);

    let mna = MnaSystem::build_dc(&ckt).unwrap();
    let sparse_state = mna.solve(&ckt).unwrap();
    let dense_state = mna.solve_dense(&ckt).unwrap();

    for node in [n_in, n_mid, n_out] {
        let v_sparse = sparse_state.voltage(node);
        let v_dense = dense_state.voltage(node);
        assert!(
            (v_sparse - v_dense).abs() < 1e-12,
            "Node {:?} mismatch: sparse={}, dense={}",
            node,
            v_sparse,
            v_dense
        );
    }
}
