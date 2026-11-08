//! Thin GDExtension adapter for Tech World Engine.
//!
//! This crate contains **no simulation logic**. It only:
//! - loads `.twscene` worlds through `tw-scene`,
//! - converts Godot frame deltas into integer ticks,
//! - maps `StateValue`/`Port`/`TraceNode` to Godot `Variant`s,
//! - exposes a pull-based inspection API to GDScript.
//!
//! The engine remains fully usable from the CLI without Godot installed.

mod clock;
mod convert;
mod schematic;
mod schematic_convert;

use std::io::{Read, Write};

use godot::classes::file_access::ModeFlags;
use godot::classes::{INode, Node};
use godot::global::Error;
use godot::prelude::*;
use godot::tools::GFile;
use tw_core::{ComponentId, TraceId, TracingMode};
use tw_scene::{Registry, Session, World};

use clock::TickClock;

pub use schematic::SchematicHost;

/// Hard cap so a long editor/debugger pause cannot make one frame execute an
/// unbounded number of simulation events.
const MAX_TICKS_PER_FRAME: u64 = 10_000;

struct TwGodot;

#[gdextension]
unsafe impl ExtensionLibrary for TwGodot {}

/// Godot node that owns one `tw_scene::Session`.
///
/// State changes are **pulled** from GDScript via [`SimulationHost::take_state_changes`].
/// This avoids calling back into GDScript while Rust holds a mutable borrow.
#[derive(GodotClass)]
#[class(base = Node)]
pub struct SimulationHost {
    base: Base<Node>,
    session: Option<Session>,
    clock: TickClock,
    paused: bool,
    auto_advance: bool,
}

#[godot_api]
impl INode for SimulationHost {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            session: None,
            clock: TickClock::new(1000),
            paused: false,
            auto_advance: true,
        }
    }

    fn ready(&mut self) {
        // Be explicit: frame-driven stepping must be enabled for `process` to run.
        self.base_mut().set_process(true);
    }

    fn process(&mut self, delta: f32) {
        if self.paused || !self.auto_advance || self.session.is_none() {
            return;
        }

        let ticks = self.clock.ticks_for(delta as f64).min(MAX_TICKS_PER_FRAME);
        if ticks > 0 {
            self.step_ticks(ticks);
        }
    }
}

impl SimulationHost {
    fn step_ticks(&mut self, ticks: u64) -> u64 {
        let Some(session) = self.session.as_mut() else {
            return 0;
        };
        if ticks == 0 {
            return 0;
        }
        session.step(ticks).events_executed
    }
}

#[godot_api]
impl SimulationHost {
    // --- Loading & control -------------------------------------------------

    /// Load a `.twscene` world.
    ///
    /// `res://` and `user://` paths are read through Godot's `FileAccess`, so
    /// exported PCK builds work too. Other paths are read through the OS.
    #[func]
    fn load_world(&mut self, path: GString) -> Error {
        let path = path.to_string();

        let result = if path.starts_with("res://") || path.starts_with("user://") {
            read_godot_text(&path)
                .and_then(|text| World::from_json_str(&text).map_err(|e| e.to_string()))
                .and_then(|world| {
                    Session::from_world(world, Registry::builtin()).map_err(|e| e.to_string())
                })
        } else {
            Session::load(&path).map_err(|e| e.to_string())
        };

        match result {
            Ok(session) => {
                self.clock = TickClock::new(session.world().ticks_per_second);
                self.session = Some(session);
                self.paused = false;
                Error::OK
            }
            Err(message) => {
                godot_error!("failed to load world '{}': {}", path, message);
                Error::FAILED
            }
        }
    }

    #[func]
    fn unload_world(&mut self) {
        self.session = None;
        self.clock.reset();
    }

    #[func]
    fn has_world(&self) -> bool {
        self.session.is_some()
    }

    #[func]
    fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    #[func]
    fn is_paused(&self) -> bool {
        self.paused
    }

    #[func]
    fn set_auto_advance(&mut self, enabled: bool) {
        self.auto_advance = enabled;
    }

    #[func]
    fn is_auto_advance(&self) -> bool {
        self.auto_advance
    }

