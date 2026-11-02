use tw_electrical::prelude::*;

fn build_flat_and(va: f64, vb: f64) -> f64 {
    let mut ckt = Circuit::new();
    let n_vdd = ckt.get_or_add_node("vdd");
    let n_a = ckt.get_or_add_node("a");
    let n_b = ckt.get_or_add_node("b");
    let n_mid = ckt.get_or_add_node("mid");
    let n_out = ckt.get_or_add_node("out");

    ckt.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, 5.0);
    ckt.add_voltage_source("Va", n_a, NodeId::GROUND, va);
    ckt.add_voltage_source("Vb", n_b, NodeId::GROUND, vb);

    ckt.add_cmos_nand("NAND1", n_a, n_b, n_mid, n_vdd, NodeId::GROUND)
        .expect("nand instantiated");
    ckt.add_cmos_inverter("INV1", n_mid, n_out, n_vdd, NodeId::GROUND)
        .expect("inverter instantiated");

    dc_op(&ckt).expect("flat AND dc solve").voltage(n_out)
}

fn nested_and_template() -> Subcircuit {
    let config = CmosGateConfig::default();
    let mut template = Subcircuit::new("nested_and", ["A", "B", "OUT", "VDD", "GND"]);

    template.add_subcircuit(
        "NAND1",
        cmos_nand(config),
        &[
            ("A", "A"),
            ("B", "B"),
            ("OUT", "nand_out"),
            ("VDD", "VDD"),
            ("GND", "GND"),
        ],
    );
    template.add_subcircuit(
        "INV1",
        cmos_inverter(config),
        &[
            ("IN", "nand_out"),
            ("OUT", "OUT"),
            ("VDD", "VDD"),
            ("GND", "GND"),
        ],
    );

    template
}

fn build_nested_and(va: f64, vb: f64) -> f64 {
    let mut ckt = Circuit::new();
    let n_vdd = ckt.get_or_add_node("vdd");
    let n_a = ckt.get_or_add_node("a");
    let n_b = ckt.get_or_add_node("b");
    let n_out = ckt.get_or_add_node("out");

    ckt.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, 5.0);
    ckt.add_voltage_source("Va", n_a, NodeId::GROUND, va);
    ckt.add_voltage_source("Vb", n_b, NodeId::GROUND, vb);

    nested_and_template()
        .instantiate(
            &mut ckt,
            "AND1",
            &[
                ("A", n_a),
                ("B", n_b),
                ("OUT", n_out),
                ("VDD", n_vdd),
                ("GND", NodeId::GROUND),
            ],
        )
        .expect("nested AND instantiated");

    dc_op(&ckt).expect("nested AND dc solve").voltage(n_out)
}

#[test]
fn nested_subcircuit_matches_flat_equivalent() {
    let cases = [
        (0.0, 0.0, 0.0),
        (0.0, 5.0, 0.0),
        (5.0, 0.0, 0.0),
        (5.0, 5.0, 5.0),
    ];

    for (va, vb, expected) in cases {
        let flat = build_flat_and(va, vb);
        let nested = build_nested_and(va, vb);
        assert!(
            (flat - expected).abs() < 0.2,
            "flat AND({va}, {vb}) = {flat:.3}V, expected {expected:.1}V"
        );
        assert!(
            (nested - flat).abs() < 1e-6,
            "nested AND({va}, {vb}) = {nested:.3}V, flat = {flat:.3}V"
        );
    }
}

