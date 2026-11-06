extends Node2D
## Minimal Godot frontend for the Tech World Engine.
##
## The engine owns all simulation semantics. This script only:
##   1. loads a `.twscene` world,
##   2. pulls state diffs every frame,
##   3. forwards user input as port/tick stimuli.

@onready var sim: SimulationHost = $SimulationHost
@onready var count_label: Label = $UI/CountLabel
@onready var status_label: Label = $UI/StatusLabel


func _ready() -> void:
	$UI/SignalButton.pressed.connect(_on_inject_tick)
	$UI/ReplayButton.pressed.connect(_on_replay)

	var error := sim.load_world("res://worlds/counter.twscene")
	if error != OK:
		status_label.text = "Failed to load world (see console)"
		return

	# Let Godot's frame delta drive the engine clock.
	sim.set_auto_advance(true)
	status_label.text = "tick %d" % sim.current_tick()


func _process(_delta: float) -> void:
	for change in sim.take_state_changes():
		if int(change["component"]) == 1 and String(change["key"]) == "count":
			count_label.text = "Count: %s / 10" % str(change["new"])

	status_label.text = "tick %d" % sim.current_tick()


func _on_inject_tick() -> void:
	sim.inject_tick(1)


func _on_replay() -> void:
	sim.load_world("res://worlds/counter.twscene")
	count_label.text = "Count: 0 / 10"
