# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

extends SceneTree

var video: KyberFileVideo
var player: VideoStreamPlayer
var stream: KyberVideoStream
var session: KyberSession
var role: String
var directory: String
var port: int
var started := false
var begin: int
var address := "127.0.0.1"


func _initialize() -> void:
	var args := OS.get_cmdline_user_args()
	if args.size() != 4 and args.size() != 5:
		quit(2)
		return
	role = args[0]
	directory = args[1]
	port = int(args[2])
	if args.size() == 5:
		address = args[4]
	video = KyberFileVideo.new()
	root.add_child.call_deferred(video)
	player = VideoStreamPlayer.new()
	player.expand = true
	root.add_child.call_deferred(player)
	session = KyberSession.new()
	root.add_child.call_deferred(session)
	begin = Time.get_ticks_msec()
	if role == "sender":
		started = video.start_sender(args[3], address, port, false)
		if not started:
			_finish(false)


func _process(_delta: float) -> bool:
	if video == null:
		return false
	if Time.get_ticks_msec() - begin > 30000:
		_finish(false)
		return false
	var code_path := directory.path_join("code.txt")
	if role == "sender" and not video.get_connection_code().is_empty() and not FileAccess.file_exists(code_path):
		var file := FileAccess.open(code_path, FileAccess.WRITE)
		file.store_string(video.get_connection_code())
		file.close()
	if role == "receiver" and not started and FileAccess.file_exists(code_path):
		var code := FileAccess.get_file_as_string(code_path)
		if code.length() == 129:
			if not session.start_receiver(address, port, code):
				_finish(false)
				return false
			stream = KyberVideoStream.new()
			stream.set_session(session)
			player.stream = stream
			player.play()
			started = true
	if role == "receiver" and started and not player.is_playing():
		var valid := session.get_state() == KyberSession.FINISHED and session.get_last_error().is_empty() and stream.get_frame_count() >= 3
		var image := player.get_video_texture().get_image()
		if image != null and image.get_width() == 640:
			var pixel := image.get_pixel(320, 180)
			valid = valid and pixel.r > 0.8 and pixel.g < 0.15 and pixel.b < 0.15
		else:
			valid = false
		_finish(valid)
	if role == "sender" and started and not video.is_active():
		var valid := not video.has_failed() and video.get_frame_count() >= 3
		_finish(valid)
	return false


func _finish(success: bool) -> void:
	var report := {"success": success, "frames": video.get_frame_count(), "status": video.get_status()}
	if role == "receiver" and stream != null:
		report = {"success": success, "frames": stream.get_frame_count(), "status": session.get_status(), "state": session.get_state(), "statistics": session.get_statistics(), "player": "VideoStreamPlayer"}
	var file := FileAccess.open(directory.path_join(role + ".json"), FileAccess.WRITE)
	file.store_string(JSON.stringify(report))
	file.close()
	video.stop()
	player.stop()
	session.close()
	quit(0 if success else 1)
