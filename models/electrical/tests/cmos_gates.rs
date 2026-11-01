use tw_electrical::prelude::*;

#[test]
fn cmos_nand_truth_table() {
    // Truth table for 2-input NAND:
    // A=0, B=0 => OUT = 1 (5V)
    // A=0, B=1 => OUT = 1 (5V)
    // A=1, B=0 => OUT = 1 (5V)
    // A=1, B=1 => OUT = 0 (0V)
    let cases = [
        (0.0, 0.0, true),
        (0.0, 5.0, true),
        (5.0, 0.0, true),
        (5.0, 5.0, false),
    ];

    for (va, vb, expected_high) in cases {
        let mut ckt = Circuit::new();
        let n_vdd = ckt.get_or_add_node("vdd");
        let n_a = ckt.get_or_add_node("a");
        let n_b = ckt.get_or_add_node("b");
        let n_out = ckt.get_or_add_node("out");

        ckt.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, 5.0);
        ckt.add_voltage_source("Va", n_a, NodeId::GROUND, va);
        ckt.add_voltage_source("Vb", n_b, NodeId::GROUND, vb);

        ckt.add_cmos_nand("NAND1", n_a, n_b, n_out, n_vdd, NodeId::GROUND)
            .expect("nand instantiated");

        let state = dc_op(&ckt).expect("nand dc solve succeeded");
        let v_out = state.voltage(n_out);

        if expected_high {
            assert!(
                v_out > 4.8,
                "NAND input (Va={}, Vb={}) should be HIGH (> 4.8V), got {:.3}V",
                va,
                vb,
                v_out
            );
        } else {
            assert!(
                v_out < 0.2,
                "NAND input (Va={}, Vb={}) should be LOW (< 0.2V), got {:.3}V",
                va,
                vb,
                v_out
            );
        }
    }
}

#[test]
fn cmos_nor_truth_table() {
    // Truth table for 2-input NOR:
    // A=0, B=0 => OUT = 1 (5V)
    // A=0, B=1 => OUT = 0 (0V)
    // A=1, B=0 => OUT = 0 (0V)
    // A=1, B=1 => OUT = 0 (0V)
    let cases = [
        (0.0, 0.0, true),
        (0.0, 5.0, false),
        (5.0, 0.0, false),
        (5.0, 5.0, false),
    ];

    for (va, vb, expected_high) in cases {
        let mut ckt = Circuit::new();
        let n_vdd = ckt.get_or_add_node("vdd");
        let n_a = ckt.get_or_add_node("a");
        let n_b = ckt.get_or_add_node("b");
        let n_out = ckt.get_or_add_node("out");

        ckt.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, 5.0);
        ckt.add_voltage_source("Va", n_a, NodeId::GROUND, va);
        ckt.add_voltage_source("Vb", n_b, NodeId::GROUND, vb);

        ckt.add_cmos_nor("NOR1", n_a, n_b, n_out, n_vdd, NodeId::GROUND)
            .expect("nor instantiated");

        let state = dc_op(&ckt).expect("nor dc solve succeeded");
        let v_out = state.voltage(n_out);

        if expected_high {
            assert!(
                v_out > 4.8,
                "NOR input (Va={}, Vb={}) should be HIGH (> 4.8V), got {:.3}V",
                va,
                vb,
                v_out
            );
        } else {
            assert!(
                v_out < 0.2,
                "NOR input (Va={}, Vb={}) should be LOW (< 0.2V), got {:.3}V",
                va,
                vb,
                v_out
            );
        }
    }
}

#[test]
fn cmos_and_compound_truth_table() {
    // Compound AND (NAND + Inverter):
    // 0, 0 => 0
    // 0, 1 => 0
    // 1, 0 => 0
    // 1, 1 => 1
    let cases = [
        (0.0, 0.0, false),
        (0.0, 5.0, false),
        (5.0, 0.0, false),
        (5.0, 5.0, true),
    ];

    for (va, vb, expected_high) in cases {
        let mut ckt = Circuit::new();
        let n_vdd = ckt.get_or_add_node("vdd");
        let n_a = ckt.get_or_add_node("a");
        let n_b = ckt.get_or_add_node("b");
        let n_out = ckt.get_or_add_node("out");

        ckt.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, 5.0);
        ckt.add_voltage_source("Va", n_a, NodeId::GROUND, va);
        ckt.add_voltage_source("Vb", n_b, NodeId::GROUND, vb);

        ckt.add_cmos_and("AND1", n_a, n_b, n_out, n_vdd, NodeId::GROUND)
            .expect("and instantiated");

        let state = dc_op(&ckt).expect("and dc solve succeeded");
        let v_out = state.voltage(n_out);

        if expected_high {
            assert!(
                v_out > 4.8,
                "AND input (Va={}, Vb={}) should be HIGH (> 4.8V), got {:.3}V",
                va,
                vb,
                v_out
            );
        } else {
            assert!(
                v_out < 0.2,
                "AND input (Va={}, Vb={}) should be LOW (< 0.2V), got {:.3}V",
                va,
                vb,
                v_out
            );
        }
    }
}
