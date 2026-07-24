#!/usr/bin/env python3
"""
sweepfix.py — Codebase magic-number & stub sweeper.

Steps:
  1. Create .sweepfix/ and gitignore it (skip if already exists).
  2. Commit & push everything; sync with remote; verify in-sync.
  3. Scan codebase, build codebase.toml (or diff-update existing one).
  4. For each marked file: find magic numbers + stubs; store findings; unmark.

Known limitation: numbers inside /* ... */ block comments whose body lines do
not start with a leading '*' are not filtered from magic-number detection.
Single-pass line scanning cannot track block-comment state.
"""

from __future__ import annotations

import re
import sys
import subprocess
from pathlib import Path

try:
    import tomllib
except ImportError:
    try:
        import tomli as tomllib  # type: ignore[no-reuse-local]
    except ImportError:
        sys.exit(
            "tomllib not found. Requires Python 3.11+ or `pip install tomli`."
        )

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

# Resolved inside main() so importing this module outside a git repo does not
# immediately crash with a CalledProcessError at import time.
REPO_ROOT: Path
SWEEPFIX_DIR: Path
CODEBASE_TOML: Path

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
        content = gitignore_path.read_text(encoding="utf-8")
        if marker not in content:
            with gitignore_path.open("a", encoding="utf-8") as fh:
                if not content.endswith("\n"):
                    fh.write("\n")
                fh.write("\n# sweepfix scratch folder\n")
                fh.write(f"{marker}\n")
            print("  Added .sweepfix/ to .gitignore")
        else:
            print("  .sweepfix/ already in .gitignore — skipping.")
    else:
        gitignore_path.write_text(f"# sweepfix scratch folder\n{marker}\n", encoding="utf-8")
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
    # --verify exits non-zero and produces empty stdout when the ref does not
    # exist. Plain `git rev-parse <ref>` echoes the ref name to stdout on
    # failure (exit 128), which would cause `if not remote_sha` to be False
    # and fall through to a confusing "not in sync" error instead of the
    # correct "remote not found" message.
    remote_sha = git("rev-parse", "--verify", f"refs/remotes/origin/{branch}", check=False)

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
# Step 3 — build / update codebase.toml
# ---------------------------------------------------------------------------

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
    """Return all files tracked by git (relative to repo root).

    Uses -z (NUL-separated output) to correctly handle filenames that contain
    spaces, non-ASCII characters, or other special characters that git would
    otherwise quote/escape in plain newline output.
    """
    raw = git("ls-files", "-z")
    return [Path(p) for p in raw.split("\x00") if p]


def _changed_files_between(old_commit: str, new_commit: str) -> tuple[list[str], list[str], list[str]]:
    """Return (added, modified, deleted) relative paths.

    Uses -z (NUL-separated output) so that filenames with spaces or non-ASCII
    characters are not quoted/escaped by git.

    git diff --name-status -z output format (NUL-separated tokens):
      A  -> "A" NUL "path" NUL
      M  -> "M" NUL "path" NUL
      D  -> "D" NUL "path" NUL
      R<score> -> "R<score>" NUL "old" NUL "new" NUL
      C<score> -> "C<score>" NUL "old" NUL "new" NUL
    """
    raw = git("diff", "--name-status", "-z", old_commit, new_commit)
    tokens = [t for t in raw.split("\x00") if t]
    added, modified, deleted = [], [], []
    i = 0
    while i < len(tokens):
        status = tokens[i].strip()
        if not status:
            i += 1
            continue
        if status.startswith(("R", "C")):
            if i + 2 < len(tokens):
                deleted.append(tokens[i + 1])  # old name is gone
                added.append(tokens[i + 2])    # new name is new
                i += 3
            else:
                i += 1  # malformed, skip
        elif status.startswith("A"):
            if i + 1 < len(tokens):
                added.append(tokens[i + 1])
            i += 2
        elif status.startswith("M"):
            if i + 1 < len(tokens):
                modified.append(tokens[i + 1])
            i += 2
        elif status.startswith("D"):
            if i + 1 < len(tokens):
                deleted.append(tokens[i + 1])
            i += 2
        else:
            i += 2  # unknown status, skip
    return added, modified, deleted


