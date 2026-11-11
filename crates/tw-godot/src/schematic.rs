//! GDExtension bridge for `tw-schematic` authoring documents.
//!
//! `SchematicHost` owns a [`SchematicSession`] and exposes editing operations,
//! snapshots, diagnostics, and DC results to GDScript. It contains no authoring
//! or solver logic; every operation is forwarded to `tw-schematic`.

use godot::classes::INode;
use godot::global::Error;
use godot::prelude::*;
use serde_json::{Map, Value};
use tw_electrical::Characterizer;
use tw_schematic::{
    ChipDefinition, ChipPin, ChipRegistry, CircuitDocument, Command, Part, PartKind, PinDirection,
    PinRef, PinSide, Point2, SchematicSession, WireId,
};

use crate::schematic_convert::{
    diagnostic_to_dict, json_to_variant, pin_role_name, variant_to_json,
};
use crate::{is_godot_path, read_godot_text, write_godot_text};

/// Godot node that owns one editable circuit document.
///
/// Editing happens through methods such as [`SchematicHost::add_part`] and
/// [`SchematicHost::connect_pins`]. GDScript should pull [`SchematicHost::snapshot`]
/// after every successful edit to render the new state.
#[derive(GodotClass)]
#[class(base = Node)]
pub struct SchematicHost {
    base: Base<Node>,
    session: Option<SchematicSession>,
    registry: ChipRegistry,
}

#[godot_api]
impl INode for SchematicHost {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            session: None,
            registry: ChipRegistry::new(),
        }
    }
}

#[godot_api]
impl SchematicHost {
    // --- Document lifecycle -------------------------------------------------

    /// Create an empty document, replacing any currently loaded one.
    #[func]
    fn new_document(&mut self, name: GString) -> Error {
        let name = if name.is_empty() {
            "untitled-circuit".to_string()
        } else {
            name.to_string()
        };
        self.session = Some(SchematicSession::with_registry(
            CircuitDocument::new(name),
            self.registry.clone(),
        ));
        Error::OK
    }

    #[func]
    fn unload_document(&mut self) {
        self.session = None;
    }

    #[func]
    fn has_document(&self) -> bool {
        self.session.is_some()
    }

    #[func]
    fn document_name(&self) -> GString {
        self.session
            .as_ref()
            .map(|session| GString::from(session.document().name()))
            .unwrap_or_else(|| GString::from(""))
    }

    #[func]
    fn set_document_name(&mut self, name: GString) -> Error {
        let Some(session) = self.session.as_mut() else {
            return Error::FAILED;
        };
        // Renaming is presentation-only, so it does not create an undo entry.
        // `SchematicSession` exposes the document immutably; clone it so the
        // rename still goes through the same owned session value.
        let mut document = session.document().clone();
        document.set_name(name.to_string());
        *session = SchematicSession::new(document);
        Error::OK
    }

    #[func]
    fn revision(&self) -> i64 {
        self.session
            .as_ref()
            .map(|session| session.revision() as i64)
            .unwrap_or(0)
    }

    /// Serialize the current document to JSON.
    #[func]
    fn document_json(&self) -> GString {
        let Some(session) = self.session.as_ref() else {
            return GString::from("");
        };
        match session.document().to_json_string() {
            Ok(json) => GString::from(json.as_str()),
            Err(error) => {
                godot_error!("document_json failed: {}", error);
                GString::from("")
            }
        }
    }

    #[func]
    fn load_document_json(&mut self, json: GString) -> Error {
        match CircuitDocument::from_json_str(&json.to_string()) {
            Ok(document) => {
                self.session = Some(SchematicSession::with_registry(
                    document,
                    self.registry.clone(),
                ));
                Error::OK
            }
            Err(error) => {
                godot_error!("load_document_json failed: {}", error);
                Error::FAILED
            }
        }
    }

