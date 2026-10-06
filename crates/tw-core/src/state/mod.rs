use crate::time::SimulationTime;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct State {
    variables: HashMap<String, String>,
}

impl State {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }

    pub fn set<K: Into<String>, V: Into<String>>(&mut self, key: K, value: V) {
        self.variables.insert(key.into(), value.into());
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.variables.get(key).map(|s| s.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub time: SimulationTime,
    pub state: State,
}

impl Snapshot {
    pub fn new(time: SimulationTime, state: State) -> Self {
        Self { time, state }
    }
}
