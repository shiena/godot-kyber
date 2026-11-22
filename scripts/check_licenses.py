# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

"""Check license coverage for project-owned files without relabeling vendors."""
import pathlib
import re
import subprocess
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[1]


def main():
    metadata = tomllib.loads((ROOT / "REUSE.toml").read_text(encoding="utf-8"))
    annotations = {}
    for entry in metadata["annotations"]:
        for name in entry["path"]:
            annotations[name] = entry["SPDX-License-Identifier"]
    names = subprocess.check_output([
        "git", "ls-files", "--cached", "--others", "--exclude-standard", "-z",
    ], cwd=ROOT).decode("utf-8").split("\0")
    checked = 0
    for name in sorted(set(names)):
        if not name or name == "LICENSE" or name.startswith(("LICENSES/", "demo/addons/gut/", "demo/.godothub/")):
            continue
        path = ROOT / name
        if not path.is_file():
            continue
        expected = "MIT"
        if name in annotations:
            actual = annotations[name]
        else:
            header = "\n".join(path.read_text(encoding="utf-8-sig").splitlines()[:12])
            match = re.search(r"SPDX-License-Identifier: ([A-Za-z0-9.-]+)", header)
            if not match or "SPDX-FileCopyrightText:" not in header:
                raise RuntimeError(f"Missing license header or REUSE annotation: {name}")
            actual = match[1]
        if actual != expected:
            raise RuntimeError(f"Unexpected license for {name}: {actual}")
        checked += 1
    for name, expected in (("Cargo.toml", "MIT"), ("media/Cargo.toml", "MIT")):
        package = tomllib.loads((ROOT / name).read_text(encoding="utf-8"))["package"]
        if package["license"] != expected:
            raise RuntimeError(f"Incorrect package license: {name}")
    if (ROOT / "LICENSE").read_bytes() != (ROOT / "LICENSES/MIT.txt").read_bytes():
        raise RuntimeError("The root MIT license and LICENSES/MIT.txt differ")
    print(f"License coverage verified for {checked} project files; vendor notices unchanged.")


if __name__ == "__main__":
    main()
