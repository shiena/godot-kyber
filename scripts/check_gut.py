#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

"""Reject failed, skipped, or incomplete GUT reports."""
import pathlib
import re
import sys
import xml.etree.ElementTree as ET

root = pathlib.Path(__file__).resolve().parents[1]
report = ET.parse(sys.argv[1]).getroot()
expected = sum(len(re.findall(r"^func test_", path.read_text(encoding="utf-8"), re.M))
    for folder in ["unit", "integration"] for path in (root / "demo/tests" / folder).glob("test_*.gd"))
actual = len(report.findall(".//testcase"))
if not expected or actual < expected or any(report.findall(".//" + tag) for tag in ["failure", "error", "skipped"]):
    raise RuntimeError(f"Incomplete or unsuccessful GUT report: {actual}/{expected} tests")
print(f"GUT: {actual} tests passed")
