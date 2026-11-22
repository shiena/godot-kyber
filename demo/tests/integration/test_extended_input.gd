# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

extends GutTest

var relay: KyberInputRelay

func before_each() -> void:
	relay = KyberInputRelay.new()
	add_child_autofree(relay)
	watch_signals(relay)

func test_unconfigured_and_unauthorized_peers_cannot_inject() -> void:
	var event := InputEventScreenTouch.new()
	event.pressed = true
	assert_false(relay.send_input_event(event))
	relay._receive_touch(0, 0, Vector2.ONE, Vector2.ZERO, true)
	relay._receive_button(JOY_BUTTON_PADDLE1, true)
	relay._receive_rumble(1, 1, 1)
	assert_signal_not_emitted(relay, "input_received")
	assert_signal_not_emitted(relay, "rumble_received")

func test_invalid_values_and_injected_events_are_rejected() -> void:
	assert_false(relay._valid_touch(-1, Vector2.ZERO, Vector2.ZERO))
	assert_false(relay._valid_touch(32, Vector2.ZERO, Vector2.ZERO))
	assert_false(relay._valid_touch(0, Vector2(INF, 0), Vector2.ZERO))
	assert_false(relay._valid_rumble(NAN, 1, 1))
	assert_false(relay._valid_rumble(1, 1, -1))
	assert_false(relay._valid_rumble(1, 2, 1))
	assert_false(relay._valid_rumble(1, 1, 66))
	var event := InputEventScreenTouch.new()
	event.device = -2
	assert_false(relay.send_input_event(event))

func test_host_touch_uses_one_finger_and_releases_on_reset() -> void:
	var session := KyberSession.new()
	add_child_autofree(session)
	relay.session = session
	assert_true(session.connect_to_host("fixture", 8080, "", "", ""))
	assert_true(await wait_until(func(): return session.get_state() == KyberSession.CONNECTED, 2.0))
	assert_true(session.start_stream(42, 64, 32, 1000000, false))
	assert_true(await wait_until(func(): return session.get_state() == KyberSession.STREAMING, 2.0))
	var touch := InputEventScreenTouch.new()
	touch.index = 3
	touch.pressed = true
	assert_true(relay.send_input_event(touch))
	touch.index = 4
	assert_false(relay.send_input_event(touch))
	var drag := InputEventScreenDrag.new()
	drag.index = 3
	drag.relative = Vector2(10, -4)
	assert_true(relay.send_input_event(drag))
	drag.index = 4
	assert_false(relay.send_input_event(drag))
	relay.reset_input()
	drag.index = 3
	assert_false(relay.send_input_event(drag))
	touch.index = 4
	assert_true(relay.send_input_event(touch))
	touch.canceled = true
	assert_true(relay.send_input_event(touch))
	assert_eq(relay._mouse_finger, -1)

func test_rumble_is_opt_in_and_host_device_must_match() -> void:
	assert_false(relay.vibration_enabled)
	relay._receive_host_rumble(7, 0.2, 0.8, 0.1)
	assert_signal_not_emitted(relay, "rumble_received")
	relay._receive_host_rumble(0, 0.2, 0.8, 0.1)
	assert_signal_emitted_with_parameters(relay, "rumble_received", [0, 0.2, 0.8, 0.1])
	assert_false(relay._vibrating)

func test_sdk_rumble_callback_reaches_session_with_normalized_values() -> void:
	var session := KyberSession.new()
	add_child_autofree(session)
	watch_signals(session)
	assert_true(session.connect_to_host("fixture", 8080, "", "", ""))
	assert_true(await wait_until(func(): return session.get_state() == KyberSession.CONNECTED, 2.0))
	assert_true(session.start_stream(42, 64, 32, 1000000, false))
	assert_true(await wait_until(func(): return session.get_state() == KyberSession.STREAMING, 2.0))
	assert_true(session.send_gamepad_button(0, JOY_BUTTON_A, true))
	await wait_process_frames(3)
	assert_signal_emitted_with_parameters(session, "gamepad_rumble_received", [0, 1.0, 0.0, 5.0])
