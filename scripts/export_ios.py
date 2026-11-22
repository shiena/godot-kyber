#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

"""Export a standalone Xcode project for the iOS demo or native runtime test."""
import argparse
import pathlib
import shutil
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--godot", required=True)
parser.add_argument("--team", required=True)
parser.add_argument("--test", action="store_true")
args = parser.parse_args()
if not args.team.isalnum() or len(args.team) != 10:
    raise ValueError("Expected a ten-character Apple development team ID")
name = "kybertest" if args.test else "kyberdemo"
work = ROOT / "target/ios" / name
project = work / "project"
output = work / "export"
output.mkdir(parents=True, exist_ok=True)
shutil.copytree(ROOT / "demo", project, dirs_exist_ok=True,
                ignore=shutil.ignore_patterns(".godot", ".godothub", "gut", "export_presets.cfg"))
settings = (project / "project.godot").read_text(encoding="utf-8")
settings = settings.replace('enabled=PackedStringArray("res://addons/gut/plugin.cfg")', 'enabled=PackedStringArray()')
settings = settings.replace('renderer/rendering_method.mobile="gl_compatibility"', 'renderer/rendering_method.mobile="mobile"')
if args.test:
    script = (ROOT / "demo/tests/process/native_sdk.gd").read_text(encoding="utf-8")
    script = script.replace("extends SceneTree", "extends Node").replace("_initialize()", "_ready()")
    script = script.replace("root.add_child", "get_tree().root.add_child").replace("await process_frame", "await get_tree().process_frame")
    script = script.replace("quit(", "get_tree().quit(")
    script = script.replace('var arguments := OS.get_cmdline_user_args()', 'var arguments := PackedStringArray([ProjectSettings.globalize_path("user://sample.mp4")])')
    script = script.replace("func _run() -> void:", '''func _run() -> void:
    var fixture := FileAccess.open("res://sample.mp4", FileAccess.READ)
    var copy := FileAccess.open("user://sample.mp4", FileAccess.WRITE)
    copy.store_buffer(fixture.get_buffer(fixture.get_length()))
    copy.close()'''.replace("    ", "\t"))
    script = script.replace('print("KYBER_NATIVE_RUNTIME_OK")', '''print("KYBER_NATIVE_RUNTIME_OK")
        var result := FileAccess.open("user://native-result.txt", FileAccess.WRITE)
        result.store_string("KYBER_NATIVE_RUNTIME_OK")
        result.close()'''.replace("        ", "\t\t"))
    (project / "native.gd").write_text(script, encoding="utf-8")
    for source, target in (("tests/video_peer.gd", "video.gd"), ("tests/multiplayer_peer.gd", "rpc.gd"), ("tests/process/manual_network.gd", "manual.gd"), ("tests/process/audio_video.gd", "audio.gd")):
        script = (ROOT / "demo" / source).read_text(encoding="utf-8")
        script = script.replace("extends SceneTree", "extends Node").replace("_initialize()", "_ready()")
        script = script.replace("root.add_child", "get_tree().root.add_child").replace("await process_frame", "await get_tree().process_frame")
        script = script.replace("quit(", "get_tree().quit(")
        script = script.replace("root.title", "get_tree().root.title").replace("await create_timer(", "await get_tree().create_timer(")
        script = script.replace("OS.get_cmdline_user_args()", 'PackedStringArray(JSON.parse_string(FileAccess.get_file_as_string("user://config.json")).args)')
        script = script.replace(" -> bool:\n\tif video", " -> void:\n\tif video").replace(" -> bool:\n\tif scene", " -> void:\n\tif scene")
        script = script.replace("return false", "return")
        script = script.replace("func _finish(success: bool) -> void:", "func _finish(success: bool) -> void:\n\tset_process(false)")
        script = script.replace("video.start_sender(args[3],", "video.start_sender(ProjectSettings.globalize_path(args[3]),")
        if target == "audio.gd":
            script = script.replace("args[2]", "ProjectSettings.globalize_path(args[2])")
        if target == "rpc.gd":
            script = script.replace("args.size() != 4", "args.size() != 5")
            script = script.replace("scene.port.value = port", 'scene.port.value = port\n\tscene.host.text = JSON.parse_string(FileAccess.get_file_as_string("user://config.json")).args[4]')
        if target == "video.gd":
            script = script.replace("player.expand = true", "player.expand = true\n\tplayer.size = Vector2(960, 540)")
        (project / target).write_text(script, encoding="utf-8")
    (project / "ios_test.gd").write_text('''extends Node
func _ready() -> void:
    var addresses := FileAccess.open("user://addresses.json", FileAccess.WRITE)
    addresses.store_string(JSON.stringify(IP.get_local_addresses()))
    addresses.close()
    var scenario := "native"
    if FileAccess.file_exists("user://config.json"):
        var config = JSON.parse_string(FileAccess.get_file_as_string("user://config.json"))
        if config is Dictionary and config.get("scenario", "") in ["native", "video", "rpc", "manual", "audio"]:
            scenario = config.scenario
    var runner := Node.new()
    runner.set_script(load("res://" + scenario + ".gd"))
    add_child.call_deferred(runner)
''', encoding="utf-8")
    (project / "ios_test.tscn").write_text('''[gd_scene load_steps=2 format=3]
[ext_resource type="Script" path="res://ios_test.gd" id="1"]
[node name="NativeTest" type="Node"]
script = ExtResource("1")
''')
    shutil.copy2(ROOT / "demo/tests/fixtures/video sample.mp4", project / "sample.mp4")
    settings = settings.replace('run/main_scene="res://demo.tscn"', 'run/main_scene="res://ios_test.tscn"')
