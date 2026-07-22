#!/usr/bin/env python3
"""
sweepfix.py — Codebase magic-number & stub sweeper.

Steps:
  1. Create .sweepfix/ and gitignore it (skip if already exists).
  2. Commit & push everything; sync with remote; verify in-sync.
  3. Scan codebase, build codebaselist.md (or diff-update existing one).
  4. For each marked file: find magic numbers + stubs; write reports; unmark.

Known limitation: numbers inside /* ... */ block comments whose body lines do
not start with a leading '*' are not filtered from magic-number detection.
Single-pass line scanning cannot track block-comment state.
"""

from __future__ import annotations

import hashlib
import re
import sys
import subprocess
from pathlib import Path

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

# Resolved inside main() so importing this module outside a git repo does not
# immediately crash with a CalledProcessError at import time.
REPO_ROOT: Path
SWEEPFIX_DIR: Path
CODEBASE_LIST: Path

# ── Extensions treated as binary / non-code ────────────────────────────────
BINARY_EXTENSIONS = {
    ".png", ".jpg", ".jpeg", ".gif", ".svg", ".ico", ".webp", ".bmp",
    ".wasm", ".so", ".dylib", ".dll", ".exe", ".a", ".lib",
    ".pdf", ".docx", ".xlsx", ".odt", ".pptx",
    ".zip", ".tar", ".gz", ".bz2", ".xz", ".zst", ".7z",
    ".mp3", ".mp4", ".avi", ".mov", ".wav",
    ".ttf", ".otf", ".woff", ".woff2", ".eot",
    ".lock",       # Cargo.lock / package-lock etc. — generated, not authored
}

# ── File-name / path patterns that are NOT our code ────────────────────────
EXCLUDED_PATH_PATTERNS = [
    re.compile(r"(^|/)target/"),
    re.compile(r"(^|/)\.git/"),
    re.compile(r"(^|/)\.sweepfix/"),
    re.compile(r"(^|/)node_modules/"),
    re.compile(r"(^|/)__pycache__/"),
    re.compile(r"(^|/)\.venv/"),
    re.compile(r"(^|/)dist/"),
    re.compile(r"(^|/)release-artifacts"),
    re.compile(r"(^|/)release-v"),
    re.compile(r"(^|/)releases/"),
    re.compile(r"(^|/)\.cargo/"),
    re.compile(r"(^|/)fuzz/corpus/"),
    # Vendored / generated
    re.compile(r"vendor/"),
    re.compile(r"generated/"),
]

# ── File names that are documentation / config, not code ───────────────────
DOC_SUFFIXES = {
    ".md", ".txt", ".rst", ".adoc", ".asciidoc",
    ".json", ".yaml", ".yml", ".toml", ".ini", ".cfg", ".conf",
    ".env", ".example",
    ".sh",   # shell completion scripts (brassclaw.bash / .zsh / .fish are thousands of lines of generated completion)
    ".bash", ".zsh", ".fish",
    ".sql",  # migrations are plain SQL, not our Rust logic
    ".html", ".css", ".scss",
    ".xml",
}

# Files whose name alone marks them as external / generated
EXCLUDED_FILENAMES = {
    "Cargo.lock",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "bun.lockb",
}

# ── Code extensions we DO care about ───────────────────────────────────────
CODE_EXTENSIONS = {
    ".rs", ".py", ".ts", ".tsx", ".js", ".jsx",
    ".go", ".c", ".cpp", ".h", ".hpp",
    ".java", ".kt", ".swift", ".rb", ".cs",
}


def run(cmd: list[str], *, capture: bool = False, check: bool = True) -> subprocess.CompletedProcess:
    return subprocess.run(
        cmd,
        cwd=REPO_ROOT,  # type: ignore[name-defined]  # set in main() before any call
        capture_output=capture,
        text=True,
        check=check,
    )


def git(*args: str, capture: bool = True, check: bool = True) -> str:
    result = run(["git", *args], capture=capture, check=check)
    return result.stdout.strip() if capture else ""


def current_commit() -> str:
    return git("rev-parse", "HEAD")


def current_branch() -> str:
    return git("rev-parse", "--abbrev-ref", "HEAD")