# Compiled once at module level so _toml_escape does not re-create it on every call.
# Matches TOML-illegal control characters that are not covered by named escapes
# (\n, \r, \t): U+0000–U+0008, U+000B–U+000C, U+000E–U+001F, U+007F, U+0080–U+009F.
_CTRL_CHAR_RE = re.compile(r"[\x00-\x08\x0b\x0c\x0e-\x1f\x7f\x80-\x9f]")


def _escape_ctrl(m: re.Match) -> str:  # type: ignore[type-arg]
    """Replacement callback for _CTRL_CHAR_RE — emits a TOML \\uXXXX escape."""
    return f"\\u{ord(m.group()):04X}"


def _toml_escape(s: str) -> str:
    """Escape a string value for embedding between double quotes in TOML.

    TOML basic strings (§2.4) forbid all C0 and C1 control characters except
    the three that have named escapes (\\t, \\n, \\r).  Characters like NUL,
    BS, VT, FF, ESC, DEL, and the C1 block are also illegal unescaped.
    We use the universal TOML \\uXXXX escape for the full illegal range.

    Processing order: backslash first to avoid double-escaping.
    """
    # Named escapes (order matters: backslash must be first)
    s = s.replace("\\", "\\\\")
    s = s.replace('"',  '\\"')
    s = s.replace("\n", "\\n")
    s = s.replace("\r", "\\r")
    s = s.replace("\t", "\\t")
    s = _CTRL_CHAR_RE.sub(_escape_ctrl, s)
    return s


def _parse_codebase_toml(text: str) -> tuple[str, list[tuple[str, bool, list[dict]]]]:
    """
    Parse codebase.toml.
    Returns (commit_sha, [(rel_path, marked, findings), ...])

    Each finding dict has keys:
      line: int, kind: str ("magic_number" | "stub"),
      value: str  (magic_number only),
      label: str  (stub only),
      context: str
    """
    try:
        data = tomllib.loads(text)
    except Exception as exc:
        print(f"  ⚠ codebase.toml parse error: {exc} — treating as empty.")
        return "", []

    commit_sha = data.get("commit", "")
    if not isinstance(commit_sha, str):
        commit_sha = ""

    entries: list[tuple[str, bool, list[dict]]] = []
    seen: set[str] = set()

    for file_entry in data.get("file", []):
        if not isinstance(file_entry, dict):
            continue
        path = file_entry.get("path", "")
        if not isinstance(path, str) or not path:
            continue
        if path in seen:
            print(f"  ⚠ Duplicate path in codebase.toml: {path!r} — keeping first occurrence.")
            continue
        seen.add(path)

        marked = bool(file_entry.get("marked", False))

        raw_findings = file_entry.get("finding", [])
        findings: list[dict] = []
        if isinstance(raw_findings, list):
            for f in raw_findings:
                if isinstance(f, dict):
                    findings.append(f)

        entries.append((path, marked, findings))

    return commit_sha, entries


def _write_codebase_toml(
    commit_sha: str,
    entries: list[tuple[str, bool, list[dict]]],
) -> None:
    """Serialise entries to .sweepfix/codebase.toml.

    Hand-written for the fixed schema — no third-party TOML writer needed.
    entries is [(path, marked, findings), ...], sorted by path on write.
    """
    lines: list[str] = []
    lines.append(f'commit = "{_toml_escape(commit_sha)}"')
    lines.append("")

    for path, marked, findings in sorted(entries, key=lambda x: x[0]):
        lines.append("[[file]]")
        lines.append(f'path = "{_toml_escape(path)}"')
        lines.append(f"marked = {'true' if marked else 'false'}")

        for finding in findings:
            lines.append("")
            lines.append("  [[file.finding]]")
            # Guard all key accesses: a finding dict parsed from a hand-edited or
            # externally-produced codebase.toml may be missing 'line' or 'context'.
            lines.append(f"  line = {int(finding.get('line', 0))}")
            lines.append(f'  kind = "{_toml_escape(str(finding.get("kind", "unknown")))}"')
            if finding.get("value") is not None:
                lines.append(f'  value = "{_toml_escape(str(finding["value"]))}"')
            if finding.get("label") is not None:
                lines.append(f'  label = "{_toml_escape(str(finding["label"]))}"')
            lines.append(f'  context = "{_toml_escape(str(finding.get("context", "")))}"')

        lines.append("")

    CODEBASE_TOML.write_text("\n".join(lines), encoding="utf-8")


