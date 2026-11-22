# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

class_name KyberInputRelay
extends Node
## Routes extended input through Godot RPC and official-host input through KyberSession.
## Use the same node path on both peers and configure remote_peer_id on each side.

signal input_received(event: InputEvent)
signal rumble_received(device: int, weak: float, strong: float, duration: float)

@export var session: KyberSession
## Only this peer may deliver input or vibration. Zero disables RPC input.
@export var remote_peer_id: int = 0
@export var inject_received_input: bool = false
@export var vibration_enabled: bool = false
## A single local controller is explicitly selected; remote IDs never select hardware.
@export var local_gamepad_device: int = 0
@export var touch_as_mouse: bool = true

var _mouse_finger: int = -1
var _touches: Dictionary = {}
var _buttons: Dictionary = {}
var _vibrating: bool = false

func _ready() -> void:
	multiplayer.peer_disconnected.connect(_peer_disconnected)
	multiplayer.server_disconnected.connect(reset_input)
	if session != null:
		session.gamepad_rumble_received.connect(_receive_host_rumble)
		session.state_changed.connect(_host_state_changed)

func _notification(what: int) -> void:
	if what == NOTIFICATION_APPLICATION_FOCUS_OUT and is_inside_tree():
		reset_input()

func _exit_tree() -> void:
	reset_input()

## Call from _input or _unhandled_input, not both.
func send_input_event(event: InputEvent) -> bool:
	if event.device == -2:
		return false
	if (event is InputEventScreenTouch or event is InputEventScreenDrag) and (event.index < 0 or event.index >= 32):
		return false
	if session != null:
		if touch_as_mouse and event is InputEventScreenTouch:
			if event.pressed and not event.canceled and _mouse_finger == -1:
				if session.send_mouse_button(0, true):
					_mouse_finger = event.index
					return true
			elif event.index == _mouse_finger and (not event.pressed or event.canceled):
				_mouse_finger = -1
				return session.send_mouse_button(0, false)
			return false
		if touch_as_mouse and event is InputEventScreenDrag:
			if event.index == _mouse_finger and event.relative.is_finite():
				return session.send_mouse_motion(int(clampf(event.relative.x, -32768, 32767)), int(clampf(event.relative.y, -32768, 32767)))
			return false
		return session.send_input_event(event)
	if not _can_send():
		return false
	var size := get_viewport().get_visible_rect().size
	if size.x <= 0 or size.y <= 0:
		return false
	if event is InputEventScreenTouch:
		return _send_touch(0, event.index, event.position / size, Vector2.ZERO, event.pressed and not event.canceled)
	if event is InputEventScreenDrag:
		return _send_touch(1, event.index, event.position / size, event.relative / size, false)
	if event is InputEventJoypadButton and event.button_index >= JOY_BUTTON_PADDLE1 and event.button_index <= JOY_BUTTON_PADDLE4:
		_receive_button.rpc_id(remote_peer_id, int(event.button_index), event.pressed)
		return true
	return false

func _can_send() -> bool:
	return remote_peer_id > 0 and multiplayer.has_multiplayer_peer() and remote_peer_id in multiplayer.get_peers()

func _authorized() -> bool:
	return remote_peer_id > 0 and multiplayer.get_remote_sender_id() == remote_peer_id

func _send_touch(kind: int, index: int, position: Vector2, relative: Vector2, pressed: bool) -> bool:
	if not _valid_touch(index, position, relative):
		return false
	_receive_touch.rpc_id(remote_peer_id, kind, index, position, relative, pressed)
	return true

func _valid_touch(index: int, position: Vector2, relative: Vector2) -> bool:
	return index >= 0 and index < 32 and position.is_finite() and relative.is_finite() and absf(position.x) <= 4 and absf(position.y) <= 4 and absf(relative.x) <= 4 and absf(relative.y) <= 4

