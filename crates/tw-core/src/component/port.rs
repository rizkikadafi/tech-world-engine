#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PortId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortDirection {
    Input,
    Output,
    Bidirectional,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Port {
    pub id: PortId,
    pub name: String,
    pub direction: PortDirection,
}

impl Port {
    pub fn new<S: Into<String>>(id: PortId, name: S, direction: PortDirection) -> Self {
        Self {
            id,
            name: name.into(),
            direction,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortBinding {
    pub external_port: PortId,
    pub internal_component: crate::component::ComponentId,
    pub internal_port: PortId,
}

impl PortBinding {
    pub fn new(
        external_port: PortId,
        internal_component: crate::component::ComponentId,
        internal_port: PortId,
    ) -> Self {
        Self {
            external_port,
            internal_component,
            internal_port,
        }
    }
}
