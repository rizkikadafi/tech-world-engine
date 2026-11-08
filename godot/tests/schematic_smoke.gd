extends SceneTree
## Headless smoke test for the `SchematicHost` authoring API.
##
## Run with:
##   godot --headless --path godot --script res://tests/schematic_smoke.gd

var _failures: int = 0


func _check(condition: bool, message: String) -> void:
	if condition:
		print("[PASS] ", message)
	else:
		_failures += 1
		printerr("[FAIL] ", message)


func _probe_voltage(host: SchematicHost, id: String) -> float:
	var state: Dictionary = host.state_snapshot()
	var probes: Dictionary = state.get("probes", {})
	if not probes.has(id):
		return NAN
	return float(probes[id])


func _initialize() -> void:
	var host := SchematicHost.new()
	root.add_child(host)

	# --- Catalog -----------------------------------------------------------
	var kinds := host.part_kinds()
	_check(kinds.size() >= 11, "catalog exposes all builtin kinds")

	var has_resistor := false
	var resistor_defaults := {}
	for kind in kinds:
		if String(kind["id"]) == "resistor":
			has_resistor = true
			resistor_defaults = kind.get("defaults", {})
	_check(has_resistor, "catalog contains the resistor")
	_check(
		absf(float(resistor_defaults.get("resistance", 0.0)) - 1000.0) < 1e-9,
		"resistor defaults are populated"
	)

	# --- Build a voltage divider ------------------------------------------
	_check(host.new_document("divider") == OK, "new_document succeeds")
	_check(host.add_part("v1", "voltage_source", 320, 224) == OK, "add voltage source")
	_check(host.add_part("r1", "resistor", 480, 160) == OK, "add r1")
	_check(host.add_part("r2", "resistor", 480, 320) == OK, "add r2")
	_check(host.add_part("gnd", "ground", 480, 448) == OK, "add ground")
	_check(host.add_part("out", "probe", 688, 224) == OK, "add probe")

	_check(host.connect_pins("v1", "pos", "r1", "a") != "", "connect v1.pos -> r1.a")
	_check(host.connect_pins("r1", "b", "r2", "a") != "", "connect r1.b -> r2.a")
	_check(host.connect_pins("r2", "b", "gnd", "gnd") != "", "connect r2.b -> gnd")
	_check(host.connect_pins("v1", "neg", "gnd", "gnd") != "", "connect v1.neg -> gnd")
	_check(host.connect_pins("out", "node", "r1", "b") != "", "connect probe")

	var snapshot := host.snapshot()
	_check(snapshot.get("parts", []).size() == 5, "snapshot reports 5 parts")
	_check(snapshot.get("wires", []).size() == 5, "snapshot reports 5 wires")
	_check(snapshot.get("can_undo", false), "undo is available after edits")
	_check(host.revision() == 10, "revision tracks every edit")

	var errors := 0
	for diagnostic in host.diagnostics():
		if String(diagnostic["severity"]) == "error":
			errors += 1
	_check(errors == 0, "divider has no error diagnostics")

	# --- Solve -------------------------------------------------------------
	_check(host.compile() == OK, "compile succeeds")
	_check(host.run_dc() == OK, "run_dc succeeds")
	_check(host.has_state(), "DC state is cached")
	_check(
		absf(_probe_voltage(host, "out") - 2.5) < 1e-6,
		"equal divider produces 2.5 V"
	)

	# --- Parameter edits invalidate and re-solve --------------------------
	_check(host.set_param("r2", "resistance", 3000.0) == OK, "set r2 resistance")
	_check(not host.has_state(), "edit invalidates the DC cache")
	_check(host.run_dc() == OK, "run_dc after edit succeeds")
	_check(
		absf(_probe_voltage(host, "out") - 3.75) < 1e-6,
		"3000 ohm divider produces 3.75 V"
	)

	# --- Undo / redo -------------------------------------------------------
	_check(host.can_undo(), "undo stack is populated")
	_check(host.undo(), "undo succeeds")
	_check(host.can_redo(), "redo stack is populated")
	_check(host.run_dc() == OK, "run_dc after undo succeeds")
	_check(
		absf(_probe_voltage(host, "out") - 2.5) < 1e-6,
		"undo restores the original resistor"
	)
	_check(host.redo(), "redo succeeds")
	_check(host.run_dc() == OK, "run_dc after redo succeeds")
	_check(
		absf(_probe_voltage(host, "out") - 3.75) < 1e-6,
		"redo reapplies the parameter"
	)

	# --- Invalid operations ------------------------------------------------
	_check(
		host.add_part("bad", "not_a_kind", 0, 0) == ERR_INVALID_PARAMETER,
		"unknown kind is rejected"
	)
	_check(
		host.set_param("r1", "resistance", Vector2(1, 2)) == ERR_INVALID_PARAMETER,
		"unsupported Variant type is rejected"
	)
	_check(
		host.connect_pins("r1", "a", "r1", "a") == "",
		"self-wire is rejected"
	)

	# --- Persistence -------------------------------------------------------
	var json := host.document_json()
	_check(json != "", "document JSON is non-empty")
	_check(host.load_document_json(json) == OK, "document JSON round-trips")
	_check(host.snapshot().get("parts", []).size() == 5, "round-trip keeps parts")

	var path := "user://schematic_smoke.twsch"
	_check(host.save_document(path) == OK, "save_document writes user:// file")
	_check(host.new_document("scratch") == OK, "new_document clears the document")
	_check(host.snapshot().get("parts", []).size() == 0, "scratch document is empty")
	_check(host.load_document(path) == OK, "load_document reads user:// file")
	_check(host.document_name() == "divider", "loaded document keeps its name")

	# --- Wire removal ------------------------------------------------------
	var wires: Array = host.snapshot().get("wires", [])
	_check(wires.size() == 5, "loaded document still has 5 wires")
	_check(host.disconnect_wire(String(wires[0]["id"])) == OK, "disconnect_wire succeeds")
	_check(host.snapshot().get("wires", []).size() == 4, "wire removal is reflected")

	# --- Lifecycle ---------------------------------------------------------
	host.unload_document()
	_check(not host.has_document(), "unload_document clears the session")

	host.queue_free()

	if _failures == 0:
		print("schematic smoke: OK")
		quit(0)
	else:
		printerr("schematic smoke: %d failure(s)" % _failures)
		quit(1)
