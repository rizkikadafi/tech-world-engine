use tw_num::error::NumericalError;
use tw_num::linear::dense::solve as solve_dense;
use tw_num::linear::matrix::Matrix;
use tw_num::linear::sparse::{solve_sparse, SparseMatrix};
use tw_num::linear::vector::Vector;

#[test]
fn test_sparse_identity_3x3() {
    let mut a = SparseMatrix::new(3, 3);
    a.set_element(0, 0, 1.0).unwrap();
    a.set_element(1, 1, 1.0).unwrap();
    a.set_element(2, 2, 1.0).unwrap();

    let b = Vector::from_slice(&[3.0, 7.0, -2.0]);
    let x = solve_sparse(&a, &b).unwrap();

    assert!((x[0] - 3.0).abs() < 1e-12);
    assert!((x[1] - 7.0).abs() < 1e-12);
    assert!((x[2] - (-2.0)).abs() < 1e-12);
}

#[test]
fn test_sparse_diagonal() {
    let mut a = SparseMatrix::new(4, 4);
    a.set_element(0, 0, 2.0).unwrap();
    a.set_element(1, 1, 4.0).unwrap();
    a.set_element(2, 2, -5.0).unwrap();
    a.set_element(3, 3, 10.0).unwrap();

    let b = Vector::from_slice(&[4.0, 8.0, 15.0, 20.0]);
    let x = solve_sparse(&a, &b).unwrap();

    assert!((x[0] - 2.0).abs() < 1e-12);
    assert!((x[1] - 2.0).abs() < 1e-12);
    assert!((x[2] - (-3.0)).abs() < 1e-12);
    assert!((x[3] - 2.0).abs() < 1e-12);
}

#[test]
fn test_sparse_zero_on_diagonal_pivoting() {
    // A = [[0, 2], [3, 4]]
    // det(A) = -6 != 0
    let mut a = SparseMatrix::new(2, 2);
    a.set_element(0, 1, 2.0).unwrap();
    a.set_element(1, 0, 3.0).unwrap();
    a.set_element(1, 1, 4.0).unwrap();

    let b = Vector::from_slice(&[4.0, 10.0]);
    let x = solve_sparse(&a, &b).unwrap();

    // 0*x0 + 2*x1 = 4 => x1 = 2
    // 3*x0 + 4*2 = 10 => 3*x0 = 2 => x0 = 2/3
    assert!((x[0] - 2.0 / 3.0).abs() < 1e-12);
    assert!((x[1] - 2.0).abs() < 1e-12);
}

#[test]
fn test_sparse_vs_dense_tridiagonal_50() {
    // Create a 50x50 tridiagonal system (common in discretized 1D PDE / resistor ladder)
    let n = 50;
    let mut a_sparse = SparseMatrix::new(n, n);
    let mut a_dense = Matrix::zeros(n, n).unwrap();

    for i in 0..n {
        let diag = 4.0;
        a_sparse.set_element(i, i, diag).unwrap();
        a_dense[(i, i)] = diag;

        if i > 0 {
            let left = -1.0;
            a_sparse.set_element(i, i - 1, left).unwrap();
            a_dense[(i, i - 1)] = left;
        }
        if i + 1 < n {
            let right = -1.5;
            a_sparse.set_element(i, i + 1, right).unwrap();
            a_dense[(i, i + 1)] = right;
        }
    }

    let b_vec: Vec<f64> = (0..n).map(|i| (i as f64 + 1.0) * 0.5).collect();
    let b = Vector::from_vec(b_vec);

    let x_sparse = solve_sparse(&a_sparse, &b).unwrap();
    let x_dense = solve_dense(&a_dense, &b).unwrap();

    let mut max_diff = 0.0f64;
    for i in 0..n {
        let diff = (x_sparse[i] - x_dense[i]).abs();
        if diff > max_diff {
            max_diff = diff;
        }
    }

    assert!(
        max_diff < 1e-11,
        "Sparse solution diverges from dense solution! Max diff = {}",
        max_diff
    );
}

#[test]
fn test_sparse_accumulation_mna_style() {
    let mut a = SparseMatrix::new(2, 2);
    // Stamp G1 = 0.5 into (0, 0)
    a.add_element(0, 0, 0.5).unwrap();
    // Stamp G2 = 0.3 into (0, 0)
    a.add_element(0, 0, 0.3).unwrap();
    // Cross elements
    a.add_element(0, 1, -0.2).unwrap();
    a.add_element(1, 0, -0.2).unwrap();
    a.add_element(1, 1, 1.0).unwrap();

    assert!((a.get(0, 0) - 0.8).abs() < 1e-12);
    assert!((a.get(0, 1) - (-0.2)).abs() < 1e-12);

    let b = Vector::from_slice(&[1.6, 2.0]);
    let x = solve_sparse(&a, &b).unwrap();

    // Verify A * x == b
    let ax0 = a.get(0, 0) * x[0] + a.get(0, 1) * x[1];
    let ax1 = a.get(1, 0) * x[0] + a.get(1, 1) * x[1];

    assert!((ax0 - 1.6).abs() < 1e-12);
    assert!((ax1 - 2.0).abs() < 1e-12);
}

#[test]
fn test_sparse_singular_detected() {
    let mut a = SparseMatrix::new(2, 2);
    a.set_element(0, 0, 1.0).unwrap();
    a.set_element(0, 1, 2.0).unwrap();
    a.set_element(1, 0, 2.0).unwrap();
    a.set_element(1, 1, 4.0).unwrap(); // rank 1

    let b = Vector::from_slice(&[1.0, 2.0]);
    let err = solve_sparse(&a, &b).unwrap_err();
    assert!(matches!(err, NumericalError::Singular { .. }));
}
