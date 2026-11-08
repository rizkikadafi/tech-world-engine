extends Node2D
## Interactive 2D circuit authoring surface for `tw-schematic`.
##
## The Rust `SchematicHost` owns the document and undo history. This script only
## draws the snapshot, translates mouse input into edit commands, and displays
## diagnostics/DC results.

const GRID := 32.0

const BG_COLOR := Color(0.075, 0.085, 0.11)
const GRID_COLOR := Color(0.13, 0.15, 0.19)
const WIRE_COLOR := Color(0.55, 0.63, 0.78)
const WIRE_SELECTED := Color(1.0, 0.82, 0.3)
const PIN_COLOR := Color(0.45, 0.68, 0.95)
const TEXT_COLOR := Color(0.82, 0.86, 0.94)
const MUTED_COLOR := Color(0.5, 0.55, 0.63)
const BODY_COLOR := Color(0.16, 0.18, 0.23)
const ERROR_COLOR := Color(1.0, 0.42, 0.42)
const WARN_COLOR := Color(1.0, 0.78, 0.32)
const POS_COLOR := Color(1.0, 0.55, 0.35)
const NEG_COLOR := Color(0.4, 0.7, 1.0)

const PIN_OFFSETS := {
	"resistor": {"a": Vector2(-48, 0), "b": Vector2(48, 0)},
	"capacitor": {"a": Vector2(-48, 0), "b": Vector2(48, 0)},
	"lamp": {"a": Vector2(-48, 0), "b": Vector2(48, 0)},
	"switch": {"a": Vector2(-48, 0), "b": Vector2(48, 0)},
	"voltage_source": {"pos": Vector2(0, -48), "neg": Vector2(0, 48)},
	"current_source": {"pos": Vector2(0, -48), "neg": Vector2(0, 48)},
	"ground": {"gnd": Vector2(0, 24)},
	"diode": {"anode": Vector2(-48, 0), "cathode": Vector2(48, 0)},
	"nmos": {
		"drain": Vector2(0, -44),
		"gate": Vector2(-48, 0),
		"source": Vector2(0, 44),
	},
	"pmos": {
		"drain": Vector2(0, -44),
		"gate": Vector2(-48, 0),
		"source": Vector2(0, 44),
	},
	"probe": {"node": Vector2(0, 0)},
}

var _snapshot: Dictionary = {}
var _state: Dictionary = {}
var _pin_voltages: Dictionary = {}
var _probe_voltages: Dictionary = {}
var _has_state: bool = false
var _show_voltages: bool = true

var _selected_part: String = ""
var _selected_wire: String = ""
var _pending_kind: String = ""
var _wire_start: Dictionary = {}
var _drag_part: String = ""
var _drag_offset: Vector2 = Vector2.ZERO
var _drag_origin: Vector2 = Vector2.ZERO
var _drag_moved: bool = false
var _mouse_pos: Vector2 = Vector2.ZERO

var _status_label: Label
var _path_edit: LineEdit
var _palette_box: VBoxContainer
var _props_box: VBoxContainer
var _diagnostics_list: ItemList
var _undo_button: Button
var _redo_button: Button

@onready var host: SchematicHost = $SchematicHost


func _ready() -> void:
	_build_ui()
	_populate_palette()
	_on_sample()


# ---------------------------------------------------------------------------
# UI construction
# ---------------------------------------------------------------------------

