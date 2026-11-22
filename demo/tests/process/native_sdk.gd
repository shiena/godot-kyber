# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

extends SceneTree


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	if not await _test_native_video():
		quit(1)
		return
	var session := KyberSession.new()
	root.add_child(session)
	# A closed local port exercises the real SDK's asynchronous failure path.
	if not session.connect_to_host("127.0.0.1", 1, "", "", ""):
		push_error("Embedded SDK rejected a valid connection configuration")
		quit(1)
		return
	var deadline := Time.get_ticks_msec() + 10000
	while session.get_state() != KyberClient.FAILED and Time.get_ticks_msec() < deadline:
		await process_frame
	var failed := session.get_state() == KyberClient.FAILED
	session.close()
	session.queue_free()
	await process_frame
	if not failed:
		push_error("Embedded SDK did not report the connection failure")
	else:
		print("KYBER_NATIVE_RUNTIME_OK")
	quit(0 if failed else 1)


func _test_native_video() -> bool:
	var sender := KyberFileVideo.new()
	var receiver := KyberFileVideo.new()
	root.add_child(sender)
	root.add_child(receiver)
	var capabilities := sender.get_media_capabilities()
	if not capabilities.file_video or not capabilities.h264_decode or not capabilities.audio_playback:
		push_error("Unexpected native media capabilities: " + str(capabilities))
		sender.queue_free()
		receiver.queue_free()
		return false
	var socket := PacketPeerUDP.new()
	if socket.bind(0, "127.0.0.1") != OK:
		sender.queue_free()
		receiver.queue_free()
		return false
	var port := socket.get_local_port()
	socket.close()
	var arguments := OS.get_cmdline_user_args()
	var path := arguments[0] if arguments.size() > 0 else ProjectSettings.globalize_path("res://tests/fixtures/video sample.mp4")
	var valid := sender.start_sender(path, "127.0.0.1", port, false)
	var deadline := Time.get_ticks_msec() + 15000
	while valid and sender.get_connection_code().is_empty() and not sender.has_failed() and Time.get_ticks_msec() < deadline:
		await process_frame
	valid = valid and not sender.has_failed() and not sender.get_connection_code().is_empty()
	if valid:
		valid = receiver.start_receiver("127.0.0.1", port, sender.get_connection_code())
	while valid and (sender.is_active() or receiver.is_active()) and Time.get_ticks_msec() < deadline:
		await process_frame
	valid = valid and not sender.is_active() and not receiver.is_active() and not sender.has_failed() and not receiver.has_failed()
	valid = valid and receiver.get_frame_count() > 0
	if valid:
		var pixel := receiver.get_texture().get_image().get_pixel(320, 180)
		valid = pixel.r > 0.8 and pixel.g < 0.15 and pixel.b < 0.15
	if not valid:
		push_error("Native media transfer failed: " + sender.get_status() + " / " + receiver.get_status())
	sender.stop()
	receiver.stop()
	sender.queue_free()
	receiver.queue_free()
	await process_frame
	return valid