    /// Load a `.twsch` document. `res://` and `user://` go through Godot's
    /// `FileAccess`, so exported PCK builds work too.
    #[func]
    fn load_document(&mut self, path: GString) -> Error {
        let path = path.to_string();
        let result = if is_godot_path(&path) {
            read_godot_text(&path).and_then(|text| {
                CircuitDocument::from_json_str(&text).map_err(|error| error.to_string())
            })
        } else {
            CircuitDocument::load(&path).map_err(|error| error.to_string())
        };

        match result {
            Ok(document) => {
                self.session = Some(SchematicSession::with_registry(
                    document,
                    self.registry.clone(),
                ));
                Error::OK
            }
            Err(message) => {
                godot_error!("failed to load schematic '{}': {}", path, message);
                Error::FAILED
            }
        }
    }

    #[func]
    fn save_document(&self, path: GString) -> Error {
        let Some(session) = self.session.as_ref() else {
            godot_error!("save_document: no document is loaded");
            return Error::FAILED;
        };
        let path = path.to_string();
        let json = match session.document().to_json_string() {
            Ok(json) => json,
            Err(error) => {
                godot_error!("save_document failed to serialize: {}", error);
                return Error::FAILED;
            }
        };

        let result = if is_godot_path(&path) {
            write_godot_text(&path, &json)
        } else {
            std::fs::write(&path, json).map_err(|error| error.to_string())
        };

        match result {
            Ok(()) => Error::OK,
            Err(message) => {
                godot_error!("failed to save schematic '{}': {}", path, message);
                Error::FAILED
            }
        }
    }

    // --- Catalog -----------------------------------------------------------

    /// Every builtin part kind, its pins, and its default parameters.
    #[func]
    fn part_kinds(&self) -> Array<VarDictionary> {
        let mut kinds = Array::<VarDictionary>::new();
        for kind in PartKind::ALL {
            let mut entry = VarDictionary::new();
            entry.set("id", kind.id());
            entry.set("name", kind.display_name());

            let mut pins = Array::<VarDictionary>::new();
            for pin in kind.pins() {
                let mut pin_entry = VarDictionary::new();
                pin_entry.set("name", pin.name);
                pin_entry.set("role", pin_role_name(pin.role));
                pins.push(&pin_entry);
            }
            entry.set("pins", &pins);

            let defaults: Map<String, Value> = kind.default_params().into_iter().collect();
            entry.set("defaults", &json_to_variant(&Value::Object(defaults)));

            kinds.push(&entry);
        }
        kinds
    }

    // --- Editing -----------------------------------------------------------

    #[func]
    fn add_part(&mut self, id: GString, kind: GString, x: f64, y: f64) -> Error {
        let Some(session) = self.session.as_mut() else {
            godot_error!("add_part: no document is loaded");
            return Error::FAILED;
        };
        let kind_id = kind.to_string();
        let Some(kind) = PartKind::from_id(&kind_id) else {
            godot_error!("add_part: unknown part kind '{}'", kind_id);
            return Error::ERR_INVALID_PARAMETER;
        };

        let part = Part::new(id.to_string(), kind, Point2::new(x, y));
        apply_command(session, &Command::AddPart { part })
    }

    #[func]
    fn remove_part(&mut self, id: GString) -> Error {
        let Some(session) = self.session.as_mut() else {
            return Error::FAILED;
        };
        apply_command(
            session,
            &Command::RemovePart {
                id: id.to_string().into(),
            },
        )
    }

    #[func]
    fn move_part(&mut self, id: GString, x: f64, y: f64) -> Error {
        let Some(session) = self.session.as_mut() else {
            return Error::FAILED;
        };
        apply_command(
            session,
            &Command::MovePart {
                id: id.to_string().into(),
                position: Point2::new(x, y),
            },
        )
    }

    /// Set one parameter. `value` must be a plain data Variant
    /// (bool/int/float/String/Array/Dictionary).
    #[func]
    fn set_param(&mut self, id: GString, key: GString, value: Variant) -> Error {
        let Some(session) = self.session.as_mut() else {
            return Error::FAILED;
        };
        let Some(value) = variant_to_json(&value) else {
            godot_error!("set_param: unsupported Variant type {:?}", value.get_type());
            return Error::ERR_INVALID_PARAMETER;
        };
        apply_command(
            session,
            &Command::SetParam {
                id: id.to_string().into(),
                key: key.to_string(),
                value: Some(value),
            },
        )
    }

