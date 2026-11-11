use std::collections::BTreeSet;

use crate::diagnostics::Diagnostic;
use crate::document::{CircuitDocument, Part, PartId, PartKind, PinRef};
use crate::net::{self, NetMap};

/// Validate a document without compiling it.
///
/// Errors block compilation; warnings are advisory (dangling pins are common
/// while the user is still drawing).
pub fn validate(document: &CircuitDocument) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    validate_structure(document, &mut diagnostics);
    validate_parameters(document, &mut diagnostics);

    let has_structural_errors = diagnostics.iter().any(Diagnostic::is_error);
    if !has_structural_errors {
        match net::resolve(document) {
            Ok(nets) => validate_nets(document, &nets, &mut diagnostics),
            Err(error) => diagnostics.push(
                Diagnostic::error("net_resolution", error.to_string()),
            ),
        }
    }

    diagnostics
}

fn validate_structure(document: &CircuitDocument, diagnostics: &mut Vec<Diagnostic>) {
    let mut part_ids = BTreeSet::new();
    for part in document.parts() {
        if !part_ids.insert(part.id.clone()) {
            diagnostics.push(
                Diagnostic::error(
                    "duplicate_part",
                    format!("duplicate part id '{}'", part.id),
                )
                .with_part(part.id.clone()),
            );
        }
    }

    let mut wire_ids = BTreeSet::new();
    for wire in document.wires() {
        if !wire_ids.insert(wire.id.clone()) {
            diagnostics.push(
                Diagnostic::error(
                    "duplicate_wire",
                    format!("duplicate wire id '{}'", wire.id),
                )
                .with_wire(wire.id.clone()),
            );
        }

        check_pin_ref(document, &wire.from, diagnostics, Some(&wire.id));
        check_pin_ref(document, &wire.to, diagnostics, Some(&wire.id));

        if wire.from == wire.to {
            diagnostics.push(
                Diagnostic::error(
                    "self_wire",
                    format!("wire '{}' connects pin '{}' to itself", wire.id, wire.from),
                )
                .with_wire(wire.id.clone()),
            );
        }
    }

    let mut wire_pairs = BTreeSet::new();
    for wire in document.wires() {
        let pair = if wire.from <= wire.to {
            (wire.from.clone(), wire.to.clone())
        } else {
            (wire.to.clone(), wire.from.clone())
        };
        if !wire_pairs.insert(pair) {
            diagnostics.push(
                Diagnostic::warning(
                    "duplicate_connection",
                    format!(
                        "wire '{}' duplicates an existing connection between '{}' and '{}'",
                        wire.id, wire.from, wire.to
                    ),
                )
                .with_wire(wire.id.clone()),
            );
        }
    }
}

fn check_pin_ref(
    document: &CircuitDocument,
    pin: &PinRef,
    diagnostics: &mut Vec<Diagnostic>,
    wire: Option<&crate::document::WireId>,
) {
    match document.parts().iter().find(|part| part.id == pin.part) {
        None => {
            let mut diagnostic = Diagnostic::error(
                "unknown_part",
                format!("wire references unknown part '{}'", pin.part),
            )
            .with_part(pin.part.clone());
            if let Some(wire) = wire {
                diagnostic = diagnostic.with_wire(wire.clone());
            }
            diagnostics.push(diagnostic);
        }
        Some(part) if !part.has_pin(&pin.pin) => {
            let mut diagnostic = Diagnostic::error(
                "unknown_pin",
                format!(
                    "wire references unknown pin '{}' on part '{}'",
                    pin.pin, part.id
                ),
            )
            .with_part(part.id.clone());
            if let Some(wire) = wire {
                diagnostic = diagnostic.with_wire(wire.clone());
            }
            diagnostics.push(diagnostic);
        }
        Some(_) => {}
    }
}

