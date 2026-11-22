# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

extends Node


func _ready() -> void:
	# Run the shared test configuration in a visible GUT window with F6.
	var runner = load("res://addons/gut/gui/GutRunner.tscn").instantiate()
	var config = load("res://addons/gut/gut_config.gd").new()
	config.load_options("res://.gutconfig.json")
	config.options.should_exit = false
	runner.gut_config = config
	add_child(runner)
	runner.call_deferred("run_tests", true)
