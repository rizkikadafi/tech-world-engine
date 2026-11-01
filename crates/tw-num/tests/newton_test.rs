use tw_num::prelude::*;

#[test]
fn newton_scalar_sqrt2() {
    let f = |x: f64| x * x - 2.0;
    let df = |x: f64| 2.0 * x;

    let root = newton_scalar(1.0, f, df, 1e-10, 20).expect("scalar solve succeeded");
    assert!((root - 2.0f64.sqrt()).abs() < 1e-10);
}

#[test]
fn newton_system_circle_and_line() {
    let solver = NewtonSolver::new();

    // Circle: x1^2 + x2^2 = 4
    // Line:   x1 - x2 = 0
    let eval_f = |x: &Vector| {
        let f1 = x[0] * x[0] + x[1] * x[1] - 4.0;
        let f2 = x[0] - x[1];
        Ok(Vector::from_slice(&[f1, f2]))
    };

    let eval_j = |x: &Vector| {
        let rows = vec![
            vec![2.0 * x[0], 2.0 * x[1]],
            vec![1.0, -1.0],
        ];
        Matrix::from_rows(rows)
    };

    let x0 = Vector::from_slice(&[1.0, 1.0]);
    let result = solver.solve(&x0, eval_f, eval_j).expect("2D solve succeeded");

    let expected = 2.0f64.sqrt();
    assert!((result.x[0] - expected).abs() < 1e-6);
    assert!((result.x[1] - expected).abs() < 1e-6);
    assert!(result.iterations < 10);
    assert!(result.residual_norm < 1e-6);
}
