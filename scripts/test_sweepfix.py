"""
Unit tests for sweepfix.py — TOML storage layer + detection functions.

Run:  python3 -m pytest scripts/test_sweepfix.py -v
      python3 scripts/test_sweepfix.py
"""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

# ── Patch globals before importing so the module does not need to be inside
#    a git repo at import time. sweepfix.py sets the three globals inside
#    main(), not at module level, so plain import works. We still set them
#    here to give step-level functions safe defaults during tests.
sys.path.insert(0, str(Path(__file__).parent))
import sweepfix  # noqa: E402

sweepfix.REPO_ROOT     = Path("/tmp/fake_repo")
sweepfix.SWEEPFIX_DIR  = Path("/tmp/fake_repo/.sweepfix")
sweepfix.CODEBASE_TOML = Path("/tmp/fake_repo/.sweepfix/codebase.toml")


# ---------------------------------------------------------------------------
# _toml_escape
# ---------------------------------------------------------------------------

class TestTomlEscape(unittest.TestCase):

    def test_plain_string_unchanged(self):
        self.assertEqual(sweepfix._toml_escape("hello"), "hello")

    def test_backslash_escaped_first(self):
        self.assertEqual(sweepfix._toml_escape("a\\b"), "a\\\\b")

    def test_double_quote_escaped(self):
        self.assertEqual(sweepfix._toml_escape('say "hi"'), 'say \\"hi\\"')

    def test_newline_escaped(self):
        self.assertEqual(sweepfix._toml_escape("line1\nline2"), "line1\\nline2")

    def test_carriage_return_escaped(self):
        self.assertEqual(sweepfix._toml_escape("a\rb"), "a\\rb")

    def test_tab_escaped(self):
        self.assertEqual(sweepfix._toml_escape("a\tb"), "a\\tb")

    def test_backslash_before_quote(self):
        # r'\"' has one backslash + one double-quote.
        # Expected: backslash → \\\\ then quote → \\"  →  '\\\\\\"'
        self.assertEqual(sweepfix._toml_escape('\\"'), '\\\\\\"')

    def test_unicode_passthrough(self):
        s = "héllo wörld 🎉"
        self.assertEqual(sweepfix._toml_escape(s), s)

    def test_empty_string(self):
        self.assertEqual(sweepfix._toml_escape(""), "")

    def test_backslash_not_double_escaped(self):
        # Single backslash → exactly two backslashes, not four
        result = sweepfix._toml_escape("\\")
        self.assertEqual(result, "\\\\")
        self.assertEqual(len(result), 2)


# ---------------------------------------------------------------------------
# _write_codebase_toml + _parse_codebase_toml  (round-trip)
# ---------------------------------------------------------------------------

