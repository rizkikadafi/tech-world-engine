//! Type mapping between `tw-schematic` authoring data and Godot `Variant`s.
//!
//! This module contains no authoring logic: it only translates plain data so
//! GDScript can render and edit a document.

use godot::builtin::VariantType;
use godot::prelude::*;
use serde_json::{Map, Number, Value};
use tw_schematic::{Diagnostic, PinRole};

/// Convert an arbitrary JSON value into a Godot `Variant`.
pub fn json_to_variant(value: &Value) -> Variant {
    match value {
        Value::Null => Variant::nil(),
        Value::Bool(value) => value.to_variant(),
        Value::Number(value) => number_to_variant(value),
        Value::String(value) => GString::from(value.as_str()).to_variant(),
        Value::Array(values) => {
            let mut array = Array::<Variant>::new();
            for value in values {
                array.push(&json_to_variant(value));
            }
            array.to_variant()
        }
        Value::Object(entries) => {
            let mut dict = VarDictionary::new();
            for (key, value) in entries {
                dict.set(key.as_str(), &json_to_variant(value));
            }
            dict.to_variant()
        }
    }
}

fn number_to_variant(value: &Number) -> Variant {
    if let Some(value) = value.as_i64() {
        value.to_variant()
    } else if let Some(value) = value.as_u64() {
        // Godot integers are signed 64-bit; saturate rather than wrap.
        i64::try_from(value).unwrap_or(i64::MAX).to_variant()
    } else {
        value.as_f64().unwrap_or(0.0).to_variant()
    }
}

/// Convert a Godot `Variant` into JSON.
///
/// Only plain data (nil/bool/int/float/String/Array/Dictionary) is accepted.
/// Objects and other engine types yield `None` so callers can report a clean
/// Godot error instead of panicking.
pub fn variant_to_json(value: &Variant) -> Option<Value> {
    match value.get_type() {
        VariantType::NIL => Some(Value::Null),
        VariantType::BOOL => Some(Value::Bool(value.to::<bool>())),
        VariantType::INT => Some(Value::from(value.to::<i64>())),
        VariantType::FLOAT => {
            let number = value.to::<f64>();
            Number::from_f64(number).map(Value::Number)
        }
        VariantType::STRING => Some(Value::String(value.to::<GString>().to_string())),
        VariantType::ARRAY => {
            let array = value.to::<Array<Variant>>();
            let mut values = Vec::with_capacity(array.len());
            for item in array.iter_shared() {
                values.push(variant_to_json(&item)?);
            }
            Some(Value::Array(values))
        }
        VariantType::DICTIONARY => {
            let dict = value.to::<VarDictionary>();
            let mut entries = Map::new();
            for (key, item) in &dict {
                let key = key.to::<GString>().to_string();
                entries.insert(key, variant_to_json(&item)?);
            }
            Some(Value::Object(entries))
        }
        _ => None,
    }
}

pub fn pin_role_name(role: PinRole) -> &'static str {
    match role {
        PinRole::Passive => "passive",
        PinRole::Power => "power",
        PinRole::Ground => "ground",
        PinRole::Signal => "signal",
    }
}

pub fn diagnostic_to_dict(diagnostic: &Diagnostic) -> VarDictionary {
    let mut dict = VarDictionary::new();
    dict.set(
        "severity",
        match diagnostic.severity {
            tw_schematic::Severity::Error => "error",
            tw_schematic::Severity::Warning => "warning",
        },
    );
    dict.set("code", diagnostic.code.as_str());
    dict.set("message", diagnostic.message.as_str());

    let mut parts = Array::<GString>::new();
    for part in &diagnostic.parts {
        parts.push(&GString::from(part.0.as_str()));
    }
    dict.set("parts", &parts);

    let mut wires = Array::<GString>::new();
    for wire in &diagnostic.wires {
        wires.push(&GString::from(wire.0.as_str()));
    }
    dict.set("wires", &wires);

    dict
}
