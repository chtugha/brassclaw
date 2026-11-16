#!/usr/bin/env python3
# Requires Python 3.10+ for PEP 604 union syntax such as `int | None`.

import argparse
import pathlib
import re
import subprocess
import sys
import unittest
from dataclasses import dataclass


PANIC_PATTERN = re.compile(r"\.(?:unwrap|expect)\(|(?<!_)assert(?:_eq|_ne)?!")
TEST_ATTR_PATTERN = re.compile(
    r"^\s*#\s*\[\s*(?:"
    r"test"
    r"|tokio::test(?:\s*\([^]]*\))?"
    r"|rstest(?:\s*\([^]]*\))?"
    r"|test_case(?:\s*\([^]]*\))?"
    r")\s*\]"
)
CFG_ATTR_PATTERN = re.compile(
    r"^\s*#\s*(!?)\s*\[\s*cfg\s*\((.*)\)\s*\]\s*(?://.*)?$"
)
CFG_TOKEN_PATTERN = re.compile(r'[A-Za-z_][A-Za-z0-9_]*|"(?:[^"\\]|\\.)*"|[(),=]')
ITEM_PATTERN = re.compile(
    r"^\s*"
    r"(?:(?:pub(?:\([^)]*\))?|crate)\s+)?"
    r"(?:(?:async|unsafe|const)\s+)*"
    r"(fn|mod|struct|enum|trait|union|impl)\b"
    r"(?:\s+([A-Za-z_][A-Za-z0-9_]*))?"
)


@dataclass
class LexerState:
    block_comment_depth: int = 0
    in_string: bool = False
    string_escape: bool = False
    in_char: bool = False
    char_escape: bool = False
    raw_string_hashes: int | None = None