fn validate_parameters(document: &CircuitDocument, diagnostics: &mut Vec<Diagnostic>) {
    for part in document.parts() {
        match part.kind {
            PartKind::Resistor => {
                check_positive(part, "resistance", 1000.0, diagnostics);
            }
            PartKind::Capacitor => {
                check_positive(part, "capacitance", 1e-6, diagnostics);
            }
            PartKind::Lamp => {
                check_positive(part, "resistance", 100.0, diagnostics);
            }
            PartKind::VoltageSource => {
                check_finite(part, "voltage", 5.0, diagnostics);
            }
            PartKind::CurrentSource => {
                check_finite(part, "current", 0.001, diagnostics);
            }
            PartKind::Nmos | PartKind::Pmos => {
                check_finite(part, "vth", 0.8, diagnostics);
                check_positive(part, "beta", 1e-3, diagnostics);
            }
            PartKind::Switch => {
                let on = check_positive(part, "on_resistance", 1e-3, diagnostics);
                let off = check_positive(part, "off_resistance", 1e9, diagnostics);
                if let (Some(on), Some(off)) = (on, off) {
                    if off <= on {
                        diagnostics.push(
                            Diagnostic::error(
                                "invalid_parameter",
                                format!(
                                    "off_resistance on part '{}' must be greater than on_resistance",
                                    part.id
                                ),
                            )
                            .with_part(part.id.clone()),
                        );
                    }
                }
            }
            PartKind::Ground | PartKind::Diode | PartKind::Probe | PartKind::IoPin | PartKind::Chip => {}
        }
    }
}

fn check_finite(
    part: &Part,
    key: &str,
    default: f64,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<f64> {
    let value = match read_number(part, key, default, diagnostics) {
        Some(value) => value,
        None => return None,
    };
    if !value.is_finite() {
        diagnostics.push(invalid_parameter(part, key, "must be finite"));
        return None;
    }
    Some(value)
}

fn check_positive(
    part: &Part,
    key: &str,
    default: f64,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<f64> {
    let value = check_finite(part, key, default, diagnostics)?;
    if value <= 0.0 {
        diagnostics.push(invalid_parameter(part, key, "must be greater than zero"));
        return None;
    }
    Some(value)
}

fn read_number(
    part: &Part,
    key: &str,
    default: f64,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<f64> {
    match part.params.get(key) {
        None => Some(default),
        Some(value) => match value.as_f64() {
            Some(value) => Some(value),
            None => {
                diagnostics.push(invalid_parameter(part, key, "must be a number"));
                None
            }
        },
    }
}

fn invalid_parameter(part: &Part, key: &str, reason: &str) -> Diagnostic {
    Diagnostic::error(
        "invalid_parameter",
        format!("{} on part '{}' {}", key, part.id, reason),
    )
    .with_part(part.id.clone())
}

fn validate_nets(
    document: &CircuitDocument,
    nets: &NetMap,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let ground_parts: BTreeSet<PartId> = document
        .parts()
        .iter()
        .filter(|part| part.kind.is_ground())
        .map(|part| part.id.clone())
        .collect();

    if ground_parts.is_empty() {
        diagnostics.push(Diagnostic::error(
            "missing_ground",
            "circuit has no ground part; electrical analysis needs a reference node",
        ));
    }

    if let Some(ground_net) = nets.nets.iter().find(|net| net.is_ground) {
        let only_ground_pins = ground_net
            .pins
            .iter()
            .all(|pin| ground_parts.contains(&pin.part));
        if only_ground_pins {
            diagnostics.push(
                Diagnostic::error(
                    "ground_not_connected",
                    "ground part is not connected to any non-ground pin",
                )
                .with_part(ground_net.pins[0].part.clone()),
            );
        }
    }

    let mut used_pins = BTreeSet::new();
    for wire in document.wires() {
        used_pins.insert(wire.from.clone());
        used_pins.insert(wire.to.clone());
    }

    for part in document.parts() {
        for pin_name in part.pin_names() {
            let pin = PinRef::new(part.id.clone(), pin_name.clone());
            if !used_pins.contains(&pin) {
                diagnostics.push(
                    Diagnostic::warning(
                        "dangling_pin",
                        format!(
                            "pin '{}' on part '{}' is not connected",
                            pin_name, part.id
                        ),
                    )
                    .with_part(part.id.clone()),
                );
            }
        }
    }

    for part in document.parts() {
        if !matches!(
            part.kind,
            PartKind::VoltageSource | PartKind::CurrentSource
        ) {
            continue;
        }

        let positive = PinRef::new(part.id.clone(), "pos");
        let negative = PinRef::new(part.id.clone(), "neg");
        let positive_net = nets.net_for_pin(&positive);
        let negative_net = nets.net_for_pin(&negative);

        if positive_net.is_some() && positive_net == negative_net {
            diagnostics.push(
                Diagnostic::error(
                    "source_short",
                    format!(
                        "source '{}' has both terminals on the same net",
                        part.id
                    ),
                )
                .with_part(part.id.clone()),
            );
        }
    }
}
