//! RC charging with Backward Euler (fixed timestep).
//! Circuit: Vsrc step to VDD at t=0, R in series to node, C to GND.
//! Differential: C*dV/dt + (V - Vsrc)/R = 0
//! Backward Euler: C*(V_{n+1}-V_n)/dt + (V_{n+1}-Vsrc)/R = 0
//! -> V_{n+1}(C/dt + 1/R) = C/dt*V_n + Vsrc/R
//! Solution: V(t) = VDD * (1 - exp(-t/(R*C)))

use tw_num::prelude::*;

#[test]
fn rc_charging_backward_euler() {
    let vdd = 5.0f64;
    let r = 1000.0f64;
    let c = 1e-6f64;
    let tau = r * c;
    let dt = 1e-5f64; // 10us steps
    let t_end = 5.0 * tau;
    let steps = ((t_end / dt) as usize).max(1);

    // Backward Euler: (C/dt + 1/R) * V_{n+1} = C/dt * V_n + Vsrc/R, Vsrc = VDD for t>=0
    let g_eq = c / dt + 1.0 / r;
    let a = Matrix::from_rows(vec![vec![g_eq]]).unwrap();
    let mut v = 0.0f64;
    for step in 0..steps {
        let t = (step as f64 + 1.0) * dt;
        let rhs = (c / dt) * v + vdd / r;
        let b = Vector::from_slice(&[rhs]);
        let x = solve(&a, &b).unwrap();
        v = x.as_slice()[0];
        // Check against analytic at this time
        let v_analytic = vdd * (1.0 - (-t / tau).exp());
        // Loose tolerance for coarse fixed step (Backward Euler is first order)
        assert!(
            (v - v_analytic).abs() < 2e-2,
            "step {} t={} v={} analytic={}",
            step,
            t,
            v,
            v_analytic
        );
    }
    // At end should be very close to VDD
    let v_final_analytic = vdd * (1.0 - (-(t_end) / tau).exp());
    assert!((v - v_final_analytic).abs() < 5e-3);
}
