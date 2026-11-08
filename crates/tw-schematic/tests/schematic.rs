use tw_schematic::{
    Command, Part, PartKind, PinRef, Point2, SchematicSession, WireId,
};
use tw_schematic::{CircuitDocument, NetId};

fn part(id: &str, kind: PartKind) -> Part {
    Part::new(id, kind, Point2::default())
}

fn connect(
    document: &mut CircuitDocument,
    from_part: &str,
    from_pin: &str,
    to_part: &str,
    to_pin: &str,
) -> WireId {
    document
        .connect(
            PinRef::new(from_part, from_pin),
            PinRef::new(to_part, to_pin),
        )
        .expect("wire should be valid")
}

fn voltage_divider() -> CircuitDocument {
    let mut document = CircuitDocument::new("voltage-divider");

    document
        .add_part(part("v1", PartKind::VoltageSource).with_param("voltage", 12.0))
        .unwrap();
    document
        .add_part(part("r1", PartKind::Resistor).with_param("resistance", 1000.0))
        .unwrap();
    document
        .add_part(part("r2", PartKind::Resistor).with_param("resistance", 2000.0))
        .unwrap();
    document
        .add_part(part("gnd", PartKind::Ground))
        .unwrap();
    document
        .add_part(part("probe", PartKind::Probe))
        .unwrap();

    connect(&mut document, "v1", "pos", "r1", "a");
    connect(&mut document, "r1", "b", "r2", "a");
    connect(&mut document, "r2", "b", "gnd", "gnd");
    connect(&mut document, "v1", "neg", "gnd", "gnd");
    connect(&mut document, "probe", "node", "r1", "b");

    document
}

#[test]
fn part_kind_ids_roundtrip_and_new_parts_have_defaults() {
    for kind in PartKind::ALL {
        assert_eq!(PartKind::from_id(kind.id()), Some(kind));
    }
    assert_eq!(PartKind::from_id("not-a-kind"), None);

    let resistor = Part::new("r1", PartKind::Resistor, Point2::default());
    assert_eq!(resistor.f64_param("resistance", 0.0), 1000.0);

    let switch = Part::new("sw", PartKind::Switch, Point2::default());
    assert!(!switch.bool_param("closed", true));
    assert_eq!(switch.f64_param("off_resistance", 0.0), 1e9);
}

#[test]
fn voltage_divider_solves_dc() {
    let document = voltage_divider();

    let diagnostics = document.validate();
    assert!(
        diagnostics.iter().all(|diagnostic| !diagnostic.is_error()),
        "unexpected errors: {diagnostics:?}"
    );

    let compiled = document.compile().unwrap();
    let state = compiled.run_dc().unwrap();

    let probe_voltage = compiled
        .probe_voltage(&state, "probe")
        .expect("probe should be compiled");
    assert!(
        (probe_voltage - 8.0).abs() < 1e-6,
        "expected 8 V at the divider midpoint, got {probe_voltage}"
    );

    assert_eq!(compiled.nets.nets.len(), 3);
    assert_eq!(
        compiled.nets.nets.iter().filter(|net| net.is_ground).count(),
        1
    );
    assert!(compiled.nets.net(NetId::GROUND).is_some());
}

#[test]
fn missing_ground_is_an_error() {
    let mut document = CircuitDocument::new("no-ground");
    document
        .add_part(part("v1", PartKind::VoltageSource).with_param("voltage", 5.0))
        .unwrap();
    document
        .add_part(part("r1", PartKind::Resistor).with_param("resistance", 1000.0))
        .unwrap();
    connect(&mut document, "v1", "pos", "r1", "a");
    connect(&mut document, "r1", "b", "v1", "neg");

    let diagnostics = document.validate();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "missing_ground" && diagnostic.is_error()),
        "expected missing_ground: {diagnostics:?}"
    );
    assert!(document.compile().is_err());
}

