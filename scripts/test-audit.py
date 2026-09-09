#!/usr/bin/env python3
"""Bounded audit runner contracts; no network, Cargo builds, or real providers."""
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
import audit
import audit_replay


class RunnerTests(unittest.TestCase):
    def test_smoke_without_bootstrap_stops_before_any_build(self):
        audit.BASE.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=audit.BASE) as directory:
            with patch.object(audit, "BASE", Path(directory)), patch.object(audit, "run") as child:
                with self.assertRaisesRegex(audit.AuditError, "dependencies are absent.*bootstrap"):
                    audit.smoke(None)
                child.assert_not_called()

    def test_cargo_json_discovers_windows_executable_without_target_path_guess(self):
        record = {"reason": "compiler-artifact", "target": {"name": "ui_inventory", "kind": ["example"]},
                  "executable": "custom target/debug/examples/ui_inventory.exe"}
        self.assertEqual(audit.compiler_executable("diagnostic\n" + json.dumps(record)), Path(record["executable"]))
        with self.assertRaisesRegex(audit.AuditError, "found 0"):
            audit.compiler_executable('{"reason":"build-finished","success":true}')

    def test_compiler_ambiguity_is_rejected(self):
        records = [{"reason": "compiler-artifact", "target": {"name": "ui_inventory", "kind": ["example"]},
                    "executable": name} for name in ["one", "two"]]
        with self.assertRaisesRegex(audit.AuditError, "found 2"):
            audit.compiler_executable("\n".join(map(json.dumps, records)))

    def test_zero_tests_or_other_fixture_never_counts_as_success(self):
        correct = f"running 1 test\ntest {audit.FIXTURE} ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 8 filtered out; finished in 0.00s\n"
        audit.validate_fixture(correct)
        for output in ["running 0 tests\ntest result: ok. 0 passed; 0 failed;", correct.replace(audit.FIXTURE, "another_test"), correct.replace("running 1 test", "running 2 tests"), correct.replace("... ok", "... ignored")]:
            with self.subTest(output=output), self.assertRaises(audit.AuditError):
                audit.validate_fixture(output)

    def test_missing_python_and_wrong_version_have_bootstrap_instructions(self):
        with patch.object(audit, "run", return_value='{"version":[3,14,0],"executable":"python3"}'):
            with self.assertRaisesRegex(audit.AuditError, "bootstrap --python"):
                audit.python_info("python3")
        with patch.object(audit, "find_python", wraps=audit.find_python), patch.object(audit, "python_info", side_effect=audit.AuditError("missing")), patch.object(audit.shutil, "which", return_value=None):
            with self.assertRaisesRegex(audit.AuditError, "Python 3.12 was not found"):
                audit.find_python()

    def test_missing_cargo_has_toolchain_instruction(self):
        with patch.object(audit.shutil, "which", return_value=None):
            with self.assertRaisesRegex(audit.AuditError, "rustup toolchain install"):
                audit.cargo_info()

    def test_outputs_cannot_overwrite_history_or_previous_run(self):
        with self.assertRaisesRegex(audit.AuditError, "historical"):
            audit.output_directory(str(audit.ROOT / "docs/design-audit/iteration-7"))
        audit.BASE.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=audit.BASE) as directory:
            with self.assertRaisesRegex(audit.AuditError, "already exists"):
                audit.output_directory(directory)
            self.assertTrue(audit.output_directory(str(Path(directory) / "new")).is_relative_to(audit.BASE))

    def test_capture_environment_removes_render_and_python_overrides(self):
        with patch.dict(os.environ, {"THEYWORK_ENCODING": "none", "NO_COLOR": "1", "PYTHONPATH": "/unexpected", "CARGO_TARGET_DIR": "/custom-target"}):
            env = audit.environment()
        self.assertEqual(env["THEYWORK_ENCODING"], "quadrants")
        self.assertNotIn("NO_COLOR", env)
        self.assertNotIn("PYTHONPATH", env)
        self.assertEqual(env["CARGO_NET_OFFLINE"], "true")
        self.assertEqual(env["RUSTUP_AUTO_INSTALL"], "0")
        self.assertEqual(env["CARGO_TARGET_DIR"], "/custom-target")

    def test_single_screen_checks_route_image_and_actual_hits(self):
        audit.BASE.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=audit.BASE) as directory:
            raw = Path(directory)
            case = {"id": audit.CASE, "mode": "image", "automatic": {"route": True, "hit_bounds": True},
                    "configuration": {"cell_pixels": [8, 16], "effective_encoding": "Image"}}
            hits = [{"x": 0, "y": 0, "width": 80, "height": 24, "action": "Tower"}]
            audit.write_json(raw / "inventory.json", {"cases": [case]})
            audit.write_json(raw / f"{audit.CASE}.hits.json", hits)
            self.assertEqual(audit.validate_screen(raw)[1], 1)
            hits[0]["width"] = 81
            audit.write_json(raw / f"{audit.CASE}.hits.json", hits)
            with self.assertRaisesRegex(audit.AuditError, "hit regions"):
                audit.validate_screen(raw)
            audit.write_json(raw / "inventory.json", {"cases": []})
            with self.assertRaisesRegex(audit.AuditError, "exactly one"):
                audit.validate_screen(raw)