func _build_ui() -> void:
	var layer := CanvasLayer.new()
	layer.name = "EditorUI"
	add_child(layer)

	var root := Control.new()
	layer.add_child(root)
	root.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	root.mouse_filter = Control.MOUSE_FILTER_IGNORE

	# Top toolbar.
	var top := _make_panel(root, 0.0, 0.0, 1.0, 0.0, 216, 8, -300, 56)
	var top_row := HBoxContainer.new()
	top_row.add_theme_constant_override("separation", 6)
	top.add_child(top_row)

	_undo_button = _make_button(top_row, "Undo", _on_undo)
	_redo_button = _make_button(top_row, "Redo", _on_redo)
	_make_button(top_row, "Run DC", _on_run_dc)
	_make_button(top_row, "Sample", _on_sample)
	_make_button(top_row, "New", _on_new)
	_make_button(top_row, "Load", _on_load)
	_make_button(top_row, "Save", _on_save)
	_make_button(top_row, "Sim demo", _on_open_demo)

	_path_edit = LineEdit.new()
	_path_edit.text = "user://circuit.twsch"
	_path_edit.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_path_edit.custom_minimum_size = Vector2(160, 0)
	top_row.add_child(_path_edit)

	_status_label = Label.new()
	_status_label.text = ""
	_status_label.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_status_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_RIGHT
	_status_label.add_theme_color_override("font_color", MUTED_COLOR)
	top_row.add_child(_status_label)

	# Left palette.
	var left := _make_panel(root, 0.0, 0.0, 0.0, 1.0, 8, 64, 208, -176)
	var left_scroll := ScrollContainer.new()
	left.add_child(left_scroll)
	_palette_box = VBoxContainer.new()
	_palette_box.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_palette_box.add_theme_constant_override("separation", 5)
	left_scroll.add_child(_palette_box)

	# Right properties.
	var right := _make_panel(root, 1.0, 0.0, 1.0, 1.0, -300, 64, -8, -176)
	var right_scroll := ScrollContainer.new()
	right.add_child(right_scroll)
	_props_box = VBoxContainer.new()
	_props_box.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_props_box.add_theme_constant_override("separation", 5)
	right_scroll.add_child(_props_box)

	# Bottom diagnostics.
	var bottom := _make_panel(root, 0.0, 1.0, 1.0, 1.0, 216, -168, -300, -8)
	var bottom_box := VBoxContainer.new()
	bottom.add_child(bottom_box)

	var title_row := HBoxContainer.new()
	var title := Label.new()
	title.text = "Diagnostics"
	title.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	title_row.add_child(title)
	var toggle := CheckButton.new()
	toggle.text = "Voltages"
	toggle.button_pressed = _show_voltages
	toggle.toggled.connect(_on_toggle_voltages)
	title_row.add_child(toggle)
	bottom_box.add_child(title_row)

	_diagnostics_list = ItemList.new()
	_diagnostics_list.size_flags_vertical = Control.SIZE_EXPAND_FILL
	bottom_box.add_child(_diagnostics_list)


func _make_panel(
	parent: Control,
	anchor_left: float,
	anchor_top: float,
	anchor_right: float,
	anchor_bottom: float,
	left: float,
	top: float,
	right: float,
	bottom: float
) -> PanelContainer:
	var panel := PanelContainer.new()
	panel.anchor_left = anchor_left
	panel.anchor_top = anchor_top
	panel.anchor_right = anchor_right
	panel.anchor_bottom = anchor_bottom
	panel.offset_left = left
	panel.offset_top = top
	panel.offset_right = right
	panel.offset_bottom = bottom
	parent.add_child(panel)
	return panel


func _make_button(parent: Node, text: String, callback: Callable) -> Button:
	var button := Button.new()
	button.text = text
	button.pressed.connect(callback)
	parent.add_child(button)
	return button


func _populate_palette() -> void:
	for child in _palette_box.get_children():
		_palette_box.remove_child(child)
		child.queue_free()

	var title := Label.new()
	title.text = "Parts"
	_palette_box.add_child(title)

	var hint := Label.new()
	hint.text = "Pick a part, then click the canvas. Click a pin twice to wire."
	hint.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	hint.add_theme_color_override("font_color", MUTED_COLOR)
	_palette_box.add_child(hint)

	for kind in host.part_kinds():
		var button := Button.new()
		button.text = String(kind["name"])
		var pin_names := PackedStringArray()
		for pin in kind["pins"]:
			pin_names.append(String(pin["name"]))
		button.tooltip_text = "%s\npins: %s" % [String(kind["id"]), ", ".join(pin_names)]
		button.pressed.connect(_select_kind.bind(String(kind["id"])))
		_palette_box.add_child(button)


# ---------------------------------------------------------------------------
# Refresh
# ---------------------------------------------------------------------------

func _refresh_all() -> void:
	_refresh_after_edit(true)


func _refresh_after_edit(rebuild_props: bool) -> void:
	_snapshot = host.snapshot()
	_refresh_state()
	_update_status()
	_sync_buttons()
	_rebuild_diagnostics()
	if rebuild_props:
		_rebuild_props()
	queue_redraw()


func _refresh_state() -> void:
	_has_state = host.has_state()
	if _has_state:
		_state = host.state_snapshot()
		_pin_voltages = _state.get("pins", {})
		_probe_voltages = _state.get("probes", {})
	else:
		_state = {}
		_pin_voltages = {}
		_probe_voltages = {}


func _update_status() -> void:
	var doc_name := String(_snapshot.get("name", ""))
	var revision := int(_snapshot.get("revision", 0))
	var parts: Array = _snapshot.get("parts", [])
	var wires: Array = _snapshot.get("wires", [])
	_status_label.text = "%s · rev %d · %d parts · %d wires" % [
		doc_name, revision, parts.size(), wires.size()
	]


func _sync_buttons() -> void:
	if _undo_button:
		_undo_button.disabled = not bool(_snapshot.get("can_undo", false))
	if _redo_button:
		_redo_button.disabled = not bool(_snapshot.get("can_redo", false))