    /// Step exactly `ticks` simulation ticks. Returns the number of executed events.
    #[func]
    fn advance_ticks(&mut self, ticks: i64) -> i64 {
        if ticks <= 0 {
            return 0;
        }
        self.step_ticks(ticks as u64) as i64
    }

    /// Manually feed a frame delta (seconds). Useful when `auto_advance` is off.
    #[func]
    fn step_frame(&mut self, delta: f64) -> i64 {
        let ticks = self.clock.ticks_for(delta).min(MAX_TICKS_PER_FRAME);
        self.step_ticks(ticks) as i64
    }

    // --- Time --------------------------------------------------------------

    #[func]
    fn current_tick(&self) -> i64 {
        self.session
            .as_ref()
            .map(|session| session.current_tick() as i64)
            .unwrap_or(0)
    }

    #[func]
    fn ticks_per_second(&self) -> i64 {
        self.clock.ticks_per_second() as i64
    }

    #[func]
    fn set_ticks_per_second(&mut self, ticks_per_second: i64) {
        if ticks_per_second > 0 {
            self.clock.set_ticks_per_second(ticks_per_second as u64);
        }
    }

    #[func]
    fn world_name(&self) -> GString {
        self.session
            .as_ref()
            .map(|session| GString::from(session.world().name.as_str()))
            .unwrap_or_else(|| GString::from(""))
    }

    // --- Inspection --------------------------------------------------------

    #[func]
    fn component_ids(&self) -> Array<i64> {
        let Some(session) = self.session.as_ref() else {
            return Array::new();
        };
        session
            .component_ids()
            .into_iter()
            .map(|id| id.0 as i64)
            .collect()
    }

    #[func]
    fn component_name(&self, component_id: i64) -> GString {
        self.session
            .as_ref()
            .and_then(|session| session.component_name(ComponentId(component_id as u64)))
            .map(GString::from)
            .unwrap_or_else(|| GString::from(""))
    }

    #[func]
    fn component_state(&self, component_id: i64) -> VarDictionary {
        self.session
            .as_ref()
            .and_then(|session| session.component_state(ComponentId(component_id as u64)))
            .map(convert::state_to_dict)
            .unwrap_or_default()
    }

    #[func]
    fn component_ports(&self, component_id: i64) -> Array<VarDictionary> {
        let Some(session) = self.session.as_ref() else {
            return Array::new();
        };
        session
            .component_ports(ComponentId(component_id as u64))
            .map(|ports| ports.iter().map(convert::port_to_dict).collect())
            .unwrap_or_default()
    }

    #[func]
    fn resource_ids(&self) -> Array<i64> {
        let Some(session) = self.session.as_ref() else {
            return Array::new();
        };
        session
            .resource_ids()
            .into_iter()
            .map(|id| id.0 as i64)
            .collect()
    }

    #[func]
    fn resource_snapshots(&self) -> Array<VarDictionary> {
        let Some(session) = self.session.as_ref() else {
            return Array::new();
        };
        session
            .resource_ids()
            .into_iter()
            .filter_map(|id| session.resource_snapshot(id))
            .map(|snapshot| convert::resource_snapshot_to_dict(&snapshot))
            .collect()
    }

    /// Pull all state variables that changed since the previous call.
    #[func]
    fn take_state_changes(&mut self) -> Array<VarDictionary> {
        let Some(session) = self.session.as_mut() else {
            return Array::new();
        };
        session
            .take_state_changes()
            .iter()
            .map(convert::state_change_to_dict)
            .collect()
    }

    // --- Causality ---------------------------------------------------------

    #[func]
    fn explain(&self, trace_id: i64) -> Array<VarDictionary> {
        let Some(session) = self.session.as_ref() else {
            return Array::new();
        };
        session
            .explain(TraceId(trace_id as u64))
            .iter()
            .map(|node| convert::trace_to_dict(node))
            .collect()
    }

    #[func]
    fn traces_for(&self, component_id: i64) -> Array<i64> {
        let Some(session) = self.session.as_ref() else {
            return Array::new();
        };
        session
            .traces_for(ComponentId(component_id as u64))
            .iter()
            .map(|trace| trace.0 as i64)
            .collect()
    }

