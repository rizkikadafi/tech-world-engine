use tw_core::{
    Component, ComponentId, Event, Port, PortDirection, PortId, SimulationContext, State,
};
use tw_sim::Simulation;

#[derive(Debug)]
struct SampleDevice {
    id: ComponentId,
    name: String,
    ports: Vec<Port>,
    state: State,
}

impl SampleDevice {
    fn new(id: ComponentId, name: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            ports: vec![
                Port::new(PortId(1), "in", PortDirection::Input),
                Port::new(PortId(2), "out", PortDirection::Output),
            ],
            state: State::new(),
        }
    }
}

impl Component for SampleDevice {
    fn id(&self) -> ComponentId {
        self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn ports(&self) -> &[Port] {
        &self.ports
    }
    fn state(&self) -> &State {
        &self.state
    }
    fn handle_event(&mut self, _event: &Event, _ctx: &mut SimulationContext) -> Vec<Event> {
        Vec::new()
    }
}

#[test]
fn reflection_enumerates_ids_names_and_ports() {
    let mut sim = Simulation::new();
    let dev1 = SampleDevice::new(ComponentId(10), "dev1");
    let dev2 = SampleDevice::new(ComponentId(5), "dev2");

    sim.register_component(Box::new(dev1));
    sim.register_component(Box::new(dev2));

    assert_eq!(sim.component_ids(), vec![ComponentId(5), ComponentId(10)]);
    assert_eq!(sim.component_name(ComponentId(5)), Some("dev2"));
    assert_eq!(sim.component_name(ComponentId(10)), Some("dev1"));
    assert_eq!(sim.component_name(ComponentId(99)), None);

    let ports = sim.component_ports(ComponentId(5)).unwrap();
    assert_eq!(ports.len(), 2);
    assert_eq!(ports[0].name, "in");
    assert_eq!(ports[1].name, "out");

    let in_port = sim.port_by_name(ComponentId(5), "in").unwrap();
    assert_eq!(in_port.id, PortId(1));
    assert_eq!(in_port.direction, PortDirection::Input);

    assert!(sim.port_by_name(ComponentId(5), "nonexistent").is_none());
}

#[test]
fn fluent_connect_wires_by_port_names() {
    let mut sim = Simulation::new();
    let dev1 = SampleDevice::new(ComponentId(1), "sender");
    let dev2 = SampleDevice::new(ComponentId(2), "receiver");

    sim.register_component(Box::new(dev1));
    sim.register_component(Box::new(dev2));

    // Connect sender "out" -> receiver "in"
    assert!(sim.connect(ComponentId(1), "out", ComponentId(2), "in").is_ok());

    // Invalid port name
    assert!(sim.connect(ComponentId(1), "bad_port", ComponentId(2), "in").is_err());

    // Incompatible direction: sender "in" -> receiver "out"
    assert!(sim.connect(ComponentId(1), "in", ComponentId(2), "out").is_err());
}
