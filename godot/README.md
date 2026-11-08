# Tech World Engine — Godot Demo

Thin GDExtension adapter (`crates/tw-godot`) plus a minimal Godot 4.7 project.

The engine owns all simulation semantics. Godot only loads worlds, pulls state
diffs, renders, and injects stimuli.

## Prerequisites

- Godot **4.7.x** (matches `api-4-7` and `compatibility_minimum = "4.7"`).
- Rust **1.95.0** (pinned by `rust-toolchain.toml`; `godot` 0.5.5 requires ≥1.94).
- No Godot binary is needed to compile the Rust crate.

## Build

```bash
./godot/build.sh            # debug
./godot/build.sh release    # release
```

This produces `target/{debug,release}/libtw_godot.so` and copies it into
`godot/addons/tw_godot/bin/` with the name expected by `tw_godot.gdextension`.

Manual equivalent:

```bash
cargo build -p tw-godot --release
mkdir -p godot/addons/tw_godot/bin
cp target/release/libtw_godot.so \
   godot/addons/tw_godot/bin/libtw_godot.linux.release.x86_64.so
```

## Run the demo

```bash
godot --path godot
```

Open `scenes/demo.tscn` if it is not the main scene. The demo loads
`res://worlds/counter.twscene`, advances the engine from Godot's frame delta, and
shows `count` changes through `take_state_changes()`.

## Circuit editor

`scenes/schematic_editor.tscn` is a playable 2D schematic editor driven by
`SchematicHost` (`tw-schematic`):

```bash
godot --path godot res://scenes/schematic_editor.tscn
```

It opens with a voltage-divider sample. Controls:

- pick a part from the left palette, then click the canvas to place it,
- click a pin, then another pin, to draw a wire,
- drag parts to move them; `Delete` removes the selected part or wire,
- `Ctrl+Z` / `Ctrl+Y` undo and redo,
- `Run DC` solves the circuit and colours pins/wires by node voltage,
- the right panel edits parameters (`resistance`, `voltage`, `closed`, ...),
- the bottom panel lists validation errors and warnings,
- `Load` / `Save` use the path field (`user://` and `res://` work).

The editor never mutates the solver directly: every click becomes a
`tw_schematic::Command`, and the rendered view is a snapshot pulled from Rust.

## Headless smoke test

On a fresh checkout, import the project once so Godot registers the
GDExtension (`SimulationHost`), then run the test:

```bash
godot --headless --path godot --import
godot --headless --path godot --script res://tests/smoke.gd
godot --headless --path godot --script res://tests/schematic_smoke.gd
```

Without the import step, GDScript fails with
`Identifier "SimulationHost" not declared in the current scope` (and likewise
for `SchematicHost`).

Covered checks:

- load `.twscene` through `FileAccess` (`res://` works in exported PCK builds),
- `advance_ticks`, `current_tick`, and state diffing,
- checkpoint JSON round-trip,
- causal tracing (`last_trace_for` / `explain`),
- port inspection and `inject_signal`,
- rejection of unsupported `Variant` types.

`schematic_smoke.gd` additionally covers:

- the builtin part catalog and default parameters,
- building and solving a voltage divider through `SchematicHost`,
- parameter edits invalidating the DC cache and re-solving,
- undo/redo and invalid edit rejection,
- JSON / `user://` persistence and wire removal.

## API surface (GDScript)

