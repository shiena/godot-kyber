#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

"""Install a checksum-verified Godot Linux or macOS executable."""
import argparse
import hashlib
import pathlib
import re
import urllib.request
import zipfile
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--platform", choices=["linux", "macos"], default="linux")
parser.add_argument("--version", default="4.7.2-stable")
parser.add_argument("--destination", required=True, type=pathlib.Path)
args = parser.parse_args()
if not re.fullmatch(r"\d+\.\d+(\.\d+)?-stable", args.version):
    raise ValueError("Expected a stable Godot version")
args.destination.mkdir(parents=True, exist_ok=True)
name = f"Godot_v{args.version}_" + ("macos.universal" if args.platform == "macos" else "linux.x86_64")
archive = args.destination / (name + ".zip")
base = f"https://github.com/godotengine/godot-builds/releases/download/{args.version}"
with urllib.request.urlopen(base + "/SHA512-SUMS.txt", timeout=60) as response:
    sums = response.read().decode()
matches = re.findall(r"^([a-fA-F0-9]{128})\s+\*?" + re.escape(archive.name) + r"$", sums, re.M)
if len(matches) != 1:
    raise ValueError("Missing or ambiguous Godot checksum")
if not archive.exists() or hashlib.sha512(archive.read_bytes()).hexdigest() != matches[0].lower():
    urllib.request.urlretrieve(base + "/" + archive.name, archive)
if hashlib.sha512(archive.read_bytes()).hexdigest() != matches[0].lower():
    raise ValueError("Godot checksum mismatch")
if args.platform == "macos":
    subprocess.run(["ditto", "-x", "-k", str(archive), str(args.destination)], check=True)
    executable = args.destination / "Godot.app/Contents/MacOS/Godot"
else:
    with zipfile.ZipFile(archive) as package:
        executable = args.destination / name
        executable.write_bytes(package.read(name))
executable.chmod(0o755)
print(executable.resolve())
