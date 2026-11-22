# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

extends Control

var peer: KyberMultiplayerPeer
var host: LineEdit
var port: SpinBox
var code: LineEdit
var message: LineEdit
var status: Label
var history: TextEdit
var send_button: Button
var hosting := false


func _ready() -> void:
	var margin := MarginContainer.new()
	margin.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	for side in ["left", "right", "top", "bottom"]:
		margin.add_theme_constant_override("margin_" + side, 16)
	add_child(margin)
	var column := VBoxContainer.new()
	column.add_theme_constant_override("separation", 10)
	margin.add_child(column)
	var title := Label.new()
	title.text = "Godot RPC × Kyber"
	title.add_theme_font_size_override("font_size", 24)
	column.add_child(title)
	var help := Label.new()
	help.text = "Start a server, copy its connection code, then connect from the other app.\nMessages use Godot @rpc for one-to-one communication."
	column.add_child(help)
	var address_row := HBoxContainer.new()
	column.add_child(address_row)
	host = _field(address_row, "Listen address / remote host", "127.0.0.1")
	port = SpinBox.new()
	port.min_value = 1
	port.max_value = 65535
	port.value = 4434
	address_row.add_child(port)
	var code_row := HBoxContainer.new()
	column.add_child(code_row)
	code = _field(code_row, "Server connection code", "")
	_button(code_row, "Copy", func(): DisplayServer.clipboard_set(code.text))
	var buttons := HBoxContainer.new()
	column.add_child(buttons)
	_button(buttons, "Start server", _host)
	_button(buttons, "Connect", _connect_host)
	_button(buttons, "Disconnect", _disconnect)
	status = Label.new()
	status.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	column.add_child(status)
	history = TextEdit.new()
	history.editable = false
	history.size_flags_vertical = Control.SIZE_EXPAND_FILL
	column.add_child(history)
	var message_row := HBoxContainer.new()
	column.add_child(message_row)
	message = _field(message_row, "Message to send", "Hello from Godot")
	message.max_length = 256
	message.text_submitted.connect(func(_text): _send())
	send_button = _button(message_row, "Send via RPC", _send)


func _process(_delta: float) -> void:
	var connected := peer != null and multiplayer.get_peers().size() > 0
	send_button.disabled = not connected
	if peer == null:
		status.text = "Disconnected"
		return
	status.text = "Peer ID: %d · Connections: %d" % [peer.get_unique_id(), multiplayer.get_peers().size()]
	if not peer.get_last_error().is_empty():
		status.text = peer.get_last_error()
	elif peer.get_connection_status() == MultiplayerPeer.CONNECTION_DISCONNECTED:
		status.text = "Disconnected. Restart the server to reconnect."
	elif not connected:
		status.text += " · Waiting for a connection"
	if hosting:
		code.text = peer.get_connection_code()


func _host() -> void:
	_disconnect()
	peer = KyberMultiplayerPeer.new()
	hosting = true
	code.editable = false
	if peer.create_server(int(port.value), host.text.strip_edges()) == OK:
		multiplayer.multiplayer_peer = peer


func _connect_host() -> void:
	var connection_code := code.text.strip_edges()
	_disconnect()
	peer = KyberMultiplayerPeer.new()
	if peer.create_client(host.text.strip_edges(), int(port.value), connection_code) == OK:
		multiplayer.multiplayer_peer = peer


func _disconnect() -> void:
	multiplayer.multiplayer_peer = null
	if peer != null:
		peer.close()
	peer = null
	hosting = false
	if code != null:
		code.editable = true


func _exit_tree() -> void:
	_disconnect()


func _send() -> void:
	if peer == null or multiplayer.get_peers().is_empty() or message.text.is_empty():
		return
	var result := rpc_id(multiplayer.get_peers()[0], "chat", message.text)
	if result == OK:
		history.text += "You: " + message.text + "\n"
	else:
		history.text += "Send error: %d\n" % result


@rpc("any_peer", "call_remote", "reliable")
func chat(text: String) -> void:
	history.text += "Peer %d: %s\n" % [multiplayer.get_remote_sender_id(), text.left(256)]


func _field(parent: Node, placeholder: String, value: String) -> LineEdit:
	var field := LineEdit.new()
	field.placeholder_text = placeholder
	field.text = value
	field.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	parent.add_child(field)
	return field


func _button(parent: Node, text: String, action: Callable) -> Button:
	var button := Button.new()
	button.text = text
	button.pressed.connect(action)
	parent.add_child(button)
	return button
