# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

extends GutTest


func test_texture_change_can_query_playback() -> void:
	var resource := KyberVideoStream.new()
	resource.source = KyberVideoStream.LOCAL_DEMO
	var view := VideoStreamPlayer.new()
	add_child_autofree(view)
	view.stream = resource
	view.play()
	var texture := view.get_video_texture()
	var queried: Array = []
	var on_change := func(): queried.append(view.get_video_texture())
	texture.changed.connect(on_change)
	await wait_process_frames(3)
	assert_false(queried.is_empty(), "Texture notification must allow playback queries")
	if not queried.is_empty():
		assert_same(queried[0], texture)
	texture.changed.disconnect(on_change)
	queried.clear()
	view.stop()

var player: VideoStreamPlayer
var stream: KyberVideoStream


func before_each() -> void:
	player = VideoStreamPlayer.new()
	player.expand = true
	add_child_autofree(player)
	stream = KyberVideoStream.new()
	watch_signals(player)


func after_each() -> void:
	player.stop()
	player.stream = null


func test_standard_player_pause_resume_and_stable_texture() -> void:
	stream.source = KyberVideoStream.LOCAL_DEMO
	player.stream = stream
	player.play()
	assert_true(await wait_until(func(): return stream.get_frame_count() >= 2, 2.0))
	var texture := player.get_video_texture()
	assert_eq(texture.get_width(), 320)
	var material := StandardMaterial3D.new()
	material.albedo_texture = texture
	player.paused = true
	var count := stream.get_frame_count()
	await wait_process_frames(5)
	assert_eq(stream.get_frame_count(), count)
	player.paused = false
	assert_true(await wait_until(func(): return stream.get_frame_count() > count, 2.0))
	assert_same(player.get_video_texture(), texture)
	assert_same(material.albedo_texture, texture)
	player.stop()
	assert_false(player.is_playing())
	player.play()
	assert_true(await wait_until(func(): return stream.get_frame_count() > 0, 2.0))


func test_resource_roundtrip_excludes_credentials() -> void:
	stream.source = KyberVideoStream.LOCAL_DEMO
	stream.host = "example.test"
	stream.port = 1234
	stream.set_password("secret-password-test")
	stream.set_connection_code("secret-code-test")
	var path := "user://kyber-stream-test.tres"
	assert_eq(ResourceSaver.save(stream, path), OK)
	var text := FileAccess.get_file_as_string(path)
	assert_false(text.contains("secret-password-test"))
	assert_false(text.contains("secret-code-test"))
	var restored := ResourceLoader.load(path, "", ResourceLoader.CACHE_MODE_IGNORE) as KyberVideoStream
	assert_not_null(restored)
	assert_eq(restored.host, "example.test")
	assert_eq(restored.port, 1234)
	assert_eq(restored.source, KyberVideoStream.LOCAL_DEMO)
	DirAccess.remove_absolute(path)


func test_sdk_fixture_plays_through_standard_player() -> void:
	stream.source = KyberVideoStream.KYBER_HOST
	stream.host = "fixture"
	stream.width = 64
	stream.height = 32
	player.stream = stream
	player.play()
	assert_true(await wait_until(func(): return stream.get_frame_count() > 0, 3.0), stream.get_last_error())
	assert_eq(player.get_video_texture().get_width(), 64)
	assert_eq(player.get_video_texture().get_height(), 32)
	assert_eq(stream.get_last_error(), "")


func test_invalid_configuration_fails_without_playing() -> void:
	stream.port = 0
	player.stream = stream
	player.play()
	await wait_process_frames(2)
	assert_false(player.is_playing())
	assert_false(stream.get_last_error().is_empty())


func test_network_video_reaches_player_and_emits_finished() -> void:
	var socket := PacketPeerUDP.new()
	assert_eq(socket.bind(0, "127.0.0.1"), OK)
	stream.port = socket.get_local_port()
	socket.close()
	var sender := KyberFileVideo.new()
	add_child_autofree(sender)
	assert_true(sender.start_sender(ProjectSettings.globalize_path("res://tests/fixtures/video sample.mp4"), "127.0.0.1", stream.port, false))
	assert_true(await wait_until(func(): return not sender.get_connection_code().is_empty(), 5.0))
	stream.set_connection_code(sender.get_connection_code())
	player.stream = stream
	player.play()
	assert_true(await wait_until(func(): return stream.get_frame_count() >= 2, 10.0), stream.get_last_error())
	var pixel := player.get_video_texture().get_image().get_pixel(320, 180)
	assert_gt(pixel.r, 0.8)
	assert_lt(pixel.g, 0.15)
	assert_true(await wait_until(func(): return not player.is_playing(), 10.0))
	assert_signal_emit_count(player, "finished", 1)
	assert_eq(stream.get_last_error(), "")
	sender.stop()
