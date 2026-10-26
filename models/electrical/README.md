# tw-electrical

Domain electrical model & automated MNA (Modified Nodal Analysis) solver for Tech World Engine (Phase 7 & 8).

## Principles
- **Domain-Specific**: Models concepts of electricity: `NodeId`, `Resistor`, `Capacitor`, `VoltageSource`, `CurrentSource`.
- **Pure Dependency Direction**: Depends on `tw-core` and `tw-num`. Neither `tw-core` nor `tw-num` depends on `tw-electrical`.
- **Automated MNA Stamping**: Translates circuit netlists into MNA system equations ($[G\ B;\ C\ D] [v;\ j] = [i;\ e]$) and uses `tw_num::linear::solve` to compute node voltages and branch currents.
- **Analysis Modes**:
  - `dc_op(&circuit)`: DC Operating Point analysis (capacitors open-circuit).
  - `transient(&circuit, t_stop, dt, initial_state)`: Dynamic time-stepping with Backward Euler capacitor companion models.

## Usage

```rust
use tw_electrical::prelude::*;

let mut ckt = Circuit::new();
let n_in = ckt.get_or_add_node("in");
let n_mid = ckt.get_or_add_node("mid");

// 5V source, 2k and 3k divider
ckt.add_voltage_source("Vdd", n_in, NodeId::GROUND, 5.0);
ckt.add_resistor("R1", n_in, n_mid, 2000.0);
ckt.add_resistor("R2", n_mid, NodeId::GROUND, 3000.0);

let state = dc_op(&ckt).unwrap();
assert_eq!(state.voltage(n_mid), 3.0);
```

## Testing
```bash
cargo test -p tw-electrical
```