def _build_codebaselist_fresh(current_sha: str, *, rebuild: bool = False) -> None:
    """Scan all tracked files and write a fresh codebase.toml."""
    all_files = _all_tracked_files()
    entries: list[tuple[str, bool, list[dict]]] = []
    skipped = 0
    for f in all_files:
        if _passes_gating(f):
            entries.append((str(f), True, []))
        else:
            skipped += 1
    _write_codebase_toml(current_sha, entries)
    label = "Rebuilding" if rebuild else "First run"
    print(f"  {label} — {len(entries)} files added to list ({skipped} skipped).")


def step3_build_codebaselist() -> None:
    print("\n── Step 3: codebase.toml ────────────────────────────────────────")

    current_sha = current_commit()

    if not CODEBASE_TOML.exists():
        _build_codebaselist_fresh(current_sha)
        return

    # Existing list — compare commits
    text = CODEBASE_TOML.read_text(encoding="utf-8")
    old_sha, existing = _parse_codebase_toml(text)

    if not old_sha or not _SHA_RE.match(old_sha):
        print(f"  ⚠ codebase.toml has invalid/missing commit SHA ({old_sha!r}) — rebuilding from scratch.")
        CODEBASE_TOML.unlink()
        _build_codebaselist_fresh(current_sha, rebuild=True)
        return

    if old_sha == current_sha:
        print(f"  List is up to date at {current_sha[:12]} — no changes needed.")
        return

    print(f"  List commit : {old_sha[:12]}")
    print(f"  Repo commit : {current_sha[:12]}")
    print("  Diffing...")

    try:
        added_paths, modified_paths, deleted_paths = _changed_files_between(old_sha, current_sha)
    except subprocess.CalledProcessError as exc:
        if exc.returncode == 128:
            # SHA is syntactically valid but does not exist in this repo
            # (e.g. codebase.toml was copied from another machine).
            print(f"  ⚠ Commit {old_sha[:12]} not found in this repo — rebuilding from scratch.")
            CODEBASE_TOML.unlink()
            _build_codebaselist_fresh(current_sha, rebuild=True)
            return
        raise

    entry_map: dict[str, tuple[bool, list[dict]]] = {
        path: (marked, findings) for path, marked, findings in existing
    }

    # Deleted — remove from list
    removed = 0
    for p in deleted_paths:
        if p in entry_map:
            del entry_map[p]
            removed += 1

    # Modified — mark as needing re-scan; clear stale findings
    remarked = 0
    for p in modified_paths:
        if p in entry_map:
            entry_map[p] = (True, [])
            remarked += 1

    # Added — add if they pass gating, mark them
    new_added = 0
    for p in added_paths:
        rel = Path(p)
        if _passes_gating(rel):
            entry_map[p] = (True, [])
            new_added += 1

    _write_codebase_toml(
        current_sha,
        [(path, marked, findings) for path, (marked, findings) in entry_map.items()],
    )
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
    "0", "1", "-1", "2", "3", "4", "5", "6", "7", "8",
    "9", "10", "11", "12", "15", "16", "20", "24", "30", "32",
    "60", "64", "90", "100", "128", "256", "512", "1024",
    "0.0", "1.0", "-1.0", "0.5", "2.0", "0.0_f32", "0.0_f64",
    "0u8",  "0u16",  "0u32",  "0u64",  "0usize",  "0i32",  "0i64",
    "1u8",  "1u16",  "1u32",  "1u64",  "1usize",  "1i32",  "1i64",
    "2u8",  "2u16",  "2u32",  "2u64",  "2usize",
    "4u8",  "4u16",  "4u32",  "4u64",  "4usize",
    "8u8",  "8u16",  "8u32",  "8u64",  "8usize",
    "16u8", "16u16", "16u32", "16u64", "16usize",
    "32u8", "32u16", "32u32", "32u64", "32usize",
    "64u8", "64u16", "64u32", "64u64", "64usize",
    # HTTP status codes — universally understood, never magic
    "200", "201", "202", "204", "301", "302", "304",
    "400", "401", "403", "404", "405", "409", "410", "422", "429",
    "500", "501", "502", "503", "504",
}

