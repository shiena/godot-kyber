# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

extends GutTest

var client: KyberClient


func before_each() -> void:
	client = KyberClient.new()
	add_child_autofree(client)
	watch_signals(client)


func test_initial_state_has_no_connection_or_frames() -> void:
	assert_eq(client.get_state(), KyberClient.DISCONNECTED)
	assert_null(client.get_texture())
	assert_eq(client.get_received_frames(), 0)
	assert_eq(client.get_dropped_frames(), 0)
	assert_eq(client.get_displays().size(), 0)


func test_input_is_rejected_without_a_stream() -> void:
	assert_false(client.send_key(4, true))
	assert_false(client.send_mouse_motion(1, -1))
	assert_false(client.send_mouse_button(0, true))
	assert_false(client.start_stream(42, 64, 32, 1000000, false))
	assert_signal_emitted(client, "error_occurred")
	assert_eq(client.get_state(), KyberClient.DISCONNECTED)


func test_demo_creates_texture_without_claiming_a_network_connection() -> void:
	client.start_demo()
	await wait_process_frames(3)
	assert_eq(client.get_state(), KyberClient.DEMO)
	assert_signal_not_emitted(client, "connected")
	assert_signal_emitted(client, "frame_received")
	var texture := client.get_texture()
	assert_not_null(texture)
	if texture == null:
		return
	assert_eq(texture.get_size(), Vector2(320, 180))
	assert_gt(client.get_received_frames(), 0)


func test_demo_updates_the_same_texture_resource() -> void:
	client.start_demo()
	await wait_process_frames(3)
	var texture := client.get_texture()
	var count := client.get_received_frames()
	await wait_process_frames(3)
	assert_same(client.get_texture(), texture)
	assert_gt(client.get_received_frames(), count)


func test_close_clears_frames_and_is_idempotent() -> void:
	client.start_demo()
	await wait_process_frames(3)
	client.close()
	client.close()
	await wait_process_frames(3)
	assert_eq(client.get_state(), KyberClient.DISCONNECTED)
	assert_null(client.get_texture())
	assert_eq(client.get_received_frames(), 0)
	assert_signal_emit_count(client, "disconnected", 1)


func test_empty_host_reports_failure() -> void:
	assert_false(client.connect_to_host("", 8080, "", "", ""))
	assert_eq(client.get_state(), KyberClient.FAILED)
	assert_string_contains(client.get_last_error(), "Host")
	assert_signal_emitted(client, "error_occurred")
	assert_signal_not_emitted(client, "connected")


func test_invalid_ports_are_rejected() -> void:
	for port in [-1, 0, 65536]:
		assert_false(client.connect_to_host("localhost", port, "", "", ""))
		assert_eq(client.get_state(), KyberClient.FAILED)
		assert_false(client.get_last_error().is_empty())


func test_demo_recovers_after_connection_failure() -> void:
	client.connect_to_host("", 8080, "", "", "")
	client.start_demo()
	await wait_process_frames(3)
	assert_eq(client.get_state(), KyberClient.DEMO)
	assert_eq(client.get_last_error(), "")
	assert_not_null(client.get_texture())