def ask(prompt: str) -> str:
    """Print prompt and return stripped lower-case reply."""
    try:
        answer = input(prompt + " ").strip().lower()
    except (EOFError, KeyboardInterrupt):
        print()
        sys.exit(1)
    return answer


def _safe_branch_name(br: str) -> str:
    """Return a commit-message-safe representation of a branch name.

    Replaces any character outside [a-zA-Z0-9/_.-] with an underscore so
    that malicious branch names (containing newlines, shell metacharacters,
    ANSI escapes, etc.) cannot corrupt the git log or downstream tooling
    that parses commit messages.
    """
    return re.sub(r"[^a-zA-Z0-9/_.\-]", "_", br)


def die(msg: str) -> None:
    print(f"\n❌  {msg}", file=sys.stderr)
    sys.exit(1)


# ---------------------------------------------------------------------------
# Step 1 — create .sweepfix/ and gitignore it
# ---------------------------------------------------------------------------

def step1_create_sweepfix() -> None:
    print("\n── Step 1: .sweepfix directory ──────────────────────────────────")

    if SWEEPFIX_DIR.exists():
        print("  .sweepfix/ already exists — skipping creation.")
    else:
        SWEEPFIX_DIR.mkdir(parents=True)
        print("  Created .sweepfix/")

    gitignore_path = REPO_ROOT / ".gitignore"
    marker = ".sweepfix/"

    if gitignore_path.exists():
        content = gitignore_path.read_text()
        if marker not in content:
            with gitignore_path.open("a") as fh:
                if not content.endswith("\n"):
                    fh.write("\n")
                fh.write("\n# sweepfix scratch folder\n")
                fh.write(f"{marker}\n")
            print("  Added .sweepfix/ to .gitignore")
        else:
            print("  .sweepfix/ already in .gitignore — skipping.")
    else:
        gitignore_path.write_text(f"# sweepfix scratch folder\n{marker}\n")
        print("  Created .gitignore with .sweepfix/ entry.")


# ---------------------------------------------------------------------------
# Step 2 — commit, push, sync
# ---------------------------------------------------------------------------

def step2_commit_push_sync() -> None:
    print("\n── Step 2: commit / push / sync ────────────────────────────────")

    branch = current_branch()
    if branch == "HEAD":
        die(
            "Repository is in detached HEAD state — not on any branch.\n"
            "Check out a branch first (e.g. `git checkout main`) and re-run."
        )
    print(f"  Current branch: {branch}")

    # Check for uncommitted changes BEFORE staging so the user sees what will be committed
    pre_status = git("status", "--porcelain")
    if pre_status:
        print("  Uncommitted changes detected — staging and committing all.")
        safe_branch = _safe_branch_name(branch)
        try:
            run(["git", "add", "-A"], capture=False)
            run(["git", "commit", "-m", f"chore: sweepfix pre-run commit [{safe_branch}]"], capture=False)
        except subprocess.CalledProcessError as exc:
            die(
                f"git commit failed (exit {exc.returncode}).\n"
                "A pre-commit hook may have rejected the commit.\n"
                "Fix the issue manually and re-run the script."
            )
        print("  Committed local changes.")
    else:
        print("  Nothing to commit.")

    if branch == "main":
        _handle_main_branch_push()
    else:
        _push_branch(branch)

    # Single fetch here: covers both the push above and any side-branch pushes
    # done inside _handle_main_branch_push. Avoids repeated fetches per branch.
    try:
        git("fetch", "origin")
    except subprocess.CalledProcessError:
        die(
            "Could not fetch from remote 'origin'.\n"
            "Check that the remote exists (`git remote -v`) and that you have network access."
        )
    _verify_in_sync_no_fetch(branch)


