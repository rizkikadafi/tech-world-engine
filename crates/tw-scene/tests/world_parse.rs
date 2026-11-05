use tw_core::ComponentId;
use tw_scene::{PortEndpointSpec, World};

const SAMPLE_JSON: &str = r#"{
  "name": "counter-demo",
  "ticks_per_second": 1000,
  "components": [
    {
      "id": 1,
      "kind": "Counter",
      "params": { "limit": 5, "interval": 2 }
    },
    {
      "id": 2,
      "kind": "Gate",
      "params": { "gate": "AND" }
    }
  ],
  "connections": [
    {
      "from": { "component": 1, "port": "out" },
      "to":   { "component": 2, "port": "in_a" }
    }
  ],
  "resources": [
    { "id": 10, "name": "SharedDisk", "capacity": 1, "service_time": 3 }
  ],
  "stimuli": [
    { "at": 0, "component": 1, "tick": true },
    { "at": 10, "component": 2, "port": "in_a", "value": 1 }
  ]
}"#;

#[test]
fn parse_valid_world_json() {
    let world = World::from_json_str(SAMPLE_JSON).expect("valid world JSON");
    assert_eq!(world.name, "counter-demo");
    assert_eq!(world.ticks_per_second, 1000);
    assert_eq!(world.components.len(), 2);
    assert_eq!(world.components[0].id, ComponentId(1));
    assert_eq!(world.components[0].kind, "Counter");
    assert_eq!(world.components[1].id, ComponentId(2));
    assert_eq!(world.components[1].kind, "Gate");

    assert_eq!(world.connections.len(), 1);
    assert_eq!(
        world.connections[0].from,
        PortEndpointSpec {
            component: ComponentId(1),
            port: "out".to_string()
        }
    );
    assert_eq!(
        world.connections[0].to,
        PortEndpointSpec {
            component: ComponentId(2),
            port: "in_a".to_string()
        }
    );

    assert_eq!(world.resources.len(), 1);
    assert_eq!(world.resources[0].name, "SharedDisk");
    assert_eq!(world.resources[0].capacity, 1);
    assert_eq!(world.resources[0].service_time, 3);

    assert_eq!(world.stimuli.len(), 2);
    assert!(world.stimuli[0].tick);
    assert_eq!(world.stimuli[1].port.as_deref(), Some("in_a"));
}

#[test]
fn roundtrip_serialization() {
    let world = World::from_json_str(SAMPLE_JSON).unwrap();
    let serialized = world.to_json_string().unwrap();
    let deserialized = World::from_json_str(&serialized).unwrap();
    assert_eq!(world, deserialized);
}
