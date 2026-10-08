use crate::component::{ComponentId, PortId};
use crate::resource::ResourceId;
use crate::time::SimulationTime;
use crate::tracing::TraceId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventId(pub u64);

#[derive(Debug, Clone, PartialEq)]
pub enum EventPayload {
    SignalChanged { name: String, value: String },
    PortSignal { port_id: PortId, value: String },
    Tick,
    Custom(String),
    /// A component is requesting `amount` units from a resource.
    ResourceRequested {
        resource_id: ResourceId,
        amount: u64,
        requester: ComponentId,
    },
    /// The simulation granted `amount` units from a resource to the target component.
    ResourceGranted { resource_id: ResourceId, amount: u64 },
    /// The simulation denied a resource request (resource not registered or invalid request).
    ResourceDenied {
        resource_id: ResourceId,
        amount: u64,
        reason: String,
    },
    /// A component is releasing `amount` units back to a resource.
    ResourceReleased {
        resource_id: ResourceId,
        amount: u64,
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
}
