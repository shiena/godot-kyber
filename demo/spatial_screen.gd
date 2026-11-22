# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

extends MeshInstance3D

@export var client_path: NodePath
var client: KyberClient
var video_material: StandardMaterial3D


func _ready() -> void:
	client = get_node(client_path) as KyberClient
	video_material = StandardMaterial3D.new()
	video_material.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	material_override = video_material
	client.frame_received.connect(_update_frame, CONNECT_DEFERRED)
	client.disconnected.connect(_clear_frame, CONNECT_DEFERRED)


func _update_frame() -> void:
	video_material.albedo_texture = client.get_texture()


func _clear_frame() -> void:
	video_material.albedo_texture = null
