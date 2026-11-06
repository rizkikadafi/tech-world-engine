//! Type mapping between Tech World Engine values and Godot `Variant`s.
//!
//! This module contains no simulation logic: it only translates plain data.

use godot::builtin::VariantType;
use godot::prelude::*;
use tw_core::{Amount, Port, PortDirection, ResourceSnapshot, State, StateValue, TraceNode};
use tw_scene::StateChange;

pub fn state_value_to_variant(value: &StateValue) -> Variant {
    match value {
        StateValue::Bool(v) => v.to_variant(),
        StateValue::Int(v) => v.to_variant(),
        StateValue::Float(v) => v.to_variant(),
        StateValue::Text(v) => v.to_variant(),
    }
}

/// Convert a Godot `Variant` into an engine `StateValue`.
///
/// Only the four value kinds the engine understands are accepted. Anything
/// else (objects, arrays, dictionaries, ...) yields `None` so the adapter can
/// report a clean Godot error instead of panicking.
pub fn variant_to_state_value(value: &Variant) -> Option<StateValue> {
    match value.get_type() {
        VariantType::BOOL => Some(StateValue::Bool(value.to::<bool>())),
        VariantType::INT => Some(StateValue::Int(value.to::<i64>())),
        VariantType::FLOAT => Some(StateValue::Float(value.to::<f64>())),
        VariantType::STRING => Some(StateValue::Text(value.to::<GString>().to_string())),
        _ => None,
    }
}

pub fn state_to_dict(state: &State) -> VarDictionary {
    let mut dict = VarDictionary::new();
    for (key, value) in state.iter() {
        let value = state_value_to_variant(value);
        dict.set(key.as_str(), &value);
    }
    dict
}

pub fn state_change_to_dict(change: &StateChange) -> VarDictionary {
    let mut dict = VarDictionary::new();
    dict.set("component", change.component.0 as i64);
    dict.set("key", change.key.as_str());

    let old = change
        .old
        .as_ref()
        .map(state_value_to_variant)
        .unwrap_or_else(Variant::nil);
    dict.set("old", &old);

    let new = state_value_to_variant(&change.new);
    dict.set("new", &new);
    dict
}

pub fn port_to_dict(port: &Port) -> VarDictionary {
    let mut dict = VarDictionary::new();
    dict.set("id", port.id.0 as i64);
    dict.set("name", port.name.as_str());
    dict.set("direction", port_direction_name(port.direction));
    dict
}

fn port_direction_name(direction: PortDirection) -> &'static str {
    match direction {
        PortDirection::Input => "input",
        PortDirection::Output => "output",
        PortDirection::Bidirectional => "bidirectional",
    }
}

pub fn trace_to_dict(node: &TraceNode) -> VarDictionary {
    let mut dict = VarDictionary::new();
    dict.set("id", node.id.0 as i64);
    dict.set("event_id", node.event_id.0 as i64);
    dict.set("tick", node.time.ticks() as i64);
    dict.set("description", node.description.as_str());

    let subject = node
        .subject
        .map(|id| id.0 as i64)
        .map(Variant::from)
        .unwrap_or_else(Variant::nil);
    dict.set("subject", &subject);
    dict
}

pub fn resource_snapshot_to_dict(snapshot: &ResourceSnapshot) -> VarDictionary {
    let mut dict = VarDictionary::new();
    dict.set("id", snapshot.id.0 as i64);

    let capacity = amount_to_variant(&snapshot.capacity);
    dict.set("capacity", &capacity);

    let available = amount_to_variant(&snapshot.available);
    dict.set("available", &available);
    dict.set("pending", snapshot.pending_queue.len() as i64);

    let mut queue = Array::<VarDictionary>::new();
    for request in &snapshot.pending_queue {
        let mut item = VarDictionary::new();
        item.set("requester", request.requester.0 as i64);
        item.set("priority", request.priority);
        let amount = amount_to_variant(&request.amount);
        item.set("amount", &amount);
        queue.push(&item);
    }
    dict.set("pending_queue", &queue);

    dict
}

fn amount_to_variant(amount: &Amount) -> Variant {
    match amount {
        Amount::Discrete(value) => (*value as i64).to_variant(),
        Amount::Continuous(value) => value.to_variant(),
    }
}
