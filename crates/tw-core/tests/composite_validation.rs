use tw_core::{
    Component, ComponentId, CompositeComponent, CompositeError, Connection, Event, Port,
    PortBinding, PortDirection, PortId, SimulationContext, State,
};

#[derive(Debug)]
struct Leaf {
    id: ComponentId,
    ports: Vec<Port>,
    state: State,
}

impl Leaf {
    fn new(id: u64, ports: Vec<Port>) -> Self {
        Self {
            id: ComponentId(id),
            ports,
            state: State::new(),
        }
    }
}

impl Component for Leaf {
    fn id(&self) -> ComponentId {
        self.id
    }

    fn name(&self) -> &str {
        "leaf"
    }

    fn ports(&self) -> &[Port] {
        &self.ports
    }

    fn handle_event(&mut self, _event: &Event, _ctx: &mut SimulationContext) -> Vec<Event> {
        Vec::new()
    }

    fn state(&self) -> &State {
        &self.state
    }
}

const EXT_IN: PortId = PortId(1);
const EXT_OUT: PortId = PortId(2);
const SUB: ComponentId = ComponentId(10);
const SUB_IN: PortId = PortId(100);
const SUB_OUT: PortId = PortId(101);

fn composite_with_valid_wiring() -> CompositeComponent {
    let mut composite = CompositeComponent::new(ComponentId(1), "adder");
    composite.add_port(Port::new(EXT_IN, "a", PortDirection::Input));
    composite.add_port(Port::new(EXT_OUT, "sum", PortDirection::Output));
    composite.add_subcomponent(Box::new(Leaf::new(
        10,
        vec![
            Port::new(SUB_IN, "in", PortDirection::Input),
            Port::new(SUB_OUT, "out", PortDirection::Output),
        ],
    )));
    composite.bind_port(PortBinding::new(EXT_IN, SUB, SUB_IN));
    composite.bind_port(PortBinding::new(EXT_OUT, SUB, SUB_OUT));
    composite.add_internal_connection(Connection::new(SUB, SUB_OUT, SUB, SUB_IN));
    composite
}

#[test]
fn valid_wiring_produces_no_errors() {
    let composite = composite_with_valid_wiring();
    assert_eq!(composite.validate(), Ok(()));
}

#[test]
fn unknown_external_port_is_reported() {
    let mut composite = composite_with_valid_wiring();
    composite.bind_port(PortBinding::new(PortId(999), SUB, SUB_IN));

    let errors = composite.validate().unwrap_err();
    assert!(errors.contains(&CompositeError::UnknownExternalPort {
        external_port: PortId(999),
    }));
}

#[test]
fn unknown_subcomponent_is_reported() {
    let mut composite = composite_with_valid_wiring();
    composite.bind_port(PortBinding::new(EXT_IN, ComponentId(99), SUB_IN));

    let errors = composite.validate().unwrap_err();
    assert!(errors.contains(&CompositeError::UnknownSubcomponent {
        component: ComponentId(99),
    }));
}

#[test]
fn unknown_subcomponent_port_is_reported() {
    let mut composite = composite_with_valid_wiring();
    composite.bind_port(PortBinding::new(EXT_IN, SUB, PortId(999)));

    let errors = composite.validate().unwrap_err();
    assert!(errors.contains(&CompositeError::UnknownSubcomponentPort {
        component: SUB,
        port: PortId(999),
    }));
}

#[test]
fn direction_mismatch_is_reported() {
    let mut composite = composite_with_valid_wiring();
    // External input must not bind to an internal *output* port.
    composite.bind_port(PortBinding::new(EXT_IN, SUB, SUB_OUT));

    let errors = composite.validate().unwrap_err();
    assert!(errors.contains(&CompositeError::DirectionMismatch {
        external_port: EXT_IN,
        internal_component: SUB,
        internal_port: SUB_OUT,
    }));
}

#[test]
fn duplicate_binding_is_reported_once() {
    let mut composite = composite_with_valid_wiring();
    composite.bind_port(PortBinding::new(EXT_IN, SUB, SUB_IN)); // same as the valid one

    let errors = composite.validate().unwrap_err();
    let duplicates: Vec<_> = errors
        .iter()
        .filter(|e| matches!(e, CompositeError::DuplicateBinding { .. }))
        .collect();
    assert_eq!(duplicates.len(), 1);
}

#[test]
fn unknown_connection_endpoint_is_reported() {
    let mut composite = composite_with_valid_wiring();
    composite.add_internal_connection(Connection::new(SUB, SUB_OUT, ComponentId(99), SUB_IN));

    let errors = composite.validate().unwrap_err();
    assert!(errors.contains(&CompositeError::UnknownConnectionEndpoint {
        component: ComponentId(99),
        port: SUB_IN,
    }));
}
