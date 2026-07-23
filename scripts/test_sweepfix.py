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

    def test_nul_byte_escaped(self):
        # NUL is forbidden in TOML basic strings; must be \uXXXX
        result = sweepfix._toml_escape("a\x00b")
        self.assertNotIn("\x00", result)
        # _toml_escape emits \uXXXX with uppercase hex digits — compare case-insensitively
        self.assertIn("\\u0000", result.lower())

    def test_control_chars_escaped_and_valid_toml(self):
        # BS, VT, FF, ESC are all forbidden unescaped in TOML.
        # After escaping, the result must parse as valid TOML.
        import tomllib
        for ch, name in [("\x08", "BS"), ("\x0b", "VT"), ("\x0c", "FF"), ("\x1b", "ESC")]:
            with self.subTest(char=name):
                result = sweepfix._toml_escape(ch)
                self.assertNotIn(ch, result)
                toml = f'v = "{result}"\n'
                parsed = tomllib.loads(toml)
                self.assertEqual(parsed["v"], ch)

    def test_control_chars_survive_toml_roundtrip(self):
        # A context string containing an ESC char must survive write→parse roundtrip.
        tmp = Path(tempfile.mkdtemp())
        orig_toml = sweepfix.CODEBASE_TOML
        sweepfix.CODEBASE_TOML = tmp / "codebase.toml"
        try:
            context = "let x = \x1b[0m3000;"  # ANSI escape in source
            finding = {"line": 1, "kind": "magic_number", "value": "3000", "context": context}
            sweepfix._write_codebase_toml("abc", [("f.rs", False, [finding])])
            text = sweepfix.CODEBASE_TOML.read_text(encoding="utf-8")
            _, entries = sweepfix._parse_codebase_toml(text)
            self.assertEqual(entries[0][2][0]["context"], context)
        finally:
            sweepfix.CODEBASE_TOML = orig_toml


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
# _strip_inline_comment  (handles both ' and " strings)
# ---------------------------------------------------------------------------

class TestStripInlineComment(unittest.TestCase):

    def test_no_comment(self):
        self.assertEqual(sweepfix._strip_inline_comment("let x = 9000;"), "let x = 9000;")

    def test_double_slash_stripped(self):
        self.assertEqual(sweepfix._strip_inline_comment("let x = 1; // was 42"), "let x = 1; ")

    def test_hash_stripped(self):
        self.assertEqual(sweepfix._strip_inline_comment("x = 1  # magic"), "x = 1  ")

    def test_double_quoted_protects_slash(self):
        self.assertEqual(sweepfix._strip_inline_comment('let s = "a // b";'), 'let s = "a // b";')

    def test_escaped_quote_then_comment_stripped(self):
        self.assertEqual(sweepfix._strip_inline_comment(r'let s = "a\""; // c'), r'let s = "a\""; ')

    def test_double_backslash_closes_string(self):
        self.assertEqual(sweepfix._strip_inline_comment(r'let s = "a\\"; // c'), r'let s = "a\\"; ')

    def test_triple_backslash_keeps_string_open(self):
        # 3 backslashes before " → odd → quote is escaped → string stays open → // NOT stripped
        self.assertEqual(sweepfix._strip_inline_comment(r'let s = "a\\\"; // c'), r'let s = "a\\\"; // c')

    def test_single_quoted_simple_then_comment(self):
        self.assertEqual(sweepfix._strip_inline_comment("const s = 'hello'; // c"), "const s = 'hello'; ")

    def test_single_quoted_slash_inside_protected(self):
        self.assertEqual(
            sweepfix._strip_inline_comment("const s = 'it // works'; let x = 3000; // end"),
            "const s = 'it // works'; let x = 3000; "
        )

    def test_python_hash_after_single_quote(self):
        self.assertEqual(sweepfix._strip_inline_comment("x = 'hello' # py"), "x = 'hello' ")

    def test_python_hash_inside_single_quoted(self):
        self.assertEqual(sweepfix._strip_inline_comment("x = 'it # works' # real"), "x = 'it # works' ")

    def test_rust_lifetime_not_string(self):
        self.assertEqual(
            sweepfix._strip_inline_comment("fn f<'a>(x: &'a str) { let n = 3000; // c"),
            "fn f<'a>(x: &'a str) { let n = 3000; "
        )

    def test_rust_char_literal(self):
        self.assertEqual(
            sweepfix._strip_inline_comment("let c = 'x'; let n = 3000; // c"),
            "let c = 'x'; let n = 3000; "
        )

    def test_rust_static_lifetime(self):
        self.assertEqual(
            sweepfix._strip_inline_comment("fn f(s: &'static str) { let n = 3000; // c"),
            "fn f(s: &'static str) { let n = 3000; "
        )

    def test_trait_bound_lifetime_with_space(self):
        self.assertEqual(
            sweepfix._strip_inline_comment("fn f<T: Trait + 'a>() { let n = 3000; // c"),
            "fn f<T: Trait + 'a>() { let n = 3000; "
        )


# ---------------------------------------------------------------------------
# _is_in_string
# ---------------------------------------------------------------------------

class TestIsInString(unittest.TestCase):

    def test_outside_string(self):
        self.assertFalse(sweepfix._is_in_string("let x = 3000;", 8))

    def test_inside_double_quoted(self):
        self.assertTrue(sweepfix._is_in_string('let x = "3000";', 9))

    def test_after_double_quoted(self):
        self.assertFalse(sweepfix._is_in_string('let s = "hi"; let x = 3000;', 22))

    def test_inside_single_quoted(self):
        self.assertTrue(sweepfix._is_in_string("let x = '3000';", 9))

    def test_after_single_quoted(self):
        self.assertFalse(sweepfix._is_in_string("let s = 'hi'; let x = 3000;", 22))

    def test_escaped_quote_false_negative_fixed(self):
        # "a\"b" — the \" is escaped; string closes at the unescaped ".
        # Position of '3' in '3000' must be OUTSIDE the string.
        line = r'let s = "a\"b"; let x = 3000;'
        pos = line.index("3000")
        self.assertFalse(sweepfix._is_in_string(line, pos))