func _clear_transient() -> void:
	_selected_part = ""
	_selected_wire = ""
	_pending_kind = ""
	_wire_start = {}
	_drag_part = ""
	_drag_moved = false
	_has_state = false
	_state = {}
	_pin_voltages = {}
	_probe_voltages = {}


# ---------------------------------------------------------------------------
# Editing actions
# ---------------------------------------------------------------------------

func _select_kind(kind_id: String) -> void:
	_pending_kind = kind_id
	_wire_start = {}
	_selected_part = ""
	_selected_wire = ""
	_status_label.text = "Placing %s — click the canvas (right-click to cancel)" % kind_id
	_rebuild_props()
	queue_redraw()


func _place_part(pos: Vector2) -> void:
	var kind := _pending_kind
	var id := _next_part_id(kind)
	if host.add_part(id, kind, pos.x, pos.y) != OK:
		_status_label.text = "Add part failed (see console)"
		_rebuild_diagnostics()
		return
	_selected_part = id
	_selected_wire = ""
	_pending_kind = ""
	_refresh_all()


func _begin_drag(part_id: String, pos: Vector2) -> void:
	var part := _find_part(part_id)
	if part.is_empty():
		return
	_selected_part = part_id
	_selected_wire = ""
	_pending_kind = ""
	_wire_start = {}
	_drag_part = part_id
	_drag_origin = Vector2(float(part["x"]), float(part["y"]))
	_drag_offset = _drag_origin - pos
	_drag_moved = false
	_rebuild_props()
	queue_redraw()


func _preview_drag(target: Vector2) -> void:
	var parts: Array = _snapshot.get("parts", [])
	for index in parts.size():
		var part: Dictionary = parts[index]
		if String(part["id"]) == _drag_part:
			part["x"] = target.x
			part["y"] = target.y
			parts[index] = part
			break
	_snapshot["parts"] = parts


func _finish_drag() -> void:
	if _drag_part == "":
		return
	var part_id := _drag_part
	var moved := _drag_moved
	var target := _drag_origin
	var part := _find_part(part_id)
	if not part.is_empty():
		target = Vector2(float(part["x"]), float(part["y"]))

	_drag_part = ""
	_drag_moved = false

	if moved and target != _drag_origin:
		if host.move_part(part_id, target.x, target.y) != OK:
			_status_label.text = "Move failed (see console)"
	_refresh_all()


func _handle_pin_click(pin: Dictionary) -> void:
	if _wire_start.is_empty():
		_wire_start = pin
		_status_label.text = "Wire from %s:%s — click another pin (right-click to cancel)" % [
			String(pin["part"]), String(pin["pin"])
		]
		queue_redraw()
		return

	var from_part := String(_wire_start["part"])
	var from_pin := String(_wire_start["pin"])
	var to_part := String(pin["part"])
	var to_pin := String(pin["pin"])

	if from_part == to_part and from_pin == to_pin:
		_wire_start = {}
		_status_label.text = "Wire cancelled"
		queue_redraw()
		return

	var wire_id := host.connect_pins(from_part, from_pin, to_part, to_pin)
	_wire_start = {}
	if wire_id == "":
		_status_label.text = "Connect failed (see console)"
		_rebuild_diagnostics()
	else:
		_selected_wire = wire_id
		_selected_part = ""
		_refresh_all()


func _handle_left_click(pos: Vector2) -> void:
	var pin := _pin_at(pos)
	if not pin.is_empty():
		_handle_pin_click(pin)
		return

	var part_id := _part_at(pos)
	if part_id != "":
		_begin_drag(part_id, pos)
		return

	var wire_id := _wire_at(pos)
	if wire_id != "":
		_selected_part = ""
		_selected_wire = wire_id
		_pending_kind = ""
		_rebuild_props()
		_status_label.text = "Selected wire %s (Delete to remove)" % wire_id
		return

	if _pending_kind != "":
		_place_part(_snapped(pos))
		return

	_selected_part = ""
	_selected_wire = ""
	_rebuild_props()
	queue_redraw()


func _delete_selection() -> void:
	if _selected_wire != "":
		if host.disconnect_wire(_selected_wire) == OK:
			_selected_wire = ""
			_refresh_all()
		return
	if _selected_part != "":
		if host.remove_part(_selected_part) == OK:
			_selected_part = ""
			_selected_wire = ""
			_refresh_all()
		return


func _cancel_pending() -> void:
	_pending_kind = ""
	_wire_start = {}
	queue_redraw()


# ---------------------------------------------------------------------------
# Buttons
# ---------------------------------------------------------------------------

