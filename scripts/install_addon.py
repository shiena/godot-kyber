#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

"""Install the complete runtime and generate Godot export dependencies."""
import argparse
import hashlib
import json
import os
import pathlib
import shutil
import platform
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--native", action="store_true")
    mode.add_argument("--mock", action="store_true")
    host = "windows" if os.name == "nt" else "macos" if platform.system() == "Darwin" else "linux"
    parser.add_argument("--platform", choices=["windows", "linux", "macos", "android", "ios"], default=host)
    parser.add_argument("--addon-dir", type=pathlib.Path, default=ROOT / "demo/addons/kyber")
    args = parser.parse_args()
    mac_arch = "aarch64" if platform.machine() == "arm64" else "x86_64"
    targets = {
        "ios": ("ios/GodotKyber.framework/GodotKyber", "aarch64-apple-ios", "avfoundation",
                ["AVFoundation.framework", "VideoToolbox.framework", "CoreMedia.framework",
                 "CoreVideo.framework", "Foundation.framework", "CoreGraphics.framework",
                 "UIKit.framework", "UniformTypeIdentifiers.framework"]),
        "android": ("android/libgodot_kyber.so", "aarch64-linux-android", "mediacodec",
                    ["libmediandk.so", "libandroid.so", "libc.so", "libdl.so", "libm.so"]),
        "windows": ("godot_kyber.dll", "x86_64-pc-windows-msvc", "media_foundation",
                    ["mfplat.dll", "mfreadwrite.dll", "ole32.dll"]),
        "linux": ("libgodot_kyber.so", "x86_64-unknown-linux-gnu", "gstreamer",
                  ["libgstreamer-1.0.so.0", "libgstapp-1.0.so.0", "libgstvideo-1.0.so.0"]),
        "macos": ("libgodot_kyber.dylib", mac_arch + "-apple-darwin", "avfoundation",
                  ["AVFoundation.framework", "VideoToolbox.framework", "CoreMedia.framework",
                   "CoreVideo.framework", "Foundation.framework", "CoreGraphics.framework"]),
    }
    binary, triple, backend, system_libraries = targets[args.platform]
    architecture = "arm64" if triple.startswith("aarch64") else "x86_64"
    addon = args.addon_dir
    for name in ("input_relay.gd", "input_relay.gd.uid", "icon.svg"):
        source = ROOT / "demo/addons/kyber" / name
        destination = addon / name
        if source.resolve() != destination.resolve():
            shutil.copy2(source, destination)
    configuration = (ROOT / "sdk/kyber.gdextension.in").read_text()
    if args.native:
        previous = addon / "runtime-manifest.json"
        if previous.exists():
            for name in json.loads(previous.read_text())["files"]:
                if not name.startswith("bin/"):
                    continue
                path = (addon / name).resolve()
                if not path.is_relative_to((addon / "bin").resolve()):
                    raise RuntimeError("Invalid previous runtime path")
                if path.is_file():
                    path.unlink()
        # Replace generated notices, including obsolete media-runtime notices.
        licenses = (addon / "licenses").resolve()
        if licenses.is_relative_to(addon.resolve()) and licenses.is_dir():
            shutil.rmtree(licenses)
        licenses.mkdir(parents=True, exist_ok=True)
        project_notices = licenses / "godot-kyber"
        project_notices.mkdir()
        for name in ("LICENSE", "NOTICE.md", "REUSE.toml", "BINARY_LICENSE.md"):
            shutil.copy2(ROOT / name, project_notices / name)
        shutil.copytree(ROOT / "LICENSES", project_notices / "LICENSES")
        # SDK notices live above the Cargo packages and are not found below.
        sdk_notices = licenses / "kyber-sdk"
        sdk_notices.mkdir()
        for name in ("COPYING.md", "COPYING.AGPLv3"):
            shutil.copy2(ROOT / ".deps/kysdk" / name, sdk_notices / name)
        shutil.copy2(ROOT / "BINARY_LICENSE.md", addon / "BINARY_LICENSE.md")
        metadata = json.loads(subprocess.check_output([
            "cargo", "metadata", "--locked", "--format-version", "1",
            "--filter-platform", triple,
        ], cwd=ROOT, text=True, encoding="utf-8"))
        packages = []
        for package in metadata["packages"]:
            packages.append({key: package.get(key) for key in ("name", "version", "license", "source", "repository")})
            source = pathlib.Path(package["manifest_path"]).parent
            target = addon / "licenses/rust" / f'{package["name"]}-{package["version"]}'
            for pattern in ("LICENSE*", "COPYING*", "NOTICE*"):
                for notice in source.glob(pattern):
                    if notice.is_file():
                        target.mkdir(parents=True, exist_ok=True)
                        shutil.copy2(notice, target / notice.name)
            if package["name"] == "audiopus_sys":
                opus_notices = target / "opus"
                opus_notices.mkdir(parents=True, exist_ok=True)
                for name in ("COPYING",):
                    shutil.copy2(source / "opus" / name, opus_notices / name)
        (addon / "licenses/rust-packages.json").write_text(json.dumps(packages, indent=2) + "\n")
        (addon / "runtime-manifest.json").write_text(json.dumps({
            "backend": backend, "files": {}, "system_libraries": system_libraries,
        }, indent=2) + "\n")
        if args.platform not in ("android", "ios"):
            configuration += '\n[dependencies]\n' + args.platform + '.' + architecture + ' = { "res://addons/kyber/licenses": "" }\n'
    (addon / "kyber.gdextension").write_text(configuration, newline="\n")
    (addon / "bin/build-info.json").write_text(json.dumps({
        "backend": "native" if args.native else "mock",
        "platform": args.platform,
        "architecture": architecture,
        "sha256": hashlib.sha256((addon / "bin" / binary).read_bytes()).hexdigest(),
    }) + "\n")


if __name__ == "__main__":
    main()
