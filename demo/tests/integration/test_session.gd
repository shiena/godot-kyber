# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

extends GutTest


func test_native_media_capabilities_are_separate_from_connection_state() -> void:
	var node := KyberSession.new()
	add_child_autofree(node)
	var capabilities := node.get_media_capabilities()
	assert_eq(capabilities.backend, "gstreamer" if OS.get_name() == "Linux" else "avfoundation" if OS.get_name() in ["macOS", "iOS"] else "mediacodec" if OS.get_name() == "Android" else "media_foundation")
	assert_true(capabilities.h264_decode)
	assert_true(capabilities.file_video)
	assert_true(capabilities.jpeg_receive)
	assert_true(capabilities.audio_playback)
	assert_true(node.get_capabilities().native_audio_request)

var session: KyberSession


func _sdk_stream(host: String) -> VideoStreamPlayer:
	assert_true(session.connect_to_host(host, 8080, "", "", ""))
	assert_true(await wait_until(func(): return session.get_state() == KyberSession.CONNECTED, 2.0))
	assert_true(session.start_stream(42, 64, 32, 1000000, false))
	return _player()


func test_sdk_reconnect_events_restore_stream_on_same_handle() -> void:
	session.auto_reconnect = true
	var player := await _sdk_stream("sdk-reconnect")
	watch_signals(player)
	assert_true(await wait_until(func(): return session.get_state() == KyberSession.RECONNECTING, 2.0))
	assert_true(player.is_playing())
	assert_true(await wait_until(func(): return session.get_reconnect_attempts() == 1 and session.get_state() == KyberSession.STREAMING, 3.0))
	await wait_process_frames(4)
	assert_signal_not_emitted(player, "finished")
	assert_eq(player.get_video_texture().get_width(), 64)
	assert_true(session.send_key(4, false))


func test_sdk_reconnection_failure_is_terminal() -> void:
	session.auto_reconnect = true
	var player := await _sdk_stream("sdk-reconnect-fail")
	assert_true(await wait_until(func(): return session.get_state() == KyberSession.FAILED, 3.0))
	assert_string_contains(session.get_last_error(), "ReconnectionFailed")
	await wait_process_frames(3)
	assert_false(player.is_playing())
	assert_eq(session.get_state(), KyberSession.FAILED)


func test_close_during_sdk_reconnect_prevents_restart() -> void:
	session.auto_reconnect = true
	var player := await _sdk_stream("sdk-reconnect")
	assert_true(await wait_until(func(): return session.get_state() == KyberSession.RECONNECTING, 2.0))
	session.close()
	await wait_seconds(0.3)
	assert_eq(session.get_state(), KyberSession.DISCONNECTED)
	assert_false(player.is_playing())


func test_sdk_reconnect_option_defaults_off() -> void:
	assert_false(session.auto_reconnect)
	var player := await _sdk_stream("sdk-reconnect")
	await wait_seconds(0.3)
	assert_eq(session.get_reconnect_attempts(), 0)
	assert_true(player.is_playing())


func before_each() -> void:
	session = KyberSession.new()
	add_child_autofree(session)
	watch_signals(session)


func _player() -> VideoStreamPlayer:
	var player := VideoStreamPlayer.new()
	player.expand = true
	add_child_autofree(player)
	var stream := KyberVideoStream.new()
	stream.set_session(session)
	player.stream = stream
	player.play()
	return player


func test_shared_players_pause_and_stop_without_closing_connection() -> void:
	assert_true(session.start_demo())
	var first := _player()
	var second := _player()
	assert_true(await wait_until(func(): return session.get_frame_count() >= 3, 2.0))
	first.paused = true
	var first_count: int = first.stream.get_frame_count()
	var second_count: int = second.stream.get_frame_count()
	var received := session.get_frame_count()
	await wait_process_frames(6)
	assert_eq(first.stream.get_frame_count(), first_count)
	assert_gt(second.stream.get_frame_count(), second_count)
	assert_gt(session.get_frame_count(), received)
	first.stop()
	assert_true(session.is_active())
	assert_true(second.is_playing())
	first.paused = false
	first.play()
	await wait_process_frames(3)
	assert_gt(first.stream.get_frame_count(), 0)
	assert_eq(session.get_statistics().width, 320)
	assert_eq(session.get_statistics().height, 180)
	session.close()
	await wait_process_frames(3)
	assert_false(first.is_playing())
	assert_false(second.is_playing())
	assert_eq(session.get_state(), KyberSession.DISCONNECTED)


func test_sdk_connection_supports_playback_and_input_together() -> void:
	assert_true(session.connect_to_host("fixture", 8080, "", "", ""))
	assert_true(await wait_until(func(): return session.get_state() == KyberSession.CONNECTED, 2.0))
	assert_eq(session.get_displays()[0].id, 42)
	assert_true(session.get_capabilities().display_selection)
	assert_true(session.start_stream(42, 64, 32, 1000000, false))
	var player := _player()
	assert_true(await wait_until(func(): return player.stream.get_frame_count() >= 2, 2.0))
	assert_eq(player.get_video_texture().get_width(), 64)
	assert_eq(player.get_video_texture().get_image().get_pixel(0, 0), Color8(25, 100, 200, 255))
	assert_true(session.get_capabilities().input)
	var key := InputEventKey.new()
	key.physical_keycode = KEY_A
	key.pressed = true
	player.paused = true
	assert_true(session.send_input_event(key))
	key.pressed = false
	assert_true(session.send_input_event(key))
	player.stop()
	assert_true(session.send_mouse_motion(1, -1))
	assert_true(session.send_key(4, false))
	assert_true(session.send_mouse_button(0, false))
	assert_eq(session.get_state(), KyberSession.STREAMING)
	assert_true(session.get_capabilities().godot_audio)
	assert_false(session.get_capabilities().seek)
	session.close()
	assert_false(session.send_input_event(key))


