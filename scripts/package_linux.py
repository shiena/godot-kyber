#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

"""Package the tested Linux extension without system media libraries."""
import hashlib
import json
import pathlib
import tarfile
import tomllib

root = pathlib.Path(__file__).resolve().parents[1]
addon = root / "demo/addons/kyber"
info = json.loads((addon / "bin/build-info.json").read_text())
proof = json.loads((root / "target/test-results/native-runtime.json").read_text())
binary = addon / "bin/libgodot_kyber.so"
checksum = hashlib.sha256(binary.read_bytes()).hexdigest()
if info.get("platform") != "linux" or info["backend"] != "native" or checksum != info["sha256"] or checksum != proof["sha256"]:
    raise RuntimeError("Run scripts/test_linux.sh against this native Linux binary first")
if hashlib.sha256((addon / "runtime-manifest.json").read_bytes()).hexdigest().lower() != proof["runtime"].lower():
    raise RuntimeError("Runtime manifest changed after testing")
version = tomllib.loads((root / "Cargo.toml").read_text())["package"]["version"]
destination = root / "target/packages" / f"godot-kyber-{version}-linux-x86_64.tar.gz"
destination.parent.mkdir(parents=True, exist_ok=True)
with tarfile.open(destination, "w:gz") as package:
    for path in [binary, addon / "kyber.gdextension", addon / "runtime-manifest.json", addon / "licenses", addon / "input_relay.gd", addon / "input_relay.gd.uid", addon / "icon.svg"]:
        package.add(path, arcname="godot-kyber/addons/kyber/" + path.relative_to(addon).as_posix())
    for path in [root / "README.md", root / "README_ja.md", root / "LICENSE", root / "LICENSES", root / "NOTICE.md", root / "BINARY_LICENSE.md", root / "REUSE.toml", root / "docs"]:
        package.add(path, arcname="godot-kyber/" + path.relative_to(root).as_posix())
print(destination)
