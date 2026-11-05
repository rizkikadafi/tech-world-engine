//! Construction-time fidelity selection.
//!
//! A single logical component (for example a NAND gate) can be represented at
//! several levels of detail while keeping an identical discrete interface:
//!
//! ```text
//! Transistor  ── raw netlist, solved by MNA
//! Bridge      ── mixed-signal `CircuitComponent` wrapping that same netlist
//! Derived     ── discrete `DerivedGate`/`DerivedDff` built from measurements
//! ```
//!
//! [`FidelityFactory`] characterizes a circuit once, caches the measurement, and
//! builds any fidelity from it. Because the bridge is constructed from the exact
//! [`CircuitTemplate`] that was measured, the abstract and mixed-signal models
//! cannot silently diverge from the physical circuit.
//!
//! This module selects a fidelity at construction time only. Switching a live
//! component mid-simulation is intentionally out of scope.

use std::collections::HashMap;

use tw_core::{Component, ComponentId};

use crate::bridge::{BridgeConfig, BridgeInput, BridgeOutput, CircuitComponent};
use crate::characterization::{
    Characterizer, CircuitTemplate, GateCharacterization, GateKind, Measurement,
};
use crate::circuit::Circuit;
use crate::derived::{DerivedDff, DerivedError, DerivedGate};
use crate::error::CircuitError;
use crate::time_base::TimeBase;

/// Level of detail used to represent a component.
///
/// Detail increases from `Derived` to `Bridge` to `Transistor`; see
/// [`Fidelity::more_detailed`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Fidelity {
    /// Raw transistor netlist, simulated by the analog solver.
    Transistor,
    /// Mixed-signal bridge exposing the transistor netlist as a discrete component.
    Bridge,
    /// Discrete component built from measured timing and logic.
    Derived,
}

impl Fidelity {
    /// All fidelities, from least to most detailed.
    pub const ALL: [Fidelity; 3] = [Fidelity::Derived, Fidelity::Bridge, Fidelity::Transistor];

    /// Human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Fidelity::Transistor => "transistor",
            Fidelity::Bridge => "bridge",
            Fidelity::Derived => "derived",
        }
    }

    /// The next fidelity with more detail, if any.
    pub fn more_detailed(self) -> Option<Self> {
        match self {
            Fidelity::Derived => Some(Fidelity::Bridge),
            Fidelity::Bridge => Some(Fidelity::Transistor),
            Fidelity::Transistor => None,
        }
    }

    /// The next fidelity with less detail, if any.
    pub fn less_detailed(self) -> Option<Self> {
        match self {
            Fidelity::Derived => None,
            Fidelity::Bridge => Some(Fidelity::Derived),
            Fidelity::Transistor => Some(Fidelity::Bridge),
        }
    }

    /// Whether this fidelity is exposed as a `tw_core::Component`.
    pub fn is_component(self) -> bool {
        matches!(self, Fidelity::Bridge | Fidelity::Derived)
    }
}

/// Description of the logical component to build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentSpec {
    /// Component id used in the discrete simulation.
    pub id: ComponentId,
    /// Component name used in the discrete simulation.
    pub name: String,
    /// Physical circuit the component is derived from.
    pub kind: GateKind,
}

impl ComponentSpec {
    /// Creates a component specification.
    pub fn new(id: ComponentId, name: impl Into<String>, kind: GateKind) -> Self {
        Self {
            id,
            name: name.into(),
            kind,
        }
    }
}

/// A component represented at a particular fidelity.
#[derive(Debug)]
pub enum Model {
    /// Raw transistor netlist.
    Transistor(Circuit),
    /// Mixed-signal bridge wrapping the measured netlist.
    Bridge(Box<dyn Component>),
    /// Discrete component built from measurements.
    Derived(Box<dyn Component>),
}

impl Model {
    /// Fidelity of this model.
    pub fn fidelity(&self) -> Fidelity {
        match self {
            Model::Transistor(_) => Fidelity::Transistor,
            Model::Bridge(_) => Fidelity::Bridge,
            Model::Derived(_) => Fidelity::Derived,
        }
    }

    /// Borrows the model as a discrete component, if it has one.
    pub fn as_component(&self) -> Option<&dyn Component> {
        match self {
            Model::Transistor(_) => None,
            Model::Bridge(component) | Model::Derived(component) => Some(component.as_ref()),
        }
    }

    /// Consumes the model and returns its discrete component, if it has one.
    pub fn into_component(self) -> Result<Box<dyn Component>, FidelityError> {
        match self {
            Model::Transistor(_) => Err(FidelityError::NotAComponent(Fidelity::Transistor)),
            Model::Bridge(component) | Model::Derived(component) => Ok(component),
        }
    }
}

