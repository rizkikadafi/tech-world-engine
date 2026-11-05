use serde::{Deserialize, Serialize};
use tw_core::{ComponentId, ResourceId, StateValue};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct World {
    #[serde(default = "default_world_name")]
    pub name: String,

    #[serde(default = "default_ticks_per_second")]
    pub ticks_per_second: u64,

    #[serde(default)]
    pub components: Vec<ComponentSpec>,

    #[serde(default)]
    pub connections: Vec<ConnectionSpec>,

    #[serde(default)]
    pub resources: Vec<ResourceSpec>,

    #[serde(default)]
    pub stimuli: Vec<StimulusSpec>,
}

fn default_world_name() -> String {
    "untitled-world".to_string()
}

fn default_ticks_per_second() -> u64 {
    1000
}

impl Default for World {
    fn default() -> Self {
        Self {
            name: default_world_name(),
            ticks_per_second: default_ticks_per_second(),
            components: Vec::new(),
            connections: Vec::new(),
            resources: Vec::new(),
            stimuli: Vec::new(),
        }
    }
}

impl World {
    pub fn from_json_str(json: &str) -> Result<Self, crate::error::SceneError> {
        Ok(serde_json::from_str(json)?)
    }

    pub fn to_json_string(&self) -> Result<String, crate::error::SceneError> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentSpec {
    pub id: ComponentId,
    pub kind: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortEndpointSpec {
    pub component: ComponentId,
    pub port: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConnectionSpec {
    pub from: PortEndpointSpec,
    pub to: PortEndpointSpec,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceSpec {
    pub id: ResourceId,
    pub name: String,
    pub capacity: u64,
    #[serde(default)]
    pub service_time: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StimulusSpec {
    pub at: u64,
    pub component: ComponentId,
    #[serde(default)]
    pub tick: bool,
    #[serde(default)]
    pub port: Option<String>,
    #[serde(default)]
    pub value: Option<StateValue>,
}