func test_state_callbacks_can_query_and_close_session() -> void:
	var observed: Array[int] = []
	session.state_changed.connect(func(state: int):
		observed.append(state)
		if state == KyberSession.DEMO:
			assert_eq(session.get_state(), KyberSession.DEMO)
			session.close()
	)
	assert_true(session.start_demo())
	await wait_process_frames(3)
	assert_has(observed, KyberSession.DEMO)
	assert_has(observed, KyberSession.DISCONNECTED)
	assert_false(session.is_active())


func test_failure_preserves_details_and_allows_reconnect() -> void:
	assert_false(session.start_receiver("127.0.0.1", 4433, "invalid"))
	assert_eq(session.get_state(), KyberSession.FAILED)
	assert_false(session.get_last_error().is_empty())
	assert_eq(session.get_status(), session.get_last_error())
	await wait_process_frames(2)
	assert_signal_emitted(session, "error_occurred")
	assert_true(session.start_demo())
	assert_eq(session.get_last_error(), "")
	assert_true(await wait_until(func(): return session.get_frame_count() > 0, 2.0))


func test_removing_or_freeing_session_stops_shared_playback() -> void:
	assert_true(session.start_demo())
	var player := _player()
	await wait_process_frames(3)
	remove_child(session)
	await wait_process_frames(2)
	assert_false(session.is_active())
	assert_false(player.is_playing())
	add_child(session)
	assert_true(session.start_demo())
	player.play()
	await wait_process_frames(2)
	assert_true(player.is_playing())
	# A stream must not dereference a Node after the scene frees it.
	var transient := KyberSession.new()
	add_child(transient)
	transient.start_demo()
	var stream := KyberVideoStream.new()
	stream.set_session(transient)
	player.stream = stream
	player.play()
	transient.free()
	await wait_process_frames(2)
	assert_false(player.is_playing())
	assert_null(stream.get_session())
	assert_string_contains(stream.get_last_error(), "freed")


func test_demo_preserves_aspect_ratio_and_reports_reception_while_paused() -> void:
	var scene := Control.new()
	scene.set_script(load("res://file_video.gd"))
	add_child_autofree(scene)
	scene._change_mode(1)
	scene.session.start_demo()
	scene.stream = KyberVideoStream.new()
	scene.stream.set_session(scene.session)
	scene.player.stream = scene.stream
	scene.player.play()
	await wait_process_frames(4)
	scene.player.paused = true
	var received: int = scene.session.get_frame_count()
	await wait_process_frames(4)
	assert_gt(scene.session.get_frame_count(), received)
	assert_string_contains(scene.status.text, "Local demo")
	assert_string_contains(scene.status.text, "Playback paused")
	assert_almost_eq(scene.receiver_view.ratio, 320.0 / 180.0, 0.001)
	assert_eq(scene.receiver_view.stretch_mode, AspectRatioContainer.STRETCH_FIT)
	scene._stop()
	assert_false(scene.session.is_active())


func test_extended_input_and_clipboard_are_explicit() -> void:
	assert_false(session.send_gamepad_button(0, JOY_BUTTON_A, true))
	assert_false(session.send_system_clipboard())
	session.clipboard_enabled = true
	await _sdk_stream("mock")
	assert_true(await wait_until(func(): return session.get_state() == KyberSession.STREAMING, 2.0))
	assert_true(session.send_mouse_wheel(0.25, -1.0))
	assert_false(session.send_mouse_wheel(2.0, 0.0))
	var button := InputEventJoypadButton.new()
	button.device = 3
	button.button_index = JOY_BUTTON_RIGHT_STICK
	button.pressed = true
	assert_true(session.send_input_event(button))
	assert_true(session.send_gamepad_axis(3, JOY_AXIS_TRIGGER_LEFT, 0.5))
	assert_false(session.send_gamepad_axis(3, JOY_AXIS_MAX, 0.5))
	assert_true(session.send_gamepad_connection(3, false))
	assert_true(session.send_clipboard_text("日本語\nclipboard 🦀"))
	assert_true(await wait_until(func(): return get_signal_emit_count(session, "clipboard_available") > 0, 2.0))
	assert_true(session.request_clipboard_text())
	assert_true(await wait_until(func(): return get_signal_emit_count(session, "clipboard_text_received") > 0, 2.0))
	assert_signal_emitted_with_parameters(session, "clipboard_text_received", ["日本語\nclipboard 🦀"])
	assert_false(session.send_clipboard_text("x".repeat(61441)))
	session.close()
	assert_false(session.request_clipboard_text())


func test_statistics_distinguish_unavailable_measurements() -> void:
	await _sdk_stream("mock")
	assert_true(await wait_until(func(): return session.get_statistics().received_frames > 2, 2.0))
	var performance: Dictionary = session.get_statistics().performance
	assert_false(performance.end_to_end_latency_available)
	assert_false(performance.decode.available)
	assert_true(performance.texture_update.available)
	assert_true(performance.display_interval.available)
	assert_gt(performance.display_interval.fps, 0.0)


func test_clipboard_json_escaping_preserves_the_text_limit() -> void:
	session.clipboard_enabled = true
	await _sdk_stream("mock")
	assert_true(await wait_until(func(): return session.get_state() == KyberSession.STREAMING, 2.0))
	var received: Array[String] = []
	session.clipboard_text_received.connect(func(text: String): received.append(text))
	var text := "\"".repeat(61440)
	assert_true(session.send_clipboard_text(text))
	assert_true(session.request_clipboard_text())
	assert_true(await wait_until(func(): return not received.is_empty(), 2.0))
	assert_eq(received[0], text)
