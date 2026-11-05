use std::collections::HashMap;
use std::sync::Arc;
use tw_core::{Component, ResourceId};
use tw_sim::scenarios::{
    counter::Counter,
    digital::{Gate, GateKind, Probe},
    disk_service::DiskJob,
    resource_contention::WorkerProcess,
};

use crate::error::SceneError;
use crate::world::ComponentSpec;

pub type ComponentFactory =
    Arc<dyn Fn(&ComponentSpec) -> Result<Box<dyn Component>, SceneError> + Send + Sync>;

#[derive(Default, Clone)]
pub struct Registry {
    factories: HashMap<String, ComponentFactory>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a custom component factory for a given kind name.
    pub fn register<F>(&mut self, kind: impl Into<String>, factory: F)
    where
        F: Fn(&ComponentSpec) -> Result<Box<dyn Component>, SceneError> + Send + Sync + 'static,
    {
        self.factories.insert(kind.into(), Arc::new(factory));
    }

    /// Instantiate a component from its specification using the registered factory.
    pub fn create(&self, spec: &ComponentSpec) -> Result<Box<dyn Component>, SceneError> {
        let factory = self
            .factories
            .get(&spec.kind)
            .ok_or_else(|| SceneError::UnknownComponentKind {
                kind: spec.kind.clone(),
                component_id: spec.id,
            })?;
        factory(spec)
    }

    /// Return all registered kind names.
    pub fn kinds(&self) -> Vec<String> {
        let mut kinds: Vec<String> = self.factories.keys().cloned().collect();
        kinds.sort();
        kinds
    }

    /// Builtin registry containing standard components from `tw-sim::scenarios`.
    pub fn builtin() -> Self {
        let mut reg = Self::new();

        reg.register("Counter", |spec| {
            let limit = spec.params.get("limit").and_then(|v| v.as_u64()).unwrap_or(10);
            let interval = spec.params.get("interval").and_then(|v| v.as_u64()).unwrap_or(1);
            let name = spec
                .params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Counter");
            Ok(Box::new(Counter::new(spec.id, name, limit, interval)))
        });

        reg.register("Gate", |spec| {
            let gate_str = spec
                .params
                .get("gate")
                .and_then(|v| v.as_str())
                .unwrap_or("AND");
            let kind = match gate_str.to_uppercase().as_str() {
                "XOR" => GateKind::Xor,
                "AND" => GateKind::And,
                "OR" => GateKind::Or,
                other => {
                    return Err(SceneError::ComponentFactoryFailed {
                        kind: spec.kind.clone(),
                        component_id: spec.id,
                        reason: format!("unsupported gate kind '{other}' (expected XOR, AND, or OR)"),
                    });
                }
            };
            Ok(Box::new(Gate::new(spec.id, kind)))
        });

        reg.register("Probe", |spec| Ok(Box::new(Probe::new(spec.id))));

        reg.register("Worker", |spec| {
            let res_id = spec
                .params
                .get("resource")
                .and_then(|v| v.as_u64())
                .unwrap_or(1);
            let duration = spec
                .params
                .get("work_duration")
                .and_then(|v| v.as_u64())
                .unwrap_or(2);
            let name = spec
                .params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Worker");
            Ok(Box::new(WorkerProcess::new(
                spec.id,
                name,
                ResourceId(res_id),
                duration,
            )))
        });

        reg.register("DiskJob", |spec| {
            let res_id = spec
                .params
                .get("resource")
                .and_then(|v| v.as_u64())
                .unwrap_or(1);
            let name = spec
                .params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("DiskJob");
            Ok(Box::new(DiskJob::new(spec.id, name, ResourceId(res_id))))
        });

        reg
    }
}