| Method | Purpose |
| --- | --- |
| `load_world(path) -> Error` | Load `.twscene` (uses Godot `FileAccess` for `res://` / `user://`). |
| `unload_world()`, `has_world()` | Session lifecycle. |
| `set_auto_advance(bool)`, `set_paused(bool)` | Frame-driven vs manual stepping. |
| `advance_ticks(n) -> int` | Deterministic stepping; returns executed event count. |
| `step_frame(delta) -> int` | Manual delta → ticks conversion. |
| `current_tick()`, `ticks_per_second()` | Clock inspection. |
| `component_ids()`, `component_name(id)` | World enumeration. |
| `component_state(id) -> Dictionary` | Current state variables. |
| `component_ports(id) -> Array[Dictionary]` | `{id, name, direction}`. |
| `resource_ids()`, `resource_snapshots()` | Resource inspection. |
| `take_state_changes() -> Array[Dictionary]` | Pull diff since previous call. |
| `inject_signal(id, port, value) -> Error` | Drive a component port. |
| `inject_tick(id) -> Error` | Send a generic tick event. |
| `last_trace_for(id)`, `traces_for(id)`, `explain(trace_id)` | Causality. |
| `checkpoint_json()`, `restore_json(json)` | Shared checkpoint format with the CLI. |
| `set_tracing_mode(0|1|2)` | Off / abstract / hierarchical. |

## Schematic API surface (`SchematicHost`)

| Method | Purpose |
| --- | --- |
| `new_document(name)`, `unload_document()`, `has_document()` | Document lifecycle. |
| `load_document(path)`, `save_document(path)` | JSON `.twsch` files (`res://` / `user://` supported). |
| `document_json()`, `load_document_json(json)` | In-memory JSON round-trip. |
| `document_name()`, `set_document_name(name)` | Document metadata. |
| `part_kinds() -> Array[Dictionary]` | Builtin kinds with pins and default params. |
| `add_part(id, kind, x, y)`, `remove_part(id)`, `move_part(id, x, y)` | Placement. |
| `set_param(id, key, value)`, `clear_param(id, key)`, `rename_part(id, label)` | Parameter editing. |
| `connect_pins(from_part, from_pin, to_part, to_pin) -> String` | Returns the wire id, or `""` on failure. |
| `disconnect_wire(id)`, `apply_command_json(json)` | Wire removal and generic commands. |
| `undo()`, `redo()`, `can_undo()`, `can_redo()`, `revision()` | History. |
| `snapshot() -> Dictionary` | `{name, revision, parts, wires, can_undo, can_redo}`. |
| `diagnostics() -> Array[Dictionary]` | Errors and warnings with `severity/code/message`. |
| `compile()`, `run_dc()`, `has_state()` | Compile and solve. |
| `state_snapshot() -> Dictionary` | `{pins: {"part:pin": V}, probes: {id: V}, nets: {name: V}}`. |

## Limitations

- `restore_json()` restores component states and resources only. It does **not**
  rewind the simulation clock or the pending event queue; restoring mid-run
  resumes at the current tick.
- `Registry::builtin()` currently registers only `Counter`, `Gate`, `Probe`,
  `Worker`, and `DiskJob`. Electrical components are not scene-registerable yet.
- Schematic authoring is a separate lane: `tw-scene` intentionally knows nothing
  about parts, wires, nets, or MOSFETs. `SchematicHost` talks to `tw-schematic`
  directly, which compiles to `tw-electrical` when you press `Run DC`.
- The adapter is a thin bridge; all determinism, simulation rules, validation,
  and compilation live in the Rust crates.

## State flow

```text
Godot _process(delta)
   └─> SimulationHost::process   (only when auto_advance && !paused)
         └─> TickClock: delta seconds -> u64 ticks
               └─> Session::step(ticks)        (tw-sim)
GDScript _process
   └─> take_state_changes() -> apply visuals    (pull, not push)
User input
   └─> inject_signal / inject_tick -> next ticks
```

State is pulled instead of pushed so a Rust `#[func]` never calls back into
GDScript while holding a mutable borrow of the session.

Schematic editing uses the same pull model:

```text
GDScript click / drag
   └─> SchematicHost::add_part / move_part / connect_pins / set_param
         └─> SchematicSession::apply(Command)   (undo history, revision++)
GDScript after each edit
   └─> snapshot() + diagnostics() -> redraw
Run DC
   └─> SchematicHost::run_dc() -> compile to tw-electrical -> dc_op
         └─> state_snapshot() -> pin/probe voltages
```
