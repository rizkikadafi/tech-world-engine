use std::collections::HashMap;
use std::path::Path;
use tw_core::{
    Amount, ComponentId, Event, EventPayload, Port, Resource, ResourceId, ResourceSnapshot,
    SimulationTime, State, StateValue, TraceId, TraceNode, TracingMode,
};
use tw_sim::{Checkpoint, Simulation};

use crate::error::SceneError;
use crate::registry::Registry;
use crate::world::World;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StateChange {
    pub component: ComponentId,
    pub key: String,
    pub old: Option<StateValue>,
    pub new: StateValue,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FrameReport {
    pub tick: u64,
    pub events_executed: u64,
}

pub struct Session {
    sim: Simulation,
    world: World,
    prev_states: HashMap<ComponentId, State>,
}

impl Session {
    /// Load a `.twscene` file from disk using the default builtin registry.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, SceneError> {
        Self::load_with_registry(path, Registry::builtin())
    }

    /// Load a `.twscene` file from disk using a custom component registry.
    pub fn load_with_registry(
        path: impl AsRef<Path>,
        registry: Registry,
    ) -> Result<Self, SceneError> {
        let content = std::fs::read_to_string(path)?;
        let world = World::from_json_str(&content)?;
        Self::from_world(world, registry)
    }

    /// Construct a session from a parsed [`World`] description and a [`Registry`].
    pub fn from_world(world: World, registry: Registry) -> Result<Self, SceneError> {
        let mut sim = Simulation::new();

        // 1. Register resources
        for res in &world.resources {
            sim.register_resource(
                Resource::new(res.id, &res.name, Amount::Discrete(res.capacity))
                    .with_service_time(res.service_time),
            );
        }

        // 2. Register components
        for comp_spec in &world.components {
            let component = registry.create(comp_spec)?;
            sim.register_component(component);
        }

        // 3. Connect components using port names
        for conn in &world.connections {
            if let Err(e) = sim.connect(
                conn.from.component,
                &conn.from.port,
                conn.to.component,
                &conn.to.port,
            ) {
                return Err(SceneError::WiringFailed(vec![e]));
            }
        }

        // 4. Schedule initial stimuli
        for stim in &world.stimuli {
            let at = SimulationTime::new(stim.at);
            if stim.tick {
                sim.schedule_event(Event::at(at, stim.component, stim.component, EventPayload::Tick));
            } else if let (Some(port_name), Some(val)) = (&stim.port, &stim.value) {
                let port = sim
                    .port_by_name(stim.component, port_name)
                    .ok_or_else(|| SceneError::PortNotFound {
                        component: stim.component,
                        port: port_name.clone(),
                    })?;
                let port_id = port.id;
                sim.schedule_event(Event::at(
                    at,
                    stim.component,
                    stim.component,
                    EventPayload::PortSignal {
                        port_id,
                        value: val.clone(),
                    },
                ));
            }
        }

        // 5. Initialize previous states cache
        let mut prev_states = HashMap::new();
        for id in sim.component_ids() {
            if let Some(state) = sim.component_state(id) {
                prev_states.insert(id, state.clone());
            }
        }

        Ok(Self {
            sim,
            world,
            prev_states,
        })
    }

    /// Current simulation tick.
    pub fn current_tick(&self) -> u64 {
        self.sim.current_time().ticks()
    }

    /// Advance the simulation by `ticks` ticks, executing all events in that window.
    pub fn step(&mut self, ticks: u64) -> FrameReport {
        let target_tick = self.current_tick() + ticks;
        let mut events_executed = 0;

        while let Some(peek_time) = self.sim.peek_time() {
            if peek_time.ticks() > target_tick {
                break;
            }
            if self.sim.step().is_none() {
                break;
            }
            events_executed += 1;
        }

        self.sim.advance_time_to(SimulationTime::new(target_tick));

        FrameReport {
            tick: self.current_tick(),
            events_executed,
        }
    }

    /// Execute a single discrete event, if any are pending.
    pub fn step_event(&mut self) -> Option<Event> {
        self.sim.step()
    }

    /// Inspect which component variables have changed since the last call to `take_state_changes`.
    pub fn take_state_changes(&mut self) -> Vec<StateChange> {
        let mut changes = Vec::new();

        for id in self.sim.component_ids() {
            let current = match self.sim.component_state(id) {
                Some(s) => s,
                None => continue,
            };

            let prev = self.prev_states.entry(id).or_default();

            for (key, val) in current.iter() {
                let prev_val = prev.get(key);
                if prev_val != Some(val) {
                    changes.push(StateChange {
                        component: id,
                        key: key.clone(),
                        old: prev_val.cloned(),
                        new: val.clone(),
                    });
                }
            }

            *prev = current.clone();
        }

        changes
    }

    /// Inject a port signal dynamically from frontend.
    pub fn inject_port_signal(
        &mut self,
        id: ComponentId,
        port: &str,
        value: StateValue,
    ) -> Result<(), SceneError> {
        let port_id = self
            .sim
            .port_by_name(id, port)
            .ok_or_else(|| SceneError::PortNotFound {
                component: id,
                port: port.to_string(),
            })?
            .id;

        self.sim.schedule_event(Event::at(
            self.sim.current_time(),
            id,
            id,
            EventPayload::PortSignal { port_id, value },
        ));
        Ok(())
    }

    /// Inject a tick event to a component.
    pub fn inject_tick(&mut self, id: ComponentId) -> Result<(), SceneError> {
        if self.sim.component(id).is_none() {
            return Err(SceneError::ComponentNotFound { id });
        }
        self.sim.schedule_event(Event::at(
            self.sim.current_time(),
            id,
            id,
            EventPayload::Tick,
        ));
        Ok(())
    }

    // --- Inspection ---

    pub fn component_ids(&self) -> Vec<ComponentId> {
        self.sim.component_ids()
    }

    pub fn component_name(&self, id: ComponentId) -> Option<&str> {
        self.sim.component_name(id)
    }

    pub fn component_state(&self, id: ComponentId) -> Option<&State> {
        self.sim.component_state(id)
    }

    pub fn component_ports(&self, id: ComponentId) -> Option<&[Port]> {
        self.sim.component_ports(id)
    }

    pub fn resource_ids(&self) -> Vec<ResourceId> {
        self.sim.resource_ids()
    }

    pub fn resource_snapshot(&self, id: ResourceId) -> Option<ResourceSnapshot> {
        self.sim.resource_snapshot(id)
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn set_tracing_mode(&mut self, mode: TracingMode) {
        self.sim.set_tracing_mode(mode);
    }

    pub fn explain(&self, trace: TraceId) -> Vec<&TraceNode> {
        self.sim.explain(trace)
    }

    pub fn traces_for(&self, component: ComponentId) -> &[TraceId] {
        self.sim.traces_for(component)
    }

    pub fn last_trace_for(&self, component: ComponentId) -> Option<TraceId> {
        self.sim.last_trace_for(component)
    }

    pub fn checkpoint(&mut self) -> Checkpoint {
        self.sim.checkpoint()
    }

    pub fn restore(&mut self, checkpoint: &Checkpoint) {
        self.sim.restore(checkpoint);
    }

    /// Serialize the current checkpoint to JSON.
    ///
    /// This is the single checkpoint format shared by the CLI, tests, and
    /// frontend adapters (e.g. Godot).
    pub fn checkpoint_json(&mut self) -> Result<String, SceneError> {
        Ok(serde_json::to_string(&self.checkpoint())?)
    }

    /// Restore a checkpoint previously produced by [`Session::checkpoint_json`].
    pub fn restore_json(&mut self, json: &str) -> Result<(), SceneError> {
        let checkpoint: Checkpoint = serde_json::from_str(json)?;
        self.restore(&checkpoint);
        Ok(())
    }

    pub fn inner_simulation(&self) -> &Simulation {
        &self.sim
    }

    pub fn inner_simulation_mut(&mut self) -> &mut Simulation {
        &mut self.sim
    }
}