# ---------------------------------------------------------------------------
# _find_magic_numbers  (detection)
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

    def test_number_inside_double_string_not_reported(self):
        hits = sweepfix._find_magic_numbers('let s = "timeout 60000ms";\n')
        self.assertEqual(hits, [])

    def test_number_inside_single_string_not_reported(self):
        hits = sweepfix._find_magic_numbers("let s = 'timeout 60000ms';\n")
        self.assertEqual(hits, [])

    def test_escaped_quote_false_negative_fixed(self):
        # Before fix: 'let s = "a\"b"; let x = 3000;' had 3 '"' chars before
        # 3000 → odd count → falsely suppressed. After fix: correctly found.
        hits = sweepfix._find_magic_numbers(r'let s = "a\"b"; let x = 3000;' + "\n")
        lits = [h[1] for h in hits]
        self.assertIn("3000", lits)

    def test_number_after_single_quoted_string_found(self):
        hits = sweepfix._find_magic_numbers("const s = 'hello'; let x = 3000;\n")
        lits = [h[1] for h in hits]
        self.assertIn("3000", lits)

    def test_context_line_returned(self):
        src = "let port = 8080;\n"
        lineno, lit, context = sweepfix._find_magic_numbers(src)[0]
        self.assertIn("8080", context)


# ---------------------------------------------------------------------------
# _find_stubs  (detection)
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
# _write_codebase_toml — robustness against missing finding keys
# ---------------------------------------------------------------------------

class TestWriteTomlMissingFindingKeys(unittest.TestCase):

    def setUp(self):
        self._tmpdir = tempfile.TemporaryDirectory()
        tmp = Path(self._tmpdir.name)
        self._orig_toml = sweepfix.CODEBASE_TOML
        sweepfix.SWEEPFIX_DIR  = tmp
        sweepfix.CODEBASE_TOML = tmp / "codebase.toml"

    def tearDown(self):
        sweepfix.CODEBASE_TOML = self._orig_toml
        self._tmpdir.cleanup()

    def test_missing_context_does_not_crash(self):
        # A finding without 'context' must not raise KeyError
        finding = {"line": 42, "kind": "magic_number", "value": "3000"}
        sweepfix._write_codebase_toml("abc", [("f.rs", False, [finding])])
        text = sweepfix.CODEBASE_TOML.read_text(encoding="utf-8")
        _, entries = sweepfix._parse_codebase_toml(text)
        # context falls back to ""
        self.assertEqual(entries[0][2][0]["context"], "")

    def test_missing_line_does_not_crash(self):
        # A finding without 'line' must not raise KeyError; falls back to 0
        finding = {"kind": "stub", "label": "todo!()", "context": "todo!()"}
        sweepfix._write_codebase_toml("abc", [("f.rs", False, [finding])])
        text = sweepfix.CODEBASE_TOML.read_text(encoding="utf-8")
        _, entries = sweepfix._parse_codebase_toml(text)
        self.assertEqual(entries[0][2][0]["line"], 0)

    def test_missing_kind_does_not_crash(self):
        # A finding without 'kind' must not raise KeyError; falls back to "unknown"
        finding = {"line": 5, "context": "x = 3000"}
        sweepfix._write_codebase_toml("abc", [("f.rs", False, [finding])])
        text = sweepfix.CODEBASE_TOML.read_text(encoding="utf-8")
        _, entries = sweepfix._parse_codebase_toml(text)
        self.assertEqual(entries[0][2][0]["kind"], "unknown")


# ---------------------------------------------------------------------------
# _scan_string_state — the shared state-machine helper
# ---------------------------------------------------------------------------

class TestScanStringState(unittest.TestCase):

    def test_empty_line(self):
        self.assertEqual(sweepfix._scan_string_state("", 0), (False, False))

    def test_before_any_quote(self):
        self.assertEqual(sweepfix._scan_string_state('let x = "hello"', 0), (False, False))

    def test_inside_double_quoted(self):
        in_d, in_s = sweepfix._scan_string_state('let x = "3000"', 9)
        self.assertTrue(in_d)
        self.assertFalse(in_s)

    def test_after_double_quoted(self):
        in_d, in_s = sweepfix._scan_string_state('let x = "hi"; let y = 0', 14)
        self.assertFalse(in_d)
        self.assertFalse(in_s)

    def test_inside_single_quoted(self):
        in_d, in_s = sweepfix._scan_string_state("let x = '3000'", 9)
        self.assertFalse(in_d)
        self.assertTrue(in_s)

    def test_rust_lifetime_does_not_open_string(self):
        # &'a str — the ' after & must not open in_single
        line = "&'a str"
        in_d, in_s = sweepfix._scan_string_state(line, len(line))
        self.assertFalse(in_d)
        self.assertFalse(in_s)

    def test_consistency_with_is_in_string(self):
        # _is_in_string must always agree with _scan_string_state
        line = r'let s = "a\"b"; let x = 3000;'
        pos = line.index("3000")
        in_d, in_s = sweepfix._scan_string_state(line, pos)
        self.assertEqual(sweepfix._is_in_string(line, pos), in_d or in_s)


# ---------------------------------------------------------------------------

if __name__ == "__main__":
    unittest.main()
