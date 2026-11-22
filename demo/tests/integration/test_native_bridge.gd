# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

extends GutTest

var client: KyberClient


func before_each() -> void:
	client = KyberClient.new()
	add_child_autofree(client)
	watch_signals(client)
	# The fixture is an ABI test double, never a real Kyber host.


func _connect_fixture() -> bool:
	assert_true(client.connect_to_host("fixture", 8080, "", "", ""))
	var ready: bool = await wait_until(func(): return client.get_state() == KyberClient.CONNECTED, 2.0)
	assert_true(ready, "Native connected callback must arrive within two seconds")
	return ready


func _stream_fixture() -> bool:
	if not await _connect_fixture():
		return false
	assert_true(client.start_stream(42, 64, 32, 1000000, false))
	var ready: bool = await wait_until(func(): return client.get_state() == KyberClient.STREAMING and client.get_texture() != null, 2.0)
	assert_true(ready, "Native frame must reach the Godot texture within two seconds")
	return ready


func test_connection_delivers_display_metadata_and_signal() -> void:
	if not await _connect_fixture():
		return
	assert_signal_emit_count(client, "connected", 1)
	assert_signal_emitted_with_parameters(client, "state_changed", [KyberClient.CONNECTING], 0)
	assert_eq(client.get_displays().size(), 1)
	assert_eq(client.get_displays()[0], {"id": 42, "width": 640, "height": 360})


func test_invalid_stream_parameters_leave_connection_usable() -> void:
	if not await _connect_fixture():
		return
	for args in [[999, 64, 32, 1000000], [42, 0, 32, 1000000], [42, 4097, 32, 1000000], [42, 64, 32, 0]]:
		assert_false(client.start_stream(args[0], args[1], args[2], args[3], false))
		assert_eq(client.get_state(), KyberClient.CONNECTED)
	assert_true(client.start_stream(42, 64, 32, 1000000, false))


func test_native_frame_preserves_size_and_rgba_pixels() -> void:
	if not await _stream_fixture():
		return
	var texture := client.get_texture()
	assert_eq(texture.get_size(), Vector2(64, 32))
	var image := texture.get_image()
	assert_true(image.get_pixel(0, 0).is_equal_approx(Color8(25, 100, 200, 255)))
	assert_true(image.get_pixel(63, 31).is_equal_approx(Color8(25, 100, 200, 255)))
	assert_signal_emitted(client, "frame_received")
	assert_gt(client.get_received_frames(), 0)


func test_input_ranges_and_stream_state_are_enforced() -> void:
	if not await _stream_fixture():
		return
	assert_true(client.send_key(4, true))
	assert_true(client.send_key(4, false))
	assert_true(client.send_mouse_motion(-32768, 32767))
	assert_true(client.send_mouse_button(0, true))
	assert_true(client.send_mouse_button(0, false))
	assert_false(client.send_key(-1, true))
	assert_false(client.send_key(65536, true))
	assert_false(client.send_mouse_motion(32768, 0))
	assert_false(client.send_mouse_button(3, true))
	client.close()
	assert_false(client.send_key(4, true))


func test_godot_input_events_use_native_bridge() -> void:
	if not await _stream_fixture():
		return
	var key := InputEventKey.new()
	key.physical_keycode = KEY_A
	key.pressed = true
	assert_true(client.send_input_event(key))
	key.pressed = false
	assert_true(client.send_input_event(key))
	key.echo = true
	assert_false(client.send_input_event(key))
	var motion := InputEventMouseMotion.new()
	motion.relative = Vector2(40000, -40000)
	assert_true(client.send_input_event(motion))
	var button := InputEventMouseButton.new()
	button.button_index = MOUSE_BUTTON_RIGHT
	button.pressed = true
	assert_true(client.send_input_event(button))
	button.button_index = MOUSE_BUTTON_WHEEL_UP
	assert_true(client.send_input_event(button))
	button.pressed = false
	assert_false(client.send_input_event(button))
	assert_true(client.send_input_event(InputEventJoypadButton.new()))
	client.close()
	assert_false(client.send_input_event(motion))


func test_close_joins_worker_and_prevents_late_frames() -> void:
	if not await _stream_fixture():
		return
	client.close()
	var frames_before: int = get_signal_emit_count(client, "frame_received")
	await wait_process_frames(5)
	assert_eq(client.get_state(), KyberClient.DISCONNECTED)
	assert_null(client.get_texture())
	assert_eq(client.get_displays().size(), 0)
	assert_eq(client.get_received_frames(), 0)
	assert_signal_emit_count(client, "frame_received", frames_before)


func test_reconnect_five_times_without_stale_frames() -> void:
	for iteration in range(5):
		if not await _stream_fixture():
			return
		client.close()
		await wait_process_frames(2)
		assert_null(client.get_texture(), "Texture cleared after iteration %s" % iteration)
		assert_eq(client.get_state(), KyberClient.DISCONNECTED)


func test_removing_streaming_node_from_tree_stops_session() -> void:
	if not await _stream_fixture():
		return
	remove_child(client)
	assert_eq(client.get_state(), KyberClient.DISCONNECTED)
	assert_null(client.get_texture())
	add_child(client)
