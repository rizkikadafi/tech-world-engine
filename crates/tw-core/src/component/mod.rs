pub mod composite;
pub mod port;

pub use composite::{CompositeComponent, CompositeError};
pub use port::{Port, PortBinding, PortDirection, PortId};

use crate::context::SimulationContext;
use crate::event::Event;
use crate::state::State;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ComponentId(pub u64);

pub trait Component: std::fmt::Debug {
    fn id(&self) -> ComponentId;
    fn name(&self) -> &str;
    fn ports(&self) -> &[Port];
    fn handle_event(&mut self, event: &Event, ctx: &mut SimulationContext) -> Vec<Event>;
    fn state(&self) -> &State;

    /// Lifecycle hook called once when the component is registered with the
    /// simulation. Any returned events are scheduled. The default is a no-op.
    fn on_mount(&mut self, _ctx: &mut SimulationContext) -> Vec<Event> {
        Vec::new()
    }

    /// Lifecycle hook called when the simulation shuts down. Any returned events
    /// are scheduled. The default is a no-op.
    fn on_unmount(&mut self, _ctx: &mut SimulationContext) -> Vec<Event> {
        Vec::new()
    }

    /// Restore component-owned state from a checkpoint snapshot.
    ///
    /// The default implementation does nothing and returns `false`, meaning the
    /// component does not support state restore. Components that keep their
    /// observable state in [`State`] should copy the snapshot into `self.state`
    /// (and into any mirrored fields) and return `true`.
    ///
    /// This hook is called by `Simulation::restore_components` in `tw-sim`, not
    /// by the engine automatically.
    fn restore_state(&mut self, _state: &State) -> bool {
        false
    }
}