def _handle_main_branch_push() -> None:
    """
    On main: detect if there are local commits that belong to other branches,
    give the user a choice, then push main.
    """
    # Find local branches other than main that have commits not yet on main
    other_branches_raw = git("branch", "--format=%(refname:short)").split()
    divergent: list[str] = []
    for br in other_branches_raw:
        if br == "main":
            continue
        # commits on br not yet reachable from main
        count_str = git("rev-list", "--count", f"main..{br}", check=False)
        try:
            count = int(count_str)
        except ValueError:
            print(f"  ⚠ Could not check divergence for branch '{br}' — skipping.")
            count = 0
        if count > 0:
            divergent.append(br)

    if divergent:
        print(
            f"\n  ⚠  The following branches have commits NOT yet merged into main:\n"
            + "\n".join(f"    • {b}" for b in divergent)
        )
        print()
        print("  For each branch choose an action:")
        print("    [m] merge to main and push")
        print("    [p] push that branch to its own remote (no merge)")
        print("    [s] leave it as-is (no push, no merge)")

        for br in divergent:
            while True:
                choice = ask(f"  Branch '{br}': [m/p/s]?")
                if choice in ("m", "p", "s"):
                    break
                print("  Please enter m, p, or s.")

            if choice == "m":
                safe_br = _safe_branch_name(br)
                result = run(["git", "merge", "--no-ff", br, "-m", f"chore: merge {safe_br} → main"], check=False, capture=True)
                if result.returncode != 0:
                    die(
                        f"Merge of '{br}' into main failed:\n{result.stderr}\n"
                        "Resolve conflicts manually, then re-run the script."
                    )
                print(f"  Merged '{br}' → main.")
            elif choice == "p":
                _push_branch(br)
                # Verified by the fetch + _verify_in_sync_no_fetch in step2_commit_push_sync
                # after all pushes complete. No early per-branch fetch needed here.

    # Now push main
    _push_branch("main")


def _push_branch(branch: str) -> None:
    result = run(
        ["git", "push", "--set-upstream", "origin", branch],
        capture=True,
        check=False,
    )
    if result.returncode != 0:
        die(
            f"Push of '{branch}' failed:\n{result.stderr}\n"
            "Fix the above error and re-run the script."
        )
    print(f"  Pushed branch '{branch}'.")


def _verify_in_sync_no_fetch(branch: str) -> None:
    """Verify local == remote for `branch`. Caller must have already run `git fetch`."""
    local_sha = git("rev-parse", f"refs/heads/{branch}")
    remote_sha = git("rev-parse", f"refs/remotes/origin/{branch}", check=False)

    if not remote_sha:
        die(
            f"Cannot find remote tracking branch 'origin/{branch}'.\n"
            "Push the branch first and re-run."
        )

    if local_sha != remote_sha:
        behind = git("rev-list", "--count", f"{branch}..origin/{branch}")
        ahead  = git("rev-list", "--count", f"origin/{branch}..{branch}")
        die(
            f"Local branch '{branch}' is NOT in sync with 'origin/{branch}'.\n"
            f"  Local  SHA : {local_sha}\n"
            f"  Remote SHA : {remote_sha}\n"
            f"  Commits ahead  of remote : {ahead}\n"
            f"  Commits behind remote    : {behind}\n\n"
            "Resolve by pulling/rebasing and re-running the script."
        )

    print(f"  ✓ Local and remote '{branch}' are in sync ({local_sha[:12]}).")


# ---------------------------------------------------------------------------
# Step 3 — build / update codebaselist.md
# ---------------------------------------------------------------------------

MARK_CHAR = "[x]"
DONE_CHAR  = "[ ]"

# Valid git commit SHA: 7–40 lowercase hex chars (short or full SHA).
_SHA_RE = re.compile(r"^[0-9a-f]{7,40}$")


def _passes_gating(rel_path: Path) -> bool:
    """Return True if this file is 'our code' and should be in the list."""
    rel_str = str(rel_path)

    # 1. Excluded path patterns (target/, .git/, etc.)
    for pat in EXCLUDED_PATH_PATTERNS:
        if pat.search(rel_str):
            return False

    # 2. Explicit excluded filenames
    # Note: files sourced from git ls-files / git diff are always tracked, so they
    # can never be gitignored — no need to call git check-ignore here.
    if rel_path.name in EXCLUDED_FILENAMES:
        return False

    # 3. Binary extensions
    if rel_path.suffix.lower() in BINARY_EXTENSIONS:
        return False

    # 4. Documentation / config extensions (not code)
    if rel_path.suffix.lower() in DOC_SUFFIXES:
        return False

    # 5. Must be a known code extension
    if rel_path.suffix.lower() not in CODE_EXTENSIONS:
        return False

    return True