(project / "project.godot").write_text(settings, encoding="utf-8")
(project / "export_presets.cfg").write_text(f'''[preset.0]
name="iOS"
platform="iOS"
runnable=true
export_filter="all_resources"
include_filter="*.mp4,addons/kyber/licenses/*,addons/kyber/bin/ios/licenses/*"
exclude_filter="tests/*,addons/gut/*"
[preset.0.options]
application/app_store_team_id="{args.team}"
application/bundle_identifier="org.godotengine.{name}"
application/export_project_only=true
application/min_ios_version="15.0"
application/short_version="0.1.0"
application/version="1"
application/additional_plist_content="<key>NSLocalNetworkUsageDescription</key><string>Connect to Kyber video and RPC peers on your local network.</string><key>UIFileSharingEnabled</key><true/><key>LSSupportsOpeningDocumentsInPlace</key><true/>"
architectures/arm64=true
entitlements/game_center=false
''')
with (work / "export.log").open("w", encoding="utf-8") as log:
    subprocess.run([args.godot, "--headless", "--path", str(project), "--editor", "--quit-after", "60"], check=True, stdout=log, stderr=log)
    subprocess.run([args.godot, "--headless", "--path", str(project), "--export-debug", "iOS", str(output / (name + ".ipa"))], check=True, stdout=log, stderr=log)
pbxproj = output / (name + ".xcodeproj/project.pbxproj")
if not pbxproj.is_file():
    raise RuntimeError("Xcode export is incomplete; inspect export.log")
# The framework contains an iPhoneOS slice, not an iOS Simulator slice.
build_settings = pbxproj.read_text(encoding="utf-8")
build_settings = build_settings.replace(
    "SDKROOT = iphoneos;",
    "SDKROOT = iphoneos;\n\t\t\t\tSUPPORTED_PLATFORMS = iphoneos;\n\t\t\t\tSUPPORTS_MACCATALYST = NO;\n\t\t\t\tSUPPORTS_MAC_DESIGNED_FOR_IPHONE_IPAD = NO;",
)
pbxproj.write_text(build_settings, encoding="utf-8")
print(output / (name + ".xcodeproj"))
