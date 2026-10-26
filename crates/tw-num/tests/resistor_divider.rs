//! Validation test: Resistor divider solved via MNA assembled manually.
//!
//! Circuit: V1 (5V) -- R1 -- node1 -- R2 -- GND, R1=R2=1k
//! Expected: V(node1) = VDD * R2 / (R1 + R2) = 2.5V
//!
//! MNA (Modified Nodal Analysis) for voltage source to ground:
//! For node1: (V1 - Vnode1)/R1 + (GND - Vnode1)/R2 + I? Actually simpler form:
//! Conductance matrix G: G[0,0] = 1/R1 + 1/R2, G[0,1] = -1/R1, G[1,0] = -1/R1, G[1,1] = 1/R1
//! But easier: treat V1 as independent source, solve for node voltage.
//! Alternatively use the standard form: [1/R1+1/R2] * V1n = VDD/R1
//! Here: G11*V1 = I1 (sum of currents leaving node) = VDD/R1 (since VDD connected via R1)
//! Yes.

use tw_num::prelude::*;

#[test]
fn resistor_divider_equal() {
    let vdd = 5.0f64;
    let r1 = 1000.0f64;
    let r2 = 1000.0f64;
    let g1 = 1.0 / r1;
    let g2 = 1.0 / r2;
    // G * V = I, G = [g1+g2], I = [g1 * vdd]
    let a = Matrix::from_rows(vec![vec![g1 + g2]]).unwrap();
    let b = Vector::from_slice(&[g1 * vdd]);
    let x = solve(&a, &b).unwrap();
    let v_node = x.as_slice()[0];
    let v_expected = vdd * r2 / (r1 + r2);
    assert!(
        (v_node - v_expected).abs() < 1e-12,
        "got {}, expected {}",
        v_node,
        v_expected
    );
}

#[test]
fn resistor_divider_different() {
    let vdd = 5.0f64;
    let r1 = 2000.0f64;
    let r2 = 1000.0f64;
    let g1 = 1.0 / r1;
    let g2 = 1.0 / r2;
    let a = Matrix::from_rows(vec![vec![g1 + g2]]).unwrap();
    let b = Vector::from_slice(&[g1 * vdd]);
    let x = solve(&a, &b).unwrap();
    let v_node = x.as_slice()[0];
    let v_expected = vdd * r2 / (r1 + r2);
    assert!(
        (v_node - v_expected).abs() < 1e-12,
        "got {}, expected {}",
        v_node,
        v_expected
    );
}

#[test]
fn resistor_divider_two_nodes_mna() {
    // Two nodes: V1 connected to VDD via R1, V2 connected to V1 via R2, V2 connected to GND via R3
    // Simpler: VDD --R1-- node1 --R2-- GND (single node) done above.
    // Alternative 2x2 form:
    // G11*V1 + G12*V2 = I1, G21*V1 + G22*V2 = I2
    // Let us do: VDD(5V) feeds R1 to node1, node1 feeds R2 to node2, node2 to GND
    let vdd = 5.0f64;
    let r1 = 1000.0f64;
    let r2 = 1000.0f64;
    let r3 = 1000.0f64;
    let g1 = 1.0 / r1;
    let g2 = 1.0 / r2;
    let g3 = 1.0 / r3;
    // node1: (V1-Vdd)/R1 + (V1-V2)/R2 = 0 -> (g1+g2)V1 - g2*V2 = g1*Vdd
    // node2: (V2-V1)/R2 + (V2)/R3 = 0 -> -g2*V1 + (g2+g3)V2 = 0
    let a = Matrix::from_rows(vec![
        vec![g1 + g2, -g2],
        vec![-g2, g2 + g3],
    ])
    .unwrap();
    let b = Vector::from_slice(&[g1 * vdd, 0.0]);
    let x = solve(&a, &b).unwrap();
    let v1 = x.as_slice()[0];
    let v2 = x.as_slice()[1];
    // Expected: V2 = VDD * R3 / (R1+R2+R3) in some forms? No, current same: I = VDD/(R1+R2+R3), V2 = I*R3
    let v2_exp = vdd * r3 / (r1 + r2 + r3);
    assert!((v2 - v2_exp).abs() < 1e-12, "v2 got {} exp {}", v2, v2_exp);
    // V1 = VDD - I*R1
    let v1_exp = vdd - (vdd / (r1 + r2 + r3)) * r1;
    assert!((v1 - v1_exp).abs() < 1e-12, "v1 got {} exp {}", v1, v1_exp);
}