    #[func]
    fn clear_param(&mut self, id: GString, key: GString) -> Error {
        let Some(session) = self.session.as_mut() else {
            return Error::FAILED;
        };
        apply_command(
            session,
            &Command::SetParam {
                id: id.to_string().into(),
                key: key.to_string(),
                value: None,
            },
        )
    }

    #[func]
    fn rename_part(&mut self, id: GString, label: GString) -> Error {
        let Some(session) = self.session.as_mut() else {
            return Error::FAILED;
        };
        let label = label.to_string();
        apply_command(
            session,
            &Command::RenamePart {
                id: id.to_string().into(),
                label: if label.is_empty() { None } else { Some(label) },
            },
        )
    }

    /// Connect two pins and return the generated wire id.
    ///
    /// Returns an empty string on failure, so callers must check the result.
    #[func]
    fn connect_pins(
        &mut self,
        from_part: GString,
        from_pin: GString,
        to_part: GString,
        to_pin: GString,
    ) -> GString {
        let Some(session) = self.session.as_mut() else {
            return GString::from("");
        };
        let wire_id = session.document().fresh_wire_id();
        let command = Command::Connect {
            id: Some(wire_id.clone()),
            from: PinRef::new(from_part.to_string(), from_pin.to_string()),
            to: PinRef::new(to_part.to_string(), to_pin.to_string()),
        };
        match session.apply(&command) {
            Ok(_) => GString::from(wire_id.0.as_str()),
            Err(error) => {
                godot_error!("connect_pins failed: {}", error);
                GString::from("")
            }
        }
    }

    #[func]
    fn disconnect_wire(&mut self, wire_id: GString) -> Error {
        let Some(session) = self.session.as_mut() else {
            return Error::FAILED;
        };
        apply_command(
            session,
            &Command::Disconnect {
                id: WireId::new(wire_id.to_string()),
            },
        )
    }

    /// Apply a serialized [`tw_schematic::Command`] (the JSON shared with Rust).
    #[func]
    fn apply_command_json(&mut self, json: GString) -> Error {
        let Some(session) = self.session.as_mut() else {
            return Error::FAILED;
        };
        let command: Command = match serde_json::from_str(&json.to_string()) {
            Ok(command) => command,
            Err(error) => {
                godot_error!("apply_command_json failed to parse: {}", error);
                return Error::ERR_INVALID_PARAMETER;
            }
        };
        apply_command(session, &command)
    }

    // --- History -----------------------------------------------------------

    #[func]
    fn can_undo(&self) -> bool {
        self.session
            .as_ref()
            .is_some_and(|session| session.can_undo())
    }

    #[func]
    fn can_redo(&self) -> bool {
        self.session
            .as_ref()
            .is_some_and(|session| session.can_redo())
    }

    #[func]
    fn undo(&mut self) -> bool {
        self.session.as_mut().is_some_and(|session| session.undo())
    }

    #[func]
    fn redo(&mut self) -> bool {
        self.session.as_mut().is_some_and(|session| session.redo())
    }

    // --- Inspection --------------------------------------------------------

    /// A render-ready snapshot: parts, wires, positions, params, and history flags.
    #[func]
    fn snapshot(&self) -> VarDictionary {
        let mut result = VarDictionary::new();
        let Some(session) = self.session.as_ref() else {
            return result;
        };
        let document = session.document();

        result.set("name", document.name());
        result.set("version", document.version() as i64);
        result.set("revision", session.revision() as i64);
        result.set("can_undo", session.can_undo());
        result.set("can_redo", session.can_redo());

        let mut parts = Array::<VarDictionary>::new();
        for part in document.parts() {
            let mut entry = VarDictionary::new();
            entry.set("id", part.id.0.as_str());
            entry.set("kind", part.kind.id());
            entry.set("kind_name", part.kind.display_name());
            entry.set("x", part.position.x);
            entry.set("y", part.position.y);
            entry.set("rotation", part.rotation);
            entry.set("label", part.label.as_deref().unwrap_or(""));

            let mut pins = Array::<GString>::new();
            for pin_name in part.pin_names() {
                pins.push(&GString::from(pin_name.as_str()));
            }
            entry.set("pins", &pins);

            let params: Map<String, Value> = part.params.clone().into_iter().collect();
            entry.set("params", &json_to_variant(&Value::Object(params)));

            parts.push(&entry);
        }
        result.set("parts", &parts);

        let mut wires = Array::<VarDictionary>::new();
        for wire in document.wires() {
            let mut entry = VarDictionary::new();
            entry.set("id", wire.id.0.as_str());
            entry.set("from_part", wire.from.part.0.as_str());
            entry.set("from_pin", wire.from.pin.as_str());
            entry.set("to_part", wire.to.part.0.as_str());
            entry.set("to_pin", wire.to.pin.as_str());
            wires.push(&entry);
        }
        result.set("wires", &wires);

        result
    }