func _on_new() -> void:
	host.new_document("playground")
	_clear_transient()
	_refresh_all()


func _on_sample() -> void:
	host.new_document("voltage-divider")
	host.add_part("v1", "voltage_source", 320, 224)
	host.add_part("r1", "resistor", 480, 160)
	host.add_part("r2", "resistor", 480, 320)
	host.add_part("gnd", "ground", 480, 448)
	host.add_part("out", "probe", 688, 224)
	host.connect_pins("v1", "pos", "r1", "a")
	host.connect_pins("r1", "b", "r2", "a")
	host.connect_pins("r2", "b", "gnd", "gnd")
	host.connect_pins("v1", "neg", "gnd", "gnd")
	host.connect_pins("out", "node", "r1", "b")
	_clear_transient()
	_refresh_all()
	_on_run_dc()


func _on_load() -> void:
	var path := _path_edit.text.strip_edges()
	if path == "":
		_status_label.text = "Enter a path first"
		return
	if host.load_document(path) == OK:
		_clear_transient()
		_refresh_all()
		_status_label.text = "Loaded %s" % path
	else:
		_status_label.text = "Load failed (see console)"


func _on_save() -> void:
	var path := _path_edit.text.strip_edges()
	if path == "":
		_status_label.text = "Enter a path first"
		return
	if host.save_document(path) == OK:
		_status_label.text = "Saved %s" % path
	else:
		_status_label.text = "Save failed (see console)"


func _on_run_dc() -> void:
	if host.run_dc() == OK:
		_refresh_state()
		_rebuild_diagnostics()
		_status_label.text = "DC solved · %d net(s) · %d probe(s)" % [
			_state.get("nets", {}).size(), _probe_voltages.size()
		]
	else:
		_status_label.text = "DC solve failed — check diagnostics"
		_rebuild_diagnostics()
	queue_redraw()


func _on_undo() -> void:
	if host.undo():
		_selected_part = ""
		_selected_wire = ""
		_wire_start = {}
		_drag_part = ""
		_refresh_all()
	else:
		_status_label.text = "Nothing to undo"


func _on_redo() -> void:
	if host.redo():
		_selected_part = ""
		_selected_wire = ""
		_wire_start = {}
		_drag_part = ""
		_refresh_all()
	else:
		_status_label.text = "Nothing to redo"


func _on_toggle_voltages(pressed: bool) -> void:
	_show_voltages = pressed
	queue_redraw()


func _on_open_demo() -> void:
	get_tree().change_scene_to_file("res://scenes/demo.tscn")


# ---------------------------------------------------------------------------
# Properties panel
# ---------------------------------------------------------------------------

func _rebuild_props() -> void:
	if _props_box == null:
		return
	for child in _props_box.get_children():
		_props_box.remove_child(child)
		child.queue_free()

	var title := Label.new()
	title.text = "Properties"
	_props_box.add_child(title)

	if _selected_part != "":
		var part := _find_part(_selected_part)
		if not part.is_empty():
			_build_part_props(part)
			return

	if _selected_wire != "":
		_build_wire_props()
		return

	var hint := Label.new()
	hint.text = "Select a part or wire. Pick a palette part, then click the canvas to place it."
	hint.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	hint.add_theme_color_override("font_color", MUTED_COLOR)
	_props_box.add_child(hint)


func _build_part_props(part: Dictionary) -> void:
	var id := String(part["id"])

	var info := Label.new()
	info.text = "%s  ·  %s" % [id, String(part["kind_name"])]
	info.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_props_box.add_child(info)

	var label_row := HBoxContainer.new()
	var label_caption := Label.new()
	label_caption.text = "label"
	label_caption.custom_minimum_size = Vector2(86, 0)
	var label_edit := LineEdit.new()
	label_edit.text = String(part.get("label", ""))
	label_edit.placeholder_text = id
	label_edit.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	label_edit.text_submitted.connect(_on_rename_submitted.bind(id))
	label_row.add_child(label_caption)
	label_row.add_child(label_edit)
	_props_box.add_child(label_row)

	var params: Dictionary = part.get("params", {})
	for key in params:
		_add_param_row(id, String(key), params[key])

	var delete_button := Button.new()
	delete_button.text = "Delete part"
	delete_button.pressed.connect(_delete_selection)
	_props_box.add_child(delete_button)


