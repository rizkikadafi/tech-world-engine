use crate::component::{ComponentId, PortId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connection {
    pub source_component: ComponentId,
    pub source_port: PortId,
    pub target_component: ComponentId,
    pub target_port: PortId,
}

impl Connection {
    pub fn new(
        source_component: ComponentId,
        source_port: PortId,
        target_component: ComponentId,
        target_port: PortId,
    ) -> Self {
        Self {
            source_component,
            source_port,
            target_component,
            target_port,
        }
    }
}
