#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

"""Test an installed iPhone test app against the native macOS addon."""
import argparse
import json
import pathlib
import subprocess
import time
import uuid

ROOT = pathlib.Path(__file__).resolve().parents[1]
PACKAGE = "org.godotengine.kybertest"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--codec", choices=["jpeg", "h264"], default="jpeg")
    parser.add_argument("--audio", action="store_true", help="Test synchronized file audio in both directions")
    parser.add_argument("--device", required=True)
    parser.add_argument("--godot", required=True)
    parser.add_argument("--mac-ip", required=True)
    parser.add_argument("--device-ip", required=True)
    args = parser.parse_args()
    work = ROOT / "target/ios-tests" / uuid.uuid4().hex[:12]
    work.mkdir(parents=True)
    container = ["--device", args.device, "--domain-type", "appDataContainer",
                 "--domain-identifier", PACKAGE]

    def control(*command, check=True):
        return subprocess.run(["xcrun", "devicectl", "device", *command], check=check,
                              capture_output=True, text=True, timeout=30)

    def upload(source, destination):
        control("copy", "to", *container, "--source", str(source), "--destination", "Documents/" + destination)

    def download(source, destination):
        return control("copy", "from", *container, "--source", "Documents/" + source,
                       "--destination", str(destination), check=False).returncode == 0

    def launch():
        control("process", "launch", "--device", args.device, "--terminate-existing", PACKAGE)

    def stop_phone():
        report = work / "processes.json"
        control("info", "processes", "--device", args.device, "--json-output", str(report))
        for process in json.loads(report.read_text())["result"]["runningProcesses"]:
            if "/kybertest.app/kybertest" in process.get("executable", ""):
                control("process", "terminate", "--device", args.device,
                        "--pid", str(process["processIdentifier"]), check=False)

    if args.audio:
        fixture = work / "audio.mp4"
        subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "testsrc2=size=640x360:rate=24", "-f", "lavfi", "-i", "sine=frequency=880:sample_rate=44100", "-t", "2", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac", str(fixture)], check=True)
        upload(fixture, "audio.mp4")
        results = {}
        for index, phone_role in enumerate(("receiver", "sender")):
            directory = work / ("audio-" + phone_role)
            directory.mkdir()
            (directory / "ready").write_text("ok")
            remote = work.name + "-audio-" + phone_role
            upload(directory, remote)
            port = 14610 + index
            mac_role = "sender" if phone_role == "receiver" else "receiver"
            config = {"scenario": "audio", "args": [phone_role, "user://" + remote, "user://audio.mp4", str(port), args.mac_ip if phone_role == "receiver" else "0.0.0.0", "20" if phone_role == "receiver" else "90"]}
            config["args"] += ["0", "video-stream", args.codec, "0"]
            config_file = work / "config.json"
            config_file.write_text(json.dumps(config))
            upload(config_file, "config.json")
            command = [args.godot, "--headless", "--path", str(ROOT / "demo"), "--script", "tests/process/audio_video.gd", "--", mac_role, str(directory), str(fixture), str(port), "0.0.0.0" if mac_role == "sender" else args.device_ip, "90" if mac_role == "sender" else "20"]
            command += ["0", "video-stream", args.codec, "0"]
            process = None
            with (directory / "mac.log").open("w") as log:
                try:
                    if phone_role == "sender":
                        launch()
                    else:
                        process = subprocess.Popen(command, stdout=log, stderr=log)
                    code = directory / "code.txt"
                    deadline = time.monotonic() + 30
                    while time.monotonic() < deadline:
                        if phone_role == "sender":
                            download(remote + "/code.txt", code)
                        if code.exists() and len(code.read_text()) == 129:
                            break
                        time.sleep(0.2)
                    else:
                        raise TimeoutError("Audio sender did not publish a code")
                    if phone_role == "receiver":
                        upload(code, remote + "/code.txt")
                        launch()
                    else:
                        process = subprocess.Popen(command, stdout=log, stderr=log)
                    report = directory / "receiver.json"
                    deadline = time.monotonic() + 60
                    while time.monotonic() < deadline:
                        if phone_role == "receiver":
                            download(remote + "/receiver.json", report)
                        try:
                            result = json.loads(report.read_text())
                            if result.get("samples"):
                                break
                        except (FileNotFoundError, json.JSONDecodeError):
                            pass
                        time.sleep(0.5)
                    else:
                        raise TimeoutError("Missing audio receiver report")
                    results[phone_role] = result
                    (work / "audio-results.json").write_text(json.dumps(results, indent=2))
                    if not (result["captured_samples"] > 48000 and result["energy"] > 1 and result["samples"][-1]["received_frames"] > 50 and result["sync_p95_max_ms"] < 180):
                        raise RuntimeError("Audio validation failed: " + str(report))
                    print("iPhone " + phone_role + ": PASS; samples=" + str(result["captured_samples"]) + "; sync p95=" + str(result["sync_p95_max_ms"]), flush=True)
                finally:
                    (directory / "stop").write_text("done")
                    stop_phone()
                    if process is not None:
                        try:
                            process.wait(timeout=10)
                        except subprocess.TimeoutExpired:
                            process.terminate()
                            process.wait(timeout=5)
        print(work / "audio-results.json", flush=True)
        return

    # Preserve the same RPC node path on both platforms.
    rpc = (ROOT / "demo/tests/multiplayer_peer.gd").read_text(encoding="utf-8")
    rpc = rpc.replace("args.size() != 4", "args.size() != 5")
    rpc = rpc.replace("scene.port.value = port", "scene.port.value = port\n\tscene.host.text = OS.get_cmdline_user_args()[4]")
    rpc_script = work / "rpc.gd"
    rpc_script.write_text(rpc, encoding="utf-8")
    results = {}
    try:
        for index, (scenario, phone_role) in enumerate((("video", "receiver"), ("video", "sender"),
                                                       ("rpc", "receiver"), ("rpc", "sender"))):
            name = scenario + "-" + phone_role
            directory = work / name
            directory.mkdir()
            (directory / "ready").write_text("ok")
            remote = work.name + "-" + name
            upload(directory, remote)
            port = 14500 + index
            mac_role = "sender" if phone_role == "receiver" else "receiver"
            config = {"scenario": scenario, "args": [phone_role, "user://" + remote, str(port),
                      "user://sample.mp4", args.mac_ip if phone_role == "receiver" else "0.0.0.0"]}
            config["args"] += ["0", "video-stream", args.codec, "0"]
            config_file = work / "config.json"
            config_file.write_text(json.dumps(config))
            upload(config_file, "config.json")
            fixture = ROOT / "demo/tests/fixtures/video sample.mp4"
            upload(fixture, "sample.mp4")
            script = str(rpc_script) if scenario == "rpc" else "res://tests/video_peer.gd"
            command = [args.godot, "--headless", "--path", str(ROOT / "demo"), "--script", script,
                       "--", mac_role, str(directory), str(port), str(fixture),
                       args.device_ip if mac_role == "receiver" else "0.0.0.0"]
            process = None
            with (directory / "mac.log").open("w") as log:
                try:
                    if phone_role == "sender":
                        launch()
                    else:
                        process = subprocess.Popen(command, stdout=log, stderr=log)
                    deadline = time.monotonic() + 12
                    code = directory / "code.txt"
                    while time.monotonic() < deadline:
                        if phone_role == "sender":
                            download(remote + "/code.txt", code)
                        if code.exists() and len(code.read_text()) == 129:
                            break
                        time.sleep(0.2)
                    else:
                        raise RuntimeError(name + ": no connection code")
                    if phone_role == "receiver":
                        upload(code, remote + "/code.txt")
                        launch()
                    else:
                        process = subprocess.Popen(command, stdout=log, stderr=log)
                    deadline = time.monotonic() + 35
                    phone_report = directory / (phone_role + ".json")
                    mac_report = directory / (mac_role + ".json")
                    while time.monotonic() < deadline:
                        if scenario == "rpc":
                            marker = directory / (mac_role + ".received")
                            if marker.exists():
                                upload(marker, remote + "/" + marker.name)
                            download(remote + "/" + phone_role + ".received", directory / (phone_role + ".received"))
                        download(remote + "/" + phone_role + ".json", phone_report)
                        if phone_report.exists() and mac_report.exists():
                            break
                        time.sleep(0.2)
                    reports = {role: json.loads((directory / (role + ".json")).read_text())
                               for role in ("sender", "receiver")}
                    results[name] = reports
                    (work / "results.json").write_text(json.dumps(results, indent=2), encoding="utf-8")
                    if not all(report["success"] for report in reports.values()):
                        raise RuntimeError(name + ": failed; inspect " + str(directory))
                    print(name + ": PASS", flush=True)
                finally:
                    if process is not None and process.poll() is None:
                        process.terminate()
                        try:
                            process.wait(timeout=5)
                        except subprocess.TimeoutExpired:
                            process.kill()
                            process.wait()
                    stop_phone()
    finally:
        stop_phone()
    print(work / "results.json", flush=True)


if __name__ == "__main__":
    main()
