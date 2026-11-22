# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

extends SceneTree

var scene
var role: String
var directory: String
var port: int
var started := false
var sent := false
var begin: int
var relay: KyberInputRelay
var input_count := 0
var rumble_count := 0
var input_valid := true


func _initialize() -> void:
	var args := OS.get_cmdline_user_args()
	if args.size() != 4:
		quit(2)
		return
	role = args[0]
	directory = args[1]
	port = int(args[2])
	begin = Time.get_ticks_msec()
	_setup.call_deferred()


func _setup() -> void:
	var script = load("res://multiplayer_demo.gd")
	if script == null or not script.can_instantiate():
		quit(2)
		return
	scene = Control.new()
	scene.set_script(script)
	scene.name = "RpcDemo"
	root.add_child(scene)
	relay = KyberInputRelay.new()
	relay.name = "InputRelay"
	relay.remote_peer_id = 2 if role == "sender" else 1
	relay.input_received.connect(_received_input)
	relay.rumble_received.connect(func(_device, weak, strong, duration):
		if is_equal_approx(weak, 0.25) and is_equal_approx(strong, 0.75) and is_equal_approx(duration, 0.1):
			rumble_count += 1)
	scene.add_child(relay)
	scene.port.value = port
	if role == "sender":
		scene._host()
		started = true


func _process(_delta: float) -> bool:
	if scene == null:
		return false
	if Time.get_ticks_msec() - begin > 15000:
		_finish(false)
		return false
	var code_path := directory.path_join("code.txt")
	if role == "sender" and scene.peer != null and not scene.peer.get_connection_code().is_empty() and not FileAccess.file_exists(code_path):
		var file := FileAccess.open(code_path, FileAccess.WRITE)
		file.store_string(scene.peer.get_connection_code())
		file.close()
	if role == "receiver" and not started and FileAccess.file_exists(code_path):
		var code := FileAccess.get_file_as_string(code_path)
		if code.length() == 129:
			scene.code.text = code
			scene._connect_host()
			started = true
	if started and not sent and scene.multiplayer.get_peers().size() > 0:
		scene.message.text = "rpc-" + role
		scene._send()
		_send_extended_input()
		sent = true
	var expected := "rpc-receiver" if role == "sender" else "rpc-sender"
	if sent and expected in scene.history.text and input_count == 9 and rumble_count == 1 and input_valid:
		# Both processes must receive their RPC before either closes the connection.
		var marker := FileAccess.open(directory.path_join(role + ".received"), FileAccess.WRITE)
		marker.store_string("ok")
		marker.close()
		var other := "receiver" if role == "sender" else "sender"
		if FileAccess.file_exists(directory.path_join(other + ".received")):
			_finish(true)
	return false


func _finish(success: bool) -> void:
	var file := FileAccess.open(directory.path_join(role + ".json"), FileAccess.WRITE)
	file.store_string(JSON.stringify({"success": success, "messages": 1 if success else 0, "status": scene.history.text}))
	file.close()
	scene._disconnect()
	quit(0 if success else 1)


func _send_extended_input() -> void:
	var touch := InputEventScreenTouch.new()
	touch.index = 1
	touch.position = Vector2(100, 100)
	touch.pressed = true
	relay.send_input_event(touch)
	var drag := InputEventScreenDrag.new()
	drag.index = 1
	drag.position = Vector2(110, 105)
	drag.relative = Vector2(10, 5)
	relay.send_input_event(drag)
	touch.pressed = false
	relay.send_input_event(touch)
	var button := InputEventJoypadButton.new()
	button.button_index = JOY_BUTTON_PADDLE1
	button.pressed = true
	relay.send_input_event(button)
	button.pressed = false
	relay.send_input_event(button)
	touch.index = 2
	touch.pressed = true
	relay.send_input_event(touch)
	button.button_index = JOY_BUTTON_PADDLE2
	button.pressed = true
	relay.send_input_event(button)
	relay.send_rumble(0.25, 0.75, 0.1)
	relay._release_remote.rpc_id(relay.remote_peer_id)


func _received_input(event: InputEvent) -> void:
	input_valid = input_valid and event.device == -2
	if event is InputEventScreenTouch:
		input_valid = input_valid and event.index in [1, 2]
	elif event is InputEventScreenDrag:
		input_valid = input_valid and event.index == 1 and event.relative.is_equal_approx(Vector2(10, 5)) and event.position.is_equal_approx(Vector2(110, 105))
	elif event is InputEventJoypadButton:
		input_valid = input_valid and event.button_index in [JOY_BUTTON_PADDLE1, JOY_BUTTON_PADDLE2]
	else:
		input_valid = false
	input_count += 1