#[test]
fn nested_instance_nodes_are_namespaced() {
    let mut ckt = Circuit::new();
    let n_vdd = ckt.get_or_add_node("vdd");
    let n_a1 = ckt.get_or_add_node("a1");
    let n_b1 = ckt.get_or_add_node("b1");
    let n_a2 = ckt.get_or_add_node("a2");
    let n_b2 = ckt.get_or_add_node("b2");
    let n_out1 = ckt.get_or_add_node("out1");
    let n_out2 = ckt.get_or_add_node("out2");

    ckt.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, 5.0);
    ckt.add_voltage_source("Va1", n_a1, NodeId::GROUND, 5.0);
    ckt.add_voltage_source("Vb1", n_b1, NodeId::GROUND, 5.0);
    ckt.add_voltage_source("Va2", n_a2, NodeId::GROUND, 5.0);
    ckt.add_voltage_source("Vb2", n_b2, NodeId::GROUND, 0.0);

    let template = nested_and_template();
    template
        .instantiate(
            &mut ckt,
            "U1",
            &[
                ("A", n_a1),
                ("B", n_b1),
                ("OUT", n_out1),
                ("VDD", n_vdd),
                ("GND", NodeId::GROUND),
            ],
        )
        .expect("U1 instantiated");
    template
        .instantiate(
            &mut ckt,
            "U2",
            &[
                ("A", n_a2),
                ("B", n_b2),
                ("OUT", n_out2),
                ("VDD", n_vdd),
                ("GND", NodeId::GROUND),
            ],
        )
        .expect("U2 instantiated");

    assert!(
        ckt.node_by_name("U1_NAND1_n_mid").is_some(),
        "U1 internal node should be namespaced"
    );
    assert!(
        ckt.node_by_name("U2_NAND1_n_mid").is_some(),
        "U2 internal node should be namespaced"
    );
    assert_ne!(
        ckt.node_by_name("U1_NAND1_n_mid"),
        ckt.node_by_name("U2_NAND1_n_mid"),
        "instances must not share internal nodes"
    );

    let state = dc_op(&ckt).expect("two-instance dc solve");
    assert!(
        state.voltage(n_out1) > 4.8,
        "U1 AND(1, 1) should be HIGH, got {:.3}V",
        state.voltage(n_out1)
    );
    assert!(
        state.voltage(n_out2) < 0.2,
        "U2 AND(1, 0) should be LOW, got {:.3}V",
        state.voltage(n_out2)
    );
}

#[test]
fn unknown_child_port_is_rejected() {
    let config = CmosGateConfig::default();
    let mut template = Subcircuit::new("bad", ["A", "B", "OUT", "VDD", "GND"]);
    template.add_subcircuit(
        "NAND1",
        cmos_nand(config),
        &[
            ("A", "A"),
            ("B", "B"),
            ("OUT", "OUT"),
            ("VDD", "VDD"),
            ("NOPE", "GND"),
        ],
    );

    let err = template.validate().expect_err("unknown port must fail");
    assert!(
        format!("{err}").contains("no port 'NOPE'"),
        "unexpected error: {err}"
    );
}

#[test]
fn missing_child_port_is_rejected() {
    let config = CmosGateConfig::default();
    let mut template = Subcircuit::new("bad", ["A", "B", "OUT", "VDD", "GND"]);
    template.add_subcircuit(
        "NAND1",
        cmos_nand(config),
        &[("A", "A"), ("B", "B"), ("OUT", "OUT"), ("VDD", "VDD")],
    );

    let err = template.validate().expect_err("missing port must fail");
    assert!(
        format!("{err}").contains("port 'GND' is not bound"),
        "unexpected error: {err}"
    );
}

#[test]
fn duplicate_instance_name_is_rejected() {
    let config = CmosGateConfig::default();
    let mut template = Subcircuit::new("bad", ["A", "B", "OUT", "VDD", "GND"]);
    let bindings = [
        ("A", "A"),
        ("B", "B"),
        ("OUT", "nand_out"),
        ("VDD", "VDD"),
        ("GND", "GND"),
    ];
    template.add_subcircuit("NAND1", cmos_nand(config), &bindings);
    template.add_subcircuit("NAND1", cmos_nand(config), &bindings);

    let err = template
        .validate()
        .expect_err("duplicate instance must fail");
    assert!(
        format!("{err}").contains("duplicate subcircuit instance 'NAND1'"),
        "unexpected error: {err}"
    );
}
