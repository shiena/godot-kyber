#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

"""Assemble all native platforms into addon-only and demo-project archives."""
import argparse
import hashlib
import json
import pathlib
import re
import tempfile
import shutil
import tomllib
import zipfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
BINARIES = {
    "windows": ("godot_kyber.dll", "x86_64"),
    "linux": ("libgodot_kyber.so", "x86_64"),
    "macos": ("libgodot_kyber.dylib", "arm64"),
    "android": ("android/libgodot_kyber.so", "arm64"),
    "ios": ("ios/GodotKyber.framework/GodotKyber", "arm64"),
}


def version_for(root, requested=None):
    version = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?", version):
        raise ValueError("Invalid Cargo package version")
    if requested and requested.removeprefix("v") != version:
        raise ValueError("Release version must match Cargo.toml; update the crate version first")
    return "v" + version


def archive_tree(source, destination, prefix=""):
    with zipfile.ZipFile(destination, "w", zipfile.ZIP_DEFLATED) as archive:
        for path in sorted(source.rglob("*")):
            if path.is_file():
                archive.write(path, prefix + path.relative_to(source).as_posix())


def package(root, staged, destination, requested=None):
    version = version_for(root, requested)
    destination.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as temporary:
        project = pathlib.Path(temporary)
        addon = project / "addons/kyber"
        addon.mkdir(parents=True)
        # Keep reusable addon resources, excluding generated host-specific files.
        for source in (root / "demo/addons/kyber").iterdir():
            if source.name in {"bin", "licenses", "runtime-manifest.json", "kyber.gdextension"} or source.name.startswith("."):
                continue
            if source.is_dir():
                shutil.copytree(source, addon / source.name)
            else:
                shutil.copy2(source, addon / source.name)
        for platform, (relative, architecture) in BINARIES.items():
            source = staged / ("addon-" + platform)
            binary = source / "bin" / relative
            info = json.loads((source / "bin/build-info.json").read_text())
            checksum = hashlib.sha256(binary.read_bytes()).hexdigest()
            if (info.get("platform"), info.get("architecture"), info.get("backend"), info.get("sha256")) != (platform, architecture, "native", checksum):
                raise ValueError("Invalid native artifact: " + platform)
            target = addon / "bin" / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            if platform == "ios":
                if not (binary.parent / "Info.plist").is_file():
                    raise ValueError("iOS framework is missing Info.plist")
                shutil.copytree(binary.parent, target.parent, dirs_exist_ok=True)
            else:
                shutil.copy2(binary, target)
            if platform in {"linux", "macos", "ios"}:
                target.chmod(0o755)
            licenses = source / "licenses"
            if not licenses.is_dir() or not any(licenses.rglob("*")):
                raise ValueError("Missing dependency licenses: " + platform)
            for name in ("COPYING.md", "COPYING.AGPLv3"):
                notice = licenses / "kyber-sdk" / name
                if not notice.is_file() or not notice.stat().st_size:
                    raise ValueError(f"Missing Kyber SDK notice: {platform}/{name}")
            shutil.copytree(licenses, addon / "licenses" / platform)
            metadata = addon / "metadata" / platform
            metadata.mkdir(parents=True)
            shutil.copy2(source / "bin/build-info.json", metadata / "build-info.json")
            shutil.copy2(source / "runtime-manifest.json", metadata / "runtime-manifest.json")
        config = (root / "sdk/kyber.gdextension.in").read_text(encoding="utf-8")
        config += '\n[dependencies]\n'
        for platform, arch in (("windows", "x86_64"), ("linux", "x86_64"), ("macos", "arm64")):
            config += f'{platform}.{arch} = {{ "res://addons/kyber/licenses": "" }}\n'
        (addon / "kyber.gdextension").write_text(config, encoding="utf-8")
        for name in ("LICENSE", "NOTICE.md", "REUSE.toml", "BINARY_LICENSE.md"):
            shutil.copy2(root / name, addon / name)
        # Windows treats LICENSES and the existing licenses directory as identical.
        # Use one portable location for project notices on every build host.
        project_notices = addon / "licenses/godot-kyber"
        project_notices.mkdir(parents=True, exist_ok=True)
        for name in ("LICENSE", "NOTICE.md", "REUSE.toml", "BINARY_LICENSE.md"):
            shutil.copy2(root / name, project_notices / name)
        shutil.copytree(root / "LICENSES", project_notices / "LICENSES", dirs_exist_ok=True)
        addon_archive = destination / f"godot-kyber-{version}.zip"
        archive_tree(project, addon_archive)
        # The demo is a complete editable project, not an exported application.
        for source in (root / "demo").iterdir():
            if source.is_file() and (source.suffix in {".gd", ".tscn", ".tres", ".svg", ".uid"} or source.name in {"project.godot", "export_presets.cfg"}):
                shutil.copy2(source, project / source.name)
        settings = (project / "project.godot").read_text(encoding="utf-8")
        settings = settings.replace('enabled=PackedStringArray("res://addons/gut/plugin.cfg")', 'enabled=PackedStringArray()')
        (project / "project.godot").write_text(settings, encoding="utf-8")
        for name in ("README.md", "README_ja.md", "LICENSE", "NOTICE.md", "REUSE.toml", "BINARY_LICENSE.md"):
            shutil.copy2(root / name, project / name)
        shutil.copytree(root / "docs", project / "docs")
        shutil.copytree(root / "LICENSES", project / "LICENSES")
        # Seed extension discovery without shipping machine-specific editor caches.
        (project / ".godot").mkdir()
        (project / ".godot/extension_list.cfg").write_text("res://addons/kyber/kyber.gdextension\n", newline="\n")
        demo_archive = destination / f"godot-kyber-demo-{version}.zip"
        archive_tree(project, demo_archive, f"godot-kyber-demo-{version}/")
    return addon_archive, demo_archive


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--staged", type=pathlib.Path, required=True)
    parser.add_argument("--destination", type=pathlib.Path, default=ROOT / "target/packages/release")
    parser.add_argument("--version")
    args = parser.parse_args()
    for output in package(ROOT, args.staged, args.destination, args.version):
        print(output)