def run_git(*args: str) -> str:
    result = subprocess.run(
        ["git", *args],
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout


def sanitize_line(line: str, state: LexerState) -> str:
    chars = list(line)
    out = [" "] * len(chars)
    i = 0

    while i < len(chars):
        ch = chars[i]
        nxt = chars[i + 1] if i + 1 < len(chars) else ""

        if state.block_comment_depth:
            if ch == "/" and nxt == "*":
                state.block_comment_depth += 1
                i += 2
                continue
            if ch == "*" and nxt == "/":
                state.block_comment_depth -= 1
                i += 2
                continue
            i += 1
            continue

        if state.raw_string_hashes is not None:
            if ch == '"':
                hashes = 0
                j = i + 1
                while j < len(chars) and chars[j] == "#":
                    hashes += 1
                    j += 1
                if hashes == state.raw_string_hashes:
                    state.raw_string_hashes = None
                    i = j
                    continue
            i += 1
            continue

        if state.in_string:
            if state.string_escape:
                state.string_escape = False
            elif ch == "\\":
                state.string_escape = True
            elif ch == '"':
                state.in_string = False
            i += 1
            continue

        if state.in_char:
            if state.char_escape:
                state.char_escape = False
            elif ch == "\\":
                state.char_escape = True
            elif ch == "'":
                state.in_char = False
            i += 1
            continue

        if ch == "/" and nxt == "/":
            break
        if ch == "/" and nxt == "*":
            state.block_comment_depth += 1
            i += 2
            continue
        if ch == "r":
            j = i + 1
            while j < len(chars) and chars[j] == "#":
                j += 1
            if j < len(chars) and chars[j] == '"':
                state.raw_string_hashes = j - i - 1
                i = j + 1
                continue
        if ch == '"':
            state.in_string = True
            i += 1
            continue
        if ch == "'":
            # Distinguish char literals ('x', '\n') from lifetime annotations
            # ('a, 'static).  Lifetimes are an apostrophe followed by an ASCII
            # letter or underscore and then a non-apostrophe (identifiers, not
            # closing-quote).  Misclassifying a lifetime as a char literal
            # blanks the rest of the line, hiding braces and causing the
            # brace-depth tracker to desync across the whole file.
            if nxt and (nxt.isalpha() or nxt == "_"):
                # Peek past the identifier to see if it's 'x' (char) or 'ident (lifetime).
                j = i + 2
                while j < len(chars) and (chars[j].isalnum() or chars[j] == "_"):
                    j += 1
                if j < len(chars) and chars[j] == "'":
                    # Closing quote found -> char literal like 'a' or 'ab' (invalid but safe to skip).
                    state.in_char = True
                else:
                    # No closing quote -> lifetime annotation; skip the apostrophe.
                    out[i] = " "
            else:
                state.in_char = True
            i += 1
            continue

        out[i] = ch
        i += 1

    # Rust char literals cannot span lines; reset if still open at EOL.
    if state.in_char:
        state.in_char = False
        state.char_escape = False

    return "".join(out)


def requires_test_configuration(predicate: str) -> bool:
    """Prove a cfg predicate cannot be true with Rust's `test` flag unset.

    Other configuration options are unknown, not disabled. Abstract boolean
    sets deliberately overapproximate them, so unsupported syntax or correlated
    options can only lose an exemption, never conceal production code.
    """
    tokens = []
    end = 0
    for match in CFG_TOKEN_PATTERN.finditer(predicate):
        if predicate[end:match.start()].strip():
            return False
        tokens.append(match.group())
        end = match.end()
    if predicate[end:].strip():
        return False
    position = 0

    def take() -> str:
        nonlocal position
        if position == len(tokens):
            raise ValueError("incomplete cfg predicate")
        token = tokens[position]
        position += 1
        return token

    def parse() -> set[bool]:
        name = take()
        if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", name) is None:
            raise ValueError("cfg option required")
        following = tokens[position] if position < len(tokens) else None
        if following == "=":
            take()
            if not take().startswith('"'):
                raise ValueError("cfg option string required")
            return {False, True}
        if following != "(":
            if name in {"test", "false"}:
                return {False}
            return {True} if name == "true" else {False, True}
        if name not in {"all", "any", "not"}:
            raise ValueError("unsupported cfg predicate")
        take()
        children = []
        if position < len(tokens) and tokens[position] != ")":
            while True:
                children.append(parse())
                if position == len(tokens) or tokens[position] != ",":
                    break
                take()
                if position < len(tokens) and tokens[position] == ")":
                    if name == "not":
                        raise ValueError("not accepts one predicate without a list")
                    break
        if take() != ")":
            raise ValueError("unclosed cfg predicate")
        if name == "not":
            if len(children) != 1:
                raise ValueError("not requires one predicate")
            return {not value for value in children[0]}
        if name == "all":
            can_be_true = all(True in child for child in children)
            can_be_false = any(False in child for child in children)
        else:
            can_be_true = any(True in child for child in children)
            can_be_false = all(False in child for child in children)
        return {value for value, possible in ((True, can_be_true), (False, can_be_false)) if possible}

    try:
        values = parse()
    except (ValueError, RecursionError):
        return False
    return position == len(tokens) and values == {False}


def is_test_item(line: str, pending_test_attr: bool) -> tuple[bool, bool]:
    match = ITEM_PATTERN.match(line)
    if not match:
        return False, False

    return True, pending_test_attr


def line_test_contexts(lines: list[str]) -> list[bool]:
    contexts = [False] * len(lines)
    lexer = LexerState()
    block_stack: list[bool] = []
    pending_test_attr = False
    pending_block_context: bool | None = None
    file_test_context = False

    for idx, raw in enumerate(lines):
        code = sanitize_line(raw, lexer)
        stripped = code.strip()
        # Read actual attribute strings only when the Rust lexer confirms this
        # is code, not a lookalike inside a comment or source-text literal.
        cfg = CFG_ATTR_PATTERN.match(raw) if stripped.startswith("#") else None
        if cfg and requires_test_configuration(cfg.group(2)):
            if cfg.group(1):
                if block_stack:
                    block_stack[-1] = True
                else:
                    file_test_context = True
            else:
                pending_test_attr = True
        current_context = file_test_context or (block_stack[-1] if block_stack else False)

        if TEST_ATTR_PATTERN.match(stripped):
            pending_test_attr = True

        item_found, item_is_test = is_test_item(code, pending_test_attr)
        if item_found:
            pending_block_context = item_is_test or current_context
            pending_test_attr = False
        elif stripped and not stripped.startswith("#[") and pending_test_attr:
            pending_test_attr = False

        contexts[idx] = current_context or bool(pending_block_context)

        for ch in code:
            if ch == "{":
                if pending_block_context is not None:
                    block_stack.append(pending_block_context)
                    pending_block_context = None
                else:
                    block_stack.append(block_stack[-1] if block_stack else file_test_context)
            elif ch == "}" and block_stack:
                block_stack.pop()

        if stripped.endswith(";"):
            pending_block_context = None

    return contexts


def is_test_only_path(path: str) -> bool:
    """Return True for files that live in Rust test-only locations.

    Files under ``src/**/tests/*.rs`` and ``src/**/tests.rs`` are Rust test
    sub-modules, typically included behind ``#[cfg(test)]`` and never compiled
    in production. Top-level ``tests/*.rs`` integration test files are already
    outside ``src/`` / ``crates/`` and therefore never checked.
    """
    posix_path = pathlib.PurePosixPath(path)
    parts = posix_path.parts
    return "tests" in parts or posix_path.name == "tests.rs"


def comparison_refs(base: str, head: str, comparison: str) -> tuple[str, ...]:
    if comparison == "endpoints":
        return (base, head)
    if comparison == "merge-base":
        return (f"{base}...{head}",)
    raise ValueError(f"Unknown comparison: {comparison}")


def changed_rust_files(base: str, head: str, comparison: str = "merge-base") -> list[pathlib.Path]:
    output = run_git("-c", "core.quotepath=false", "diff", "--no-renames", "--name-only", *comparison_refs(base, head, comparison), "--", "src", "crates")
    files = []
    for line in output.splitlines():
        if line.endswith(".rs") and (line.startswith("src/") or line.startswith("crates/")):
            if not is_test_only_path(line):
                files.append(pathlib.Path(line))
    return files


def added_lines_for_file(base: str, head: str, path: pathlib.Path, comparison: str = "merge-base") -> set[int]:
    diff = run_git("diff", "--no-renames", "--unified=0", *comparison_refs(base, head, comparison), "--", str(path))
    added: set[int] = set()
    current_line = 0

    for line in diff.splitlines():
        if line.startswith("@@"):
            match = re.search(r"\+(\d+)(?:,(\d+))?", line)
            if not match:
                continue
            current_line = int(match.group(1))
            continue
        if line.startswith("+++ ") or line.startswith("--- "):
            continue
        if line.startswith("+"):
            added.add(current_line)
            current_line += 1
        elif line.startswith("-"):
            continue
        else:
            current_line += 1

    return added


def collect_violations(base: str, head: str, comparison: str = "merge-base") -> list[tuple[str, int, str]]:
    violations: list[tuple[str, int, str]] = []

    for path in changed_rust_files(base, head, comparison):
        if not path.exists():
            continue
        added_lines = added_lines_for_file(base, head, path, comparison)
        if not added_lines:
            continue

        lines = path.read_text(encoding="utf-8").splitlines()
        contexts = line_test_contexts(lines)
        lexer = LexerState()
        sanitized = [sanitize_line(line, lexer) for line in lines]

        for line_no in sorted(added_lines):
            if line_no < 1 or line_no > len(lines):
                continue
            if contexts[line_no - 1]:
                continue
            if "// safety:" in lines[line_no - 1]:
                continue
            if PANIC_PATTERN.search(sanitized[line_no - 1]):
                violations.append((str(path), line_no, lines[line_no - 1].rstrip()))

    return violations


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base", required=False, default="origin/staging")
    parser.add_argument("--head", required=False, default="HEAD")
    parser.add_argument("--comparison", choices=("merge-base", "endpoints"), default="merge-base",
                        help="PR/merge queue use merge-base; main push uses exact endpoints")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(CheckNoPanicsTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1

    violations = collect_violations(args.base, args.head, args.comparison)
    if not violations:
        print("OK: No panic-inducing calls in changed production code.")
        return 0

    print("::error::Found panic-style calls outside test-only Rust code.")
    print("Production code must use proper error handling instead of panicking.")
    print("Suppress false positives with an inline '// safety: <reason>' comment.")
    print("")
    for path, line_no, line in violations[:20]:
        print(f"{path}:{line_no}: {line}")
    print("")
    print(f"Total: {len(violations)} violation(s)")
    return 1


class CheckNoPanicsTests(unittest.TestCase):
    def test_comparison_refs(self) -> None:
        self.assertEqual(comparison_refs("base", "head", "merge-base"), ("base...head",))
        self.assertEqual(comparison_refs("base", "head", "endpoints"), ("base", "head"))
        with self.assertRaises(ValueError):
            comparison_refs("base", "head", "unknown")

    def test_cfg_test_module_marks_inner_lines(self) -> None:
        lines = [
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    assert!(true);\n",
            "}\n",
            "fn prod() {\n",
            "    value.expect(\"boom\");\n",
            "}\n",
        ]

        contexts = line_test_contexts(lines)

        self.assertTrue(contexts[1])
        self.assertTrue(contexts[2])
        self.assertFalse(contexts[4])
        self.assertFalse(contexts[5])

    def test_test_function_marks_body_only(self) -> None:
        lines = [
            "#[test]\n",
            "fn it_works(\n",
            ") {\n",
            "    assert_eq!(2 + 2, 4);\n",
            "}\n",
            "fn prod() {\n",
            "    assert!(ready);\n",
            "}\n",
        ]

        contexts = line_test_contexts(lines)

        self.assertTrue(contexts[1])
        self.assertTrue(contexts[2])
        self.assertTrue(contexts[3])
        self.assertFalse(contexts[5])
        self.assertFalse(contexts[6])

    def test_proc_macro_test_attrs_mark_body_only(self) -> None:
        attrs = [
            "tokio::test",
            'tokio::test(flavor = "multi_thread", worker_threads = 4)',
            "rstest",
            "test_case(1, 2)",
            "cfg(all(test, unix))",
        ]

        for attr in attrs:
            with self.subTest(attr=attr):
                lines = [
                    f"#[{attr}]\n",
                    "fn it_works() {\n",
                    '    value.expect("allowed in test");\n',
                    "}\n",
                    "fn prod() {\n",
                    '    value.expect("boom");\n',
                    "}\n",
                ]

                contexts = line_test_contexts(lines)

                self.assertTrue(contexts[1])
                self.assertTrue(contexts[2])
                self.assertFalse(contexts[4])
                self.assertFalse(contexts[5])

    def test_test_only_path_detection(self) -> None:
        self.assertTrue(is_test_only_path("src/channels/web/tests/multi_tenant.rs"))
        self.assertTrue(is_test_only_path("crates/foo/src/tests/helpers.rs"))
        self.assertTrue(is_test_only_path("crates/foo/src/tests.rs"))
        self.assertTrue(is_test_only_path("crates/foo/src/nested/tests.rs"))
        self.assertFalse(is_test_only_path("src/channels/web/mod.rs"))
        self.assertFalse(is_test_only_path("src/channels/web/test_helpers.rs"))
        self.assertFalse(is_test_only_path("crates/foo/src/lib.rs"))

    def test_lifetime_annotations_do_not_desync_braces(self) -> None:
        """Lifetime annotations ('a, 'static) must not be parsed as char literals.

        If they are, the sanitizer blanks the rest of the line — including any
        opening brace — and the brace-depth tracker desyncs.  This caused
        false positives in large test modules (e.g. server.rs).
        """
        lines = [
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    fn set_env_var(key: &'static str) -> Guard {\n",
            "        let original = std::env::var(key).ok();\n",
            "        Guard { key, original }\n",
            "    }\n",
            "    fn later_helper() {\n",
            '        value.expect("should be test context");\n',
            "    }\n",
            "}\n",
        ]

        contexts = line_test_contexts(lines)

        # All lines inside mod tests must be test context, even after
        # a function signature containing a lifetime annotation.
        self.assertTrue(contexts[2], "fn with 'static should be test context")
        self.assertTrue(contexts[7], "later helper should still be test context")

    def test_module_name_does_not_hide_production_calls(self) -> None:
        lines = [
            "mod tests {\n",
            "    fn helper() {\n",
            "        assert!(true);\n",
            "    }\n",
            "}\n",
        ]

        contexts = line_test_contexts(lines)

        self.assertFalse(any(contexts))

    def test_cfg_requires_test_instead_of_merely_mentioning_it(self) -> None:
        for predicate in [
            "test", "all(test, unix)", 'all(feature = "db", test,)',
            "not(not(test))", "any(all(test, unix), all(test, windows))",
            "all(test, any(unix, windows))", "false", "any()",
        ]:
            with self.subTest(predicate=predicate):
                self.assertTrue(requires_test_configuration(predicate))
        for predicate in [
            "not(test)", "any(test, unix)", 'any(test, feature = "db")',
            'feature = "test"', 'target_os = "test"', "all()", "true",
            "not(all(test, unix))", "unix", "test = \"yes\"",
            "", "all(test", "test, unix", "not(test, unix)",
            "not(test,)", "unknown(test)", 'all(test, feature = r#"db"#)',
        ]:
            with self.subTest(predicate=predicate):
                self.assertFalse(requires_test_configuration(predicate))

    def test_production_cfg_does_not_exempt_next_item(self) -> None:
        for predicate in ["not(test)", "any(test, unix)", 'feature = "test"']:
            with self.subTest(predicate=predicate):
                contexts = line_test_contexts([
                    f"#[cfg({predicate})]\n", "fn production() {\n",
                    '    value.unwrap();\n', "}\n",
                ])
                self.assertFalse(any(contexts))

    def test_inner_test_guard_covers_helpers_and_nested_items(self) -> None:
        lines = [
            "#![cfg(test)]\n", "use super::*;\n", "async fn helper() {\n",
            "    value.unwrap();\n", "}\n", "mod nested {\n",
            "    fn helper() { assert!(true); }\n", "}\n",
        ]
        self.assertTrue(all(line_test_contexts(lines)))
        nested = line_test_contexts([
            "mod fixtures {\n", "    #![cfg(all(test, unix))]\n",
            "    fn helper() { value.unwrap(); }\n", "}\n",
            "fn production() { value.unwrap(); }\n",
        ])
        self.assertTrue(nested[2])
        self.assertFalse(nested[4])

    def test_inner_production_cfg_and_literal_attributes_stay_scanned(self) -> None:
        lines = [
            "#![cfg(not(test))]\n", "fn production() { value.unwrap(); }\n",
            'const TEXT: &str = r#"\n', "#![cfg(test)]\n", '"#;\n',
            "/*\n", "#![cfg(test)]\n", "*/\n",
            "fn also_production() { value.unwrap(); }\n",
        ]
        self.assertFalse(any(line_test_contexts(lines)))


if __name__ == "__main__":
    sys.exit(main())