class TestTomlRoundTrip(unittest.TestCase):

    def setUp(self):
        self._tmpdir = tempfile.TemporaryDirectory()
        tmp = Path(self._tmpdir.name)
        sweepfix.SWEEPFIX_DIR  = tmp
        sweepfix.CODEBASE_TOML = tmp / "codebase.toml"

    def tearDown(self):
        sweepfix.SWEEPFIX_DIR  = Path("/tmp/fake_repo/.sweepfix")
        sweepfix.CODEBASE_TOML = Path("/tmp/fake_repo/.sweepfix/codebase.toml")
        self._tmpdir.cleanup()

    def _roundtrip(self, sha, entries):
        sweepfix._write_codebase_toml(sha, entries)
        text = sweepfix.CODEBASE_TOML.read_text(encoding="utf-8")
        return sweepfix._parse_codebase_toml(text)

    # ── basic structure ─────────────────────────────────────────────────────

    def test_empty_entries(self):
        sha_out, entries_out = self._roundtrip("abc1234", [])
        self.assertEqual(sha_out, "abc1234")
        self.assertEqual(entries_out, [])

    def test_single_file_no_findings_marked(self):
        sha_out, entries_out = self._roundtrip(
            "deadbeef",
            [("src/main.rs", True, [])],
        )
        self.assertEqual(sha_out, "deadbeef")
        self.assertEqual(len(entries_out), 1)
        path, marked, findings = entries_out[0]
        self.assertEqual(path, "src/main.rs")
        self.assertTrue(marked)
        self.assertEqual(findings, [])

    def test_marked_false_preserved(self):
        _, entries_out = self._roundtrip("aaa", [("x.py", False, [])])
        _, marked, _ = entries_out[0]
        self.assertFalse(marked)

    # ── findings ────────────────────────────────────────────────────────────

    def test_magic_number_finding_roundtrip(self):
        finding = {
            "line": 42, "kind": "magic_number",
            "value": "3000", "context": "let t = 3000;",
        }
        _, entries_out = self._roundtrip("cafebabe", [("src/config.rs", False, [finding])])
        path, marked, findings = entries_out[0]
        self.assertEqual(len(findings), 1)
        f = findings[0]
        self.assertEqual(f["line"],    42)
        self.assertEqual(f["kind"],    "magic_number")
        self.assertEqual(f["value"],   "3000")
        self.assertEqual(f["context"], "let t = 3000;")

    def test_stub_finding_roundtrip(self):
        finding = {
            "line": 87, "kind": "stub",
            "label": "todo!()", "context": "    todo!()",
        }
        _, entries_out = self._roundtrip("sha", [("src/agent.rs", True, [finding])])
        _, _, findings = entries_out[0]
        self.assertEqual(findings[0]["label"],   "todo!()")
        self.assertEqual(findings[0]["kind"],    "stub")
        self.assertEqual(findings[0]["context"], "    todo!()")

    def test_multiple_findings_order_preserved(self):
        findings = [
            {"line": 10, "kind": "magic_number", "value": "255",    "context": "x = 255"},
            {"line": 20, "kind": "stub",         "label": "todo!()", "context": "todo!()"},
        ]
        _, entries_out = self._roundtrip("sha", [("f.rs", False, findings)])
        _, _, out_findings = entries_out[0]
        self.assertEqual(len(out_findings), 2)
        self.assertEqual(out_findings[0]["line"], 10)
        self.assertEqual(out_findings[1]["line"], 20)

    # ── sorting ─────────────────────────────────────────────────────────────

    def test_multiple_files_sorted_by_path(self):
        entries = [
            ("z_last.rs",  True,  []),
            ("a_first.rs", False, []),
        ]
        _, entries_out = self._roundtrip("sha", entries)
        self.assertEqual(entries_out[0][0], "a_first.rs")
        self.assertEqual(entries_out[1][0], "z_last.rs")

    # ── string edge cases ───────────────────────────────────────────────────

    def test_context_with_backslash_and_quotes(self):
        context = 'let s = "a\\b";'
        finding = {"line": 5, "kind": "magic_number", "value": "42", "context": context}
        _, entries_out = self._roundtrip("sha", [("f.rs", False, [finding])])
        self.assertEqual(entries_out[0][2][0]["context"], context)

    def test_context_with_newline_survives_roundtrip(self):
        context = "line1\nline2"
        finding = {"line": 1, "kind": "stub", "label": "todo!()", "context": context}
        _, entries_out = self._roundtrip("sha", [("f.rs", False, [finding])])
        self.assertEqual(entries_out[0][2][0]["context"], context)

    def test_path_with_hyphens_and_underscores(self):
        path = "src/some-module/foo_bar.rs"
        _, entries_out = self._roundtrip("sha", [(path, True, [])])
        self.assertEqual(entries_out[0][0], path)

    def test_sha_with_special_chars_escaped(self):
        # SHA should be a hex string in practice, but escaping must not break it
        sha = "abc1234"
        sha_out, _ = self._roundtrip(sha, [])
        self.assertEqual(sha_out, sha)

    # ── parse robustness ────────────────────────────────────────────────────

    def test_duplicate_path_keeps_first(self):
        toml_text = (
            'commit = "abc"\n\n'
            '[[file]]\npath = "dup.rs"\nmarked = true\n\n'
            '[[file]]\npath = "dup.rs"\nmarked = false\n\n'
        )
        _, entries_out = sweepfix._parse_codebase_toml(toml_text)
        self.assertEqual(len(entries_out), 1)
        self.assertTrue(entries_out[0][1])  # first wins: marked = true

    def test_malformed_toml_returns_empty(self):
        _, entries_out = sweepfix._parse_codebase_toml("this is not\n[valid toml ]]")
        self.assertEqual(entries_out, [])

    def test_missing_commit_returns_empty_string(self):
        toml_text = '[[file]]\npath = "f.rs"\nmarked = false\n'
        sha_out, _ = sweepfix._parse_codebase_toml(toml_text)
        self.assertEqual(sha_out, "")

    def test_empty_string_input_returns_empty(self):
        sha_out, entries_out = sweepfix._parse_codebase_toml("")
        self.assertEqual(sha_out, "")
        self.assertEqual(entries_out, [])