/// Errors produced while selecting or refining a fidelity.
#[derive(Debug, thiserror::Error)]
pub enum FidelityError {
    /// The underlying circuit failed to build or simulate.
    #[error("circuit error: {0}")]
    Circuit(#[from] CircuitError),
    /// A derived component could not be built from the measurement.
    #[error("derived component error: {0}")]
    Derived(#[from] DerivedError),
    /// The requested fidelity has no `tw_core::Component` representation.
    #[error("fidelity {0:?} is not a discrete component")]
    NotAComponent(Fidelity),
    /// `refine` was called on the most detailed fidelity.
    #[error("fidelity {0:?} is already the most detailed")]
    AlreadyMostDetailed(Fidelity),
    /// The gate kind has no implementation at the requested fidelity.
    #[error("gate kind {0:?} is not supported at fidelity {1:?}")]
    UnsupportedKind(GateKind, Fidelity),
}

/// Builds components at any fidelity from a shared characterization cache.
///
/// Measurement is expensive (a DFF takes seconds), so each [`GateKind`] is
/// characterized at most once per factory.
#[derive(Debug)]
pub struct FidelityFactory {
    characterizer: Characterizer,
    time_base: TimeBase,
    cache: HashMap<GateKind, Measurement>,
}

impl FidelityFactory {
    /// Creates a factory with an explicit characterizer and time base.
    pub fn new(characterizer: Characterizer, time_base: TimeBase) -> Result<Self, FidelityError> {
        time_base.validate()?;
        Ok(Self {
            characterizer,
            time_base,
            cache: HashMap::new(),
        })
    }

    /// Creates a factory with the default characterizer and a 1 ns tick.
    pub fn with_defaults() -> Self {
        Self {
            characterizer: Characterizer::with_defaults(),
            time_base: TimeBase::default(),
            cache: HashMap::new(),
        }
    }

    /// Characterizer used for uncached measurements.
    pub fn characterizer(&self) -> &Characterizer {
        &self.characterizer
    }

    /// Time base used to quantize derived delays.
    pub fn time_base(&self) -> TimeBase {
        self.time_base
    }

    /// Returns the measurement for `kind`, characterizing it on first use.
    pub fn measurement(&mut self, kind: GateKind) -> Result<&Measurement, FidelityError> {
        if !self.cache.contains_key(&kind) {
            let measurement = self.characterizer.measure(kind)?;
            self.cache.insert(kind, measurement);
        }
        Ok(&self.cache[&kind])
    }

    /// Returns the measured characterization for `kind`, characterizing it on
    /// first use.
    pub fn characterization(
        &mut self,
        kind: GateKind,
    ) -> Result<&GateCharacterization, FidelityError> {
        Ok(&self.measurement(kind)?.gate)
    }

    /// Returns the physical template for `kind`, characterizing it on first use.
    pub fn template(&mut self, kind: GateKind) -> Result<&CircuitTemplate, FidelityError> {
        Ok(&self.measurement(kind)?.template)
    }

    /// Whether `kind` has already been characterized.
    pub fn is_cached(&self, kind: GateKind) -> bool {
        self.cache.contains_key(&kind)
    }

    /// Number of circuits characterized so far.
    pub fn cached_count(&self) -> usize {
        self.cache.len()
    }

    /// Builds `spec` at the requested fidelity.
    pub fn select(
        &mut self,
        spec: &ComponentSpec,
        fidelity: Fidelity,
    ) -> Result<Model, FidelityError> {
        match fidelity {
            Fidelity::Transistor => {
                let template = self.template(spec.kind)?;
                Ok(Model::Transistor(template.circuit.clone()))
            }
            Fidelity::Bridge => Ok(Model::Bridge(self.build_bridge(spec)?)),
            Fidelity::Derived => Ok(Model::Derived(self.build_derived(spec)?)),
        }
    }

    /// Builds `spec` as a discrete `tw_core::Component`.
    ///
    /// [`Fidelity::Transistor`] is rejected with [`FidelityError::NotAComponent`];
    /// wrap it in a [`Fidelity::Bridge`] to run it inside `tw-sim`.
    pub fn select_component(
        &mut self,
        spec: &ComponentSpec,
        fidelity: Fidelity,
    ) -> Result<Box<dyn Component>, FidelityError> {
        match fidelity {
            Fidelity::Transistor => Err(FidelityError::NotAComponent(fidelity)),
            Fidelity::Bridge => self.build_bridge(spec),
            Fidelity::Derived => self.build_derived(spec),
        }
    }

    /// Moves `spec` one step toward more detail.
    ///
    /// `Derived -> Bridge -> Transistor`. Refining a transistor model returns
    /// [`FidelityError::AlreadyMostDetailed`].
    pub fn refine(
        &mut self,
        spec: &ComponentSpec,
        current: Fidelity,
    ) -> Result<Model, FidelityError> {
        let next = current
            .more_detailed()
            .ok_or(FidelityError::AlreadyMostDetailed(current))?;
        self.select(spec, next)
    }

    fn build_bridge(&mut self, spec: &ComponentSpec) -> Result<Box<dyn Component>, FidelityError> {
        let measurement = self.measurement(spec.kind)?.clone();
        let gate = &measurement.gate;
        let template = &measurement.template;

        let inputs = template
            .inputs
            .iter()
            .map(|input| {
                BridgeInput::new(
                    input.port_id,
                    input.name,
                    input.source_name.clone(),
                    0.0,
                    gate.config.vdd,
                )
            })
            .collect();

        let config = BridgeConfig {
            time_base: self.time_base,
            solver_dt: gate.config.dt,
            sync_interval_ticks: 1,
            supply_source: template.supply_source.clone(),
            inputs,
            outputs: vec![BridgeOutput::new(
                template.output_port,
                spec.kind.output_name(),
                template.output_node,
                gate.levels.threshold(gate.config.mid_threshold),
            )],
        };

        let component =
            CircuitComponent::new(spec.id, spec.name.clone(), template.circuit.clone(), config)?;
        Ok(Box::new(component))
    }

    fn build_derived(&mut self, spec: &ComponentSpec) -> Result<Box<dyn Component>, FidelityError> {
        let measurement = self.measurement(spec.kind)?.clone();
        let gate = &measurement.gate;
        let component: Box<dyn Component> = match spec.kind {
            GateKind::Inverter | GateKind::Nand2 => Box::new(DerivedGate::from_characterization(
                spec.id,
                spec.name.clone(),
                gate,
                self.time_base,
            )?),
            GateKind::Dff => Box::new(DerivedDff::from_characterization(
                spec.id,
                spec.name.clone(),
                gate,
                self.time_base,
            )?),
        };
        Ok(component)
    }
}
