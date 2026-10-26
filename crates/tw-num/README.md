# tw-num

Domain-agnostic numerical foundation for the Tech World Engine (Phase 6).

## Principles
- **Domain-agnostic**: No knowledge of `Component`, `Event`, `Circuit`, `Transistor`, etc. Only linear algebra and numerical methods.
- **Independent**: Can be tested without `tw-core` or `tw-sim`.
- **Learn from ngspice, don't clone**: Inspired by `ngspice-47/src/maths/` (dense LU, sparse factorization ideas) but implemented idiomatically in Rust.
- **Minimal first**: Dense LU with partial pivoting → resistor divider validation → RC transient (Backward Euler).

## Scope (current)
- Dense `Matrix`/`Vector` (row-major `Vec<f64>`)
- LU factorization with partial pivoting (`P*A = L*U`)
- `solve(A, b)` via forward/back substitution
- Deterministic, f64 only

## Out of scope (for now)
- Sparse matrices/ordering (Markowitz/AMD)
- Newton-Raphson, nonlinear devices
- BDF/adaptive timestep/LTE
- Complex matrices
- Integration with engine

## Usage
```rust
use tw_num::prelude::*;

let a = Matrix::from_rows(vec![
    vec![4.0, 3.0],
    vec![6.0, 3.0],
]).unwrap();
let b = Vector::from_slice(&[10.0, 12.0]);
let x = solve(&a, &b).unwrap();
// a*x ≈ b
```

## Testing
```bash
cargo test -p tw-num
```

## Validation
- Unit tests for LU/solve
- `resistor_divider` test builds MNA manually and compares against analytic solution
- `rc_transient_be` compares to analytic `V(t)=VDD*(1-exp(-t/(RC)))`