# Line-level patterns that disqualify the *entire line* from magic-number reporting.
# Order matters: each pattern is tried in sequence; first match skips the line.
_MAGIC_LINE_SKIP_RE: list[re.Pattern[str]] = [
    # Named constant / static declarations — these ARE the fix for magic numbers
    re.compile(r"^\s*(pub\s+)?(const|static)\s+\w+"),
    # UPPER_SNAKE_CASE assignment at line start — named constant in Python/JS
    re.compile(r"^\s*[A-Z][A-Z0-9_]{2,}\s*=\s*[\d.]"),
    # assert / assert_eq / assert_ne / assert_matches — test expected values
    re.compile(r"\bassert(?:_eq|_ne|_matches|_approx_eq)?\s*[!!(]"),
    # SQL positional placeholders ($1, $2 …) anywhere on the line
    re.compile(r"\$\d+"),
    # SQL LIMIT / OFFSET clauses
    re.compile(r"\bLIMIT\s+\d|\bOFFSET\s+\d", re.IGNORECASE),
    # Datetime constructor calls — year/month/day/hour/min/sec are not magic
    re.compile(r"\b(?:with_ymd_and_hms|ymd_hms|NaiveDate|NaiveDateTime|NaiveTime|DateTime)\s*[:(]"),
    # Hex byte-array lines (≥3 hex literals separated by commas)
    re.compile(r"(?:0[xX][0-9a-fA-F_]+,?\s*){3,}"),
    # Decimal byte-array lines (≥4 small integers separated by commas on one line)
    re.compile(r"(?:\b\d{1,3}\b,\s*){4,}"),
    # Named struct / tuple-struct field initialisation  (field: <value>)
    re.compile(r"\b\w{2,}\s*:\s*[-\d]"),
    # Duration / sleep / delay / timeout constructors
    re.compile(r"\b(?:Duration|Instant|Delay|sleep|timeout|interval)\s*(?:::|\()"),
    # timeout= / timeout_secs= / timeout_ms= as keyword arg or field
    re.compile(r"\btimeout(?:_secs|_ms|_s)?\s*[=:]"),
    # wait_for* / waitFor* calls (Playwright, tokio, etc.)
    re.compile(r"\bwait_for\b|\bwaitFor\b|\bwaitUntil\b"),
    # setTimeout / setInterval / refetchInterval (JS)
    re.compile(r"\b(?:setTimeout|setInterval|refetchInterval)\s*[=(,]"),
    # perf_counter / monotonic — raw timing * 1000 ms conversions
    re.compile(r"\bperf_counter\b|\bmonotonic\(\)"),
    # * 1000 ms / secs conversions
    re.compile(r"\* 1000\b|secs\s*\*\s*1000|timestamp_millis"),
    # expires_in / expires_at (OAuth token lifetimes)
    re.compile(r"\bexpires_(?:in|at)\b"),
    # Easing / interpolation / animation frame ranges
    re.compile(r"\b(?:interpolate|bezier|Easing|fps\b)"),
    # Viewport / screen dimensions
    re.compile(r"\bviewport\b|\bwidth.*height\b|\bheight.*width\b", re.IGNORECASE),
    # render_to_buffer / render_markdown width args
    re.compile(r"\brender_(?:to_buffer|markdown)\b"),
    # ts() wrapper — unix timestamp passed to test helpers
    re.compile(r"\bts\s*\(\s*\d"),
    # Bit-mask / flags lines
    re.compile(r"(?:byte|flags?|mask|bits?)\s*[&|]|[&|]\s*0[xX]"),
    # Unix-epoch literal (10 digit, starts 1xxx_xxx_xxx)
    re.compile(r"\b1[0-9_]{9,12}\b"),
    # file permission octal literals
    re.compile(r"\b0o[0-7]+\b"),
    # IPv4 constructor (Ipv4Addr::new / SocketAddr::from)
    re.compile(r"\bIpv4Addr\b|\bSocketAddr\b|\bIpAddr\b"),
    # Network port numbers on lines that mention "port"
    re.compile(r"\bport\b", re.IGNORECASE),
    # match arm  N => "string"  (enum/class-code tables)
    re.compile(r"\b\d+\s*=>"),
    # loop / range constructs  for _ in 0..N  or  i < N;
    re.compile(r"\bfor\b.+\b0\.\.|i\s*<\s*\d+\s*[;)]"),
    # .repeat(N) — string/vec padding in tests
    re.compile(r"\.repeat\s*\(\s*\d"),
    # row.get(N) — SQL column index access
    re.compile(r"\brow\.get\s*\(\s*\d"),
    # ASCII character code comments  ch === 60 /* < */
    re.compile(r"/\*\s*[<>=!&|^]\s*\*/"),
    # .clamp(lo, hi) — both bounds contextually obvious
    re.compile(r"\.clamp\s*\("),
    # range checks  (N..=M).contains
    re.compile(r"\(\d+\.\.=\d+\)\.contains"),
    # CSS/Tailwind opacity suffix on colour hex  /40  /10
    re.compile(r"[0-9a-fA-F]{3,6}/\d+"),
    # wrapping_mul / wrapping_add — intentional overflow constants (LCG etc.)
    re.compile(r"\bwrapping_(?:mul|add|sub)\b"),
    # wait_for_timeout(N) / waitForTimeout(N) — Playwright positional ms arg
    re.compile(r"\bwait_for_timeout\s*\(|\bwaitForTimeout\s*\("),
    # dec!(N) — Decimal cost/rate literals, always named by context
    re.compile(r"\bdec!\s*\("),
    # Some(N) / Ok(N) wrapping a single literal — field value, not magic
    re.compile(r"\b(?:Some|Ok)\s*\(\s*[-\d]"),
    # matches!(expr, N..=M | N) — range / pattern match guards
    re.compile(r"\bmatches!\s*\("),
    # math operations that name the constant's role: *3600, /60, %60, %24
    re.compile(r"[%*/]\s*(?:60|3600|86400|24|1000|1024)\b"),
    # colour component assignment  r: N  g: N  b: N  a: N
    re.compile(r"\b[rgba]\s*:\s*\d"),
    # terminal / buffer geometry  Rect::new(x, y, w, h)
    re.compile(r"\bRect\s*(?:::new)?\s*\("),
    # status-code range checks  500..=599  200..=299
    re.compile(r"\b[1-5]\d\d\.\.=[1-5]\d\d\b"),
    # version number lines  >=1.73  v2.1  3.2:
    re.compile(r">=?\s*\d+\.\d+|\bv\d+\.\d+|\b\d+\.\d+:"),
    # test fixture bare positional integers followed by , or ) with no operator
    # e.g.  replay_with_activity_thread(&scope, 11, 11, "thread-b")
    # Heuristic: integer immediately surrounded by ,/( on both sides
    re.compile(r"(?:,\s*)\d+(?:\s*[,)])"),
]


