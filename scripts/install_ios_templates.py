#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

"""Install the checksum-verified iOS template for the selected Godot release."""
import argparse
import hashlib
import pathlib
import re
import urllib.request
import zipfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--version", default="4.7.2-stable")
args = parser.parse_args()
if not re.fullmatch(r"\d+\.\d+(\.\d+)?-stable", args.version):
    raise ValueError("Expected a stable Godot version")
root = pathlib.Path(__file__).resolve().parents[1]
cache = root / "target/tools"
cache.mkdir(parents=True, exist_ok=True)
name = f"Godot_v{args.version}_export_templates.tpz"
archive = cache / name
base = f"https://github.com/godotengine/godot-builds/releases/download/{args.version}"
with urllib.request.urlopen(base + "/SHA512-SUMS.txt", timeout=60) as response:
    sums = response.read().decode()
matches = re.findall(r"^([a-fA-F0-9]{128})\s+\*?" + re.escape(name) + r"$", sums, re.M)
if len(matches) != 1:
    raise ValueError("Missing or ambiguous Godot checksum")
def digest():
    with archive.open("rb") as source:
        return hashlib.file_digest(source, "sha512").hexdigest()
if not archive.exists() or digest() != matches[0].lower():
    urllib.request.urlretrieve(base + "/" + name, archive)
if digest() != matches[0].lower():
    raise ValueError("Godot template checksum mismatch")
destination = pathlib.Path.home() / "Library/Application Support/Godot/export_templates" / args.version.replace("-stable", ".stable")
destination.mkdir(parents=True, exist_ok=True)
with zipfile.ZipFile(archive) as package:
    for filename in ("ios.zip", "version.txt"):
        (destination / filename).write_bytes(package.read("templates/" + filename))
print(destination)
