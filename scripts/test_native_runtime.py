#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

"""Require native smoke-test completion, even if Godot returns zero on load errors."""
import argparse
import pathlib
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--godot', required=True)
args = parser.parse_args()
root = pathlib.Path(__file__).resolve().parents[1]
result = subprocess.run([
    args.godot, '--headless', '--path', str(root / 'demo'),
    '--script', 'tests/process/native_sdk.gd',
], capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=45)
print(result.stdout, end='')
print(result.stderr, end='')
if result.returncode or 'KYBER_NATIVE_RUNTIME_OK' not in result.stdout.splitlines():
    raise SystemExit('Native runtime test did not complete successfully')
