# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

extends GutTest

var sender: KyberFileVideo
var receiver: KyberFileVideo
var listen_port: int
var fixture: String


func before_each() -> void:
	sender = KyberFileVideo.new()
	receiver = KyberFileVideo.new()
	add_child_autofree(sender)
	add_child_autofree(receiver)
	# Ask the OS for a free UDP port instead of assuming a fixed port is available.
	var socket := PacketPeerUDP.new()
	assert_eq(socket.bind(0, "127.0.0.1"), OK)
	listen_port = socket.get_local_port()
	socket.close()
	fixture = ProjectSettings.globalize_path("res://tests/fixtures/video sample.mp4")


func after_each() -> void:
	sender.stop()
	receiver.stop()


func _start_sender(loop_video: bool = true) -> bool:
	assert_true(FileAccess.file_exists(fixture), "Run scripts/test.ps1 to generate the video fixture")
	assert_true(sender.start_sender(fixture, "127.0.0.1", listen_port, loop_video))
	var ready: bool = await wait_until(func(): return not sender.get_connection_code().is_empty() or sender.has_failed(), 5.0)
	assert_true(ready)
	assert_false(sender.has_failed(), sender.get_status())
	return ready and not sender.has_failed()


func _connect_receiver() -> bool:
	assert_true(receiver.start_receiver("127.0.0.1", listen_port, sender.get_connection_code()))
	var ready: bool = await wait_until(func(): return receiver.get_frame_count() >= 3 or receiver.has_failed(), 15.0)
	assert_true(ready, receiver.get_status())
	assert_false(receiver.has_failed(), receiver.get_status())
	assert_gte(receiver.get_frame_count(), 3)
	return ready and not receiver.has_failed()


func test_real_kyproto_video_transfers_pixels_and_stops() -> void:
	if not await _start_sender() or not await _connect_receiver():
		return
	assert_eq(receiver.get_texture().get_width(), 640)
	assert_eq(receiver.get_texture().get_height(), 360)
	var pixel: Color = receiver.get_texture().get_image().get_pixel(320, 180)
	assert_gt(pixel.r, 0.8, "The red video fixture must survive Media Foundation, KyProto, JPEG decoding and texture upload")
	assert_lt(pixel.g, 0.15)
	assert_lt(pixel.b, 0.15)
	sender.stop()
	receiver.stop()
	await wait_process_frames(3)
	assert_false(sender.is_active())
	assert_false(receiver.is_active())
	assert_null(receiver.get_texture())
	assert_eq(receiver.get_frame_count(), 0)


func test_end_of_file_finishes_both_peers_without_error() -> void:
	if not await _start_sender(false) or not await _connect_receiver():
		return
	assert_true(await wait_until(func(): return not receiver.is_active() and not sender.is_active(), 15.0))
	assert_false(receiver.has_failed(), receiver.get_status())
	assert_false(sender.has_failed(), sender.get_status())
	assert_not_null(receiver.get_texture(), "Keep the final frame visible until stopped")


func test_wrong_token_is_rejected_and_sender_accepts_next_receiver() -> void:
	if not await _start_sender():
		return
	var parts := sender.get_connection_code().split(":")
	assert_true(receiver.start_receiver("127.0.0.1", listen_port, parts[0] + ":" + "0".repeat(64)))
	assert_true(await wait_until(func(): return not receiver.is_active(), 15.0))
	assert_true(receiver.has_failed())
	assert_null(receiver.get_texture())
	assert_true(sender.is_active())
	await _connect_receiver()


func test_invalid_file_and_connection_settings_do_not_start_workers() -> void:
	assert_false(sender.start_sender("missing.mp4", "127.0.0.1", listen_port, false))
	assert_false(sender.is_active())
	assert_true(sender.has_failed())
	assert_false(receiver.start_receiver("localhost", 0, "invalid"))
	assert_false(receiver.start_receiver("localhost", listen_port, "invalid"))
	assert_false(receiver.is_active())


func test_mode_switch_stops_sender_and_file_dialog_uses_filesystem() -> void:
	var scene := Control.new()
	scene.set_script(load("res://file_video.gd"))
	add_child_autofree(scene)
	assert_eq(scene.dialog.access, FileDialog.ACCESS_FILESYSTEM)
	assert_eq(scene.dialog.file_mode, FileDialog.FILE_MODE_OPEN_FILE)
	scene._select_file(fixture)
	assert_eq(scene.file_path.text, fixture)
	scene.port.value = listen_port
	scene._start()
	assert_true(scene.video.is_active())
	scene._change_mode(1)
	assert_false(scene.video.is_active())
	assert_false(scene.sender_fields.visible)
	assert_true(scene.code.editable)
	assert_null(scene.screen.texture)