func _add_param_row(id: String, key: String, value: Variant) -> void:
	var row := HBoxContainer.new()
	var caption := Label.new()
	caption.text = key
	caption.custom_minimum_size = Vector2(86, 0)
	row.add_child(caption)

	match typeof(value):
		TYPE_BOOL:
			var check := CheckBox.new()
			check.button_pressed = bool(value)
			check.text = "on"
			check.toggled.connect(_on_param_bool.bind(id, key))
			row.add_child(check)
		TYPE_INT:
			var int_edit := LineEdit.new()
			int_edit.text = str(int(value))
			int_edit.size_flags_horizontal = Control.SIZE_EXPAND_FILL
			int_edit.text_submitted.connect(_on_param_text.bind(id, key, true))
			row.add_child(int_edit)
		TYPE_FLOAT:
			var float_edit := LineEdit.new()
			float_edit.text = str(float(value))
			float_edit.size_flags_horizontal = Control.SIZE_EXPAND_FILL
			float_edit.text_submitted.connect(_on_param_text.bind(id, key, false))
			row.add_child(float_edit)
		_:
			var text_edit := LineEdit.new()
			text_edit.text = str(value)
			text_edit.size_flags_horizontal = Control.SIZE_EXPAND_FILL
			text_edit.text_submitted.connect(_on_param_text.bind(id, key, false))
			row.add_child(text_edit)

	_props_box.add_child(row)


func _build_wire_props() -> void:
	var wire := _find_wire(_selected_wire)
	var info := Label.new()
	if wire.is_empty():
		info.text = "Wire %s (missing)" % _selected_wire
	else:
		info.text = "Wire %s\n%s:%s → %s:%s" % [
			String(wire["id"]),
			String(wire["from_part"]), String(wire["from_pin"]),
			String(wire["to_part"]), String(wire["to_pin"]),
		]
	info.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_props_box.add_child(info)

	var delete_button := Button.new()
	delete_button.text = "Delete wire"
	delete_button.pressed.connect(_delete_selection)
	_props_box.add_child(delete_button)


func _on_rename_submitted(text: String, id: String) -> void:
	if host.rename_part(id, text.strip_edges()) == OK:
		_refresh_after_edit(false)


func _on_param_bool(pressed: bool, id: String, key: String) -> void:
	if host.set_param(id, key, pressed) == OK:
		_refresh_after_edit(false)


func _on_param_text(text: String, id: String, key: String, is_int: bool) -> void:
	var trimmed := text.strip_edges()
	var value: Variant
	if is_int:
		value = int(trimmed)
	else:
		value = trimmed.to_float()
	if host.set_param(id, key, value) == OK:
		_refresh_after_edit(false)


func _rebuild_diagnostics() -> void:
	if _diagnostics_list == null:
		return
	_diagnostics_list.clear()

	var items := host.diagnostics()
	if items.is_empty():
		_diagnostics_list.add_item("No problems detected.")
		_diagnostics_list.set_item_custom_fg_color(0, Color(0.5, 0.8, 0.6))
		return

	for index in items.size():
		var diagnostic: Dictionary = items[index]
		var severity := String(diagnostic.get("severity", "warning"))
		var line := "[%s] %s: %s" % [
			severity.to_upper(),
			String(diagnostic.get("code", "")),
			String(diagnostic.get("message", "")),
		]
		_diagnostics_list.add_item(line)
		_diagnostics_list.set_item_custom_fg_color(
			index, ERROR_COLOR if severity == "error" else WARN_COLOR
		)


# ---------------------------------------------------------------------------
# Hit testing
# ---------------------------------------------------------------------------

func _pin_offsets(kind: String) -> Dictionary:
	return PIN_OFFSETS.get(kind, {})


func _pin_position(part: Dictionary, pin: String) -> Vector2:
	var offsets := _pin_offsets(String(part["kind"]))
	var offset: Vector2 = offsets.get(pin, Vector2.ZERO)
	return Vector2(float(part["x"]), float(part["y"])) + offset


func _pin_position_by_id(part_id: String, pin: String) -> Vector2:
	var part := _find_part(part_id)
	if part.is_empty():
		return Vector2.ZERO
	return _pin_position(part, pin)


func _pin_at(pos: Vector2) -> Dictionary:
	var parts: Array = _snapshot.get("parts", [])
	for index in range(parts.size() - 1, -1, -1):
		var part: Dictionary = parts[index]
		var offsets := _pin_offsets(String(part["kind"]))
		for pin_name in offsets:
			var pin_pos := _pin_position(part, String(pin_name))
			if pin_pos.distance_to(pos) <= 11.0:
				return {
					"part": String(part["id"]),
					"pin": String(pin_name),
					"pos": pin_pos,
				}
	return {}


func _part_at(pos: Vector2) -> String:
	var parts: Array = _snapshot.get("parts", [])
	for index in range(parts.size() - 1, -1, -1):
		var part: Dictionary = parts[index]
		var center := Vector2(float(part["x"]), float(part["y"]))
		if Rect2(center - Vector2(58, 58), Vector2(116, 116)).has_point(pos):
			return String(part["id"])
	return ""