# Matches comment-only lines across Rust/Python/JS/C — but NOT Rust attributes (#[...]).
# Also matches the closing '*/' line. Lines inside /* ... */ without a leading '*'
# are not filtered — see the module docstring for this known limitation.
_COMMENT_ONLY_LINE_RE = re.compile(r"^\s*(//|/\*|\*/|\*(?!/)|#(?!\[))")

# Compiled once for the Rust-lifetime heuristic used inside _scan_string_state.
# prev_ns char set: word char, '<', '&', '+', ',' — all positions where 'ident appears.
# next char: must be a word char (the first letter of the lifetime name).
_LIFETIME_PREV_RE = re.compile(r"[\w<&+,]")
_LIFETIME_NEXT_RE = re.compile(r"\w")


def _scan_string_state(line: str, stop: int) -> tuple[bool, bool]:
    """Walk *line* from 0 up to (not including) *stop*, tracking string state.

    Returns ``(in_double, in_single)`` — the string-delimiter state at position
    *stop*.  This is the single source of truth for quote-tracking logic shared
    by :func:`_strip_inline_comment` and :func:`_is_in_string`.

    **Quote rules (same for both callers):**

    * ``"`` — toggles ``in_double`` when the number of immediately-preceding
      backslashes is even (an odd run means the quote itself is escaped).
      Only processed when ``not in_single``.
    * ``'`` — opens ``in_single`` when ``not in_double`` AND the token does not
      look like a Rust lifetime.  A ``'`` closes ``in_single`` when already
      inside one (no heuristic applied to the closing quote).

    **Rust lifetime heuristic:** a ``'`` is treated as a lifetime opener — and
    therefore does NOT open a string — when the previous *non-space* character
    matches ``[\\w<&+,]`` AND the next character is a word character.  This
    covers all Rust lifetime syntactic positions: ``<'a>``, ``&'a``, ``+'a``,
    ``,'b``, and ``T + 'a`` (with a space before the apostrophe).
    """
    in_double = False
    in_single = False
    i = 0
    while i < stop:
        c = line[i]

        if c == '"' and not in_single:
            num_bs = 0
            j = i - 1
            while j >= 0 and line[j] == "\\":
                num_bs += 1
                j -= 1
            if num_bs % 2 == 0:
                in_double = not in_double

        elif c == "'" and not in_double:
            num_bs = 0
            j = i - 1
            while j >= 0 and line[j] == "\\":
                num_bs += 1
                j -= 1
            if num_bs % 2 == 0:
                if in_single:
                    in_single = False
                else:
                    # Rust lifetime heuristic: look at the previous non-space char.
                    k = i - 1
                    while k >= 0 and line[k] == " ":
                        k -= 1
                    prev_ns = line[k] if k >= 0 else ""
                    next_c  = line[i + 1] if i + 1 < len(line) else ""
                    if not (_LIFETIME_PREV_RE.match(prev_ns) and _LIFETIME_NEXT_RE.match(next_c)):
                        in_single = True
        i += 1
    return in_double, in_single


