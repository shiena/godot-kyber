# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

"""Exercise Godot bus capture and AudioStream playback through every KyProto mode."""
import argparse
import json
import os
import pathlib
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--godot", required=True)
    args = parser.parse_args()
    reports = []
    for mode, kind in [(0, "shared"), (1, "shared"), (2, "shared"), (2, "standalone")]:
        with tempfile.TemporaryDirectory(prefix="live-audio-", dir=ROOT / "target") as temp:
            directory = pathlib.Path(temp)
            with (directory / "godot.log").open("w", encoding="utf-8") as log:
                process = subprocess.Popen([args.godot, "--headless", "--path", str(ROOT / "demo"), "--script", "tests/process/live_audio.gd", "--", temp, str(mode), "14480", kind], stdout=log, stderr=subprocess.STDOUT)
                try:
                    code = process.wait(timeout=40)
                except subprocess.TimeoutExpired:
                    if os.name == "nt":
                        subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"], capture_output=True)
                    else:
                        process.kill()
                    process.wait(timeout=10)
                    log.flush()
                    raise RuntimeError((directory / "godot.log").read_text(encoding="utf-8"))
            report = json.loads((directory / "live-audio.json").read_text(encoding="utf-8"))
            if code or not report["success"]:
                raise RuntimeError((directory / "godot.log").read_text(encoding="utf-8"))
            reports.append({"mode": mode, "kind": kind, **report})
            print("Live audio", kind, "mode", mode, "passed:", report["samples"], "samples", flush=True)
    output = ROOT / "target/test-results/live-audio.json"
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(reports, indent=2), encoding="utf-8")

if __name__ == "__main__":
    main()
