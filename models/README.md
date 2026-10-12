# models/

Domain-specific models (CPUs, memory, storage, networks, …) belong here.

The core engine is intentionally **domain-agnostic**: `tw-core` exposes only
generic primitives — time, state, events, components, ports, connections,
resources, observation, and causal tracing. Anything that names a specific
domain concept is a *model*, not part of the engine.

## Rules

- Models depend on `tw-core` (and optionally `tw-sim`), never the other way
  around; dependency direction stays `models → tw-sim → tw-core`.
- The engine must keep compiling and testing with this directory empty.
- Prefer keeping model state in `State` so checkpointing/observers work.

This directory is a placeholder. Concrete models will be added as their own
crates or modules once the numerical foundation (Phase 6: matrices, sparse
representations, linear/nonlinear solvers) is in place.