def _strip_inline_comment(line: str) -> str:
    """Strip trailing ``//`` and ``#`` inline comments, respecting string literals.

    Scans the line for comment markers (``//``, ``#``) by checking the string
    state *before* each character using :func:`_scan_string_state`.  Lines are
    typically ≤200 characters so the per-character helper call is negligible.
    """
    for i, c in enumerate(line):
        in_double, in_single = _scan_string_state(line, i)
        if not in_double and not in_single:
            if line[i : i + 2] == "//":
                return line[:i]
            if c == "#":
                return line[:i]
    return line


def _is_in_string(line: str, pos: int) -> bool:
    """Return ``True`` if character at *pos* in *line* is inside a string literal.

    Delegates entirely to :func:`_scan_string_state`.  The two functions share
    the exact same quote-tracking logic so they can never diverge.
    """
    in_double, in_single = _scan_string_state(line, pos)
    return in_double or in_single


def _find_magic_numbers(source: str) -> list[tuple[int, str, str]]:
    """Return list of (line_number, literal, full_line) tuples."""
    results = []
    for lineno, raw_line in enumerate(source.splitlines(), start=1):
        # Skip pure comment lines (// ... # ... /* ... * ...) but NOT Rust #[attributes]
        if _COMMENT_ONLY_LINE_RE.match(raw_line):
            continue
        # Skip entire lines that match a disqualifying context pattern.
        if any(p.search(raw_line) for p in _MAGIC_LINE_SKIP_RE):
            continue
        # Strip trailing inline comment before scanning so numbers inside comment
        # tails (e.g. "let x = 9000; // was 42") are not falsely reported.
        scan_line = _strip_inline_comment(raw_line)
        for m in _MAGIC_NUMBER_RE.finditer(scan_line):
            lit = m.group("lit")
            if lit in _TRIVIAL_NUMBERS:
                continue
            # Skip if the literal is inside a string (either ' or ").
            # Uses _is_in_string which shares the exact same state-machine as
            # _strip_inline_comment — handles escaped quotes and Rust lifetimes.
            if _is_in_string(scan_line, m.start()):
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


