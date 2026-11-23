# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT
extends SceneTree

var ui: Control
var output: String

func _initialize() -> void:
	_run.call_deferred()

func save_image(name: String) -> void:
	await process_frame
	await RenderingServer.frame_post_draw
	var result := root.get_texture().get_image().save_png(output.path_join(name + ".png"))
	assert(result == OK)

func _run() -> void:
	var args := OS.get_cmdline_user_args()
	var kind := args[0]
	var role := args[1]
	output = args[2]
	root.size = Vector2i(1280, 900)
	root.title = "Godot Kyber documentation - " + kind + " " + role
	if kind == "host":
		ui = load("res://main.tscn").instantiate()
	else:
		ui = Control.new()
		ui.name = "Demo"
		ui.set_script(load("res://multiplayer_demo.gd" if kind == "rpc" else ("res://viewport_video.gd" if kind == "viewport" else "res://file_video.gd")))
	root.add_child(ui)
	ui.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	await process_frame
	if kind == "host":
		await save_image("host-settings")
		quit()
		return
	ui.code.secret = true
	if kind == "settings":
		ui.codec.select(1)
		await save_image("h264-settings")
		quit()
		return
	var code_path := output.path_join(kind + "-code.txt")
	var deadline := Time.get_ticks_msec() + 25000
	ui.port.value = 24434 if kind == "rpc" else 24433
	if role == "sender":
		if kind == "rpc":
			ui._host()
		elif kind == "viewport":
			ui._start()
		else:
			ui._select_file(args[3])
			ui._start()
			# Hide the machine-specific path without changing the selected media.
			ui.file_path.text = "Test video"
		var code := ""
		while code.is_empty() and Time.get_ticks_msec() < deadline:
			await process_frame
			code = ui.peer.get_connection_code() if kind == "rpc" else ui.video.get_connection_code()
		var f := FileAccess.open(code_path, FileAccess.WRITE)
		f.store_string(code)
		f.close()
	else:
		while not FileAccess.file_exists(code_path) and Time.get_ticks_msec() < deadline:
			await process_frame
		if kind != "rpc":
			ui._change_mode(1)
		ui.code.text = FileAccess.get_file_as_string(code_path)
		if kind == "rpc":
			ui._connect_host()
		else:
			ui._start()
	await create_timer(7).timeout
	if kind == "rpc":
		ui.message.text = "Hello from " + role
		ui._send()
		await create_timer(2).timeout
		assert(not ui.multiplayer.get_peers().is_empty())
	else:
		var frames: int = ui.video.get_frame_count() if role == "sender" else ui.stream.get_frame_count()
		assert(frames > 10)
	await save_image(kind + "-" + role)
	await create_timer(3).timeout
	if kind == "rpc":
		ui._disconnect()
	else:
		ui._stop()
	quit()