class ReplayTests(unittest.TestCase):
    def setUp(self):
        from PIL import Image
        self.Image = Image
        audit.BASE.mkdir(parents=True, exist_ok=True)
        self.tmp = tempfile.TemporaryDirectory(dir=audit.BASE)
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.source = self.base / f"{audit.CASE}.cells.json"
        self.data = {"columns": 80, "rows": 24, "cell_width": 8, "cell_height": 16,
                     "image": {"x": 0, "y": 0, "pixel_width": 640, "pixel_height": 384},
                     "cells": [[{"text": " ", "fg": "#ffffff", "bg": "#123456", "native": False, "bold": False} for _ in range(80)] for _ in range(24)]}
        (self.base / f"{audit.CASE}.rgba").write_bytes(self.Image.new("RGBA", (640, 384), "#ff0000").tobytes())

    def export(self):
        audit.write_json(self.source, self.data)
        return audit_replay.replay(self.source, self.base / "screen.png")

    def test_full_viewport_rgba_and_opaque_native_backgrounds(self):
        self.data["cells"][0][0]["native"] = True
        result = self.export()
        image = self.Image.open(self.base / "screen.png")
        self.assertEqual(image.size, (640, 384))
        self.assertEqual(image.getpixel((0, 0)), (18, 52, 86))
        self.assertEqual(image.getpixel((8, 0)), (255, 0, 0))
        self.assertEqual(image.getpixel((639, 383)), (255, 0, 0))
        self.assertEqual(result["visual_review"], "not-tested")
        self.assertEqual(result["sha256"], self.export()["sha256"])

    def test_unknown_native_glyph_is_an_error_not_silent_font_fallback(self):
        self.data["cells"][0][0].update(native=True, text="\U0010ffff")
        with self.assertRaisesRegex(ValueError, "U\\+10FFFF"):
            self.export()

    def test_font_integrity_and_required_glyphs(self):
        audit_replay.verify_fonts()
        for font in ["DejaVuSansMono.ttf", "DejaVuSansMono-Bold.ttf"]:
            self.assertTrue({ord(c) for c in "ABC abc 012 │…→"}.issubset(audit_replay.glyphs(audit_replay.FONT_DIR / font)))
        font_dir = self.base / "fonts"
        font_dir.mkdir()
        audit.write_json(font_dir / "manifest.json", {"files": {"font.ttf": "invalid"}})
        (font_dir / "font.ttf").write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "checksum mismatch"):
            audit_replay.verify_fonts(font_dir)

    def test_bad_pixel_length_or_geometry_is_rejected(self):
        (self.base / f"{audit.CASE}.rgba").write_bytes(b"bad")
        with self.assertRaisesRegex(ValueError, "RGBA byte length"):
            self.export()
        self.data["cell_height"] = 20
        with self.assertRaisesRegex(ValueError, "exactly 80x24"):
            self.export()


if __name__ == "__main__":
    unittest.main()