def step4_scan_marked_files() -> None:
    print("\n── Step 4: scanning marked files ────────────────────────────────")

    if not CODEBASE_TOML.exists():
        die("codebase.toml not found. Run step 3 first.")

    text = CODEBASE_TOML.read_text(encoding="utf-8")
    _, entries = _parse_codebase_toml(text)
    entry_map: dict[str, tuple[bool, list[dict]]] = {
        path: (marked, findings) for path, marked, findings in entries
    }

    marked_files = [p for p, (marked, _findings) in entry_map.items() if marked]

    if not marked_files:
        print("  No marked files — nothing to scan.")
        return

    print(f"  {len(marked_files)} file(s) to scan.")
    total_magic = 0
    total_stubs = 0
    processed = 0  # files actually scanned (excludes gone/unreadable)

    for rel_path in marked_files:
        abs_path = REPO_ROOT / rel_path
        # Guard against path traversal: a tampered codebase.toml could contain
        # relative paths like "../../etc/passwd". Resolve symlinks and verify the
        # result is inside REPO_ROOT before reading anything.
        if not abs_path.resolve().is_relative_to(REPO_ROOT.resolve()):
            print(f"  ⚠ Path escapes repository root: {rel_path!r} — skipping for safety.")
            entry_map[rel_path] = (False, [])
            continue
        if not abs_path.exists():
            print(f"  ⚠ File gone from disk: {rel_path} — removing mark.")
            entry_map[rel_path] = (False, [])
            continue

        try:
            source = abs_path.read_text(encoding="utf-8", errors="replace")
        except OSError as exc:
            print(f"  ⚠ Cannot read {rel_path}: {exc} — unmarking to prevent infinite retry.")
            entry_map[rel_path] = (False, [])
            continue

        magic = _find_magic_numbers(source)
        stubs = _find_stubs(source)

        findings: list[dict] = []
        for lineno, value, context in magic:
            findings.append({
                "line":    lineno,
                "kind":    "magic_number",
                "value":   value,
                "context": context,
            })
        for lineno, label, context in stubs:
            findings.append({
                "line":    lineno,
                "kind":    "stub",
                "label":   label,
                "context": context,
            })

        total_magic += len(magic)
        total_stubs += len(stubs)

        entry_map[rel_path] = (False, findings)  # unmark, store findings
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

    # Persist updated list (marks cleared, findings embedded).
    # Re-read the current SHA: step3 has already updated it, "old_sha" is stale.
    current_sha = current_commit()
    _write_codebase_toml(
        current_sha,
        [(path, marked, findings) for path, (marked, findings) in entry_map.items()],
    )
    print(
        f"\n  Done. Total: {total_magic} magic numbers, "
        f"{total_stubs} stubs across {processed} file(s)."
    )
    print(f"  Results written to: {CODEBASE_TOML}")


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

def main() -> None:
    # Resolve REPO_ROOT here so importing this module outside a git repo does not
    # crash at import time with a bare CalledProcessError.
    global REPO_ROOT, SWEEPFIX_DIR, CODEBASE_TOML
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
    CODEBASE_TOML = SWEEPFIX_DIR / "codebase.toml"

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
