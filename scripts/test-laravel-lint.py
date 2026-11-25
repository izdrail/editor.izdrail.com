#!/usr/bin/env python3
"""Regression tests for the syntax-gate harness, not PHP/Laravel acceptance."""
import contextlib
import importlib.util
import io
import json
import pathlib
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("lint_gate", pathlib.Path(__file__).with_name("lint-laravel-expansions.py"))
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


class GateTests(unittest.TestCase):
    def invoke(self, entries, runner):
        with tempfile.TemporaryDirectory() as directory:
            source = pathlib.Path(directory) / "expansions.json"
            source.write_text(json.dumps(entries), encoding="utf-8")
            with patch.object(gate.sys, "argv", ["lint", str(source)]), patch.object(gate.subprocess, "run", side_effect=runner), contextlib.redirect_stdout(io.StringIO()):
                return gate.main()

    def test_full_files_are_not_rescued_by_wrappers(self):
        self.assertEqual(list(gate.contexts("<?php broken")), ["file"])

    def test_member_context_supplies_parent_without_modifying_body(self):
        body = "protected static function boot(): void { parent::boot(); }"
        wrapped = gate.contexts(body)["class-members"]
        self.assertIn("extends SnippetParent", wrapped)
        self.assertIn(body, wrapped)

    def test_empty_export_is_rejected(self):
        with self.assertRaises(ValueError):
            self.invoke([], lambda *a, **k: self.fail("PHP must not run"))

    def test_invalid_expansion_fails_every_context(self):
        def reject(*args, **kwargs):
            self.assertEqual(args[0], ["php", "-l"])
            self.assertNotIn("shell", kwargs)
            return subprocess.CompletedProcess(args[0], 1, "invalid")
        self.assertTrue(self.invoke([{"prefix": ["lv-invalid"], "text": "broken"}], reject))

    def test_success_does_not_execute_code(self):
        def accept(*args, **kwargs):
            self.assertEqual(args[0], ["php", "-l"])
            self.assertEqual(kwargs["input"], "<?php\nreturn 1;")
            return subprocess.CompletedProcess(args[0], 0, "valid")
        self.assertFalse(self.invoke([{"prefix": ["lv-ok"], "text": "return 1;"}], accept))


if __name__ == "__main__":
    unittest.main()
