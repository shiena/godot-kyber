#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

"""Run video or RPC peers in separate Godot processes."""
import argparse
import json
import pathlib
import socket
import subprocess
import time
import uuid

ROOT = pathlib.Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--godot", required=True)
parser.add_argument("--scenario", choices=["video", "rpc"], default="video")
args = parser.parse_args()
directory = ROOT / "target" / (args.scenario + "-processes") / uuid.uuid4().hex
directory.mkdir(parents=True)
with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
    sock.bind(("127.0.0.1", 0))
    port = sock.getsockname()[1]
script = "tests/video_peer.gd" if args.scenario == "video" else "tests/multiplayer_peer.gd"
processes, logs = [], []
try:
    for role in ["sender", "receiver"]:
        out = (directory / (role + ".stdout.log")).open("w")
        err = (directory / (role + ".stderr.log")).open("w")
        logs.extend([out, err])
        processes.append(subprocess.Popen([
            args.godot, "--headless", "--path", str(ROOT / "demo"), "--script", script,
            "--", role, str(directory), str(port), str(ROOT / "demo/tests/fixtures/video sample.mp4"),
        ], stdout=out, stderr=err))
    deadline = time.monotonic() + 45
    for process in processes:
        if process.wait(timeout=max(1, deadline - time.monotonic())) != 0:
            raise RuntimeError(f"Godot peer failed; see {directory}")
    for role in ["sender", "receiver"]:
        report = json.loads((directory / (role + ".json")).read_text())
        if not report["success"]:
            raise RuntimeError(f"{role} failed: {report}")
    print(f"Two-process {args.scenario} passed: {directory}")
finally:
    for process in processes:
        if process.poll() is None:
            process.kill()
        process.wait()
    for log in logs:
        log.close()