func _wire_at(pos: Vector2) -> String:
	for wire in _snapshot.get("wires", []):
		var a := _pin_position_by_id(String(wire["from_part"]), String(wire["from_pin"]))
		var b := _pin_position_by_id(String(wire["to_part"]), String(wire["to_pin"]))
		if _point_segment_distance(pos, a, b) <= 7.0:
			return String(wire["id"])
	return ""


func _point_segment_distance(point: Vector2, a: Vector2, b: Vector2) -> float:
	var ab := b - a
	var length_squared := ab.length_squared()
	if length_squared < 0.0001:
		return point.distance_to(a)
	var t := clampf((point - a).dot(ab) / length_squared, 0.0, 1.0)
	return point.distance_to(a + ab * t)


func _find_part(id: String) -> Dictionary:
	for part in _snapshot.get("parts", []):
		if String(part["id"]) == id:
			return part
	return {}


func _find_wire(id: String) -> Dictionary:
	for wire in _snapshot.get("wires", []):
		if String(wire["id"]) == id:
			return wire
	return {}


func _snapped(pos: Vector2) -> Vector2:
	return Vector2(roundf(pos.x / GRID) * GRID, roundf(pos.y / GRID) * GRID)


func _next_part_id(kind: String) -> String:
	var prefixes := {
		"resistor": "r",
		"capacitor": "c",
		"voltage_source": "v",
		"current_source": "i",
		"ground": "gnd",
		"diode": "d",
		"nmos": "mn",
		"pmos": "mp",
		"lamp": "lamp",
		"switch": "sw",
		"probe": "p",
	}
	var prefix := String(prefixes.get(kind, "p"))
	var index := 1
	while not _find_part("%s%d" % [prefix, index]).is_empty():
		index += 1
	return "%s%d" % [prefix, index]


func _pin_voltage(part_id: String, pin: String) -> float:
	var key := "%s:%s" % [part_id, pin]
	if _pin_voltages.has(key):
		return float(_pin_voltages[key])
	return NAN


# ---------------------------------------------------------------------------
# Input
# ---------------------------------------------------------------------------

func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouseMotion:
		var motion := event as InputEventMouseMotion
		_mouse_pos = get_global_mouse_position()
		if _drag_part != "" and (motion.button_mask & MOUSE_BUTTON_MASK_LEFT) != 0:
			_preview_drag(_snapped(_mouse_pos + _drag_offset))
			_drag_moved = true
		queue_redraw()
		return

	if event is InputEventMouseButton:
		var button := event as InputEventMouseButton
		if button.button_index == MOUSE_BUTTON_LEFT:
			if button.pressed:
				_handle_left_click(get_global_mouse_position())
			else:
				_finish_drag()
			queue_redraw()
		elif button.button_index == MOUSE_BUTTON_RIGHT and button.pressed:
			_cancel_pending()
		return

	if event is InputEventKey and event.pressed:
		var key := event as InputEventKey
		if key.ctrl_pressed and key.keycode == KEY_Z:
			if key.shift_pressed:
				_on_redo()
			else:
				_on_undo()
		elif key.ctrl_pressed and key.keycode == KEY_Y:
			_on_redo()
		elif key.keycode == KEY_DELETE or key.keycode == KEY_BACKSPACE:
			_delete_selection()
		elif key.keycode == KEY_ESCAPE:
			_cancel_pending()
			_selected_part = ""
			_selected_wire = ""
			_rebuild_props()
		queue_redraw()


# ---------------------------------------------------------------------------
# Drawing
# ---------------------------------------------------------------------------

func _draw() -> void:
	var viewport_size := get_viewport_rect().size
	draw_rect(Rect2(Vector2.ZERO, viewport_size), BG_COLOR)
	_draw_grid(viewport_size)
	_draw_wires()
	_draw_parts()
	_draw_pending_wire()


func _draw_grid(viewport_size: Vector2) -> void:
	var x := 0.0
	while x <= viewport_size.x:
		draw_line(Vector2(x, 0), Vector2(x, viewport_size.y), GRID_COLOR, 1.0)
		x += GRID

	var y := 0.0
	while y <= viewport_size.y:
		draw_line(Vector2(0, y), Vector2(viewport_size.x, y), GRID_COLOR, 1.0)
		y += GRID