    #[func]
    fn diagnostics(&self) -> Array<VarDictionary> {
        let Some(session) = self.session.as_ref() else {
            return Array::new();
        };
        session
            .document()
            .validate()
            .iter()
            .map(diagnostic_to_dict)
            .collect()
    }

    /// Compile the document without running a solve.
    #[func]
    fn compile(&mut self) -> Error {
        let Some(session) = self.session.as_mut() else {
            return Error::FAILED;
        };
        match session.compile() {
            Ok(_) => Error::OK,
            Err(error) => {
                godot_error!("compile failed: {}", error);
                Error::FAILED
            }
        }
    }

    /// Compile (if needed) and solve the DC operating point.
    #[func]
    fn run_dc(&mut self) -> Error {
        let Some(session) = self.session.as_mut() else {
            return Error::FAILED;
        };
        match session.run_dc() {
            Ok(_) => Error::OK,
            Err(error) => {
                godot_error!("run_dc failed: {}", error);
                Error::FAILED
            }
        }
    }

    #[func]
    fn has_state(&self) -> bool {
        self.session
            .as_ref()
            .is_some_and(|session| session.state().is_some())
    }

    /// Pin, probe, and net voltages from the last DC solve.
    ///
    /// Layout: `{ "pins": {"part:pin": volts}, "probes": {"part": volts},
    /// "nets": {"GND"|"N1": volts} }`. Returns an empty dictionary before the
    /// first successful solve.
    #[func]
    fn state_snapshot(&mut self) -> VarDictionary {
        let mut result = VarDictionary::new();
        let Some(session) = self.session.as_mut() else {
            return result;
        };
        let compiled = match session.compile() {
            Ok(compiled) => compiled.clone(),
            Err(_) => return result,
        };
        let Some(state) = session.state() else {
            return result;
        };

        let mut pins = VarDictionary::new();
        for (pin, node) in &compiled.pin_nodes {
            let key = format!("{}:{}", pin.part, pin.pin);
            pins.set(key.as_str(), state.voltage(*node));
        }
        result.set("pins", &pins);

        let mut probes = VarDictionary::new();
        for (id, node) in &compiled.probes {
            probes.set(id.0.as_str(), state.voltage(*node));
        }
        result.set("probes", &probes);

        let mut nets = VarDictionary::new();
        for (net, node) in &compiled.net_nodes {
            let name = net.to_string();
            nets.set(name.as_str(), state.voltage(*node));
        }
        result.set("nets", &nets);

        result
    }

    // --- Chip packaging & abstraction --------------------------------------