@rpc("any_peer", "call_remote", "reliable")
func _receive_touch(kind: int, index: int, position: Vector2, relative: Vector2, pressed: bool) -> void:
	if not _authorized() or not _valid_touch(index, position, relative):
		return
	var size := get_viewport().get_visible_rect().size
	if kind == 0:
		if pressed:
			_touches[index] = position
		elif not _touches.has(index):
			return
		else:
			_touches.erase(index)
		var event := InputEventScreenTouch.new()
		event.index = index
		event.position = position * size
		event.pressed = pressed
		_deliver(event)
	elif kind == 1 and _touches.has(index):
		_touches[index] = position
		var event := InputEventScreenDrag.new()
		event.index = index
		event.position = position * size
		event.relative = relative * size
		_deliver(event)

@rpc("any_peer", "call_remote", "reliable")
func _receive_button(button: int, pressed: bool) -> void:
	if not _authorized() or button < JOY_BUTTON_PADDLE1 or button > JOY_BUTTON_PADDLE4:
		return
	if pressed:
		_buttons[button] = true
	elif not _buttons.has(button):
		return
	else:
		_buttons.erase(button)
	var event := InputEventJoypadButton.new()
	event.button_index = button as JoyButton
	event.pressed = pressed
	_deliver(event)

func _deliver(event: InputEvent) -> void:
	# A reserved device ID prevents forwarding injected events back to the sender.
	event.device = -2
	input_received.emit(event)
	if inject_received_input:
		Input.parse_input_event(event)

## Request vibration on the other peer's explicitly selected local controller.
func send_rumble(weak: float, strong: float, duration: float = 0.0) -> bool:
	if not _can_send() or not _valid_rumble(weak, strong, duration):
		return false
	_receive_rumble.rpc_id(remote_peer_id, weak, strong, duration)
	return true

func _valid_rumble(weak: float, strong: float, duration: float) -> bool:
	return is_finite(weak) and is_finite(strong) and is_finite(duration) and weak >= 0 and weak <= 1 and strong >= 0 and strong <= 1 and duration >= 0 and duration <= 65

@rpc("any_peer", "call_remote", "reliable")
func _receive_rumble(weak: float, strong: float, duration: float) -> void:
	if _authorized() and _valid_rumble(weak, strong, duration):
		_apply_rumble(local_gamepad_device, weak, strong, duration)

func _receive_host_rumble(device: int, weak: float, strong: float, duration: float) -> void:
	if device == local_gamepad_device:
		_apply_rumble(device, weak, strong, duration)

func _apply_rumble(device: int, weak: float, strong: float, duration: float) -> void:
	rumble_received.emit(device, weak, strong, duration)
	if vibration_enabled:
		if weak == 0 and strong == 0:
			Input.stop_joy_vibration(device)
			_vibrating = false
		else:
			Input.start_joy_vibration(device, weak, strong, duration)
			_vibrating = true

func _host_state_changed(state: int) -> void:
	if state != 4:
		reset_input()

@rpc("any_peer", "call_remote", "reliable")
func _release_remote() -> void:
	if _authorized():
		reset_input(false)

func _peer_disconnected(peer: int) -> void:
	if peer == remote_peer_id:
		reset_input()

## Call before changing peers, sessions, or input settings, and on focus loss.
func reset_input(notify_remote: bool = true) -> void:
	if notify_remote and _can_send():
		_release_remote.rpc_id(remote_peer_id)
	if _mouse_finger != -1 and is_instance_valid(session):
		session.send_mouse_button(0, false)
	_mouse_finger = -1
	for index in _touches.keys():
		var event := InputEventScreenTouch.new()
		event.index = index
		event.position = _touches[index] * get_viewport().get_visible_rect().size
		event.canceled = true
		_deliver(event)
	_touches.clear()
	for button in _buttons.keys():
		var event := InputEventJoypadButton.new()
		event.button_index = button as JoyButton
		_deliver(event)
	_buttons.clear()
	if _vibrating:
		Input.stop_joy_vibration(local_gamepad_device)
		_vibrating = false