func _draw_wires() -> void:
	for wire in _snapshot.get("wires", []):
		var a := _pin_position_by_id(String(wire["from_part"]), String(wire["from_pin"]))
		var b := _pin_position_by_id(String(wire["to_part"]), String(wire["to_pin"]))
		var selected := String(wire["id"]) == _selected_wire
		var color := WIRE_SELECTED if selected else _wire_color(wire)
		var width := 4.0 if selected else 2.0
		draw_line(a, b, color, width)
		if selected:
			draw_circle(a, 5.0, WIRE_SELECTED)
			draw_circle(b, 5.0, WIRE_SELECTED)


func _wire_color(wire: Dictionary) -> Color:
	if not _show_voltages or not _has_state:
		return WIRE_COLOR
	var voltage := _pin_voltage(String(wire["from_part"]), String(wire["from_pin"]))
	if is_nan(voltage):
		voltage = _pin_voltage(String(wire["to_part"]), String(wire["to_pin"]))
	if is_nan(voltage):
		return WIRE_COLOR
	return _voltage_color(voltage)


func _voltage_color(voltage: float) -> Color:
	var base := Color(0.45, 0.75, 0.55)
	var t := clampf(absf(voltage) / 12.0, 0.0, 1.0)
	if voltage >= 0.0:
		return base.lerp(POS_COLOR, t)
	return base.lerp(NEG_COLOR, t)


func _draw_parts() -> void:
	for part in _snapshot.get("parts", []):
		_draw_part(part)


func _draw_part(part: Dictionary) -> void:
	var id := String(part["id"])
	var kind := String(part["kind"])
	var center := Vector2(float(part["x"]), float(part["y"]))
	var selected := id == _selected_part
	var outline := WIRE_SELECTED if selected else TEXT_COLOR

	var offsets := _pin_offsets(kind)
	for pin_name in offsets:
		var offset: Vector2 = offsets[pin_name]
		var pin_position := center + offset
		var lead_end := center + offset * 0.55
		draw_line(pin_position, lead_end, outline, 2.0)

		var pin_color := PIN_COLOR
		if _show_voltages and _has_state:
			var voltage := _pin_voltage(id, String(pin_name))
			if not is_nan(voltage):
				pin_color = _voltage_color(voltage)

		if (
			not _wire_start.is_empty()
			and String(_wire_start.get("part", "")) == id
			and String(_wire_start.get("pin", "")) == String(pin_name)
		):
			draw_circle(pin_position, 8.0, WIRE_SELECTED)

		draw_circle(pin_position, 6.0 if selected else 5.0, pin_color)
		draw_circle(pin_position, 2.0, BG_COLOR)

	_draw_symbol(kind, center, outline)

	var label := String(part.get("label", ""))
	var caption := id if label == "" else "%s (%s)" % [id, label]
	draw_string(
		ThemeDB.fallback_font,
		center + Vector2(-44, 70),
		caption,
		HORIZONTAL_ALIGNMENT_LEFT,
		-1,
		12,
		TEXT_COLOR
	)

	var value_text := _part_value_text(part)
	if value_text != "":
		draw_string(
			ThemeDB.fallback_font,
			center + Vector2(-44, 84),
			value_text,
			HORIZONTAL_ALIGNMENT_LEFT,
			-1,
			11,
			MUTED_COLOR
		)


