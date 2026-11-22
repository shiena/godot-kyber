# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

extends "res://file_video.gd"

var audio_source: OptionButton
var audio_bus: LineEdit
var captured: SubViewport
var moving: ColorRect
var elapsed := 0.0
var caption: Label

func _ready() -> void:
	super._ready()
	captured = SubViewport.new()
	captured.size = Vector2i(640, 360)
	captured.render_target_update_mode = SubViewport.UPDATE_ALWAYS
	add_child(captured)
	var background := ColorRect.new()
	background.size = Vector2(640, 360)
	background.color = Color("183650")
	captured.add_child(background)
	moving = ColorRect.new()
	moving.size = Vector2(100, 100)
	moving.color = Color("ffbd59")
	captured.add_child(moving)
	caption = Label.new()
	caption.text = "Godot Viewport → Kyber"
	caption.position = Vector2(20, 20)
	caption.add_theme_font_size_override("font_size", 28)
	captured.add_child(caption)
	file_path.get_parent().hide()
	repeat.hide()
	audio_source = OptionButton.new()
	audio_source.add_item("No audio")
	audio_source.add_item("Godot audio bus")
	audio_source.add_item("Microphone (requires input setup and permission)")
	sender_fields.add_child(audio_source)
	audio_bus = LineEdit.new()
	audio_bus.text = "Master"
	audio_bus.placeholder_text = "Audio bus to stream"
	sender_fields.add_child(audio_bus)

func _process(delta: float) -> void:
	elapsed += delta
	if moving != null:
		caption.text = "Godot Viewport → Kyber  %.2f s" % elapsed
		moving.position = Vector2(270 + sin(elapsed * 2.0) * 230, 140 + cos(elapsed * 1.7) * 100)
	super._process(delta)

func _start() -> void:
	if mode.selected == 0:
		video.transport_mode = transport.selected
		if video.start_viewport_sender(captured, address.text, int(port.value), 24):
			if audio_source.selected == 1:
				video.capture_audio_bus(audio_bus.text)
			elif audio_source.selected == 2:
				video.capture_microphone()
	else:
		super._start()


func _video_title() -> String:
	return "Stream a Godot viewport with Kyber"
