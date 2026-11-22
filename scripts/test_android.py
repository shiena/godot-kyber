#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

"""Export an Android test APK and verify native media and Windows/Android transport."""
import argparse
import json
import pathlib
import shutil
import subprocess
import time
import zipfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
PACKAGE = "org.godotengine.kybertest"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--adb", required=True)
    parser.add_argument("--serial", required=True)
    parser.add_argument("--godot", required=True)
    parser.add_argument("--windows-ip", required=True)
    parser.add_argument("--device-ip", required=True)
    parser.add_argument("--codec", choices=["jpeg", "h264"], default="jpeg")
    parser.add_argument("--audio", action="store_true", help="Test file audio and video in both directions")
    parser.add_argument("--skip-export", action="store_true", help="Reuse the installed test APK")
    parser.add_argument("--manual", choices=["windows-to-android", "android-to-windows"], help="Leave both video windows running for visual inspection")
    parser.add_argument("--video", type=pathlib.Path, help="Video file for the manual loop test")
    args = parser.parse_args()
    work = ROOT / "target/android-test"
    project = work / "project"
    project.mkdir(parents=True, exist_ok=True)
    adb = [args.adb, "-s", args.serial]

    def run(command, **kwargs):
        return subprocess.run(command, check=True, text=True, encoding="utf-8", errors="replace", **kwargs)

    def shell(*command, check=True):
        return subprocess.run(adb + ["shell", *command], check=check, capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=30)

    def upload(path, name):
        remote = "/data/local/tmp/kyber-" + name
        run(adb + ["push", str(path), remote], stdout=subprocess.DEVNULL)
        shell("run-as", PACKAGE, "cp", remote, "files/" + name)

    def read(name):
        result = shell("run-as", PACKAGE, "cat", "files/" + name, check=False)
        return result.stdout if result.returncode == 0 else ""

    for name in ("addons/kyber", "tests/fixtures"):
        shutil.copytree(ROOT / "demo" / name, project / name, dirs_exist_ok=True)
    for name in ("file_video.gd", "multiplayer_demo.gd"):
        shutil.copy2(ROOT / "demo" / name, project / name)
    for source, target in (("tests/video_peer.gd", "video.gd"), ("tests/multiplayer_peer.gd", "rpc.gd"), ("tests/process/native_sdk.gd", "native.gd"), ("tests/process/manual_network.gd", "manual.gd"), ("tests/process/audio_video.gd", "audio.gd")):
        script = (ROOT / "demo" / source).read_text(encoding="utf-8")
        script = script.replace("extends SceneTree", "extends Node").replace("_initialize()", "_ready()")
        script = script.replace("root.add_child", "get_tree().root.add_child").replace("await process_frame", "await get_tree().process_frame")
        script = script.replace("quit(", "get_tree().quit(")
        script = script.replace("root.title", "get_tree().root.title").replace("await create_timer(", "await get_tree().create_timer(")
        script = script.replace("OS.get_cmdline_user_args()", 'PackedStringArray(JSON.parse_string(FileAccess.get_file_as_string("user://config.json")).args)')
        script = script.replace(" -> bool:\n\tif video", " -> void:\n\tif video").replace(" -> bool:\n\tif scene", " -> void:\n\tif scene")
        if target != "native.gd":
            script = script.replace("return false", "return")
        if target == "rpc.gd":
            script = script.replace("args.size() != 4", "args.size() != 5")
            script = script.replace("scene.port.value = port", 'scene.port.value = port\n\tscene.host.text = JSON.parse_string(FileAccess.get_file_as_string("user://config.json")).args[4]')
        if target == "video.gd":
            script = script.replace("player.expand = true", "player.expand = true\n\tplayer.size = Vector2(960, 540)")
        (project / target).write_text(script, encoding="utf-8")
    (project / "main.gd").write_text('''extends Node
func _ready() -> void:
    var config = JSON.parse_string(FileAccess.get_file_as_string("user://config.json"))
    if config == null:
        return
    var runner = Node.new()
    runner.set_script(load("res://" + config.scenario + ".gd"))
    add_child.call_deferred(runner)
''')
    (project / "main.tscn").write_text('''[gd_scene load_steps=2 format=3]
[ext_resource type="Script" path="res://main.gd" id="1"]
[node name="AndroidTest" type="Node"]
script = ExtResource("1")
''')
    (project / "project.godot").write_text('''config_version=5
[application]
config/name="Kyber Android Test"
run/main_scene="res://main.tscn"
[display]
window/size/viewport_width=960
window/size/viewport_height=540
window/stretch/mode="canvas_items"
[rendering]
renderer/rendering_method="gl_compatibility"
renderer/rendering_method.mobile="gl_compatibility"
textures/vram_compression/import_etc2_astc=true
''')
    (project / "export_presets.cfg").write_text(f'''[preset.0]
name="Android"
platform="Android"
runnable=true
export_filter="all_resources"
include_filter="*.mp4,*.json,addons/kyber/licenses/*,addons/kyber/bin/android/licenses/*"
exclude_filter=""
[preset.0.options]
gradle_build/use_gradle_build=false
architectures/armeabi-v7a=false
architectures/arm64-v8a=true
architectures/x86=false
architectures/x86_64=false
package/unique_name="{PACKAGE}"
package/name="Kyber Android Test"
version/code=1
version/name="0.1"
permissions/internet=true
screen/keep_screen_on=true
''')
    if not args.skip_export:
        with (work / "export.log").open("w", encoding="utf-8") as log:
            run([args.godot, "--headless", "--path", str(project), "--editor", "--quit-after", "60"], stdout=log, stderr=log)
            run([args.godot, "--headless", "--path", str(project), "--export-debug", "Android", str(work / "kyber-test.apk")], stdout=log, stderr=log)
        with zipfile.ZipFile(work / "kyber-test.apk") as apk:
            names = apk.namelist()
            if "assets/project.binary" not in names or "lib/arm64-v8a/libgodot_kyber.so" not in names:
                raise RuntimeError("Incomplete Android export; see export.log")
        run(adb + ["start-server"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=30)
        run(adb + ["install", "--no-incremental", "-r", str(work / "kyber-test.apk")], timeout=120)
    shell("run-as", PACKAGE, "mkdir", "-p", "files")
    upload(ROOT / "demo/tests/fixtures/video sample.mp4", "sample.mp4")
    remote_dir = f"/data/user/0/{PACKAGE}/files"
    results = {}

    def launch(config, code_file=None):
        shell("am", "force-stop", PACKAGE)
        shell("run-as", PACKAGE, "rm", "-f", "files/stop", "files/code.txt", "files/sender.json", "files/receiver.json", "files/sender.received", "files/receiver.received")
        config_path = work / "config.json"
        config_path.write_text(json.dumps(config))
        upload(config_path, "config.json")
        if code_file is not None:
            upload(code_file, "code.txt")
        return shell("am", "start", "-W", "-n", PACKAGE + "/com.godot.game.GodotAppLauncher")

    if args.audio:
        fixture = work / "audio.mp4"
        run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "testsrc2=size=640x360:rate=24", "-f", "lavfi", "-i", "sine=frequency=880:sample_rate=44100", "-t", "2", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac", str(fixture)])
        upload(fixture, "audio.mp4")
        for index, phone_role in enumerate(("receiver", "sender")):
            directory = work / ("audio-" + phone_role + "-" + str(time.time_ns()))
            directory.mkdir()
            port = 14600 + index
            pc_role = "sender" if phone_role == "receiver" else "receiver"
            command = [args.godot, "--headless", "--path", str(ROOT / "demo"), "--script", "tests/process/audio_video.gd", "--", pc_role, str(directory), str(fixture), str(port), "0.0.0.0" if pc_role == "sender" else args.device_ip, "45" if pc_role == "sender" else "20"]
            config = {"scenario": "audio", "args": [phone_role, remote_dir, remote_dir + "/audio.mp4", str(port), args.windows_ip if phone_role == "receiver" else "0.0.0.0", "20" if phone_role == "receiver" else "45"]}
            command += ["0", "video-stream", args.codec, "0"]
            config["args"] += ["0", "video-stream", args.codec, "0"]
            process = None
            log = (directory / "windows.log").open("w", encoding="utf-8")
            try:
                if pc_role == "sender":
                    process = subprocess.Popen(command, stdout=log, stderr=log)
                else:
                    launch(config)
                code = directory / "code.txt"
                for _ in range(100):
                    if phone_role == "sender":
                        value = read("code.txt").strip()
                        if len(value) == 129:
                            code.write_text(value)
                    if code.exists():
                        break
                    time.sleep(0.1)
                else:
                    raise TimeoutError("Audio sender did not publish a code")
                if phone_role == "receiver":
                    launch(config, code)
                else:
                    process = subprocess.Popen(command, stdout=log, stderr=log)
                report = directory / "receiver.json"
                for _ in range(180):
                    if phone_role == "receiver":
                        text = read("receiver.json")
                        if text:
                            report.write_text(text, encoding="utf-8")
                    try:
                        result = json.loads(report.read_text(encoding="utf-8"))
                        if result.get("samples"):
                            break
                    except (FileNotFoundError, json.JSONDecodeError):
                        pass
                    time.sleep(0.25)
                else:
                    raise TimeoutError("Missing audio receiver report")
                results[phone_role] = result
                (work / "audio-results.json").write_text(json.dumps(results, indent=2), encoding="utf-8")
                if not (result["captured_samples"] > 48000 and result["energy"] > 1 and result["samples"][-1]["received_frames"] > 50 and result["sync_p95_max_ms"] < 180):
                    raise RuntimeError("Audio validation failed: " + str(report))
                print("Android " + phone_role + ": PASS; samples=" + str(result["captured_samples"]) + "; sync p95=" + str(result["sync_p95_max_ms"]), flush=True)
            finally:
                (directory / "stop").write_text("done")
                shell("am", "force-stop", PACKAGE)
                if process is not None:
                    try:
                        process.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        process.terminate()
                        process.wait(timeout=5)
                log.close()
        return

    if args.manual:
        if not args.video or not args.video.is_file():
            raise RuntimeError("Manual testing requires --video pointing to a local video")
        directory = ROOT / "target" / ("manual-" + args.manual)
        directory.mkdir(exist_ok=True)
        code_path = directory / "code.txt"
        code_path.unlink(missing_ok=True)
        upload(args.video, "manual.mp4")
        android_role = "receiver" if args.manual == "windows-to-android" else "sender"
        windows_role = "sender" if android_role == "receiver" else "receiver"
        launch({"scenario": "manual", "args": [android_role, args.windows_ip if android_role == "receiver" else "0.0.0.0", "14436", remote_dir + "/manual.mp4", remote_dir, args.codec, "0"]})
        with (directory / "windows.log").open("w", encoding="utf-8") as output:
            command = [args.godot, "--path", str(ROOT / "demo"), "--script", "res://tests/process/manual_network.gd", "--", windows_role, "0.0.0.0" if windows_role == "sender" else args.device_ip, "14436", str(args.video.resolve()), str(directory)]
            command += [args.codec, "0"]
            process = subprocess.Popen(command, stdout=output, stderr=output, creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        (directory / "windows-pid.txt").write_text(str(process.pid))
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            if windows_role == "sender" and code_path.exists() and len(code_path.read_text()) == 129:
                upload(code_path, "code.txt")
                break
            if android_role == "sender":
                code = read("code.txt")
                if len(code) == 129:
                    code_path.write_text(code)
                    break
            time.sleep(0.2)
        else:
            process.kill()
            shell("am", "force-stop", PACKAGE)
            raise RuntimeError("Manual sender did not publish its connection code")
        print(f"Manual test running: {args.manual}; Windows PID {process.pid}; logs: {directory}")
        return

    launch({"scenario": "native", "args": [remote_dir + "/sample.mp4"]})
    pid = shell("pidof", PACKAGE).stdout.strip()
    deadline = time.monotonic() + 40
    while time.monotonic() < deadline:
        log = shell("logcat", "-d", "--pid=" + pid, check=False).stdout
        if "KYBER_NATIVE_RUNTIME_OK" in log:
            results["native_runtime"] = True
            break
        time.sleep(0.5)
    (work / "native.log").write_text(log, encoding="utf-8")
    if not results.get("native_runtime"):
        raise RuntimeError("Android native runtime failed; see target/android-test/native.log")

    for scenario in ("video", "rpc"):
        for android_role in ("receiver", "sender"):
            name = scenario + "-android-" + android_role
            directory = work / name
            directory.mkdir(exist_ok=True)
            for old in directory.glob("*"):
                if old.is_file():
                    old.unlink()
            windows_role = "sender" if android_role == "receiver" else "receiver"
            port = 14434 if scenario == "video" else 14435
            host = args.windows_ip if android_role == "receiver" else args.device_ip
            android_args = [android_role, remote_dir, str(port), remote_dir + "/sample.mp4", "0.0.0.0" if android_role == "sender" else host]
            launch({"scenario": scenario, "args": android_args})
            pid = shell("pidof", PACKAGE).stdout.strip()
            windows_script = ROOT / "demo/tests/video_peer.gd"
            if scenario == "rpc":
                windows_script = directory / "rpc.gd"
                script = (ROOT / "demo/tests/multiplayer_peer.gd").read_text(encoding="utf-8")
                script = script.replace("args.size() != 4", "args.size() != 5").replace("scene.port.value = port", "scene.port.value = port\n\tscene.host.text = OS.get_cmdline_user_args()[4]")
                windows_script.write_text(script, encoding="utf-8")
            command = [args.godot, "--headless", "--path", str(ROOT / "demo"), "--script", str(windows_script), "--", windows_role, str(directory), str(port), str(ROOT / "demo/tests/fixtures/video sample.mp4"), "0.0.0.0" if windows_role == "sender" else host]
            with (directory / "windows.log").open("w", encoding="utf-8") as output:
                process = subprocess.Popen(command, stdout=output, stderr=output)
                try:
                    deadline = time.monotonic() + 40
                    copied = set()
                    while time.monotonic() < deadline:
                        for filename in ("code.txt", "sender.received", "receiver.received"):
                            if filename in copied:
                                continue
                            local = directory / filename
                            if local.exists() and local.stat().st_size:
                                upload(local, filename)
                                copied.add(filename)
                            else:
                                content = read(filename)
                                if content and (filename != "code.txt" or len(content) == 129):
                                    local.write_text(content)
                                    copied.add(filename)
                        report = read(android_role + ".json")
                        if report:
                            (directory / ("android-" + android_role + ".json")).write_text(report, encoding="utf-8")
                        if report and process.poll() is not None:
                            break
                        time.sleep(0.2)
                    process.wait(timeout=5)
                    android_report = json.loads(read(android_role + ".json"))
                    windows_report = json.loads((directory / (windows_role + ".json")).read_text(encoding="utf-8"))
                    results[name] = {"android": android_report, "windows": windows_report}
                    if process.returncode or not android_report["success"] or not windows_report["success"]:
                        raise RuntimeError(name + " failed")
                finally:
                    if process.poll() is None:
                        process.kill()
                        process.wait()
                    (directory / "android.log").write_text(shell("logcat", "-d", "--pid=" + pid).stdout, encoding="utf-8")
            print(name + " passed", flush=True)
    shell("am", "force-stop", PACKAGE)
    (work / "results.json").write_text(json.dumps(results, indent=2), encoding="utf-8")
    print("Android native runtime and bidirectional Windows video/RPC tests passed")


if __name__ == "__main__":
    main()
