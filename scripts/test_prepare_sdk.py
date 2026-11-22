# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

"""Verify player generation against the prepared, pinned SDK."""
import unittest
from unittest.mock import patch

import prepare_sdk


class PlayerGenerationTests(unittest.TestCase):
    def setUp(self):
        self.ctl = prepare_sdk.ROOT / ".deps/kysdk/kyctl"

    def test_preserves_upstream_notice_and_appends_original_adapter(self):
        generated = prepare_sdk.generate_player(self.ctl)
        self.assertIn("Copyright © 2022-2026 Kyber SAS", generated)
        self.assertIn("LicenseRef-Kyber-Commercial OR AGPL-3.0-or-later", generated)
        self.assertIn(prepare_sdk.CTL_REV, generated)
        adapter = (prepare_sdk.ROOT / "sdk/native_player.rs").read_text(encoding="utf-8")
        self.assertTrue(generated.endswith(adapter))
        for name in ("PlayerMetric", "PlayerEvent", "PlayerListener", "WindowHandle"):
            self.assertEqual(generated.count(f"pub enum {name} {{") +
                             generated.count(f"pub trait {name}:"), 1)
        self.assertEqual(generated.count("Cpu(CpuOutput),"), 1)
        self.assertNotIn("vlc::", generated)
        self.assertNotIn("RawWindowHandle", generated)

    def test_matches_materialized_output_and_is_repeatable(self):
        output = self.ctl / "kyclient/src/platform/desktop/player.rs"
        self.assertEqual(output.read_text(encoding="utf-8"), prepare_sdk.generate_player(self.ctl))
        self.assertEqual(prepare_sdk.generate_player(self.ctl), prepare_sdk.generate_player(self.ctl))

    def test_rejects_unreviewed_upstream_source(self):
        with patch.object(prepare_sdk, "git", return_value="unexpected-blob"):
            with self.assertRaisesRegex(RuntimeError, "Review SDK player extraction"):
                prepare_sdk.generate_player(self.ctl)


if __name__ == "__main__":
    unittest.main()
