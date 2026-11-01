# tw-electrical

Domain electrical model & automated MNA (Modified Nodal Analysis) solver for Tech World Engine (Phase 7, 8, 9, 10 & 11).

## Principles
- **Domain-Specific**: Models concepts of electricity: `NodeId`, `Resistor`, `Capacitor`, `VoltageSource`, `CurrentSource`, `Diode`, `Mosfet` (NMOS/PMOS).
- **Hierarchical Composition**: Modular `Subcircuit` templates for logic gates and compound blocks.
- **Pure Dependency Direction**: Depends on `tw-core` and `tw-num`. Neither `tw-core` nor `tw-num` depends on `tw-electrical`.
- **Automated MNA Stamping**: Translates circuit netlists into MNA system equations ($[G\ B;\ C\ D] [v;\ j] = [i;\ e]$) and uses `tw_num::linear::solve` or `tw_num::nonlinear::NewtonSolver` to compute node voltages and branch currents.
- **Analysis Modes**:
  - `dc_op(&circuit)`: DC Operating Point analysis (linear solve or Newton-Raphson nonlinear loop).
  - `transient(&circuit, t_stop, dt, initial_state)`: Dynamic time-stepping with Backward Euler companion models.

## Usage

### Linear Circuit (Voltage Divider)
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

### CMOS Logic Gates & Subcircuits (Phase 11)
```rust
use tw_electrical::prelude::*;

let mut ckt = Circuit::new();
let n_vdd = ckt.get_or_add_node("vdd");
let n_a = ckt.get_or_add_node("a");
let n_b = ckt.get_or_add_node("b");
let n_out = ckt.get_or_add_node("out");

ckt.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, 5.0);
ckt.add_voltage_source("Va", n_a, NodeId::GROUND, 5.0);
ckt.add_voltage_source("Vb", n_b, NodeId::GROUND, 5.0);

// Modular CMOS NAND gate instantiated from transistors
ckt.add_cmos_nand("NAND1", n_a, n_b, n_out, n_vdd, NodeId::GROUND).unwrap();

let state = dc_op(&ckt).unwrap();
// 1 AND 1 NANDed => 0 (~0.0V)
assert!(state.voltage(n_out) < 0.2);
```

### Transistor-Level Circuit (CMOS Inverter)
```rust
use tw_electrical::prelude::*;

let mut ckt = Circuit::new();
let n_vdd = ckt.get_or_add_node("vdd");
let n_in = ckt.get_or_add_node("in");
let n_out = ckt.get_or_add_node("out");

ckt.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, 5.0);
ckt.add_voltage_source("Vin", n_in, NodeId::GROUND, 0.0); // Logic 0

// PMOS pull-up, NMOS pull-down
ckt.add_pmos("Mp", n_out, n_in, n_vdd, 0.8, 1e-3);
ckt.add_nmos("Mn", n_out, n_in, NodeId::GROUND, 0.8, 1e-3);

let state = dc_op(&ckt).unwrap();
// Output inverted to Logic 1 (~5.0V)
assert!((state.voltage(n_out) - 5.0).abs() < 0.05);
```

## Testing
```bash
cargo test -p tw-electrical
```
