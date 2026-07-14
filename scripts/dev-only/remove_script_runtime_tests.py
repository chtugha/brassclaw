#!/usr/bin/env python3
"""
Drop test functions in `--target` test files whose body references any of the
script-only identifiers (the whole set of remaining RuntimeKind::Script /
ExtensionRuntime::Script / DockerScriptBackend / ScriptBackend /
ScriptRuntimeAdapter / ScriptInvocation / ScriptExecution* / etc. symbols).

Strategy (no regex-based robust parsing; we do a Rust-aware brace walk):

  1. Tokenize the file to skip Rust string literals (regular and raw), char
     literals, line comments (`//`) and block comments (`/* … */`). Maintain
     a position cursor over the emitted code.
  2. Scan for `#[test]` / `#[tokio::test]` / `#[rstest]` / `#[traced_test]`
     attribute lines followed by a `fn <name>` (possibly `pub fn`, `async fn`,
     `async pub fn`).
  3. Walk forward from the `fn`'s opening `{` balancing braces (skipping the
     same string/comment tokens) to find the matching closing `}`.
  4. If the captured body contains any identifier in `--match` (default:
     script-only symbol list), drop the entire block (attributes + fn).
  5. If `--dry-run`, just print stats instead of mutating the file.

Defaults to script-only identifiers if `--match` not given.

Limitations:
  - This script is one-shot. It will be deleted at the end of the script-
    runtime migration.
  - It strips fns wholesale — even when the script-only token appears in one
    assertion inside a fn otherwise reusable. That's deliberate: a test body
    that names RuntimeKind::Script needs to be re-thought before re-adding.
"""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

DEFAULT_MATCH = (
    "RuntimeKind::Script",
    "ExtensionRuntime::Script",
    "ExtensionRuntimeV2::Script",
    "DockerScriptBackend",
    "ScriptBackend",
    "ScriptBackendError",
    "ScriptBackendOutput",
    "ScriptBackendRequest",
    "ScriptCapabilityResult",
    "ScriptError",
    "ScriptExecutionRequest",
    "ScriptExecutionResult",
    "ScriptExecutor",
    "ScriptInvocation",
    "ScriptRuntimeAdapter",
    "ScriptBackendResult",
    "ScriptRuntimeObservation",
    "script_runtime::",
    "Self::ScriptRuntime",
    "ProductionWiringComponent::ScriptRuntime",
    "ProductionComponentType::ScriptRuntime",
    "LifecycleExtensionRuntimeKind::Script",
    "RuntimeKind::Wasm",
    "LifecycleExtensionRuntimeKind::Wasm",
    "TrustedRuntimeKindWire::Script",
    "TrustedRuntimeKindWire::Wasm",
    "DispatchError::Script",
    "DispatchError::Wasm",
)


def tokenize_skip(src: str):
    """Yield (start, end, kind) spans that should be transparent to brace counting."""
    i = 0
    n = len(src)
    while i < n:
        c = src[i]
        # line comment
        if c == "/" and i + 1 < n and src[i + 1] == "/":
            j = src.find("\n", i)
            if j == -1:
                yield (i, n, "comment")
                return
            yield (i, j, "comment")
            i = j
            continue
        # block comment
        if c == "/" and i + 1 < n and src[i + 1] == "*":
            j = src.find("*/", i + 2)
            if j == -1:
                yield (i, n, "comment")
                return
            yield (i, j + 2, "comment")
            i = j + 2
            continue
        # char literal
        if c == "'":
            # naive: skip until closing quote (handles escapes)
            j = i + 1
            while j < n:
                if src[j] == "\\" and j + 1 < n:
                    j += 2
                    continue
                if src[j] == "'":
                    j += 1
                    break
                j += 1
            yield (i, j, "literal")
            i = j
            continue
        # raw string literal r#"…"#
        if c == "r" and i + 1 < n and src[i + 1] in "#'\"":
            hash_count = 0
            j = i + 2
            while j < n and src[j] == "#":
                hash_count += 1
                j += 1
            if j < n and src[j] == '"':
                opener = "#" * hash_count
                j += 1
                closer = '"' + opener
                k = src.find(closer, j)
                if k == -1:
                    yield (i, n, "literal")
                    return
                yield (i, k + 1 + hash_count, "literal")
                i = k + 1 + hash_count
                continue
            # fall through
        # regular string literal
        if c == '"':
            j = i + 1
            while j < n:
                if src[j] == "\\" and j + 1 < n:
                    j += 2
                    continue
                if src[j] == '"':
                    j += 1
                    break
                j += 1
            yield (i, j, "literal")
            i = j
            continue
        i += 1


def in_skipped(pos: int, skip_spans) -> bool:
    for s, e, _ in skip_spans:
        if s <= pos < e:
            return True
        if s > pos:
            return False
    return False