def _all_tracked_files() -> list[Path]:
    """Return all files tracked by git (relative to repo root)."""
    raw = git("ls-files")
    return [Path(p) for p in raw.splitlines() if p.strip()]


def _changed_files_between(old_commit: str, new_commit: str) -> tuple[list[str], list[str], list[str]]:
    """Return (added, modified, deleted) relative paths.

    git diff --name-status output format:
      A  <path>              — added
      M  <path>              — modified
      D  <path>              — deleted
      R<score>  <old>  <new> — renamed (3 tab-separated fields)
      C<score>  <old>  <new> — copied  (3 tab-separated fields)
    """
    raw = git("diff", "--name-status", old_commit, new_commit)
    added, modified, deleted = [], [], []
    for line in raw.splitlines():
        parts = line.split("\t")
        if len(parts) < 2:
            continue
        status = parts[0].strip()
        if status.startswith("A"):
            added.append(parts[1].strip())
        elif status.startswith("M"):
            modified.append(parts[1].strip())
        elif status.startswith("D"):
            deleted.append(parts[1].strip())
        elif status.startswith("R") or status.startswith("C"):
            # parts[1] = old path, parts[2] = new path
            if len(parts) >= 3:
                deleted.append(parts[1].strip())   # old name is gone
                added.append(parts[2].strip())     # new name is new
            else:
                modified.append(parts[1].strip())  # malformed, treat as modified
    return added, modified, deleted


def _parse_codebaselist(text: str) -> tuple[str, list[tuple[str, bool]]]:
    """
    Parse codebaselist.md.
    Returns (commit_sha, [(rel_path, is_marked), ...])
    """
    commit_sha = ""
    entries: list[tuple[str, bool]] = []
    seen: set[str] = set()

    for line in text.splitlines():
        line = line.rstrip()
        if line.startswith("commit:"):
            commit_sha = line.split(":", 1)[1].strip()
            continue
        m = re.match(r"^(\[[ x]\])\s+(.+)$", line)
        if m:
            marked = m.group(1) == "[x]"
            path   = m.group(2).strip()
            if path in seen:
                print(f"  ⚠ Duplicate path in codebaselist.md: {path!r} — keeping first occurrence.")
                continue
            seen.add(path)
            entries.append((path, marked))

    return commit_sha, entries


def _write_codebaselist(commit_sha: str, entries: list[tuple[str, bool]]) -> None:
    lines = [
        "# Codebase File List",
        "",
        f"commit: {commit_sha}",
        "",
        "## Files",
        "",
    ]
    for path, marked in sorted(entries, key=lambda x: x[0]):
        marker = MARK_CHAR if marked else DONE_CHAR
        lines.append(f"{marker} {path}")
    lines.append("")
    CODEBASE_LIST.write_text("\n".join(lines), encoding="utf-8")


def _build_codebaselist_fresh(current_sha: str, *, rebuild: bool = False) -> None:
    """Scan all tracked files and write a fresh codebaselist.md."""
    all_files = _all_tracked_files()
    entries: list[tuple[str, bool]] = []
    skipped = 0
    for f in all_files:
        if _passes_gating(f):
            entries.append((str(f), True))
        else:
            skipped += 1
    _write_codebaselist(current_sha, entries)
    label = "Rebuilding" if rebuild else "First run"
    print(f"  {label} — {len(entries)} files added to list ({skipped} skipped).")


