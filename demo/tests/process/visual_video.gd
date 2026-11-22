# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

extends SceneTree


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	var args := OS.get_cmdline_user_args()
	if args.size() != 4:
		quit(2)
		return
	var role := args[0]
	var directory := args[1]
	var ui := Control.new()
	ui.set_script(load("res://file_video.gd"))
	root.add_child(ui)
	ui.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	root.title = "Kyber Linux test - " + role
	ui.port.value = int(args[2])
	var deadline := Time.get_ticks_msec() + 25000
	var code_path := directory.path_join("code.txt")
	var done_path := directory.path_join("done")
	if role == "sender":
		ui._select_file(args[3])
		ui._start()
		while ui.video.get_connection_code().is_empty() and Time.get_ticks_msec() < deadline:
			await process_frame
		var code_file := FileAccess.open(code_path, FileAccess.WRITE)
		code_file.store_string(ui.video.get_connection_code())
		code_file.close()
	else:
		ui._change_mode(1)
		while not FileAccess.file_exists(code_path) and Time.get_ticks_msec() < deadline:
			await process_frame
		ui.code.text = FileAccess.get_file_as_string(code_path)
		ui._start()
	var frames := 0
	while frames < 20 and Time.get_ticks_msec() < deadline:
		await process_frame
		frames = ui.video.get_frame_count() if role == "sender" else ui.stream.get_frame_count()
	var valid: bool = frames >= 20 and ui.dialog.use_native_dialog
	# Render even when the other test window occludes this one.
	RenderingServer.force_draw(false)
	var screenshot := root.get_texture().get_image()
	valid = valid and screenshot.save_png(directory.path_join(role + ".png")) == OK
	var result := FileAccess.open(directory.path_join(role + ".json"), FileAccess.WRITE)
	result.store_string(JSON.stringify({"success": valid, "frames": frames, "status": ui.status.text, "native_dialog": ui.dialog.use_native_dialog}))
	result.close()
	if role == "receiver":
		var done := FileAccess.open(done_path, FileAccess.WRITE)
		done.store_string("done")
		done.close()
	else:
		while not FileAccess.file_exists(done_path) and Time.get_ticks_msec() < deadline:
			await process_frame
	ui._stop()
	ui.queue_free()
	await process_frame
	quit(0 if valid else 1)