    /// Pack the currently active document into a reusable chip, characterizing its
    /// electrical behavior from real transistor simulation.
    #[func]
    fn pack_chip(
        &mut self,
        name: GString,
        category: GString,
        package_type: GString,
        save_path: GString,
    ) -> VarDictionary {
        let mut result = VarDictionary::new();
        let Some(session) = self.session.as_mut() else {
            result.set("error", "No document is loaded");
            return result;
        };

        let chip_name = name.to_string();
        if chip_name.trim().is_empty() {
            result.set("error", "Chip name cannot be empty");
            return result;
        }

        let compiled = match session.compile() {
            Ok(c) => c.clone(),
            Err(e) => {
                result.set("error", format!("Compilation error: {}", e).as_str());
                return result;
            }
        };

        let mut chip_pins = Vec::new();
        let mut input_nodes = Vec::new();
        let mut input_names = Vec::new();
        let mut output_nodes = Vec::new();
        let mut output_names = Vec::new();
        let mut supply_node = None;

        for part in session.document().parts() {
            if part.kind == PartKind::IoPin {
                let default_name = part.label.clone().unwrap_or_else(|| part.id.0.clone());
                let p_name = part.str_param("name", &default_name).to_string();
                let dir_str = part.str_param("direction", "input");

                let pin_ref = PinRef::new(part.id.clone(), "pin");
                let node_id = match compiled.pin_node(&pin_ref) {
                    Some(n) => n,
                    None => continue,
                };
                let c_node_name = match compiled.circuit.node_name(node_id) {
                    Some(s) => s.to_string(),
                    None => continue,
                };

                match dir_str {
                    "input" => {
                        chip_pins.push(ChipPin::new(p_name.clone(), PinDirection::Input, PinSide::Left));
                        input_nodes.push(c_node_name);
                        input_names.push(p_name);
                    }
                    "output" => {
                        chip_pins.push(ChipPin::new(p_name.clone(), PinDirection::Output, PinSide::Right));
                        output_nodes.push(c_node_name);
                        output_names.push(p_name);
                    }
                    "power" => {
                        chip_pins.push(ChipPin::new(p_name.clone(), PinDirection::Power, PinSide::Top));
                        supply_node = Some(c_node_name);
                    }
                    "ground" => {
                        chip_pins.push(ChipPin::new(p_name.clone(), PinDirection::Ground, PinSide::Bottom));
                    }
                    _ => {
                        chip_pins.push(ChipPin::new(p_name.clone(), PinDirection::Inout, PinSide::Left));
                    }
                }
            }
        }

        if chip_pins.is_empty() {
            result.set("error", "No IO Pins found in document. Place IO Pins to define the chip interface.");
            return result;
        }

        let characterizer = Characterizer::with_defaults();
        let mut characterization = None;

        if !input_nodes.is_empty() && !output_nodes.is_empty() {
            if let Ok(mut char_res) = characterizer.characterize_generic_circuit(
                &compiled.circuit,
                &input_nodes,
                &output_nodes,
                supply_node.as_deref(),
            ) {
                char_res.input_names = input_names.clone();
                char_res.output_names = output_names.clone();
                let mut remapped_outputs = std::collections::HashMap::new();
                for (c_node, user_out) in output_nodes.iter().zip(output_names.iter()) {
                    if let Some(single) = char_res.outputs.remove(c_node) {
                        remapped_outputs.insert(
                            user_out.clone(),
                            tw_electrical::SingleOutputCharacterization {
                                output_name: user_out.clone(),
                                ..single
                            },
                        );
                    }
                }
                char_res.outputs = remapped_outputs;
                characterization = Some(char_res);
            }
        }

        let cat = if category.is_empty() {
            "Custom".to_string()
        } else {
            category.to_string()
        };

        let pkg = if package_type.is_empty() {
            "DIP-8".to_string()
        } else {
            package_type.to_string()
        };

        let mut chip_def = ChipDefinition::new(
            chip_name.clone(),
            chip_pins.clone(),
            session.document().clone(),
        );
        chip_def.category = cat;
        chip_def.package_3d.package_type = pkg;
        chip_def.characterization = characterization.clone();

        self.registry.register(chip_def.clone());
        session.registry_mut().register(chip_def.clone());

        let path_str = save_path.to_string();
        if !path_str.trim().is_empty() {
            if let Ok(json) = chip_def.to_json_string() {
                let write_res = if is_godot_path(&path_str) {
                    write_godot_text(&path_str, &json)
                } else {
                    std::fs::write(&path_str, json).map_err(|e| e.to_string())
                };
                if let Err(e) = write_res {
                    godot_error!("Failed to save chip file '{}': {}", path_str, e);
                }
            }
        }

        result.set("success", true);
        result.set("name", chip_name.as_str());

        let mut pins_arr = Array::<VarDictionary>::new();
        for p in &chip_pins {
            let mut pd = VarDictionary::new();
            pd.set("name", p.name.as_str());
            pd.set("direction", format!("{:?}", p.direction).to_lowercase().as_str());
            pd.set("side", format!("{:?}", p.side).to_lowercase().as_str());
            pins_arr.push(&pd);
        }
        result.set("pins", &pins_arr);

        if let Some(char_data) = &characterization {
            let mut tt_dict = VarDictionary::new();
            let mut timings_dict = VarDictionary::new();
            for (out_name, single) in &char_data.outputs {
                let mut row_arr = Array::<bool>::new();
                for &b in &single.truth_table.outputs {
                    row_arr.push(b);
                }
                tt_dict.set(out_name.as_str(), &row_arr);

                let mut tm = VarDictionary::new();
                tm.set("t_pHL", single.timing.t_pHL);
                tm.set("t_pLH", single.timing.t_pLH);
                tm.set("t_rise", single.timing.t_rise);
                tm.set("t_fall", single.timing.t_fall);
                timings_dict.set(out_name.as_str(), &tm);
            }
            result.set("truth_tables", &tt_dict);
            result.set("timings", &timings_dict);
        }

        result
    }

