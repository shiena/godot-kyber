# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

extends SceneTree

func _initialize() -> void:
	_run.call_deferred()

func _write(path: String, value: Variant) -> void:
	var file := FileAccess.open(path, FileAccess.WRITE)
	file.store_string(JSON.stringify(value))
	file.close()

func _run() -> void:
	# role, address, port, shared result directory, duration, optional movie
	var args := OS.get_cmdline_user_args()
	if args.size() < 5:
		quit(2)
		return
	var sender := args[0] == "sender"
	var directory := args[3]
	var ui := Control.new()
	ui.set_script(load("res://viewport_video.gd" if sender and args.size() == 5 else "res://file_video.gd"))
	root.add_child(ui)
	ui.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	root.title = "Kyber multi-client — " + args[0]
	ui.port.value = int(args[2])
	if sender:
		ui.address.text = args[1]
		if args.size() > 5:
			ui._select_file(args[5])
		ui._start()
		while ui.video.get_connection_code().is_empty():
			if ui.video.has_failed():
				push_error(ui.video.get_status())
				quit(1)
				return
			await process_frame
		var file := FileAccess.open(directory.path_join("code.txt"), FileAccess.WRITE)
		file.store_string(ui.video.get_connection_code())
		file.close()
	else:
		ui._change_mode(1)
		ui.address.text = args[1]
		ui.code.text = FileAccess.get_file_as_string(directory.path_join("code.txt"))
		ui._start()
	var begin := Time.get_ticks_msec()
	var samples: Array = []
	while Time.get_ticks_msec() - begin < int(args[4]) * 1000 and not FileAccess.file_exists(directory.path_join("stop")):
		await create_timer(1.0).timeout
		var sample: Dictionary = {"elapsed_ms": Time.get_ticks_msec() - begin, "status": ui.status.text}
		if sender:
			sample.frames = ui.video.get_frame_count()
			sample.clients = ui.video.get_connected_clients()
			sample.skipped = ui.video.get_skipped_client_frames()
			sample.performance = ui.video.get_performance_statistics()
		else:
			sample.frames = ui.stream.get_frame_count()
			sample.statistics = ui.session.get_statistics()
		samples.append(sample)
		_write(directory.path_join(args[0] + ".json"), samples)
		if samples.size() == 5 and DisplayServer.get_name() != "headless":
			RenderingServer.force_draw(false)
			root.get_texture().get_image().save_png(directory.path_join(args[0] + ".png"))
	ui._stop()
	ui.queue_free()
	await process_frame
	quit(0 if not samples.is_empty() and samples[-1].frames > 10 else 1)
