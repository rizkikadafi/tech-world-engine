use tw_num::prelude::*;

#[test]
fn solve_identity() {
    let a = Matrix::identity(3).unwrap();
    let b = Vector::from_slice(&[1.0, 2.0, 3.0]);
    let x = solve(&a, &b).unwrap();
    assert!((x.as_slice()[0] - 1.0).abs() < 1e-12);
    assert!((x.as_slice()[1] - 2.0).abs() < 1e-12);
    assert!((x.as_slice()[2] - 3.0).abs() < 1e-12);
}

#[test]
fn solve_2x2() {
    // [[4,3],[6,3]] * [x1,x2]^T = [10,12]^T -> x=[0.5, 3.0]
    let a = Matrix::from_rows(vec![vec![4.0, 3.0], vec![6.0, 3.0]]).unwrap();
    let b = Vector::from_slice(&[10.0, 12.0]);
    let x = solve(&a, &b).unwrap();

    // Check A*x ≈ b
    let r1 = a.get(0, 0).unwrap() * x.as_slice()[0] + a.get(0, 1).unwrap() * x.as_slice()[1];
    let r2 = a.get(1, 0).unwrap() * x.as_slice()[0] + a.get(1, 1).unwrap() * x.as_slice()[1];
    assert!((r1 - b.as_slice()[0]).abs() < 1e-12);
    assert!((r2 - b.as_slice()[1]).abs() < 1e-12);
}

#[test]
fn solve_3x3() {
    let a = Matrix::from_rows(vec![
        vec![3.0, 1.0, 1.0],
        vec![1.0, 3.0, 1.0],
        vec![1.0, 1.0, 3.0],
    ])
    .unwrap();
    let b = Vector::from_slice(&[6.0, 6.0, 6.0]);
    let x = solve(&a, &b).unwrap();
    for i in 0..3 {
        let mut r = 0.0f64;
        for j in 0..3 {
            r += a.get(i, j).unwrap() * x.as_slice()[j];
        }
        assert!((r - b.as_slice()[i]).abs() < 1e-12);
    }
}

#[test]
fn solve_diagonal() {
    let a = Matrix::from_rows(vec![vec![2.0, 0.0], vec![0.0, 5.0]]).unwrap();
    let b = Vector::from_slice(&[4.0, 15.0]);
    let x = solve(&a, &b).unwrap();
    assert!((x.as_slice()[0] - 2.0).abs() < 1e-12);
    assert!((x.as_slice()[1] - 3.0).abs() < 1e-12);
}