    #[func]
    fn last_trace_for(&self, component_id: i64) -> i64 {
        self.session
            .as_ref()
            .and_then(|session| session.last_trace_for(ComponentId(component_id as u64)))
            .map(|trace| trace.0 as i64)
            .unwrap_or(-1)
    }

    // --- Stimulus ----------------------------------------------------------

    #[func]
    fn inject_signal(&mut self, component_id: i64, port: GString, value: Variant) -> Error {
        let Some(session) = self.session.as_mut() else {
            godot_error!("inject_signal: no world is loaded");
            return Error::FAILED;
        };

        let Some(value) = convert::variant_to_state_value(&value) else {
            godot_error!(
                "inject_signal: unsupported Variant type {:?} (expected bool/int/float/String)",
                value.get_type()
            );
            return Error::ERR_INVALID_PARAMETER;
        };

        match session.inject_port_signal(ComponentId(component_id as u64), &port.to_string(), value)
        {
            Ok(()) => Error::OK,
            Err(error) => {
                godot_error!("inject_signal failed: {}", error);
                Error::FAILED
            }
        }
    }

    #[func]
    fn inject_tick(&mut self, component_id: i64) -> Error {
        let Some(session) = self.session.as_mut() else {
            godot_error!("inject_tick: no world is loaded");
            return Error::FAILED;
        };

        match session.inject_tick(ComponentId(component_id as u64)) {
            Ok(()) => Error::OK,
            Err(error) => {
                godot_error!("inject_tick failed: {}", error);
                Error::FAILED
            }
        }
    }

    // --- Checkpoint --------------------------------------------------------

    #[func]
    fn checkpoint_json(&mut self) -> GString {
        let Some(session) = self.session.as_mut() else {
            return GString::from("");
        };
        match session.checkpoint_json() {
            Ok(json) => GString::from(json.as_str()),
            Err(error) => {
                godot_error!("checkpoint_json failed: {}", error);
                GString::from("")
            }
        }
    }

    #[func]
    fn restore_json(&mut self, json: GString) -> Error {
        // Note: this restores component states and resources only. The
        // simulation clock and already-queued events are *not* rewound (see
        // `Session::restore`), so restoring mid-run resumes at the current tick.
        let Some(session) = self.session.as_mut() else {
            godot_error!("restore_json: no world is loaded");
            return Error::FAILED;
        };
        match session.restore_json(&json.to_string()) {
            Ok(()) => Error::OK,
            Err(error) => {
                godot_error!("restore_json failed: {}", error);
                Error::FAILED
            }
        }
    }

    // --- Tracing -----------------------------------------------------------

    /// `0` = off, `1` = abstract, `2` = hierarchical.
    #[func]
    fn set_tracing_mode(&mut self, mode: i64) {
        let mode = match mode {
            0 => TracingMode::Off,
            2 => TracingMode::Hierarchical,
            _ => TracingMode::Abstract,
        };
        if let Some(session) = self.session.as_mut() {
            session.set_tracing_mode(mode);
        }
    }
}

fn is_godot_path(path: &str) -> bool {
    path.starts_with("res://") || path.starts_with("user://")
}

fn read_godot_text(path: &str) -> Result<String, String> {
    let mut file = GFile::open(&GString::from(path), ModeFlags::READ)
        .map_err(|error| format!("cannot open '{}': {}", path, error))?;
    let mut text = String::new();
    file.read_to_string(&mut text)
        .map_err(|error| format!("cannot read '{}': {}", path, error))?;
    Ok(text)
}

fn write_godot_text(path: &str, text: &str) -> Result<(), String> {
    let mut file = GFile::open(&GString::from(path), ModeFlags::WRITE)
        .map_err(|error| format!("cannot open '{}' for writing: {}", path, error))?;
    file.write_all(text.as_bytes())
        .map_err(|error| format!("cannot write '{}': {}", path, error))?;
    file.flush()
        .map_err(|error| format!("cannot flush '{}': {}", path, error))?;
    Ok(())
}
