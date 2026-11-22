# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

extends Control

var video: KyberFileVideo
var player: VideoStreamPlayer
var stream: KyberVideoStream
var session: KyberSession
var receiver_view: AspectRatioContainer
var pause_button: Button
var mode: OptionButton
var file_path: LineEdit
var address: LineEdit
var port: SpinBox
var code: LineEdit
var codec: OptionButton
var acceleration: OptionButton
var transport: OptionButton
var repeat: CheckBox
var screen: TextureRect
var status: Label
var dialog: FileDialog
var sender_fields: VBoxContainer
var start_button: Button
var open_button: Button
var address_label: Label
var code_label: Label


func _ready() -> void:
	video = KyberFileVideo.new()
	add_child(video)
	session = KyberSession.new()
	add_child(session)
	var margin := MarginContainer.new()
	margin.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	for side in ["left", "right", "top", "bottom"]:
		margin.add_theme_constant_override("margin_" + side, 16)
	add_child(margin)
	var column := VBoxContainer.new()
	column.add_theme_constant_override("separation", 8)
	margin.add_child(column)
	_label(column, _video_title(), 24)
	_label(column, "Open the receiver and paste the sender's connection code. 640×360 at up to 24 fps. Audio streams when supported by the file.")
	mode = OptionButton.new()
	mode.add_item("Sender")
	mode.add_item("Receiver")
	mode.item_selected.connect(_change_mode)
	column.add_child(mode)
	sender_fields = VBoxContainer.new()
	column.add_child(sender_fields)
	var file_row := HBoxContainer.new()
	sender_fields.add_child(file_row)
	file_path = LineEdit.new()
	file_path.placeholder_text = "Select a video file"
	file_path.editable = false
	file_path.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	file_row.add_child(file_path)
	open_button = _button(file_row, "Open video…", _open_video_dialog)
	repeat = CheckBox.new()
	repeat.text = "Loop video"
	repeat.button_pressed = true
	sender_fields.add_child(repeat)
	transport = OptionButton.new()
	transport.add_item("Reliable")
	transport.add_item("Unreliable")
	transport.add_item("Unreliable + FEC")
	sender_fields.add_child(transport)
	codec = OptionButton.new()
	codec.add_item("JPEG (broad compatibility)")
	codec.add_item("H.264 (requires receiver support)")
	sender_fields.add_child(codec)
	acceleration = OptionButton.new()
	acceleration.add_item("H.264 acceleration: Auto")
	acceleration.add_item("H.264 acceleration: Software")
	acceleration.add_item("H.264 acceleration: Hardware required")
	column.add_child(acceleration)
	address_label = _label(column, "Listen address (local: 127.0.0.1 / LAN: 0.0.0.0)")
	var network_row := HBoxContainer.new()
	column.add_child(network_row)
	address = _field(network_row, "IP address", "127.0.0.1")
	address.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_label(network_row, "UDP port")
	port = SpinBox.new()
	port.min_value = 1
	port.max_value = 65535
	port.value = 4433
	network_row.add_child(port)
	code_label = _label(column, "Connection code (available after starting)")
	var code_row := HBoxContainer.new()
	column.add_child(code_row)
	code = _field(code_row, "", "")
	code.editable = false
	code.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_button(code_row, "Copy", func(): DisplayServer.clipboard_set(code.text))
	var buttons := HBoxContainer.new()
	column.add_child(buttons)
	start_button = _button(buttons, "Start streaming", _start)
	_button(buttons, "Stop", _stop)
	pause_button = _button(buttons, "Pause / Resume", func(): player.paused = not player.paused)
	status = _label(column, "Select a video, then click Start streaming.")
	status.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	screen = TextureRect.new()
	screen.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	screen.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
	screen.size_flags_vertical = Control.SIZE_EXPAND_FILL
	column.add_child(screen)
	player = VideoStreamPlayer.new()
	player.expand = true
	receiver_view = AspectRatioContainer.new()
	receiver_view.ratio = 16.0 / 9.0
	receiver_view.stretch_mode = AspectRatioContainer.STRETCH_FIT
	receiver_view.size_flags_vertical = Control.SIZE_EXPAND_FILL
	receiver_view.visible = false
	column.add_child(receiver_view)
	receiver_view.add_child(player)
	dialog = FileDialog.new()
	dialog.use_native_dialog = true
	dialog.title = "Select a video to stream"
	dialog.access = FileDialog.ACCESS_FILESYSTEM
	dialog.file_mode = FileDialog.FILE_MODE_OPEN_FILE
	dialog.filters = PackedStringArray(["*.mp4,*.mkv,*.mov,*.avi,*.webm,*.ogv,*.mpeg,*.mpg,*.m4v;Video files", "*;All files"])
	dialog.file_selected.connect(_select_file)
	add_child(dialog)