def step3_build_codebaselist() -> None:
    print("\n── Step 3: codebaselist.md ──────────────────────────────────────")

    current_sha = current_commit()

    if not CODEBASE_LIST.exists():
        _build_codebaselist_fresh(current_sha)
        return

    # Existing list — compare commits
    text         = CODEBASE_LIST.read_text(encoding="utf-8")
    old_sha, existing = _parse_codebaselist(text)

    if not old_sha or not _SHA_RE.match(old_sha):
        print(f"  ⚠ codebaselist.md has invalid/missing commit SHA ({old_sha!r}) — rebuilding from scratch.")
        CODEBASE_LIST.unlink()
        _build_codebaselist_fresh(current_sha, rebuild=True)
        return

    if old_sha == current_sha:
        print(f"  List is up to date at {current_sha[:12]} — no changes needed.")
        return

    print(f"  List commit : {old_sha[:12]}")
    print(f"  Repo commit : {current_sha[:12]}")
    print("  Diffing...")

    added_paths, modified_paths, deleted_paths = _changed_files_between(old_sha, current_sha)

    entry_map: dict[str, bool] = dict(existing)

    # Deleted — remove from list
    removed = 0
    for p in deleted_paths:
        if p in entry_map:
            del entry_map[p]
            removed += 1

    # Modified — mark as needing re-scan
    remarked = 0
    for p in modified_paths:
        if p in entry_map:
            entry_map[p] = True
            remarked += 1

    # Added — add if they pass gating, mark them
    new_added = 0
    for p in added_paths:
        rel = Path(p)
        if _passes_gating(rel):
            entry_map[p] = True
            new_added += 1

    _write_codebaselist(current_sha, list(entry_map.items()))
    print(
        f"  Updated list: +{new_added} new, {remarked} re-marked, "
        f"{removed} removed → {len(entry_map)} total files."
    )


# ---------------------------------------------------------------------------
# Step 4 — scan marked files for magic numbers + stubs
# ---------------------------------------------------------------------------

# ── Magic-number detection ──────────────────────────────────────────────────
# Matches integer / float literals that are NOT obvious non-magic values.
# Excludes: 0, 1, -1, 0.0, 1.0, 0u8, 1usize, etc.
#
# The '-' prefix uses (?:-(?=\d))? so it only matches when the minus sign is
# immediately followed by a digit. This prevents 'a - 3000' (binary subtraction
# with a space after '-') from being reported as '-3000'.
_MAGIC_NUMBER_RE = re.compile(
    r"""
    (?<![.\w])              # not preceded by . or word char
    (?P<lit>
        (?:-(?=\d))?        # optional unary '-' only when immediately followed by a digit
        (?:
            0[xXbBoO][0-9a-fA-F_]+      # hex / bin / oct
            |
            \d[\d_]*(?:\.\d[\d_]*)?     # decimal / float
        )
        (?:[uif](?:8|16|32|64|128|size))? # optional Rust type suffix
    )
    (?![.\w])               # not followed by . or word char
    """,
    re.VERBOSE,
)

# Values that are universally "not magic"
_TRIVIAL_NUMBERS = {
    "0", "1", "-1", "2", "0.0", "1.0", "-1.0",
    "0u8", "0u16", "0u32", "0u64", "0usize", "0i32", "0i64",
    "1u8", "1u16", "1u32", "1u64", "1usize", "1i32", "1i64",
}


# Matches comment-only lines across Rust/Python/JS/C — but NOT Rust attributes (#[...]).
# Also matches the closing '*/' line. Lines inside /* ... */ without a leading '*'
# are not filtered — see the module docstring for this known limitation.
_COMMENT_ONLY_LINE_RE = re.compile(r"^\s*(//|/\*|\*/|\*(?!/)|#(?!\[))")


def _strip_inline_comment(line: str) -> str:
    """Strip trailing // and # inline comments, respecting double-quoted strings.

    Correctly handles escaped backslashes before a closing quote (e.g. "a\\\\")
    by counting the run of backslashes preceding each '"': if the count is even,
    the backslashes cancel each other and the quote is a real string delimiter;
    if odd, the quote is escaped and does NOT toggle the string state.
    """
    in_string = False
    i = 0
    while i < len(line):
        c = line[i]
        if c == '"':
            # Count consecutive backslashes immediately before this quote.
            num_bs = 0
            j = i - 1
            while j >= 0 and line[j] == "\\":
                num_bs += 1
                j -= 1
            # An even number of backslashes means none of them escape this quote.
            if num_bs % 2 == 0:
                in_string = not in_string
        elif not in_string:
            if line[i : i + 2] == "//":
                return line[:i]
            if c == "#":
                return line[:i]
        i += 1
    return line