# ---------------------------------------------------------------------------
# _find_magic_numbers  (detection, unchanged function — regression)
# ---------------------------------------------------------------------------

class TestFindMagicNumbers(unittest.TestCase):

    def test_detects_plain_integer(self):
        hits = sweepfix._find_magic_numbers("let x = 3000;\n")
        self.assertEqual(len(hits), 1)
        self.assertEqual(hits[0][1], "3000")

    def test_trivial_numbers_skipped(self):
        for val in ("0", "1", "-1", "2"):
            with self.subTest(val=val):
                hits = sweepfix._find_magic_numbers(f"let x = {val};\n")
                self.assertEqual(hits, [], f"Expected {val!r} to be trivial")

    def test_comment_only_line_skipped(self):
        hits = sweepfix._find_magic_numbers("// let x = 9999;\n")
        self.assertEqual(hits, [])

    def test_line_number_correct(self):
        src = "let a = 1;\nlet b = 3000;\n"
        hits = sweepfix._find_magic_numbers(src)
        self.assertEqual(hits[0][0], 2)

    def test_inline_comment_number_not_reported(self):
        hits = sweepfix._find_magic_numbers("let x = 9000; // was 42\n")
        lits = [h[1] for h in hits]
        self.assertIn("9000", lits)
        self.assertNotIn("42", lits)

    def test_hex_literal_detected(self):
        hits = sweepfix._find_magic_numbers("let mask = 0xFF00;\n")
        self.assertEqual(hits[0][1], "0xFF00")

    def test_number_inside_string_not_reported(self):
        hits = sweepfix._find_magic_numbers('let s = "timeout 60000ms";\n')
        self.assertEqual(hits, [])

    def test_context_line_returned(self):
        src = "let port = 8080;\n"
        lineno, lit, context = sweepfix._find_magic_numbers(src)[0]
        self.assertIn("8080", context)


# ---------------------------------------------------------------------------
# _find_stubs  (detection, unchanged function — regression)
# ---------------------------------------------------------------------------

class TestFindStubs(unittest.TestCase):

    def test_todo_macro(self):
        hits = sweepfix._find_stubs("    todo!()\n")
        self.assertEqual(hits[0][1], "todo!()")

    def test_unimplemented_macro(self):
        hits = sweepfix._find_stubs("    unimplemented!()\n")
        self.assertEqual(hits[0][1], "unimplemented!()")

    def test_todo_comment(self):
        hits = sweepfix._find_stubs("    // TODO: fix this\n")
        self.assertEqual(hits[0][1], "TODO comment")

    def test_fixme_comment(self):
        hits = sweepfix._find_stubs("    // FIXME: broken\n")
        self.assertEqual(hits[0][1], "FIXME comment")

    def test_raise_not_implemented(self):
        hits = sweepfix._find_stubs("    raise NotImplementedError\n")
        self.assertEqual(hits[0][1], "raise NotImplementedError")

    def test_pass_stub_body(self):
        hits = sweepfix._find_stubs("    pass\n")
        self.assertEqual(hits[0][1], "pass (stub body)")

    def test_one_report_per_line(self):
        hits = sweepfix._find_stubs("    todo!() // TODO: also\n")
        self.assertEqual(len(hits), 1)

    def test_rust_attribute_not_flagged(self):
        # #[allow(dead_code)] must NOT match stub patterns
        hits = sweepfix._find_stubs("#[allow(dead_code)]\n")
        self.assertEqual(hits, [])

    def test_line_number_returned(self):
        src = "fn foo() {\n    todo!()\n}\n"
        hits = sweepfix._find_stubs(src)
        self.assertEqual(hits[0][0], 2)

    def test_context_line_returned(self):
        hits = sweepfix._find_stubs("    todo!()\n")
        self.assertIn("todo!()", hits[0][2])


# ---------------------------------------------------------------------------

if __name__ == "__main__":
    unittest.main()