    #[func]
    fn add_chip_part(&mut self, id: GString, chip_name: GString, x: f64, y: f64) -> Error {
        let Some(session) = self.session.as_mut() else {
            return Error::FAILED;
        };
        let c_name = chip_name.to_string();
        let pin_names: Vec<String> = if let Some(chip) = self.registry.get(&c_name) {
            chip.pins.iter().map(|p| p.name.clone()).collect()
        } else {
            godot_error!("add_chip_part: unknown chip '{}'", c_name);
            return Error::ERR_INVALID_PARAMETER;
        };

        let mut part = Part::new(id.to_string(), PartKind::Chip, Point2::new(x, y));
        part.params.insert("chip_name".to_string(), Value::from(c_name));
        part.params.insert(
            "chip_pins".to_string(),
            Value::Array(pin_names.into_iter().map(Value::from).collect()),
        );

        apply_command(session, &Command::AddPart { part })
    }

    #[func]
    fn load_chip(&mut self, path: GString) -> Error {
        let Some(session) = self.session.as_mut() else {
            return Error::FAILED;
        };
        let path_str = path.to_string();
        let json_str = if is_godot_path(&path_str) {
            match read_godot_text(&path_str) {
                Ok(s) => s,
                Err(e) => {
                    godot_error!("load_chip read error: {}", e);
                    return Error::FAILED;
                }
            }
        } else {
            match std::fs::read_to_string(&path_str) {
                Ok(s) => s,
                Err(e) => {
                    godot_error!("load_chip read error: {}", e);
                    return Error::FAILED;
                }
            }
        };

        match ChipDefinition::from_json_str(&json_str) {
            Ok(chip) => {
                self.registry.register(chip.clone());
                session.registry_mut().register(chip);
                Error::OK
            }
            Err(e) => {
                godot_error!("load_chip parse error: {}", e);
                Error::FAILED
            }
        }
    }

    #[func]
    fn list_chips(&self) -> Array<VarDictionary> {
        let mut list = Array::<VarDictionary>::new();
        for chip in self.registry.all() {
            let mut dict = VarDictionary::new();
            dict.set("name", chip.name.as_str());
            dict.set("category", chip.category.as_str());
            dict.set("description", chip.description.as_str());
            dict.set("package_type", chip.package_3d.package_type.as_str());

            let mut pins = Array::<VarDictionary>::new();
            for p in &chip.pins {
                let mut pd = VarDictionary::new();
                pd.set("name", p.name.as_str());
                pd.set("direction", format!("{:?}", p.direction).to_lowercase().as_str());
                pd.set("side", format!("{:?}", p.side).to_lowercase().as_str());
                pins.push(&pd);
            }
            dict.set("pins", &pins);
            list.push(&dict);
        }
        list
    }

    #[func]
    fn get_chip_schematic_json(&self, chip_name: GString) -> GString {
        let c_name = chip_name.to_string();
        if let Some(chip) = self.registry.get(&c_name) {
            if let Ok(json) = chip.internal_schematic.to_json_string() {
                return GString::from(json.as_str());
            }
        }
        GString::from("")
    }
}

fn apply_command(session: &mut SchematicSession, command: &Command) -> Error {
    match session.apply(command) {
        Ok(_) => Error::OK,
        Err(error) => {
            godot_error!("edit failed: {}", error);
            Error::FAILED
        }
    }
}
