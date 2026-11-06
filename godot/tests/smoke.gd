extends SceneTree
## Headless smoke test for the `tw-godot` GDExtension adapter.
##
## Run with:
##   godot --headless --path godot --script res://tests/smoke.gd

var _failures: int = 0


func _check(condition: bool, message: String) -> void:
	if condition:
		print("[PASS] ", message)
	else:
		_failures += 1
		printerr("[FAIL] ", message)


func _initialize() -> void:
	var host := SimulationHost.new()
	root.add_child(host)

	# --- Load world --------------------------------------------------------
	_check(host.load_world("res://worlds/counter.twscene") == OK, "load counter world")
	_check(host.has_world(), "world is loaded")
	_check(host.component_ids().size() == 1, "counter world has 1 component")
	_check(host.world_name() == "counter-demo", "world name is reported")

	# --- Advance & state diff ---------------------------------------------
	var events := host.advance_ticks(10)
	_check(events > 0, "advance_ticks executes events")
	_check(host.current_tick() == 10, "current tick advanced to 10")

	var changes := host.take_state_changes()
	_check(changes.size() > 0, "take_state_changes reports a diff")
	_check(int(host.component_state(1).get("count", -1)) >= 1, "counter state is readable")

	# --- Checkpoint round-trip --------------------------------------------
	var state_before: Dictionary = host.component_state(1)
	var checkpoint := host.checkpoint_json()
	_check(checkpoint != "", "checkpoint JSON is non-empty")

	host.advance_ticks(10)
	_check(host.component_state(1) != state_before, "state changes after checkpoint")

	_check(host.restore_json(checkpoint) == OK, "restore_json succeeds")
	_check(host.component_state(1) == state_before, "restored component state matches")

	# --- Causality ---------------------------------------------------------
	var trace_id := host.last_trace_for(1)
	_check(trace_id >= 0, "component has a trace")
	_check(host.explain(trace_id).size() > 0, "explain returns causal nodes")

	# --- Ports & stimulus --------------------------------------------------
	_check(host.load_world("res://worlds/logic_gates.twscene") == OK, "load gate world")
	_check(host.component_ports(2).size() == 3, "AND gate exposes 3 ports")

	_check(host.inject_signal(2, "in_a", 1) == OK, "inject in_a")
	_check(host.inject_signal(2, "in_b", 1) == OK, "inject in_b")
	host.advance_ticks(1)
	_check(int(host.component_state(2).get("y", 0)) == 1, "AND gate output is 1")

	# --- Reject unsupported Variant types ---------------------------------
	_check(
		host.inject_signal(2, "in_a", [1, 2, 3]) == ERR_INVALID_PARAMETER,
		"unsupported Variant type is rejected"
	)

	host.queue_free()

	if _failures == 0:
		print("smoke: OK")
		quit(0)
	else:
		printerr("smoke: %d failure(s)" % _failures)
		quit(1)
