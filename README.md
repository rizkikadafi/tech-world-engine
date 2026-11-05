# Tech World Engine

A domain-agnostic, discrete-event simulation engine written in Rust.

The engine knows nothing about CPUs, RAM, traffic lights, or any other domain.
Domain concepts are modeled as *components*, *ports*, *connections*, and
*resources* built on top of the generic primitives in `tw-core`.

## Crates

| Crate | Role |
| --- | --- |
| `tw-core` | Pure primitives: time, state, events, components, ports, connections, resources, observation, and causal tracing. **No** simulation loop. |
| `tw-sim` | The engine that runs it: `Simulation`, `SimulationEngine`, shared scenarios, checkpointing, and configuration. |
| `tw-scene` | Declarative world layer: `.twscene` schema (JSON), `Registry` for user component factories, and `Session` client façade for CLI & Godot. |
| `tw-cli` | Thin command-line interface. Contains **no** simulation logic. |
| `tw-num` | Domain-agnostic numerical foundation (Phase 6): Dense LU, matrix/vector, linear and nonlinear solve. |
| `tw-electrical` | Domain electrical models (Phase 7–12): Circuit netlist, automated MNA stamping, DC OP/transient solvers, CMOS gates, digital primitives (adders, D latch, DFF), automated characterizer, derived discrete components, a mixed-signal `CircuitComponent` bridge, and construction-time fidelity selection/refinement. |

The dependency direction is one-way:

```text
tw-cli  ──►  tw-scene  ──►  tw-sim  ──►  tw-core
(Godot) ──►  tw-scene                      ▲
                                            │
models/electrical ──────────────────────────┴──► tw-num
```

## Quick start

```bash
cargo build
cargo test

# Run declarative .twscene worlds
cargo run -p tw-cli -- list-kinds
cargo run -p tw-cli -- run scenes/counter.twscene --ticks 50
cargo run -p tw-cli -- run scenes/logic_gates.twscene
cargo run -p tw-cli -- run scenes/disk_service.twscene
cargo run -p tw-cli -- inspect scenes/logic_gates.twscene
cargo run -p tw-cli -- trace scenes/logic_gates.twscene --component 2

# Builtin scenarios
cargo run -p tw-cli -- list
cargo run -p tw-cli -- run counter
cargo run -p tw-cli -- run resource-contention
cargo run -p tw-cli -- run disk-service
cargo run -p tw-cli -- inspect full-adder

# Build your own component with the public engine API.
cargo run -p tw-sim --example custom_component

# Multi-fidelity: measure transistor circuits (inverter, NAND2, DFF),
# derive discrete components, then run the analog bridge and the derived
# components side by side.
cargo run -p tw-electrical --example multi_fidelity

# Select a fidelity at construction time and refine it toward more detail.
cargo run -p tw-electrical --example fidelity_selection
```

## Configuration

Engine-wide settings (tracing mode, event cap, seed) live in
`tw_sim::SimulationConfig`. The CLI reads overrides from the environment:

| Variable | Meaning | Default |
| --- | --- | --- |
| `RUST_LOG` | Log filter for the engine, e.g. `error` / `warn` / `info` / `debug` / `trace`. | off |
| `TW_MAX_EVENTS` | Safety cap on the number of executed events. | unlimited |
| `TW_SEED` | RNG seed, reserved for future stochastic models. | `0` |

Example — trace every executed event, but stop after 5:

```bash
RUST_LOG=tw_sim=trace TW_MAX_EVENTS=5 cargo run -p tw-cli -- run counter
```

## Repository layout

```text
crates/      workspace members (tw-core, tw-sim, tw-cli, tw-num)
models/      domain models built on the engine (tw-electrical)
docs/        design documents (see below)
```

## Documentation

> 📖 **[Tech World Engine Manual & Architecture Guide (`docs/manual/`)](docs/manual/README.md)**  
> Comprehensive structured guide covering Vision & Philosophy, Crate Architecture, Custom Component Authoring, Declarative `.twscene` & CLI, Physics & Multi-Fidelity, and Godot GDExtension Integration.

| Document | Contents |
| --- | --- |
| `docs/manual/README.md` | **Official structured manual & reading guide (Recommended)**. |
| `docs/implementation.md` | What is actually implemented today, and its limits. |
| `docs/custom-components.md` | Guide to implementing your own components on the public `tw-core` / `tw-sim` API. |
| `docs/engine.md` | Roadmap variant — Phase 0 to 12. |
| `docs/engine-v1.md` | Roadmap variant — Phase 0 to 16, including the Godot phase. |
| `docs/plan.md` | Architecture and system design. |
| `docs/godot-integration.md` | How the engine will be integrated into Godot (plan). |
| `docs/time-model.md` | Time model: fixed-point ticks, limits, sync options. |
| `docs/numerical/overview.md` | Numerical foundation (Phase 6) — map. |
| `docs/numerical/basics_resistor_rc.md` | Basic theory: Resistor, MNA, and RC transient (Backward Euler). |
| `docs/electrical-simulation.md` | Electrical domain models, automated MNA stamping, CMOS gates, digital primitives, and the multi-fidelity characterizer / derived component / mixed-signal bridge / fidelity selection (Phase 7–12). |

## Benchmarks

```bash
cargo bench -p tw-sim
```

This measures event throughput as the number of components grows
(10 / 100 / 1K / 10K), giving a baseline for the fidelity-vs-performance
trade-off.

Measure the cost of one NAND switching event at each fidelity:

```bash
cargo bench -p tw-electrical --bench fidelity_scaling -- --quick
```

The 1M-component stress test is opt-in:

```bash
cargo test --release -p tw-electrical --test fidelity_scale -- --ignored --nocapture
```
