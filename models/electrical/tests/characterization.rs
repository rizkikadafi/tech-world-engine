use tw_electrical::prelude::*;

#[test]
fn inverter_metrics_are_plausible() {
    let characterizer = Characterizer::with_defaults();
    let measured = characterizer.inverter().expect("inverter characterized");

    println!(
        "inverter: levels={:?} timing={:?} truth={:?}",
        measured.levels, measured.timing, measured.truth_table
    );

    assert_eq!(measured.kind, GateKind::Inverter);
    assert_eq!(measured.truth_table.inputs, 1);
    assert_eq!(measured.truth_table.outputs, vec![true, false]);
    assert!(
        measured.levels.high > 4.0,
        "expected a high output near Vdd, got {}",
        measured.levels.high
    );
    assert!(
        measured.levels.low < 1.0,
        "expected a low output near ground, got {}",
        measured.levels.low
    );

    assert!(measured.timing.t_pHL > 0.0);
    assert!(measured.timing.t_pLH > 0.0);
    assert!(measured.timing.t_rise > 0.0);
    assert!(measured.timing.t_fall > 0.0);
    assert!(measured.timing.energy_rise > 0.0);
    assert!(measured.timing.energy_per_cycle >= measured.timing.energy_rise);
    assert!(measured.timing.average_power(10e-9) > 0.0);
}

#[test]
fn inverter_truth_table_matches_dc_op() {
    let characterizer = Characterizer::with_defaults();
    let measured = characterizer.inverter().expect("inverter characterized");

    for input in [false, true] {
        let mut circuit = Circuit::new();
        let n_vdd = circuit.get_or_add_node("vdd");
        let n_in = circuit.get_or_add_node("in");
        let n_out = circuit.get_or_add_node("out");
        circuit.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, 5.0);
        circuit.add_voltage_source("Vin", n_in, NodeId::GROUND, if input { 5.0 } else { 0.0 });
        cmos_inverter(CmosGateConfig::default())
            .instantiate(
                &mut circuit,
                "INV1",
                &[
                    ("IN", n_in),
                    ("OUT", n_out),
                    ("VDD", n_vdd),
                    ("GND", NodeId::GROUND),
                ],
            )
            .expect("inverter instantiated");

        let state = dc_op(&circuit).expect("dc solve");
        let expected = state.voltage(n_out) > measured.levels.threshold(0.5);
        assert_eq!(
            measured.truth_table.output_for(&[input]),
            Some(expected),
            "truth table mismatch for input {input}"
        );
    }
}

#[test]
fn inverter_timing_converges_when_dt_halved() {
    let coarse_config = CharacterizationConfig::default();
    let mut fine_config = coarse_config.clone();
    fine_config.dt = coarse_config.dt / 2.0;

    let coarse = Characterizer::new(coarse_config)
        .expect("valid coarse config")
        .inverter()
        .expect("coarse characterization");
    let fine = Characterizer::new(fine_config)
        .expect("valid fine config")
        .inverter()
        .expect("fine characterization");

    println!(
        "coarse t_pHL={} t_pLH={} | fine t_pHL={} t_pLH={}",
        coarse.timing.t_pHL, coarse.timing.t_pLH, fine.timing.t_pHL, fine.timing.t_pLH
    );

    let relative = |a: f64, b: f64| (a - b).abs() / b.max(f64::MIN_POSITIVE);
    assert!(
        relative(coarse.timing.t_pHL, fine.timing.t_pHL) < 0.2,
        "t_pHL did not converge: coarse={}, fine={}",
        coarse.timing.t_pHL,
        fine.timing.t_pHL
    );
    assert!(
        relative(coarse.timing.t_pLH, fine.timing.t_pLH) < 0.2,
        "t_pLH did not converge: coarse={}, fine={}",
        coarse.timing.t_pLH,
        fine.timing.t_pLH
    );
}

