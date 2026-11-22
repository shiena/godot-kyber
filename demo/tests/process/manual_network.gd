# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

extends SceneTree


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	var args := OS.get_cmdline_user_args()
	if args.size() < 5 or args.size() > 7:
		quit(2)
		return
	var ui := Control.new()
	ui.set_script(load("res://file_video.gd"))
	root.add_child(ui)
	ui.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	root.title = "Kyber LAN - " + args[0]
	ui.port.value = int(args[2])
	if args.size() > 5:
		ui.codec.selected = 1 if args[5] == "h264" else 0
	if args.size() > 6:
		ui.acceleration.selected = int(args[6])
	var code_path := args[4].path_join("code.txt")
	if args[0] == "sender":
		ui._select_file(ProjectSettings.globalize_path(args[3]))
		ui.address.text = args[1]
		ui._start()
		while ui.video.get_connection_code().is_empty():
			if ui.video.has_failed():
				return
			await process_frame
		var file := FileAccess.open(code_path, FileAccess.WRITE)
		file.store_string(ui.video.get_connection_code())
		file.close()
	else:
		ui._change_mode(1)
		ui.address.text = args[1]
		while not FileAccess.file_exists(code_path) or FileAccess.get_file_as_string(code_path).length() != 129:
			await process_frame
		ui.code.text = FileAccess.get_file_as_string(code_path)
		ui._start()

	# Keep the preview visible on small mobile displays during visual tests.
	for control in ui.screen.get_parent().get_children():
		if control is Control and control not in [ui.screen, ui.receiver_view, ui.status]:
			control.hide()
	while is_instance_valid(ui):
		await create_timer(2.0).timeout
		print(ui.status.text)