def find_attr_blocks(src: str):
    """Yield (start, end) byte ranges covering each leading #[…] attribute
    block (a contiguous run of `#[...]` lines) immediately preceding a
    following `fn ...` definition. Yields span covering attrs only."""
    skip = list(tokenize_skip(src))
    n = len(src)
    i = 0
    # Find any `#[` outside of skip span
    while i < n:
        if not in_skipped(i, skip) and src.startswith("#[", i):
            # collect contiguous attribute lines
            j = i
            while j < n and (in_skipped(j, skip) or src.startswith("#[", j)):
                # advance to next line ending
                if not in_skipped(j, skip) and src.startswith("#[", j):
                    k = src.find("\n", j)
                    if k == -1:
                        k = n
                    j = k + 1
                    continue
                if in_skipped(j, skip):
                    span_end = next((e for s, e, k in skip if s <= j < e), n)
                    nl = src.find("\n", span_end)
                    j = n if nl == -1 else nl + 1
                    continue
                j += 1
            yield (i, j)
            i = j
            continue
        i += 1


def find_fn_defs(src: str):
    """Yield (start, end) byte ranges of every `fn <name>` declaration body
    (from `fn` keyword through closing brace inclusive). Only top-level
    function declarations are matched — those whose opening brace is at
    brace-depth 0 (modulo whitespace-walked skip)."""
    skip = list(tokenize_skip(src))
    n = len(src)
    i = 0
    while i < n:
        if in_skipped(i, skip):
            # advance past skip span
            for s, e, _ in skip:
                if s <= i < e:
                    i = e
                    break
            continue
        if src.startswith("fn ", i) or src.startswith("\nfn ", i) or src.startswith("\tfn ", i):
            # confirm preceded by valid boundary (start, whitespace, or newline)
            prev = src[i - 1] if i > 0 else "\n"
            if prev.isspace() or prev == "\n" or prev == "":
                # find opening brace
                j = i + 3
                while j < n and src[j] != "{":
                    if (src[j].isalpha() or src[j] == "_" or src[j] == ":"
                            or src[j] == "<" or src[j] == ">" or src[j] == "'"
                            or src[j] == "(" or src[j] == ")" or src[j] == ","
                            or src[j] == "." or src[j] == " " or src[j] == "\t"
                            or src[j] == "\n"):
                        j += 1
                        continue
                    break
                if j < n and src[j] == "{":
                    # walk braces
                    depth = 1
                    k = j + 1
                    while k < n and depth > 0:
                        if in_skipped(k, skip):
                            for s, e, _ in skip:
                                if s <= k < e:
                                    k = e
                                    break
                            continue
                        ch = src[k]
                        if ch == "{":
                            depth += 1
                        elif ch == "}":
                            depth -= 1
                            if depth == 0:
                                k += 1
                                break
                        k += 1
                    yield (i, k)
                    i = k
                    continue
        i += 1


def src_slice(src: str, span):
    s, e = span
    return src[s:e]


def is_script_only(body: str, identifiers) -> bool:
    for ident in identifiers:
        if ident in body:
            return True
    return False


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("files", nargs="+", help="Rust source files to scan")
    p.add_argument("--match", nargs="+", default=None,
                   help="Identifiers whose presence flags a fn as droppable "
                        "(default: script-only symbol list)")
    p.add_argument("--dry-run", action="store_true",
                   help="Print stats only — do not mutate")
    args = p.parse_args()

    identifiers = tuple(args.match) if args.match else DEFAULT_MATCH

    total_dropped = 0
    for filepath in args.files:
        path = Path(filepath)
        if not path.exists():
            print(f"SKIP: {filepath} (missing)", file=sys.stderr)
            continue
        src = path.read_text()
        fns = list(find_fn_defs(src))
        if not fns:
            print(f"SKIP: {filepath} (no fns)", file=sys.stderr)
            continue
        # identify fns to drop
        drops = []
        for span in fns:
            body = src_slice(src, span)
            if is_script_only(body, identifiers):
                drops.append(span)
        if not drops:
            print(f"KEEP: {filepath} (no script-only fns in {len(fns)} found)")
            continue
        # remove from src, then trim trailing blank lines
        new_src = src
        for span in sorted(drops, key=lambda x: x[0], reverse=True):
            new_src = new_src[: span[0]] + new_src[span[1] :]
        # collapse multiple consecutive blank lines
        import re
        new_src = re.sub(r"\n{3,}", "\n\n", new_src)
        # trim leading/trailing whitespace on file
        new_src = new_src.strip() + "\n"
        if args.dry_run:
            print(f"WOULD-DROP: {filepath} ({len(drops)} of {len(fns)} fns)")
            for span in drops:
                body = src_slice(src, span)
                # extract fn name
                head = body[:200].split("(")[0].split("{")[0].strip()
                # extract first identifier-containing line near "fn"
                fn_kw = body.find("fn ") if "fn " in body else body.find("fn\t")
                name = head
                if fn_kw != -1:
                    tail = body[fn_kw:fn_kw + 200].split("{")[0].strip()
                    name = tail[:120]
                print(f"  - {name}")
        else:
            path.write_text(new_src)
            print(f"DROPPED: {filepath} ({len(drops)} of {len(fns)} fns)")
            for span in drops:
                body = src_slice(src, span)
                fn_kw = body.find("fn ") if "fn " in body else body.find("fn\t")
                tail = body[fn_kw:fn_kw + 200].split("{")[0].strip() if fn_kw != -1 else "fn"
                print(f"  - {tail[:120]}")
            total_dropped += len(drops)
    print(f"\nTotal dropped: {total_dropped}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