#[test]
fn energy_sign_matches_resistive_sanity_check() {
    let mut circuit = Circuit::new();
    let n1 = circuit.get_or_add_node("1");
    circuit.add_voltage_source("V1", n1, NodeId::GROUND, 10.0);
    circuit.add_resistor("R1", n1, NodeId::GROUND, 50.0);

    let result = transient(&circuit, 1e-3, 1e-4, None).expect("transient");
    let energy = integrate_source_energy(&result, "V1", 10.0).expect("energy integration");

    // P = V^2 / R = 100 / 50 = 2 W, over 1 ms => 2 mJ.
    assert!(
        (energy - 2e-3).abs() < 1e-9,
        "expected 2 mJ delivered, got {energy}"
    );
}

#[test]
fn inverter_characterization_is_deterministic() {
    let first = Characterizer::with_defaults()
        .inverter()
        .expect("first characterization");
    let second = Characterizer::with_defaults()
        .inverter()
        .expect("second characterization");

    assert_eq!(first.truth_table, second.truth_table);
    assert!((first.levels.high - second.levels.high).abs() < 1e-9);
    assert!((first.levels.low - second.levels.low).abs() < 1e-9);
    assert!((first.timing.t_pHL - second.timing.t_pHL).abs() < 1e-15);
    assert!((first.timing.t_pLH - second.timing.t_pLH).abs() < 1e-15);
    assert!((first.timing.energy_per_cycle - second.timing.energy_per_cycle).abs() < 1e-15);
}

#[test]
fn characterization_rejects_invalid_config() {
    let config = CharacterizationConfig {
        dt: 0.0,
        ..CharacterizationConfig::default()
    };
    assert!(Characterizer::new(config).is_err());

    let default = CharacterizationConfig::default();
    let config = CharacterizationConfig {
        duration: default.dt / 2.0,
        ..default
    };
    assert!(Characterizer::new(config).is_err());

    let config = CharacterizationConfig {
        low_threshold: 0.8,
        mid_threshold: 0.5,
        ..CharacterizationConfig::default()
    };
    assert!(Characterizer::new(config).is_err());
}

#[test]
fn nand2_metrics_are_plausible() {
    let characterizer = Characterizer::with_defaults();
    let measured = characterizer.nand2().expect("nand2 characterized");

    println!(
        "nand2: levels={:?} timing={:?} truth={:?}",
        measured.levels, measured.timing, measured.truth_table
    );

    assert_eq!(measured.kind, GateKind::Nand2);
    assert_eq!(measured.truth_table.inputs, 2);
    assert_eq!(
        measured.truth_table.outputs,
        vec![true, true, true, false],
        "NAND must only output low when both inputs are high"
    );
    assert_eq!(measured.truth_table.output_for(&[false, false]), Some(true));
    assert_eq!(measured.truth_table.output_for(&[false, true]), Some(true));
    assert_eq!(measured.truth_table.output_for(&[true, false]), Some(true));
    assert_eq!(measured.truth_table.output_for(&[true, true]), Some(false));

    assert!(
        measured.levels.high > 4.0,
        "expected a high output near Vdd, got {}",
        measured.levels.high
    );
    assert!(
        measured.levels.low < 1.0,
        "expected a low output near ground, got {}",
        measured.levels.low
    );

    assert!(measured.timing.t_pHL > 0.0);
    assert!(measured.timing.t_pLH > 0.0);
    assert!(measured.timing.t_rise > 0.0);
    assert!(measured.timing.t_fall > 0.0);
    assert!(measured.timing.energy_rise > 0.0);
    assert!(measured.timing.energy_per_cycle >= measured.timing.energy_rise);
    assert!(measured.timing.average_power(10e-9) > 0.0);
}

