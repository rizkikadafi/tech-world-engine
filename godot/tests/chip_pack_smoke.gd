extends SceneTree
## Headless smoke test for Custom Chip Packaging and Hierarchical Simulation.
##
## Run with:
##   godot --headless --path godot --script res://tests/chip_pack_smoke.gd

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

	# --- 1. Build a CMOS Inverter inside the 2D editor ----------------------
	_check(host.new_document("inverter-design") == OK, "new_document succeeds")
	_check(host.add_part("mp", "pmos", 300, 150) == OK, "add PMOS")
	_check(host.add_part("mn", "nmos", 300, 300) == OK, "add NMOS")
	_check(host.add_part("gnd", "ground", 300, 420) == OK, "add Ground")

	_check(host.add_part("p_in", "io_pin", 150, 220) == OK, "add IO Pin IN")
	_check(host.set_param("p_in", "name", "in") == OK, "set in_pin name")
	_check(host.set_param("p_in", "direction", "input") == OK, "set in_pin direction")

	_check(host.add_part("p_out", "io_pin", 450, 220) == OK, "add IO Pin OUT")
	_check(host.set_param("p_out", "name", "out") == OK, "set out_pin name")
	_check(host.set_param("p_out", "direction", "output") == OK, "set out_pin direction")

	_check(host.add_part("p_vdd", "io_pin", 300, 50) == OK, "add IO Pin VDD")
	_check(host.set_param("p_vdd", "name", "vdd") == OK, "set vdd_pin name")
	_check(host.set_param("p_vdd", "direction", "power") == OK, "set vdd_pin direction")

	# Connect gates to IN
	_check(host.connect_pins("p_in", "pin", "mp", "gate") != "", "wire in -> mp.gate")
	_check(host.connect_pins("p_in", "pin", "mn", "gate") != "", "wire in -> mn.gate")

	# Connect supplies
	_check(host.connect_pins("mp", "source", "p_vdd", "pin") != "", "wire mp.source -> vdd")
	_check(host.connect_pins("mn", "source", "gnd", "gnd") != "", "wire mn.source -> gnd")

	# Connect drains to OUT
	_check(host.connect_pins("mp", "drain", "p_out", "pin") != "", "wire mp.drain -> out")
	_check(host.connect_pins("mn", "drain", "p_out", "pin") != "", "wire mn.drain -> out")

	# --- 2. Pack as Chip & Auto-Characterize --------------------------------
	var pack_res: Dictionary = host.pack_chip(
		"cmos_inverter", "Logic", "DIP-4", "user://cmos_inverter.twchip"
	)
	_check(pack_res.get("success", false), "pack_chip succeeds")
	_check(String(pack_res.get("name", "")) == "cmos_inverter", "pack_chip name matches")

	var pins: Array = pack_res.get("pins", [])
	_check(pins.size() == 3, "chip has 3 pins (in, out, vdd)")

	# Verify empirical truth table from real MNA simulation!
	var tt_dict: Dictionary = pack_res.get("truth_tables", {})
	_check(tt_dict.has("out"), "characterization measured 'out' truth table")
	var out_tt: Array = tt_dict.get("out", [])
	# CMOS inverter truth table: 0 -> 1, 1 -> 0
	_check(out_tt.size() == 2, "truth table has 2 entries")
	_check(bool(out_tt[0]) == true and bool(out_tt[1]) == false, "truth table matches Inverter (0->1, 1->0)")

	# Verify empirical dynamic timing
	var timings_dict: Dictionary = pack_res.get("timings", {})
	_check(timings_dict.has("out"), "timings measured for 'out'")
	var out_timing: Dictionary = timings_dict.get("out", {})
	_check(float(out_timing.get("t_pHL", 0.0)) > 0.0, "measured t_pHL is positive")
	_check(float(out_timing.get("t_pLH", 0.0)) > 0.0, "measured t_pLH is positive")

	# --- 3. Verify Chip in Registry & Palette -------------------------------
	var chip_list := host.list_chips()
	var found_in_list := false
	for c in chip_list:
		if String(c["name"]) == "cmos_inverter":
			found_in_list = true
	_check(found_in_list, "chip appears in registry list")

	var inner_json := host.get_chip_schematic_json("cmos_inverter")
	_check(inner_json != "", "internal schematic JSON is retrievable")

	# --- 4. Hierarchical Composition: Double Inverter in series -------------
	_check(host.new_document("double-inverter-test") == OK, "new outer document")
	_check(host.add_part("v_supply", "voltage_source", 100, 100) == OK, "add 5V supply")
	_check(host.add_part("gnd", "ground", 100, 300) == OK, "add outer ground")

	_check(host.add_chip_part("inv1", "cmos_inverter", 300, 150) == OK, "instantiate inv1 chip")
	_check(host.add_chip_part("inv2", "cmos_inverter", 500, 150) == OK, "instantiate inv2 chip")
	_check(host.add_part("pr_mid", "probe", 400, 120) == OK, "add mid probe")
	_check(host.add_part("pr_out", "probe", 620, 150) == OK, "add out probe")

	# Connect supplies
	_check(host.connect_pins("v_supply", "pos", "inv1", "vdd") != "", "wire supply -> inv1.vdd")
	_check(host.connect_pins("inv1", "vdd", "inv2", "vdd") != "", "wire inv1.vdd -> inv2.vdd")
	_check(host.connect_pins("v_supply", "neg", "gnd", "gnd") != "", "wire supply.neg -> gnd")

	# inv1.in connected to GND (input = 0)
	_check(host.connect_pins("inv1", "in", "gnd", "gnd") != "", "wire inv1.in -> gnd")

	# inv1.out -> inv2.in
	_check(host.connect_pins("inv1", "out", "inv2", "in") != "", "wire inv1.out -> inv2.in")
	_check(host.connect_pins("inv1", "out", "pr_mid", "node") != "", "wire inv1.out -> pr_mid")

	# inv2.out -> pr_out
	_check(host.connect_pins("inv2", "out", "pr_out", "node") != "", "wire inv2.out -> pr_out")

	# Solve DC on hierarchical circuit
	_check(host.run_dc() == OK, "hierarchical DC solve succeeds")

	var v_mid := _probe_voltage(host, "pr_mid")
	var v_out := _probe_voltage(host, "pr_out")

	# input 0V -> inv1 output is HIGH (~5.0V), inv2 output is LOW (~0.0V)
	_check(absf(v_mid - 5.0) < 0.2, "inv1 out is ~5V (input=0): %f" % v_mid)
	_check(absf(v_out) < 0.2, "inv2 out is ~0V (inverter 2 inverted 5V): %f" % v_out)

	# --- 5. Verify round-trip file loading of .twchip -----------------------
	_check(host.load_chip("user://cmos_inverter.twchip") == OK, "load_chip from user:// succeeds")

	if _failures == 0:
		print("chip pack smoke: OK")
		quit(0)
	else:
		printerr("chip pack smoke: FAILED (%d checks failed)" % _failures)
		quit(1)
