# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

extends Control

var client: KyberSession
var screen: TextureRect
var status: Label
var host: LineEdit
var username: LineEdit
var password: LineEdit
var fingerprint: LineEdit
var host_audio: CheckBox
var audio_player: AudioStreamPlayer
var capture: CheckBox
var pressed_keys: Dictionary = {}
var pressed_buttons: Dictionary = {}
var clipboard_preview: LineEdit


func _ready() -> void:
	client = KyberSession.new()
	add_child(client)
	var margin := MarginContainer.new()
	margin.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	for side in ["left", "right", "top", "bottom"]:
		margin.add_theme_constant_override("margin_" + side, 20)
	add_child(margin)
	var column := VBoxContainer.new()
	column.add_theme_constant_override("separation", 10)
	margin.add_child(column)
	var title := Label.new()
	title.text = "Godot × Kyber  |  Video receiver"
	title.add_theme_font_size_override("font_size", 26)
	column.add_child(title)
	host = _field(column, "Host name (port 8080)", OS.get_environment("KYBER_HOST"))
	username = _field(column, "Username", OS.get_environment("KYBER_USERNAME"))
	password = _field(column, "Password", "")
	password.secret = true
	fingerprint = _field(column, "Certificate fingerprint (leave blank to use system CAs)", "")
	var buttons := HBoxContainer.new()
	column.add_child(buttons)
	_button(buttons, "Connect", _connect_host)
	_button(buttons, "Disconnect", _close)
	_button(buttons, "Local display demo", client.start_demo)
	capture = CheckBox.new()
	capture.text = "Click the video to send input (Esc to release)"
	buttons.add_child(capture)
	var reconnect := CheckBox.new()
	reconnect.text = "Automatic reconnection (applies on next connection)"
	reconnect.toggled.connect(func(enabled: bool): client.auto_reconnect = enabled)
	column.add_child(reconnect)
	host_audio = CheckBox.new()
	host_audio.text = "Receive host audio (applies on next stream)"
	host_audio.button_pressed = true
	column.add_child(host_audio)
	audio_player = AudioStreamPlayer.new()
	add_child(audio_player)
	var clipboard_option := CheckBox.new()
	clipboard_option.text = "Allow text clipboard exchange (applies on next connection)"
	clipboard_option.toggled.connect(func(enabled: bool): client.clipboard_enabled = enabled)
	column.add_child(clipboard_option)
	var clipboard_row := HBoxContainer.new()
	column.add_child(clipboard_row)
	_button(clipboard_row, "Send clipboard", client.send_system_clipboard)
	_button(clipboard_row, "Request remote clipboard", client.request_clipboard_text)
	_button(clipboard_row, "Copy received text", func(): client.apply_clipboard_text(clipboard_preview.text))
	clipboard_preview = _field(column, "Received clipboard text", "")
	clipboard_preview.editable = false
	client.clipboard_text_received.connect(func(text: String): clipboard_preview.text = text)
	Input.joy_connection_changed.connect(func(device: int, connected: bool):
		if not connected:
			client.send_gamepad_connection(device, false))
	status = Label.new()
	status.text = "Disconnected. Select Local display demo to try it without a server."
	status.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	column.add_child(status)
	screen = TextureRect.new()
	screen.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	screen.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
	screen.size_flags_vertical = Control.SIZE_EXPAND_FILL
	screen.gui_input.connect(_screen_input)
	column.add_child(screen)
	client.connected.connect(_on_connected, CONNECT_DEFERRED)
	client.frame_received.connect(func(): screen.texture = client.get_texture(), CONNECT_DEFERRED)
	client.disconnected.connect(func(): screen.texture = null, CONNECT_DEFERRED)
	client.state_changed.connect(_on_state, CONNECT_DEFERRED)
	client.error_occurred.connect(func(message): status.text = message, CONNECT_DEFERRED)
	get_window().focus_exited.connect(_release_input)


func _field(parent: Node, placeholder: String, value: String) -> LineEdit:
	var field := LineEdit.new()
	field.placeholder_text = placeholder
	field.text = value
	parent.add_child(field)
	return field


func _button(parent: Node, text: String, action: Callable) -> void:
	var button := Button.new()
	button.text = text
	button.pressed.connect(action)
	parent.add_child(button)


func _connect_host() -> void:
	_release_input()
	client.connect_to_host(host.text, 8080, username.text, password.text, fingerprint.text)
	password.clear()


func _on_connected() -> void:
	var displays := client.get_displays()
	if displays.is_empty():
		status.text = "Connected, but no displays are available to stream."
		return
	if client.start_stream(displays[0].id, 1280, 720, 20000000, host_audio.button_pressed) and host_audio.button_pressed:
		var audio := KyberAudioStream.new()
		audio.set_session(client)
		audio_player.stream = audio
		audio_player.play()


func _on_state(state: int) -> void:
	status.text = ["Disconnected", "Connecting…", "Connected", "Starting stream…", "Receiving", "Error", "Local display demo (no network)", "Stopped", "Reconnecting through Kyber SDK…"][state]
	if state != KyberClient.STREAMING:
		_release_input()


func _close() -> void:
	audio_player.stop()
	_release_input()
	client.close()


func _screen_input(event: InputEvent) -> void:
	if capture.button_pressed and client.get_state() == KyberClient.STREAMING:
		if event is InputEventMouseButton and event.pressed:
			Input.mouse_mode = Input.MOUSE_MODE_CAPTURED
			accept_event()


func _input(event: InputEvent) -> void:
	if Input.mouse_mode != Input.MOUSE_MODE_CAPTURED:
		return
	if event is InputEventKey:
		if event.physical_keycode == KEY_ESCAPE:
			_release_input()
		elif not event.echo:
			if client.send_input_event(event):
				if event.pressed:
					pressed_keys[event.physical_keycode] = event.duplicate()
				else:
					pressed_keys.erase(event.physical_keycode)
	elif event is InputEventJoypadButton or event is InputEventJoypadMotion:
		client.send_input_event(event)
	elif event is InputEventMouseMotion:
		client.send_input_event(event)
	elif event is InputEventMouseButton:
		var mapping := {MOUSE_BUTTON_LEFT: 0, MOUSE_BUTTON_RIGHT: 1, MOUSE_BUTTON_MIDDLE: 2}
		if event.button_index in [MOUSE_BUTTON_WHEEL_UP, MOUSE_BUTTON_WHEEL_DOWN, MOUSE_BUTTON_WHEEL_LEFT, MOUSE_BUTTON_WHEEL_RIGHT]:
			client.send_input_event(event)
		elif mapping.has(event.button_index):
			var button: int = mapping[event.button_index]
			client.send_input_event(event)
			if event.pressed:
				pressed_buttons[button] = true
			else:
				pressed_buttons.erase(button)
	get_viewport().set_input_as_handled()


func _release_input() -> void:
	for device in range(32):
		client.send_gamepad_connection(device, false)
	for key: int in pressed_keys:
		var event: InputEventKey = pressed_keys[key]
		event.pressed = false
		client.send_input_event(event)
	for button: int in pressed_buttons:
		client.send_mouse_button(button, false)
	pressed_keys.clear()
	pressed_buttons.clear()
	Input.mouse_mode = Input.MOUSE_MODE_VISIBLE