#[test]
fn nand2_truth_table_matches_dc_op() {
    let measured = Characterizer::with_defaults()
        .nand2()
        .expect("nand2 characterized");

    for pattern in 0..4usize {
        let a = (pattern >> 1) & 1 == 1;
        let b = pattern & 1 == 1;

        let mut circuit = Circuit::new();
        let n_vdd = circuit.get_or_add_node("vdd");
        let n_a = circuit.get_or_add_node("a");
        let n_b = circuit.get_or_add_node("b");
        let n_out = circuit.get_or_add_node("out");
        circuit.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, 5.0);
        circuit.add_voltage_source("Vin_A", n_a, NodeId::GROUND, if a { 5.0 } else { 0.0 });
        circuit.add_voltage_source("Vin_B", n_b, NodeId::GROUND, if b { 5.0 } else { 0.0 });
        cmos_nand(CmosGateConfig::default())
            .instantiate(
                &mut circuit,
                "NAND1",
                &[
                    ("A", n_a),
                    ("B", n_b),
                    ("OUT", n_out),
                    ("VDD", n_vdd),
                    ("GND", NodeId::GROUND),
                ],
            )
            .expect("nand2 instantiated");

        let state = dc_op(&circuit).expect("dc solve");
        let expected = state.voltage(n_out) > measured.levels.threshold(0.5);
        assert_eq!(
            measured.truth_table.output_for(&[a, b]),
            Some(expected),
            "truth table mismatch for A={a} B={b}"
        );
    }
}

#[test]
fn nand2_timing_converges_when_dt_halved() {
    let coarse_config = CharacterizationConfig::default();
    let mut fine_config = coarse_config.clone();
    fine_config.dt = coarse_config.dt / 2.0;

    let coarse = Characterizer::new(coarse_config)
        .expect("valid coarse config")
        .nand2()
        .expect("coarse nand2 characterization");
    let fine = Characterizer::new(fine_config)
        .expect("valid fine config")
        .nand2()
        .expect("fine nand2 characterization");

    println!(
        "nand2 coarse t_pHL={} t_pLH={} | fine t_pHL={} t_pLH={}",
        coarse.timing.t_pHL, coarse.timing.t_pLH, fine.timing.t_pHL, fine.timing.t_pLH
    );

    let relative = |a: f64, b: f64| (a - b).abs() / b.max(f64::MIN_POSITIVE);
    assert!(
        relative(coarse.timing.t_pHL, fine.timing.t_pHL) < 0.2,
        "t_pHL did not converge: coarse={}, fine={}",
        coarse.timing.t_pHL,
        fine.timing.t_pHL
    );
    assert!(
        relative(coarse.timing.t_pLH, fine.timing.t_pLH) < 0.2,
        "t_pLH did not converge: coarse={}, fine={}",
        coarse.timing.t_pLH,
        fine.timing.t_pLH
    );
}

#[test]
fn dff_metrics_are_plausible() {
    let characterizer = Characterizer::with_defaults();
    let measured = characterizer.dff().expect("dff characterized");

    println!(
        "dff: levels={:?} timing={:?} truth={:?}",
        measured.levels, measured.timing, measured.truth_table
    );

    assert_eq!(measured.kind, GateKind::Dff);
    assert_eq!(measured.truth_table.inputs, 2);
    assert_eq!(
        measured.truth_table.outputs,
        vec![false, false, true, true],
        "next-state table must be Q=D after a rising edge"
    );
    assert_eq!(
        measured.truth_table.output_for(&[false, false]),
        Some(false)
    );
    assert_eq!(measured.truth_table.output_for(&[false, true]), Some(false));
    assert_eq!(measured.truth_table.output_for(&[true, false]), Some(true));
    assert_eq!(measured.truth_table.output_for(&[true, true]), Some(true));

    assert!(
        measured.levels.high > 4.0,
        "expected Q high near Vdd, got {}",
        measured.levels.high
    );
    assert!(
        measured.levels.low < 1.0,
        "expected Q low near ground, got {}",
        measured.levels.low
    );

    assert!(measured.timing.t_pHL > 0.0);
    assert!(measured.timing.t_pLH > 0.0);
    assert!(measured.timing.t_rise > 0.0);
    assert!(measured.timing.t_fall > 0.0);
    assert!(measured.timing.energy_rise > 0.0);
    assert!(measured.timing.energy_per_cycle >= measured.timing.energy_rise);
    assert!(measured.timing.average_power(10e-9) > 0.0);
}
