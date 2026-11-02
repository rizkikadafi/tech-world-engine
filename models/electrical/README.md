# tw-electrical

Domain electrical model & automated MNA (Modified Nodal Analysis) solver for Tech World Engine (Phase 7, 8, 9, 10, 11, and digital primitives).

## Principles
- **Domain-Specific**: Models concepts of electricity: `NodeId`, `Resistor`, `Capacitor`, `VoltageSource`, `CurrentSource`, `Diode`, `Mosfet` (NMOS/PMOS).
- **Hierarchical Composition**: `Subcircuit` templates can nest other subcircuits, so gates compose into adders, latches, and flip-flops without changing the solver.
- **Pure Dependency Direction**: Depends on `tw-core` and `tw-num`. Neither `tw-core` nor `tw-num` depends on `tw-electrical`. The standalone circuit solver uses `tw-num`; `tw-core` is reserved for the mixed-signal bridge.
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

### Nested Subcircuits (Phase 11 extension)
`Subcircuit` can contain other subcircuits. Internal nodes and device names are
prefixed per instance, so the same template can be instantiated many times
without collisions.

```rust
use tw_electrical::prelude::*;

let config = CmosGateConfig::default();

// A reusable AND gate built from a NAND and an inverter.
let mut and_gate = Subcircuit::new("my_and", ["A", "B", "OUT", "VDD", "GND"]);
and_gate.add_subcircuit(
    "NAND1",
    cmos_nand(config),
    &[("A", "A"), ("B", "B"), ("OUT", "nand_out"),
      ("VDD", "VDD"), ("GND", "GND")],
);
and_gate.add_subcircuit(
    "INV1",
    cmos_inverter(config),
    &[("IN", "nand_out"), ("OUT", "OUT"),
      ("VDD", "VDD"), ("GND", "GND")],
);

let mut ckt = Circuit::new();
let n_vdd = ckt.get_or_add_node("vdd");
let n_a = ckt.get_or_add_node("a");
let n_b = ckt.get_or_add_node("b");
let n_out = ckt.get_or_add_node("out");
ckt.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, 5.0);
ckt.add_voltage_source("Va", n_a, NodeId::GROUND, 5.0);
ckt.add_voltage_source("Vb", n_b, NodeId::GROUND, 5.0);

and_gate.instantiate(
    &mut ckt,
    "AND1",
    &[("A", n_a), ("B", n_b), ("OUT", n_out),
      ("VDD", n_vdd), ("GND", NodeId::GROUND)],
).unwrap();

let state = dc_op(&ckt).unwrap();
assert!(state.voltage(n_out) > 4.8);
```

### Digital Primitives
Combinational and sequential blocks are composed from the CMOS gate library:

| Builder | Ports |
| --- | --- |
| `cmos_xor(config)` | `A, B, OUT, VDD, GND` |
| `cmos_half_adder(config)` | `A, B, SUM, CARRY, VDD, GND` |
| `cmos_full_adder(config)` | `A, B, CIN, SUM, COUT, VDD, GND` |
| `cmos_d_latch(config)` | `D, EN, Q, Q_BAR, VDD, GND` |
| `cmos_dff(config)` | `D, CLK, Q, Q_BAR, VDD, GND` |

```rust
use tw_electrical::prelude::*;

let mut ckt = Circuit::new();
let n_vdd = ckt.get_or_add_node("vdd");
let n_a = ckt.get_or_add_node("a");
let n_b = ckt.get_or_add_node("b");
let n_sum = ckt.get_or_add_node("sum");
let n_carry = ckt.get_or_add_node("carry");

ckt.add_voltage_source("Vdd", n_vdd, NodeId::GROUND, 5.0);
ckt.add_voltage_source("Va", n_a, NodeId::GROUND, 5.0);
ckt.add_voltage_source("Vb", n_b, NodeId::GROUND, 5.0);

cmos_half_adder(CmosGateConfig::default())
    .instantiate(
        &mut ckt,
        "HA1",
        &[("A", n_a), ("B", n_b), ("SUM", n_sum), ("CARRY", n_carry),
          ("VDD", n_vdd), ("GND", NodeId::GROUND)],
    )
    .unwrap();

let state = dc_op(&ckt).unwrap();
// 1 + 1 => SUM=0, CARRY=1
assert!(state.voltage(n_sum) < 0.2);
assert!(state.voltage(n_carry) > 4.8);
```

The D latch and D flip-flop add load capacitance to their storage nodes
(`CmosGateConfig::load_capacitance`, default `1 pF`) so that they retain charge
between transient phases. The DFF is master-slave and positive-edge-triggered.

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

Test suites:
- `tests/resistor_networks.rs` — Ohm's law, dividers, ladder, Wheatstone bridge.
- `tests/rc_circuits.rs` — RC charge/discharge versus analytical solutions.
- `tests/diode_circuits.rs` — forward/reverse biased diodes.
- `tests/mosfet_circuits.rs` — NMOS/CMOS inverter and transient discharge.
- `tests/cmos_gates.rs` — NAND/NOR/AND truth tables.
- `tests/sr_latch.rs` — bistable memory retention.
- `tests/subcircuit_nesting.rs` — nested instantiation and binding validation.
- `tests/digital_combinational.rs` — XOR, half adder, and full adder truth tables.
- `tests/digital_sequential.rs` — D latch transparency/hold and DFF rising-edge capture.