def _find_magic_numbers(source: str) -> list[tuple[int, str, str]]:
    """Return list of (line_number, literal, full_line) tuples."""
    results = []
    for lineno, raw_line in enumerate(source.splitlines(), start=1):
        # Skip pure comment lines (// ... # ... /* ... * ...) but NOT Rust #[attributes]
        if _COMMENT_ONLY_LINE_RE.match(raw_line):
            continue
        # Strip trailing inline comment before scanning so numbers inside comment
        # tails (e.g. "let x = 9000; // was 42") are not falsely reported.
        scan_line = _strip_inline_comment(raw_line)
        for m in _MAGIC_NUMBER_RE.finditer(scan_line):
            lit = m.group("lit")
            # Check the literal itself (not stripped) so negative magic numbers are preserved
            if lit in _TRIVIAL_NUMBERS:
                continue
            # Skip if inside a double-quoted string literal (heuristic: odd count of " before match).
            # Single quotes are intentionally NOT used here: Rust lifetime annotations ('a, 'static)
            # and character literals produce odd single-quote counts that cause false negatives
            # (e.g. `&'a [u8]` before a number would wrongly suppress the report).
            before = scan_line[: m.start()]
            if before.count('"') % 2 == 1:
                continue
            results.append((lineno, lit, raw_line.rstrip()))
    return results


# ── Stub detection ──────────────────────────────────────────────────────────
# Each entry is (compiled_pattern, human_readable_label).
# panic!() is scoped to "not implemented" strings only — bare panic!() appears ~2511×
# in this codebase on legitimate error-handling paths and would produce unusable noise.
# #[allow(dead_code)] intentionally omitted — appears ~240× on legitimate code.
_STUB_PATTERNS: list[tuple[re.Pattern[str], str]] = [
    (re.compile(r"\btodo!\s*\("),         "todo!()"),
    (re.compile(r"\bunimplemented!\s*\("), "unimplemented!()"),
    (re.compile(r'\bpanic!\s*\(\s*["\'].*?(?:not.implemented|stub)', re.IGNORECASE),
                                          'panic!("not implemented…")'),
    # Comment-style markers — must appear inside a comment token (// or #)
    (re.compile(r"(?://|#[^!\[])[^\n]*\bTODO\b"),  "TODO comment"),
    (re.compile(r"(?://|#[^!\[])[^\n]*\bFIXME\b"), "FIXME comment"),
    (re.compile(r"(?://|#[^!\[])[^\n]*\bHACK\b"),  "HACK comment"),
    (re.compile(r"(?://|#[^!\[])[^\n]*\bXXX\b"),   "XXX comment"),
    (re.compile(r"(?://|#[^!\[])[^\n]*\bSTUB\b"),  "STUB comment"),
    # Python: only a line whose sole content is `pass` (optionally with a trailing comment)
    (re.compile(r"^\s*pass\s*(?:#.*)?$"),           "pass (stub body)"),
    (re.compile(r"\braise\s+NotImplementedError"),  "raise NotImplementedError"),
    # TypeScript / JS
    (re.compile(r"\bthrow\s+new\s+Error\s*\(\s*['\"]not\s+implemented", re.IGNORECASE),
                                                    'throw new Error("not implemented")'),
    # Generic placeholder comment
    (re.compile(r"//\s*(stub|placeholder|not yet implemented)", re.IGNORECASE), "stub/placeholder comment (//)"),
    (re.compile(r"#\s*(stub|placeholder|not yet implemented)",  re.IGNORECASE), "stub/placeholder comment (#)"),
]


def _find_stubs(source: str) -> list[tuple[int, str, str]]:
    """Return list of (line_number, label, full_line) tuples."""
    results = []
    for lineno, raw_line in enumerate(source.splitlines(), start=1):
        for pat, label in _STUB_PATTERNS:
            if pat.search(raw_line):
                results.append((lineno, label, raw_line.rstrip()))
                break  # one report per line
    return results


def _safe_folder_name(file_path: str) -> str:
    """Convert a relative file path to a collision-free directory name.

    Replaces path separators with '__' and appends a short SHA-1 of the original
    path so that 'src/foo/bar.rs' and 'src/foo__bar.rs' never collide.
    """
    flat = file_path.replace("/", "__").replace("\\", "__")
    # usedforsecurity=False: this hash is used for collision-avoidance only,
    # not for any cryptographic purpose. Required to avoid ValueError in FIPS mode.
    suffix = hashlib.sha1(file_path.encode(), usedforsecurity=False).hexdigest()[:8]
    return f"{flat}__{suffix}"


