use crate::component::{ComponentId, PortId};
use crate::resource::{Amount, ResourceId};
use crate::state::StateValue;
use crate::time::SimulationTime;
use crate::tracing::TraceId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventId(pub u64);

#[derive(Debug, Clone, PartialEq)]
pub enum EventPayload {
    SignalChanged { name: String, value: StateValue },
    PortSignal { port_id: PortId, value: StateValue },
    Tick,
    Custom(String),
    /// A component is requesting `amount` from a resource.
    ResourceRequested {
        resource_id: ResourceId,
        amount: Amount,
        requester: ComponentId,
    },
    /// The simulation granted `amount` from a resource to the target component.
    ResourceGranted { resource_id: ResourceId, amount: Amount },
    /// The simulation denied a resource request (resource not registered or invalid request).
    ResourceDenied {
        resource_id: ResourceId,
        amount: Amount,
        reason: String,
    },
    /// A component (or the engine, on service completion) is releasing `amount`
    /// back to a resource.
    ResourceReleased {
        resource_id: ResourceId,
        amount: Amount,
        releaser: ComponentId,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub id: EventId,
    pub time: SimulationTime,
    pub source: ComponentId,
    pub target: ComponentId,
    pub payload: EventPayload,
    pub cause: Option<TraceId>,
}

impl Event {
    pub fn new(
        id: EventId,
        time: SimulationTime,
        source: ComponentId,
        target: ComponentId,
        payload: EventPayload,
    ) -> Self {
        Self {
            id,
            time,
            source,
            target,
            payload,
            cause: None,
        }
    }

    pub fn with_cause(mut self, cause: TraceId) -> Self {
        self.cause = Some(cause);
        self
    }

    /// Build an event whose [`EventId`] is a placeholder (`0`).
    ///
    /// The simulation assigns the real id when the event is scheduled, so
    /// components should prefer this constructor instead of inventing ids
    /// (e.g. `EventId(event.id.0 + 10)`), which can collide with engine ids
    /// and break same-tick ordering determinism.
    pub fn at(
        time: SimulationTime,
        source: ComponentId,
        target: ComponentId,
        payload: EventPayload,
    ) -> Self {
        Self::new(EventId(0), time, source, target, payload)
    }
}
