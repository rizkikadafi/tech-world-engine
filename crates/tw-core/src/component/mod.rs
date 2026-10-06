pub mod composite;
pub mod port;

pub use composite::CompositeComponent;
pub use port::{Port, PortBinding, PortDirection, PortId};

use crate::context::SimulationContext;
use crate::event::Event;
use crate::state::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentId(pub u64);

pub trait Component: std::fmt::Debug {
    fn id(&self) -> ComponentId;
    fn name(&self) -> &str;
    fn ports(&self) -> &[Port];
    fn handle_event(&mut self, event: &Event, ctx: &mut SimulationContext) -> Vec<Event>;
    fn state(&self) -> &State;
}
