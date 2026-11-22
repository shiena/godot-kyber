# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

extends GutTest


func test_receiver_scene_displays_and_clears_local_video() -> void:
	var scene = load("res://main.tscn").instantiate()
	add_child_autofree(scene)
	scene.client.start_demo()
	await wait_process_frames(5)
	assert_not_null(scene.screen.texture)
	assert_same(scene.screen.texture, scene.client.get_texture())
	assert_string_contains(scene.status.text, "no network")
	scene._close()
	await wait_process_frames(3)
	assert_null(scene.screen.texture)
	assert_eq(scene.client.get_state(), KyberClient.DISCONNECTED)


func test_spatial_screen_receives_texture_and_clears_on_disconnect() -> void:
	var parent := Node3D.new()
	add_child_autofree(parent)
	var client := KyberClient.new()
	client.name = "Receiver"
	parent.add_child(client)
	var screen := MeshInstance3D.new()
	screen.mesh = PlaneMesh.new()
	screen.set_script(load("res://spatial_screen.gd"))
	screen.set("client_path", NodePath("../Receiver"))
	parent.add_child(screen)
	client.start_demo()
	await wait_process_frames(5)
	var material := screen.material_override as StandardMaterial3D
	assert_not_null(material.albedo_texture)
	assert_same(material.albedo_texture, client.get_texture())
	client.close()
	await wait_process_frames(3)
	assert_null(material.albedo_texture)