func _draw_symbol(kind: String, center: Vector2, outline: Color) -> void:
	match kind:
		"resistor":
			var body := Rect2(center - Vector2(34, 16), Vector2(68, 32))
			draw_rect(body, BODY_COLOR, true)
			draw_rect(body, outline, false, 2.0)
		"capacitor":
			draw_line(center + Vector2(-7, -18), center + Vector2(-7, 18), outline, 3.0)
			draw_line(center + Vector2(7, -18), center + Vector2(7, 18), outline, 3.0)
		"lamp":
			draw_circle(center, 18.0, BODY_COLOR)
			draw_arc(center, 18.0, 0.0, TAU, 32, outline, 2.0)
			draw_line(center + Vector2(-11, -11), center + Vector2(11, 11), outline, 2.0)
			draw_line(center + Vector2(11, -11), center + Vector2(-11, 11), outline, 2.0)
		"switch":
			draw_line(center + Vector2(-28, 0), center + Vector2(-11, 0), outline, 2.0)
			draw_line(center + Vector2(28, 0), center + Vector2(11, 0), outline, 2.0)
			draw_line(center + Vector2(-11, 0), center + Vector2(11, -12), outline, 2.0)
			draw_circle(center + Vector2(-11, 0), 2.5, outline)
			draw_circle(center + Vector2(11, 0), 2.5, outline)
		"voltage_source":
			draw_circle(center, 18.0, BODY_COLOR)
			draw_arc(center, 18.0, 0.0, TAU, 32, outline, 2.0)
			draw_line(center + Vector2(-7, 0), center + Vector2(7, 0), outline, 2.0)
			draw_line(center + Vector2(0, -7), center + Vector2(0, 7), outline, 2.0)
		"current_source":
			draw_circle(center, 18.0, BODY_COLOR)
			draw_arc(center, 18.0, 0.0, TAU, 32, outline, 2.0)
			draw_line(center + Vector2(0, 9), center + Vector2(0, -9), outline, 2.0)
			draw_line(center + Vector2(0, -9), center + Vector2(-5, -3), outline, 2.0)
			draw_line(center + Vector2(0, -9), center + Vector2(5, -3), outline, 2.0)
		"diode":
			var triangle := PackedVector2Array([
				center + Vector2(-12, -14),
				center + Vector2(-12, 14),
				center + Vector2(12, 0),
			])
			draw_colored_polygon(triangle, BODY_COLOR)
			draw_polyline(
				PackedVector2Array([
					center + Vector2(-12, -14),
					center + Vector2(-12, 14),
					center + Vector2(12, 0),
					center + Vector2(-12, -14),
				]),
				outline,
				2.0
			)
			draw_line(center + Vector2(12, -14), center + Vector2(12, 14), outline, 3.0)
		"ground":
			draw_line(center + Vector2(0, -14), center + Vector2(0, 4), outline, 2.0)
			draw_line(center + Vector2(-16, 4), center + Vector2(16, 4), outline, 2.0)
			draw_line(center + Vector2(-10, 10), center + Vector2(10, 10), outline, 2.0)
			draw_line(center + Vector2(-5, 16), center + Vector2(5, 16), outline, 2.0)
		"nmos", "pmos":
			draw_line(center + Vector2(-34, -18), center + Vector2(-34, 18), outline, 3.0)
			draw_line(center + Vector2(-34, 0), center + Vector2(-22, 0), outline, 2.0)
			draw_line(center + Vector2(-22, -24), center + Vector2(-22, 24), outline, 3.0)
			draw_line(center + Vector2(-22, -18), center + Vector2(0, -18), outline, 2.0)
			draw_line(center + Vector2(-22, 18), center + Vector2(0, 18), outline, 2.0)
			draw_line(center + Vector2(-22, 0), center + Vector2(0, 0), outline, 2.0)
			draw_line(center + Vector2(0, -18), center + Vector2(0, -44), outline, 2.0)
			draw_line(center + Vector2(0, 18), center + Vector2(0, 44), outline, 2.0)
			var arrow := PackedVector2Array([
				center + Vector2(-22, 0),
				center + Vector2(-10, -7),
				center + Vector2(-10, 7),
			])
			draw_colored_polygon(arrow, outline)
		"probe":
			var diamond := PackedVector2Array([
				center + Vector2(0, -10),
				center + Vector2(10, 0),
				center + Vector2(0, 10),
				center + Vector2(-10, 0),
			])
			draw_colored_polygon(diamond, BODY_COLOR)
			draw_polyline(
				PackedVector2Array([
					center + Vector2(0, -10),
					center + Vector2(10, 0),
					center + Vector2(0, 10),
					center + Vector2(-10, 0),
					center + Vector2(0, -10),
				]),
				outline,
				2.0
			)


func _part_value_text(part: Dictionary) -> String:
	var kind := String(part["kind"])
	var id := String(part["id"])
	var params: Dictionary = part.get("params", {})
	var text := ""

	match kind:
		"voltage_source":
			text = "V = %s" % str(float(params.get("voltage", 5.0)))
		"current_source":
			text = "I = %s" % str(float(params.get("current", 0.001)))
		"resistor":
			text = "R = %s" % str(float(params.get("resistance", 1000.0)))
		"capacitor":
			text = "C = %s" % str(float(params.get("capacitance", 1e-6)))
		"lamp":
			var resistance := maxf(float(params.get("resistance", 100.0)), 0.001)
			text = "R = %s" % str(resistance)
			if _has_state and _show_voltages:
				var a := _pin_voltage(id, "a")
				var b := _pin_voltage(id, "b")
				if not is_nan(a) and not is_nan(b):
					text = "P = %s" % str(pow(a - b, 2.0) / resistance)
		"switch":
			text = "closed" if bool(params.get("closed", false)) else "open"
		"probe":
			text = "probe"
			if _has_state and _probe_voltages.has(id):
				text = "V = %s" % str(float(_probe_voltages[id]))
	return text


func _draw_pending_wire() -> void:
	if _wire_start.is_empty() or not _wire_start.has("pos"):
		return
	var start: Vector2 = _wire_start["pos"]
	draw_line(start, _mouse_pos, WIRE_SELECTED, 2.0)
	draw_circle(_mouse_pos, 4.0, WIRE_SELECTED)
