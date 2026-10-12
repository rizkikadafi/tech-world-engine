use crate::time::SimulationTime;
use std::collections::HashMap;
use std::fmt;

/// A typed state value.
///
/// The engine deliberately does not know about domain variables (no
/// `CPUState`, `TransistorState`, ...). Components describe their own state
/// using these generic scalar kinds. Vectors/structured data can be encoded as
/// several keys or as `Text` until a richer representation is needed.
#[derive(Debug, Clone, PartialEq)]
pub enum StateValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
}

impl StateValue {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            StateValue::Bool(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            StateValue::Int(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            StateValue::Float(v) => Some(*v),
            StateValue::Int(v) => Some(*v as f64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            StateValue::Text(v) => Some(v.as_str()),
            _ => None,
        }
    }

    /// A short human-readable name of the value kind.
    pub fn kind(&self) -> &'static str {
        match self {
            StateValue::Bool(_) => "bool",
            StateValue::Int(_) => "int",
            StateValue::Float(_) => "float",
            StateValue::Text(_) => "text",
        }
    }
}

impl fmt::Display for StateValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StateValue::Bool(v) => write!(f, "{v}"),
            StateValue::Int(v) => write!(f, "{v}"),
            StateValue::Float(v) => write!(f, "{v}"),
            StateValue::Text(v) => write!(f, "{v}"),
        }
    }
}

impl From<bool> for StateValue {
    fn from(v: bool) -> Self {
        StateValue::Bool(v)
    }
}

impl From<f32> for StateValue {
    fn from(v: f32) -> Self {
        StateValue::Float(v as f64)
    }
}

impl From<f64> for StateValue {
    fn from(v: f64) -> Self {
        StateValue::Float(v)
    }
}

impl From<&str> for StateValue {
    fn from(v: &str) -> Self {
        StateValue::Text(v.to_string())
    }
}

impl From<String> for StateValue {
    fn from(v: String) -> Self {
        StateValue::Text(v)
    }
}

impl From<&String> for StateValue {
    fn from(v: &String) -> Self {
        StateValue::Text(v.clone())
    }
}

macro_rules! impl_from_int {
    ($($t:ty),* $(,)?) => {
        $(
            impl From<$t> for StateValue {
                fn from(v: $t) -> Self {
                    StateValue::Int(v as i64)
                }
            }
        )*
    };
}

impl_from_int!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

/// Component-owned state: a set of typed variables keyed by name.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct State {
    variables: HashMap<String, StateValue>,
}

impl State {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }

    pub fn set<K, V>(&mut self, key: K, value: V)
    where
        K: Into<String>,
        V: Into<StateValue>,
    {
        self.variables.insert(key.into(), value.into());
    }

    pub fn get(&self, key: &str) -> Option<&StateValue> {
        self.variables.get(key)
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(StateValue::as_bool)
    }

    pub fn get_int(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(StateValue::as_int)
    }

    pub fn get_float(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(StateValue::as_float)
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(StateValue::as_str)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.variables.contains_key(key)
    }

    pub fn remove(&mut self, key: &str) -> Option<StateValue> {
        self.variables.remove(key)
    }

    pub fn len(&self) -> usize {
        self.variables.len()
    }

    pub fn is_empty(&self) -> bool {
        self.variables.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &StateValue)> {
        self.variables.iter()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub time: SimulationTime,
    pub state: State,
}

impl Snapshot {
    pub fn new(time: SimulationTime, state: State) -> Self {
        Self { time, state }
    }
}