def _write_report(folder: Path, filename: str, header: str, items: list[tuple[int, str, str]]) -> None:
    lines = [f"# {header}", ""]
    if not items:
        lines.append("_None found._")
    else:
        for lineno, label, code in items:
            lines.append(f"- **Line {lineno}** `{label}`")
            lines.append("")
            lines.append("  ```")
            # Use the raw line without extra indent prefix so rendered indentation
            # matches the source exactly. The 2-space list-item indent on the fence
            # itself (above) is sufficient for CommonMark block nesting.
            lines.append(code.rstrip())
            lines.append("  ```")
            lines.append("")
    lines.append("")
    (folder / filename).write_text("\n".join(lines), encoding="utf-8")


def step4_scan_marked_files() -> None:
    print("\n── Step 4: scanning marked files ────────────────────────────────")

    if not CODEBASE_LIST.exists():
        die("codebaselist.md not found. Run step 3 first.")

    text     = CODEBASE_LIST.read_text(encoding="utf-8")
    old_sha, entries = _parse_codebaselist(text)
    entry_map: dict[str, bool] = dict(entries)

    marked_files = [p for p, marked in entry_map.items() if marked]

    if not marked_files:
        print("  No marked files — nothing to scan.")
        return

    print(f"  {len(marked_files)} file(s) to scan.")
    total_magic = 0
    total_stubs = 0
    processed = 0  # files actually scanned (excludes gone/unreadable)

    for rel_path in marked_files:
        abs_path = REPO_ROOT / rel_path
        if not abs_path.exists():
            print(f"  ⚠ File gone from disk: {rel_path} — removing mark.")
            entry_map[rel_path] = False
            continue

        try:
            source = abs_path.read_text(encoding="utf-8", errors="replace")
        except OSError as exc:
            print(f"  ⚠ Cannot read {rel_path}: {exc} — unmarking to prevent infinite retry.")
            entry_map[rel_path] = False
            continue

        folder_name = _safe_folder_name(rel_path)
        out_dir = SWEEPFIX_DIR / folder_name
        out_dir.mkdir(parents=True, exist_ok=True)

        magic = _find_magic_numbers(source)
        stubs = _find_stubs(source)

        _write_report(out_dir, "magicnumbers.md", f"Magic Numbers — {rel_path}", magic)
        _write_report(out_dir, "stubs.md",        f"Stubs — {rel_path}",         stubs)

        total_magic += len(magic)
        total_stubs += len(stubs)

        entry_map[rel_path] = False  # unmark
        processed += 1

        status = f"  ✓ {rel_path}"
        extras = []
        if magic:
            extras.append(f"{len(magic)} magic #s")
        if stubs:
            extras.append(f"{len(stubs)} stubs")
        if extras:
            status += f"  ({', '.join(extras)})"
        print(status)

    # Persist updated list (marks cleared).
    # Re-read the current SHA from the file rather than relying on the variable
    # name "old_sha" which is misleading after step3 has already updated it.
    current_sha = current_commit()
    _write_codebaselist(current_sha, list(entry_map.items()))
    print(
        f"\n  Done. Total: {total_magic} magic numbers, "
        f"{total_stubs} stubs across {processed} file(s)."
    )
    print(f"  Reports written to: {SWEEPFIX_DIR}/")


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

def main() -> None:
    # Resolve REPO_ROOT here so importing this module outside a git repo does not
    # crash at import time with a bare CalledProcessError.
    global REPO_ROOT, SWEEPFIX_DIR, CODEBASE_LIST
    try:
        REPO_ROOT = Path(
            subprocess.check_output(
                ["git", "rev-parse", "--show-toplevel"], text=True
            ).strip()
        )
    except subprocess.CalledProcessError:
        print("❌  Not inside a git repository. Aborting.", file=sys.stderr)
        sys.exit(1)
    SWEEPFIX_DIR = REPO_ROOT / ".sweepfix"
    CODEBASE_LIST = SWEEPFIX_DIR / "codebaselist.md"

    print("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
    print("  sweepfix — magic-number & stub sweeper")
    print("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")

    step1_create_sweepfix()
    step2_commit_push_sync()
    step3_build_codebaselist()
    step4_scan_marked_files()

    print("\n✅  sweepfix complete.\n")


if __name__ == "__main__":
    main()
