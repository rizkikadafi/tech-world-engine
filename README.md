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
| `tw-cli` | Thin command-line interface. Contains **no** simulation logic. |

The dependency direction is one-way:

```text
tw-cli  →  tw-sim  →  tw-core
```

## Quick start

```bash
cargo build
cargo test

cargo run -p tw-cli -- list
cargo run -p tw-cli -- run counter
cargo run -p tw-cli -- run resource-contention
cargo run -p tw-cli -- run disk-service
cargo run -p tw-cli -- inspect full-adder
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
crates/      workspace members (tw-core, tw-sim, tw-cli)
models/      domain models built on the engine (placeholder for now)
docs/        design documents (see below)
```

## Documentation

| Document | Contents |
| --- | --- |
| `docs/implementation.md` | What is actually implemented today, and its limits. |
| `docs/engine.md` | Roadmap variant — Phase 0 to 12. |
| `docs/engine-v1.md` | Roadmap variant — Phase 0 to 16, including the Godot phase. |
| `docs/plan.md` | Architecture and system design. |
| `docs/godot-integration.md` | How the engine will be integrated into Godot (plan). |
| `docs/time-model.md` | Time model: fixed-point ticks, limits, sync options. |
| `docs/numerical/overview.md` | Numerical foundation (Phase 6) — map. |

## Benchmarks

```bash
cargo bench -p tw-sim
```

This measures event throughput as the number of components grows
(10 / 100 / 1K / 10K), giving a baseline for the fidelity-vs-performance
trade-off.