#[test]
fn voltage_source_short_is_an_error() {
    let mut document = CircuitDocument::new("short");
    document
        .add_part(part("v1", PartKind::VoltageSource).with_param("voltage", 5.0))
        .unwrap();
    document
        .add_part(part("gnd", PartKind::Ground))
        .unwrap();
    connect(&mut document, "v1", "pos", "gnd", "gnd");
    connect(&mut document, "v1", "neg", "gnd", "gnd");

    let diagnostics = document.validate();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "source_short" && diagnostic.is_error()),
        "expected source_short: {diagnostics:?}"
    );
    assert!(document.compile().is_err());
}

#[test]
fn dangling_pin_is_a_warning_not_an_error() {
    let mut document = CircuitDocument::new("dangling");
    document
        .add_part(part("r1", PartKind::Resistor).with_param("resistance", 1000.0))
        .unwrap();
    document
        .add_part(part("gnd", PartKind::Ground))
        .unwrap();
    connect(&mut document, "r1", "a", "gnd", "gnd");

    let diagnostics = document.validate();
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "dangling_pin" && diagnostic.parts.iter().any(|id| id.0 == "r1")
        }),
        "expected dangling_pin: {diagnostics:?}"
    );
    assert!(diagnostics.iter().all(|diagnostic| !diagnostic.is_error()));
    assert!(document.compile().is_ok());
}

#[test]
fn duplicate_connection_is_reported_once() {
    let mut document = CircuitDocument::new("duplicate");
    document
        .add_part(part("r1", PartKind::Resistor).with_param("resistance", 1000.0))
        .unwrap();
    document
        .add_part(part("gnd", PartKind::Ground))
        .unwrap();
    connect(&mut document, "r1", "a", "gnd", "gnd");
    connect(&mut document, "r1", "a", "gnd", "gnd");

    let diagnostics = document.validate();
    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "duplicate_connection")
            .count(),
        1,
        "{diagnostics:?}"
    );
}

#[test]
fn document_json_roundtrip_preserves_everything() {
    let document = voltage_divider();
    let json = document.to_json_string().unwrap();
    let restored = CircuitDocument::from_json_str(&json).unwrap();

    assert_eq!(document, restored);
    assert_eq!(restored.version(), 1);
}

#[test]
fn command_json_roundtrip() {
    let command = Command::Connect {
        id: None,
        from: PinRef::new("r1", "a"),
        to: PinRef::new("gnd", "gnd"),
    };

    let json = serde_json::to_string(&command).unwrap();
    let restored: Command = serde_json::from_str(&json).unwrap();
    assert_eq!(command, restored);
}

#[test]
fn commands_support_undo_and_redo() {
    let mut session = SchematicSession::new(CircuitDocument::new("editable"));

    let ground = part("gnd", PartKind::Ground);
    session
        .apply(&Command::AddPart { part: ground })
        .unwrap();
    session
        .apply(&Command::AddPart {
            part: part("r1", PartKind::Resistor).with_param("resistance", 1000.0),
        })
        .unwrap();
    session
        .apply(&Command::Connect {
            id: None,
            from: PinRef::new("r1", "a"),
            to: PinRef::new("gnd", "gnd"),
        })
        .unwrap();

    assert_eq!(session.revision(), 3);
    assert_eq!(session.document().parts().len(), 2);
    assert_eq!(session.document().wires().len(), 1);

    assert!(session.undo());
    assert_eq!(session.document().wires().len(), 0);
    assert!(session.can_redo());

    assert!(session.undo());
    assert_eq!(session.document().parts().len(), 1);
    assert!(session.undo());
    assert_eq!(session.document().parts().len(), 0);
    assert!(!session.can_undo());

    assert!(session.redo());
    assert!(session.redo());
    assert!(session.redo());
    assert_eq!(session.document().parts().len(), 2);
    assert_eq!(session.document().wires().len(), 1);
    assert!(!session.can_redo());
}

#[test]
fn a_new_edit_clears_the_redo_stack() {
    let mut session = SchematicSession::new(CircuitDocument::new("redo-reset"));

    session
        .apply(&Command::AddPart {
            part: part("gnd", PartKind::Ground),
        })
        .unwrap();
    session.undo();
    assert!(session.can_redo());

    session
        .apply(&Command::AddPart {
            part: part("r1", PartKind::Resistor),
        })
        .unwrap();
    assert!(!session.can_redo());
}