func _process(_delta: float) -> void:
	if video == null:
		return
	screen.texture = video.get_texture()
	status.text = video.get_status()
	if video.get_frame_count() > 0:
		status.text += " · Displayed frames: %d" % video.get_frame_count()
	if mode.selected == 0:
		code.text = video.get_connection_code()
		status.text += " · Clients: %d · Skipped sends: %d" % [video.get_connected_clients(), video.get_skipped_client_frames()]
	var performance: Dictionary = video.get_performance_statistics()
	var active := video.is_active()
	if mode.selected == 1 and stream != null:
		active = session.is_active()
		status.text = session.get_status()
		var stats := session.get_statistics()
		performance = stats.performance
		status.text += " · Received: %d · Displayed: %d · Dropped: %d" % [stats.received_frames, stream.get_frame_count(), stats.dropped_frames]
		if stats.height > 0:
			receiver_view.ratio = float(stats.width) / stats.height
		if player.paused:
			status.text += " · Playback paused"
		if not session.get_last_error().is_empty():
			status.text += " · " + session.get_last_error()
	var stages: PackedStringArray = []
	for stage in ["capture", "encode", "decode", "queue"]:
		if performance.has(stage) and performance[stage].available:
			stages.append("%s %.1f ms (p95 %.1f)" % [stage, performance[stage].last_ms, performance[stage].p95_ms])
	if performance.get("clock_synchronized", false):
		stages.append("clock RTT %.1f ms" % performance.clock_rtt_ms)
		if performance.source_to_ready.available:
			stages.append("source→ready %.1f ms" % performance.source_to_ready.last_ms)
	if not stages.is_empty():
		status.text += "\n" + " · ".join(stages)
	pause_button.disabled = mode.selected != 1 or not player.is_playing()
	start_button.disabled = active
	open_button.disabled = active
	address.editable = not active
	port.editable = not active
	repeat.disabled = active
	code.editable = mode.selected == 1 and not active


func _select_file(path: String) -> void:
	_stop()
	file_path.text = ""
	if path.begins_with("content://"):
		# Native Android pickers grant URI access; codecs need a seekable local file.
		var source := FileAccess.open(path, FileAccess.READ)
		if source == null:
			status.text = "Cannot read the selected video."
			return
		var cached := "user://selected-video.cache"
		var destination := FileAccess.open(cached, FileAccess.WRITE)
		if destination == null:
			status.text = "Cannot create a temporary video file."
			return
		while source.get_position() < source.get_length():
			var chunk := source.get_buffer(1024 * 1024)
			if chunk.is_empty():
				status.text = "Failed to read the video."
				return
			destination.store_buffer(chunk)
			if destination.get_error() != OK:
				status.text = "Failed to save the video. Check available storage."
				return
		destination.close()
		path = ProjectSettings.globalize_path(cached)
	file_path.text = path


func _open_video_dialog() -> void:
	if OS.get_name() == "iOS":
		if not video.video_file_selected.is_connected(_select_file):
			video.video_file_selected.connect(_select_file)
		video.open_native_video_dialog()
	else:
		dialog.popup_centered_ratio(0.75)


func _change_mode(index: int) -> void:
	_stop()
	mode.select(index)
	sender_fields.visible = index == 0
	screen.visible = index == 0
	receiver_view.visible = index == 1
	address.text = "127.0.0.1"
	address_label.text = "Listen address (local: 127.0.0.1 / LAN: 0.0.0.0)" if index == 0 else "Sender IP address or host name"
	code_label.text = "Connection code (available after starting)" if index == 0 else "Paste the sender's connection code"
	code.text = ""
	code.editable = index == 1
	start_button.text = "Start streaming" if index == 0 else "Connect and receive"


func _start() -> void:
	if mode.selected == 0:
		video.transport_mode = transport.selected
		video.video_codec = codec.selected
		video.acceleration = acceleration.selected
		video.start_sender(file_path.text, address.text, int(port.value), repeat.button_pressed)
	else:
		session.acceleration = acceleration.selected
		session.start_receiver(address.text.strip_edges(), int(port.value), code.text.strip_edges())
		stream = KyberVideoStream.new()
		stream.set_session(session)
		player.stream = stream
		player.play()


func _stop() -> void:
	video.stop()
	session.close()
	player.stop()
	player.paused = false
	player.stream = null
	stream = null
	screen.texture = null


func _label(parent: Node, text: String, size: int = 16) -> Label:
	var label := Label.new()
	label.text = text
	label.add_theme_font_size_override("font_size", size)
	parent.add_child(label)
	return label


func _field(parent: Node, placeholder: String, value: String) -> LineEdit:
	var field := LineEdit.new()
	field.placeholder_text = placeholder
	field.text = value
	parent.add_child(field)
	return field


func _button(parent: Node, text: String, action: Callable) -> Button:
	var button := Button.new()
	button.text = text
	button.pressed.connect(action)
	parent.add_child(button)
	return button


func _video_title() -> String:
	return "Stream video files with Kyber"
