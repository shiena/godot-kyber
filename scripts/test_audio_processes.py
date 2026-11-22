# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

"""Exercise native file audio, Godot mixing, pause/resume, and loop synchronization."""
import argparse
import json
import pathlib
import subprocess
import tempfile
import shutil
import time
from udp_loss_proxy import LossProxy

ROOT = pathlib.Path(__file__).resolve().parents[1]

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--godot", required=True)
    parser.add_argument("--transport", type=int, choices=[0,1,2], default=0)
    parser.add_argument("--drop-every", type=int, default=0)
    parser.add_argument("--audio-stream", action="store_true")
    parser.add_argument("--codec", choices=["jpeg", "h264"], default="jpeg")
    parser.add_argument("--acceleration", type=int, choices=[0, 1, 2], default=0)
    args = parser.parse_args()
    if args.drop_every and args.drop_every < 2:
        parser.error("--drop-every must be at least 2")
    name = f"audio-video-{args.codec}-accel{args.acceleration}-mode{args.transport}-loss{args.drop_every}-{'audio' if args.audio_stream else 'video'}"
    reports = ROOT / "target/test-results" / name
    reports.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="kyber-audio-", dir=ROOT / "target") as temporary:
        directory = pathlib.Path(temporary)
        movie = directory / "audio.mp4"
        subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "testsrc2=size=640x360:rate=24", "-f", "lavfi", "-i", "sine=frequency=880:sample_rate=44100", "-t", "2", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac", str(movie)], check=True)
        base = [args.godot, "--headless", "--path", str(ROOT / "demo"), "--script", "tests/process/audio_video.gd", "--"]
        proxy = LossProxy(4448, 4447, args.drop_every) if args.drop_every else None
        processes = []
        handles = []
        try:
            for role in ("sender", "receiver"):
                if role == "receiver":
                    for _ in range(100):
                        if (directory / "code.txt").exists():
                            break
                        if processes[0].poll() is not None:
                            raise RuntimeError("Audio sender exited before publishing its code")
                        time.sleep(0.1)
                    else:
                        raise TimeoutError("Audio sender did not publish a code")
                log = (directory / (role + ".log")).open("w", encoding="utf-8")
                handles.append(log)
                processes.append(subprocess.Popen(base + [role, str(directory), str(movie), "4448" if proxy and role == "receiver" else "4447", "127.0.0.1", "16" if role == "sender" else "12", str(args.transport), "audio-stream" if args.audio_stream else "video-stream", args.codec, str(args.acceleration)], stdout=log, stderr=subprocess.STDOUT))
            codes = [process.wait(timeout=45) for process in processes]
            result = json.loads((directory / "receiver.json").read_text(encoding="utf-8"))
            if proxy:
                result["dropped_datagrams"] = proxy.dropped
                assert proxy.dropped > 0, "Proxy did not exercise packet loss"
            report = ROOT / "target/test-results/audio-video.json"
            report.parent.mkdir(parents=True, exist_ok=True)
            report.write_text(json.dumps(result, indent=2), encoding="utf-8")
            report.with_name(name + ".json").write_text(json.dumps(result, indent=2), encoding="utf-8")
            if any(codes):
                summary = {key: value for key, value in result.items() if key not in ("samples", "receiver_status")}
                sender = json.loads((directory / "sender.json").read_text(encoding="utf-8"))
                summary["sender_status"] = sender.get("sender_status", "unknown")
                raise RuntimeError(f"Audio process failed ({codes}): {json.dumps(summary)}; report: {report}")
            print("Audio/video processes passed:", result["captured_samples"], "captured samples; p95 max", result["sync_p95_max_ms"], "ms")
        except Exception:
            for log in handles:
                log.flush()
            for log in directory.glob("*.log"):
                print(log.name, log.read_text(encoding="utf-8"))
            raise
        finally:
            if proxy:
                proxy.close()
            for process in processes:
                if process.poll() is None:
                    process.terminate()
                    process.wait(timeout=10)
            for handle in handles:
                handle.close()
            for path in directory.iterdir():
                if path.suffix in (".json", ".log"):
                    shutil.copy2(path, reports / path.name)

if __name__ == "__main__":
    main()