#[test]
fn switch_and_lamp_compile_to_a_resistive_load() {
    let mut document = CircuitDocument::new("lamp");
    document
        .add_part(part("v1", PartKind::VoltageSource).with_param("voltage", 12.0))
        .unwrap();
    document
        .add_part(part("sw", PartKind::Switch).with_param("closed", true))
        .unwrap();
    document
        .add_part(part("lamp", PartKind::Lamp).with_param("resistance", 100.0))
        .unwrap();
    document
        .add_part(part("gnd", PartKind::Ground))
        .unwrap();

    connect(&mut document, "v1", "pos", "sw", "a");
    connect(&mut document, "sw", "b", "lamp", "a");
    connect(&mut document, "lamp", "b", "gnd", "gnd");
    connect(&mut document, "v1", "neg", "gnd", "gnd");

    let mut session = SchematicSession::new(document);
    let mut compiled = session.compile().unwrap().clone();
    session.run_dc().unwrap();

    fn lamp_voltage(session: &SchematicSession, compiled: &tw_schematic::CompiledCircuit) -> f64 {
        let state = session.state().unwrap();
        compiled
            .pin_voltage(state, &PinRef::new("lamp", "a"))
            .unwrap()
            - compiled
                .pin_voltage(state, &PinRef::new("lamp", "b"))
                .unwrap()
    }

    assert!(
        lamp_voltage(&session, &compiled) > 11.9,
        "closed switch should apply almost the full supply to the lamp"
    );

    session
        .apply(&Command::SetParam {
            id: "sw".into(),
            key: "closed".to_string(),
            value: Some(serde_json::Value::Bool(false)),
        })
        .unwrap();
    compiled = session.compile().unwrap().clone();
    session.run_dc().unwrap();

    assert!(
        lamp_voltage(&session, &compiled) < 0.01,
        "open switch should leave the lamp unpowered"
    );
}

#[test]
fn cmos_inverter_document_compiles_to_two_mosfets() {
    let mut document = CircuitDocument::new("cmos-inverter");
    document
        .add_part(part("vdd", PartKind::VoltageSource).with_param("voltage", 5.0))
        .unwrap();
    document
        .add_part(part("vin", PartKind::VoltageSource).with_param("voltage", 0.0))
        .unwrap();
    document
        .add_part(part("pmos", PartKind::Pmos))
        .unwrap();
    document
        .add_part(part("nmos", PartKind::Nmos))
        .unwrap();
    document
        .add_part(part("gnd", PartKind::Ground))
        .unwrap();
    document
        .add_part(part("out", PartKind::Probe))
        .unwrap();

    connect(&mut document, "vdd", "pos", "pmos", "source");
    connect(&mut document, "vdd", "neg", "gnd", "gnd");

    connect(&mut document, "vin", "pos", "pmos", "gate");
    connect(&mut document, "vin", "pos", "nmos", "gate");
    connect(&mut document, "vin", "neg", "gnd", "gnd");

    connect(&mut document, "pmos", "drain", "nmos", "drain");
    connect(&mut document, "pmos", "drain", "out", "node");
    connect(&mut document, "nmos", "source", "gnd", "gnd");

    let diagnostics = document.validate();
    assert!(
        diagnostics.iter().all(|diagnostic| !diagnostic.is_error()),
        "unexpected errors: {diagnostics:?}"
    );

    let compiled = document.compile().unwrap();
    assert_eq!(compiled.circuit.mosfets.len(), 2);
    assert!(compiled.probe_node("out").is_some());
}

#[test]
fn invalid_parameter_is_reported_before_compilation() {
    let mut document = CircuitDocument::new("bad-resistor");
    document
        .add_part(part("r1", PartKind::Resistor).with_param("resistance", -5.0))
        .unwrap();
    document
        .add_part(part("gnd", PartKind::Ground))
        .unwrap();
    connect(&mut document, "r1", "a", "gnd", "gnd");

    let diagnostics = document.validate();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "invalid_parameter"),
        "expected invalid_parameter: {diagnostics:?}"
    );
    assert!(document.compile().is_err());
}
