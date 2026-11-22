# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

"""Check distribution layout and reject incomplete or mismatched build artifacts."""
import hashlib
import json
import pathlib
import tempfile
import unittest
import zipfile

from package_release import BINARIES, ROOT, package


class ReleasePackages(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.work = pathlib.Path(self.temporary.name)
        self.staged = self.work / "staged"
        for platform, (relative, architecture) in BINARIES.items():
            addon = self.staged / ("addon-" + platform)
            binary = addon / "bin" / relative
            binary.parent.mkdir(parents=True)
            binary.write_bytes(("fixture-" + platform).encode())
            if platform == "ios":
                (binary.parent / "Info.plist").write_text("fixture")
            (addon / "bin/build-info.json").write_text(json.dumps({
                "backend": "native", "platform": platform, "architecture": architecture,
                "sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
            }))
            (addon / "runtime-manifest.json").write_text('{"files":{}}')
            (addon / "licenses").mkdir()
            (addon / "licenses/NOTICE").write_text(platform)
            (addon / "licenses/kyber-sdk").mkdir()
            for name in ("COPYING.md", "COPYING.AGPLv3"):
                (addon / "licenses/kyber-sdk" / name).write_text("upstream-fixture-" + platform + name)

    def test_archives_contain_all_platforms_and_share_the_same_addon(self):
        addon_path, demo_path = package(ROOT, self.staged, self.work / "packages")
        with zipfile.ZipFile(addon_path) as addon, zipfile.ZipFile(demo_path) as demo:
            self.assertTrue(all(name.startswith("addons/kyber/") for name in addon.namelist()))
            prefix = demo_path.stem + "/"
            for name in addon.namelist():
                self.assertEqual(addon.read(name), demo.read(prefix + name))
            for relative, _ in BINARIES.values():
                self.assertIn("addons/kyber/bin/" + relative, addon.namelist())
            self.assertIn("addons/kyber/LICENSE", addon.namelist())
            self.assertIn(b"MIT License", addon.read("addons/kyber/LICENSE"))
            self.assertIn(b"GNU AFFERO GENERAL PUBLIC LICENSE", addon.read("addons/kyber/licenses/godot-kyber/LICENSES/AGPL-3.0-or-later.txt"))
            self.assertIn(b"does not grant or sublicense Kyber commercial rights", addon.read("addons/kyber/NOTICE.md"))
            self.assertIn(b"a valid commercial license agreement with Kyber SAS", addon.read("addons/kyber/BINARY_LICENSE.md"))
            for platform in BINARIES:
                for name in ("COPYING.md", "COPYING.AGPLv3"):
                    self.assertEqual(addon.read(f"addons/kyber/licenses/{platform}/kyber-sdk/{name}"),
                                     ("upstream-fixture-" + platform + name).encode())
            for name in ("BINARY_LICENSE.md", "NOTICE.md", "REUSE.toml", "LICENSES/MIT.txt", "LICENSES/AGPL-3.0-or-later.txt"):
                self.assertIn(prefix + name, demo.namelist())
            self.assertIn("addons/kyber/input_relay.gd", addon.namelist())
            self.assertIn("addons/kyber/input_relay.gd.uid", addon.namelist())
            settings = demo.read(prefix + "project.godot").decode()
            self.assertNotIn("addons/gut/plugin.cfg", settings)
            self.assertIn(prefix + "demo.tscn", demo.namelist())
            self.assertFalse(any(".godothub" in name or "/tests/" in name for name in demo.namelist()))
            self.assertEqual(demo.read(prefix + ".godot/extension_list.cfg"), b"res://addons/kyber/kyber.gdextension\n")

    def test_missing_platform_is_rejected(self):
        (self.staged / "addon-android/bin/android/libgodot_kyber.so").unlink()
        with self.assertRaises(FileNotFoundError):
            package(ROOT, self.staged, self.work / "packages")

    def test_missing_sdk_notices_are_rejected(self):
        for name in ("COPYING.md", "COPYING.AGPLv3"):
            path = self.staged / "addon-windows/licenses/kyber-sdk" / name
            content = path.read_bytes()
            path.unlink()
            with self.assertRaisesRegex(ValueError, "Missing Kyber SDK notice"):
                package(ROOT, self.staged, self.work / "packages")
            path.write_bytes(content)

    def test_changed_binary_is_rejected(self):
        (self.staged / "addon-windows/bin/godot_kyber.dll").write_bytes(b"changed")
        with self.assertRaises(ValueError):
            package(ROOT, self.staged, self.work / "packages")

    def test_mock_binary_is_rejected(self):
        path = self.staged / "addon-linux/bin/build-info.json"
        info = json.loads(path.read_text())
        info["backend"] = "mock"
        path.write_text(json.dumps(info))
        with self.assertRaises(ValueError):
            package(ROOT, self.staged, self.work / "packages")

    def test_unmatched_release_version_is_rejected(self):
        with self.assertRaises(ValueError):
            package(ROOT, self.staged, self.work / "packages", "v9999.0.0")


if __name__ == "__main__":
    unittest.main()
