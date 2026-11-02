# target/validation evidence policy — K3 cleanup and forward runner spec

> **Status:** 2026-11-02. Plan-only document. No code or evidence is modified by
> this file. Every step below is actionable as a discrete, sequenced work item.
> The leading Root/Auth chat owns execution under the existing Cargo serial lock
> (`/private/tmp/brassclaw-target-cargo.lock`) and central build ownership rules
> from `docs/development-policy.md` §"Shared source and central build ownership".

---

## 0. Current state and motivation

### 0.1 Disk pressure (active)

`/Volumes/SSDE` (the only disk that holds `target/validation/`) is at **97%**
capacity — 16 GiB free as of 2026-11-02. `AGENTS.md` mandates selective cleanup
of **disposable build artifacts** when free space falls below 15 GiB or usage
exceeds 90%. That threshold is currently breached. The first two cleanup steps
below are mandatory now, not optional future maintenance.

### 0.2 Two historical evidence patterns

**Pattern A — old-style full-tree snapshot (2026-10-11 through 2026-10-22):**

Every validation run produced two peer directories at `target/validation/`:

```
target/validation/<work-package>-<date>-r<N>/          ← full working-tree copy
target/validation/<work-package>-artifacts-<date>-r<N>/  ← compiled binaries + receipt
```

The source directory is a complete copy of the entire repository at run time
(~54 top-level entries, ~220–240 MB per run). It contains `AGENTS.md`,
`Cargo.toml`, `crates/`, `docs/`, `path_to_v3.5.md`, all `.md` plan files, and
all other git-tracked content — including files identical across every run.
The artifacts directory contains the compiled test binaries (200–260 MB each)
plus receipt files (variously named `brassclaw-<pkg>-accepted.json`,
`failed-acceptance.json`, etc. — no consistent bare filename).

These directories are the source of the path divergence described in the prior
analysis: `target/validation/child-parent-failure-20261022-r1/path_to_v3.5.md`
is the state of that file at run time, not the current canonical version.

**Pattern B — new-style run-N with source manifest (2026-10-26 onward):**

```
target/validation/<work-package>-<date>/
    run-1/  ...  run-N/           ← one subdirectory per attempt
        source.json              ← per-file SHA-256 manifest of all git-tracked + untracked
                                    (non-ignored) files; also stores "head" (git commit),
                                    "toolchain" (rustc version), "source" (repo root abs path),
                                    "target" (CARGO_TARGET_DIR abs path)
        applied.json             ← before/after SHA-256 for every changed file in the work
                                    package; also contains peer_path, peer_sha256 and run
                                    history fields
        owned-source/            ← file-level copies of only the changed files
        original-runner.py       ← the complete runner script used
        model-source.json        ← copy of the model artifact manifest (runner-specific;
                                    not present in all work packages)
        progress.json            ← live state written atomically after every step;
                                    remains present alongside completed.json at run end
        completed.json           ← written by the runner on clean exit; contains results,
                                    artifacts map and completed_at timestamp
        accepted.json            ← written by a separate post-run acceptance step (NOT by
                                    the runner itself); contains scope, source_manifest_sha256,
                                    applied_manifest_sha256, owned_paths, fresh_rust_passes,
                                    compiler_input_drift, nonembedded_document_drift, logs,
                                    artifacts, strict_lint_log_sha256, build_log_sha256,
                                    and a status string like "SCOPED_ACCEPTANCE_PASSED_FULL_PLANS_OPEN"
        <binary files>           ← compiled test executables (47–261 MB each)
        *.log                    ← per-step log files named after each command label
```

**Two distinct evidence trees exist under `target/`:**
- `target/validation/` — the runner output tree described above (this plan)
- `target/accepted-evidence/` — a separate tree for accepted-evidence packages
  (k3-*, k1-*, k2-*, prefix-*, etc.) that follows the same `run-N/` layout but
  is cited separately in `docs/validation/*.md`. This plan covers `target/validation/`
  only; `target/accepted-evidence/` is out of scope.

### 0.3 What is genuinely required evidence and what is disposable

**Required (must not be deleted):**

| Item | Why required |
|---|---|
| `accepted.json` (written by a separate post-run acceptance step, not the runner) | Machine receipt cited by path in `docs/validation/*.md` and `docs/plans/simplified-v3-implementation.md`; SHA-256 of the file itself is cited inline in those docs |
| Binary SHA-256 hashes *in* `accepted.json` | Cited inline in `.md` plan files as the identity of shipped executables; the hash is the immutable identity (tools.md §7) |
| `source.json` | Per-file SHA-256 manifest that IS the lightweight source-identity mechanism; `source_manifest_sha256` is cited in receipts |
| `original-runner.py` | The exact runner that produced the run; 12 KB; required for reproducibility audits |
| Named log files | Specific log file paths (e.g. `prefix-model-configuration.log`) are cited by path in validation docs; their SHA-256 is in `accepted.json` |
| `applied.json` | Documents exactly which files were changed; `applied_manifest_sha256` is in receipts |

**Disposable (safe to delete without evidence loss):**

| Item | Why disposable |
|---|---|
| Old-style full-tree source snapshots (`<work-package>-<date>-rN/`) | Superseded entirely by `source.json`; the per-file SHA-256 manifest captures source identity at zero MB overhead; the snapshot duplicates thousands of unchanged files across hundreds of runs |
| `owned-source/` subdirectories inside new-style runs | The 67 changed files are fully identified by `applied.json` SHA-256 hashes; `source.json` captures the full-tree identity; the copies add MB redundancy with zero additional evidence |
| Compiled binaries from **failed** runs | Their SHA-256 is in `failed.json`; no plan or doc cites those binary paths; they have no forward consumers |
| Compiled binaries from **superseded non-final runs** (run-1 through run-N-1 where run-N is the accepted run) | Evidence is carried by accepted run's `accepted.json`; intermediate run binaries are not cited |
| Old-style `*-artifacts-*` directories whose companion source snapshot is also being deleted | Receipt (`failed.json` / `accepted.json`) must be checked first; if no doc cites the artifacts directory by path, the binaries are disposable; the receipt file must be kept |

**This plan never deletes:** `accepted.json`, `failed.json`, `source.json`,
`applied.json`, `original-runner.py`, named log files, or the binaries from the
single **accepted** (final) run of each work package if that binary's SHA-256 is
cited in an external doc. Receipts from failed runs are kept for audit continuity.

### 0.4 Directory inventory (2026-11-02)

`target/validation/` contains **1,234 entries** total:

- ~641 old-style source-snapshot directories (Pattern A)
- ~589 old-style paired artifacts directories (Pattern A)
- 4 new-style work-package directories (Pattern B):
  - `auth-dcr-common-20261026/` (runs 1–10; run-10 is the accepted run)
  - `issuer-expiry-20261026/` (runs 1–8; run-8 is the accepted run)
  - `prefix-model-config-ui-aolgri5u/` and `prefix-model-config-ui-kk0ebg8y/`
    (contain `accepted.json` and a `source/` subdirectory directly — no `run-N/`
    nesting; different layout from the other two new-style packages)

The old-style directories span work packages from `2026-10-11` through
`2026-10-22`, covering work-packages including: `atomic-tree-cancellation`,
`bootstrap-migration`, `catalogue-policy`, `catalogue-eligibility`,
`cancel-preparation`, `canonical-model-definition`, `canonical-model`,
`catalogue-pin`, `accounting-representation`, `atomic-admission`,
`child-*` (17 variants), and others.

**Important:** Many old-style source snapshot directories are cited by name in
`docs/plans/simplified-v3-implementation.md` as "frozen source" or "retained source".
At least 18 such directories (including `pg-lifecycle-20261011-r4`,
`instance-policy-adapter-20261014-r3`, `native-startup-capacity-20261014-r4`,
`pending-seed-metadata-20261014-r2`, `explicit-installation-projection-20261014-r1`,
`preload-prefix-20261011-r2`, `secret-ceremony-20261011-r1`, `upstream-lock-20261011-r2`,
`catalogue-policy-20261012-r4`, and others spanning 2026-10-11 through 2026-10-22)
have `crates/` at their top level AND are cited in docs. These are protected under
Constraint 9 below and must not be deleted by Step 1's filter. See §1 for the
corrected guard.

---

## 1. Step 1 — Delete uncited old-style full-tree source snapshots (mandatory now)

**Why:** These are the largest single waste. Each directory is ~220 MB of
duplicated source content. They are the sole reason `path_to_v3.5.md` and other
plan files appear inside `target/validation/` with stale content. Many do not
satisfy any evidence requirement and are safe to delete. **However, some are
cited by name in docs as "frozen source" or "retained source" and must be
preserved.** The guard below includes both structural detection and a doc-citation
check.

**Two-pass approach:**

**Pass 1 — enumerate all Pattern A candidates (structural check):**

```bash
# Pattern A: has crates/ subdirectory AND AGENTS.md at top level
# Use ${d%/} to strip the trailing slash from the glob — NOT dirname, which would
# give the parent directory instead of the directory itself.
for d in /Volumes/SSDE/brassclaw/target/validation/*/; do
  d="${d%/}"   # strip trailing slash: "/foo/bar/" -> "/foo/bar"
  if [ -d "$d/crates" ] && [ -f "$d/AGENTS.md" ]; then
    echo "$d"
  fi
done | sort > /tmp/pattern-a-candidates.txt
wc -l /tmp/pattern-a-candidates.txt
```

**Pass 2 — subtract dirs cited in any doc:**

```bash
# Extract every target/validation/ path cited in docs (non-artifacts)
grep -roh "target/validation/[a-zA-Z0-9_-]*" \
  /Volumes/SSDE/brassclaw/docs/ \
  /Volumes/SSDE/brassclaw/*.md \
  2>/dev/null \
  | grep -v "\-artifacts" \
  | sed 's|.*target/validation/|target/validation/|' \
  | sort -u > /tmp/cited-source-dirs.txt

# Map to absolute paths
sed 's|target/validation/|/Volumes/SSDE/brassclaw/target/validation/|' \
  /tmp/cited-source-dirs.txt > /tmp/cited-absolute.txt

# Compute safe-to-delete set: in candidates but NOT in cited
comm -23 \
  <(sort /tmp/pattern-a-candidates.txt) \
  <(sort /tmp/cited-absolute.txt) \
  > /tmp/safe-to-delete.txt

wc -l /tmp/safe-to-delete.txt
head -20 /tmp/safe-to-delete.txt
```

Review `/tmp/safe-to-delete.txt`. Verify it contains zero entries also present
in `/tmp/cited-absolute.txt` (use exact full-path match, not basename, to avoid
substring collisions like `r1` matching `r10`):

```bash
while IFS= read -r d; do
  grep -qxF "$d" /tmp/cited-absolute.txt && echo "ERROR: $d is cited"
done < /tmp/safe-to-delete.txt
echo "Cross-check complete (no ERROR lines = safe to proceed)"
```

Then delete using full-path re-check against `cited-absolute.txt`:

```bash
while IFS= read -r d; do
  [ -d "$d" ] || continue
  # Final safety re-check: must still match Pattern A AND not appear in cited list
  if [ -d "${d}/crates" ] && [ -f "${d}/AGENTS.md" ]; then
    if ! grep -qxF "$d" /tmp/cited-absolute.txt; then
      rm -rf "$d"
    fi
  fi
done < /tmp/safe-to-delete.txt
```

**Validation:** after completion:
1. All directories in `/tmp/cited-absolute.txt` that existed before must still exist:
   ```bash
   while IFS= read -r d; do [ -d "$d" ] || echo "MISSING: $d"; done < /tmp/cited-absolute.txt
   ```
2. No uncited Pattern A directory remains. Use exact full-path match (`grep -qxF`) to avoid
   substring collisions (e.g. `-r1` matching `-r10`):
   ```bash
   for d in /Volumes/SSDE/brassclaw/target/validation/*/; do
     d="${d%/}"
     [ -d "$d/crates" ] || continue
     grep -qxF "$d" /tmp/cited-absolute.txt && continue   # cited — expected to remain
     echo "UNEXPECTED RESIDUAL: $d"
   done
   ```
   Must produce no output.

**Expected space reclaim:** ~220 MB × (641 − ~80 cited) ≈ 115–130 GB.
The ~80 cited directories with `crates/` are retained.

**Note on the old-style receipt naming convention:** the old-style artifacts
directories use non-standard receipt filenames (`brassclaw-<name>-accepted.json`,
`failed-acceptance.json`, etc.) rather than bare `accepted.json`/`failed.json`.
Step 2's `find ... ! -name "*.json"` filter preserves all of these correctly
because they all end in `.json`.

---

## 2. Step 2 — Remove binaries from failed/superseded runs in artifacts directories (mandatory now)

**Why:** Old-style `*-artifacts-*` directories contain large compiled test
binaries. Where the run failed (`failed.json` present, no doc path-cites the
binary), the binary has no evidence consumers. Where the run succeeded, the
SHA-256 hash is already recorded in `accepted.json`; re-execution from that
binary is not a standing requirement.

**Precondition check — before touching any artifacts directory:**

1. Read the `accepted.json` or `failed.json` inside it.
2. Grep all files under `docs/` and all plan `.md` files at the repo root for
   the exact directory path, e.g. `grep -r "child-parent-failure-artifacts" /Volumes/SSDE/brassclaw/docs/ /Volumes/SSDE/brassclaw/*.md`. If a binary file *path* is cited (not just its hash), that binary must be kept.
3. All receipt files (`accepted.json`, `failed.json`) must be **kept**.
4. All log files (`*.log`) cited by path in any doc must be **kept** (check
   `grep -r "<work-package>" /Volumes/SSDE/brassclaw/docs/` for path citations).

**Exact deletion pattern (binary files only, keep receipts and logs):**

```bash
# For each old-style artifacts directory:
ARTDIR="/Volumes/SSDE/brassclaw/target/validation/<work-package>-artifacts-<date>-r<N>"

# Keep: *.json, *.log, *.py, *.md, *.txt
# Delete: everything else (compiled ELF/Mach-O binaries, object files, etc.)
find "$ARTDIR" -maxdepth 1 -type f \
  ! -name "*.json" ! -name "*.log" ! -name "*.py" ! -name "*.md" ! -name "*.txt" \
  -delete
```

Run this for each artifacts directory after completing the precondition check.

**For new-style runs (Pattern B):** apply the same logic to `run-1` through
`run-(N-1)` within each work-package directory, keeping only the final accepted
run's binaries. The final run is the one containing `accepted.json`.

```bash
# Example for issuer-expiry-20261026:
PKGDIR="/Volumes/SSDE/brassclaw/target/validation/issuer-expiry-20261026"
ACCEPTED_RUN=$(ls -1 "$PKGDIR" | while read r; do [ -f "$PKGDIR/$r/accepted.json" ] && echo "$r"; done)
for run in $(ls -1 "$PKGDIR"); do
  if [ "$run" != "$ACCEPTED_RUN" ]; then
    # non-accepted run: delete binaries, keep receipts/logs
    find "$PKGDIR/$run" -maxdepth 1 -type f \
      ! -name "*.json" ! -name "*.log" ! -name "*.py" ! -name "*.md" ! -name "*.txt" \
      -delete
  fi
done
```

**Validation:** after Step 2, `df -h /Volumes/SSDE` must show ≥ 15 GiB free.
If it does not, proceed to Step 3 before continuing.

**Note on `accepted.json` vs non-standard filenames:** old-style artifacts
directories use naming conventions like `brassclaw-<pkg>-accepted.json` and
`failed-acceptance.json`. The `find ! -name "*.json"` filter preserves all of
these regardless of prefix. Do not assume a standard bare `accepted.json`
exists in every old-style artifacts directory — check with `ls` before
interpreting receipt status.

---

## 3. Step 3 — Remove `owned-source/` subdirectories from new-style runs

**Why:** Each `owned-source/` directory contains copies of only the changed
files for that run. Their content is fully identified by `applied.json`
(before/after SHA-256 per file) and `source.json` (full-tree SHA-256 + HEAD).
The copies serve no evidence function that the manifests do not already provide.

**Layout note:** `prefix-model-config-ui-aolgri5u` and `prefix-model-config-ui-kk0ebg8y`
do not use `run-N/` nesting — they contain `accepted.json` and a `source/`
subdirectory directly at the package root. The `owned-source/` pattern applies
only to the `run-N/`-based packages (`auth-dcr-common-20261026` and
`issuer-expiry-20261026`). The `source/` subdirectory in the prefix-model-config
packages has a different structure (a curated model artifact root, not a
changed-file copy) and must not be deleted here.

**Precondition:** confirm `applied.json` exists AND either `source.json` or
`owned-source.json` (an equivalent per-file manifest used by earlier runner
variants that did not write `source.json`) exists in the same `run-N/` directory
before deleting `owned-source/`.

```bash
for pkg in auth-dcr-common-20261026 issuer-expiry-20261026; do
  PKGDIR="/Volumes/SSDE/brassclaw/target/validation/$pkg"
  for run in $(ls -1 "$PKGDIR" 2>/dev/null); do
    RUNDIR="$PKGDIR/$run"
    [ -d "$RUNDIR/owned-source" ] || continue
    HAS_MANIFEST=0
    [ -f "$RUNDIR/source.json" ] && HAS_MANIFEST=1
    [ -f "$RUNDIR/owned-source.json" ] && HAS_MANIFEST=1
    if [ -f "$RUNDIR/applied.json" ] && [ "$HAS_MANIFEST" -eq 1 ]; then
      rm -rf "$RUNDIR/owned-source"
    else
      echo "SKIP $RUNDIR/owned-source — missing applied.json or per-file manifest"
    fi
  done
done
```

**Expected space reclaim:** several MB per run (small compared to Steps 1–2).

---

## 4. Step 4 — Forward runner spec: what the runner must write per run

This step specifies the **required and prohibited** outputs for every future
validation run. The runner (`original-runner.py` or its successor) must produce
exactly the layout below. `accepted.json` is written by a **separate post-run
acceptance step**, not by the runner itself.

### Required output layout for each `run-N/`

```
target/validation/<work-package>-<date>/run-<N>/
    source.json          REQUIRED  — per-file SHA-256 manifest. Written once at
                                     the start of the run before any Cargo execution.
                                     Never re-written after the run starts.
                                     Fields: "files" (dict path→sha256hex), "head"
                                     (git commit hash from rev-parse HEAD), "toolchain"
                                     (rustc --version string), "source" (absolute repo
                                     root path), "target" (CARGO_TARGET_DIR abs path).
                                     Enumerated via git ls-files --cached --others
                                     --exclude-standard, so includes tracked and
                                     non-ignored untracked files (but not target/ build
                                     output, which is gitignored).
    applied.json         REQUIRED  — copied from the external per-run applied-rN.json
                                     before any Cargo execution. Contains "applied"
                                     (dict path→sha256hex of the post-apply state) and
                                     "before" (dict of pre-apply hashes). Runner verifies
                                     every path in applied matches the current working tree.
    original-runner.py   REQUIRED  — verbatim copy of the runner script. Copied before
                                     any Cargo execution via shutil.copyfile(__file__, ...).
    progress.json        REQUIRED  — written atomically after every step via a .tmp rename.
                                     Remains present at run end alongside completed.json.
                                     Contains: status, runner_pid, results[], artifacts{},
                                     source, target, stage, process_pid.
    completed.json       REQUIRED  — written by the runner on clean successful exit.
                                     Contains same fields as final progress.json plus
                                     completed_at. Does NOT contain source_manifest_sha256,
                                     owned_paths or fresh_rust_passes — those are in
                                     accepted.json, written externally.
    accepted.json        REQUIRED  — written by a separate post-run acceptance step
                                     (not the runner). Contains: status string
                                     (e.g. "SCOPED_ACCEPTANCE_PASSED_FULL_PLANS_OPEN"),
                                     accepted_at, scope, source_root,
                                     source_manifest_sha256 (SHA-256 of source.json file),
                                     applied_manifest_sha256 (SHA-256 of applied.json file),
                                     owned_paths (list of changed file paths),
                                     compiler_input_drift, nonembedded_document_drift,
                                     fresh_rust_passes, results, logs, artifacts,
                                     strict_lint_log_sha256, build_log_sha256.
    *.log                REQUIRED  — one log file per command() call, named using the
                                     label argument (e.g. "strict-affected-tests.log",
                                     "auth-units.log", "prefix-model-configuration.log").
    <binary-name>        REQUIRED  — compiled test executable(s) copied from
                                     CARGO_TARGET_DIR by preserve() after --no-run build.
                                     Named as "test-<crate_name>" or "bin-<binary_name>".
                                     SHA-256 recorded in artifacts{} in progress/completed.
```

### Prohibited outputs

```
owned-source/            PROHIBITED — do not copy changed files; applied.json manifests suffice
<work-package>-rN/       PROHIBITED — do not create a full working-tree copy of the repo
crates/                  PROHIBITED — never copy crates/ into target/validation/
docs/                    PROHIBITED — never copy docs/ into target/validation/
AGENTS.md                PROHIBITED — never copy root docs into target/validation/
path_to_v3.5.md          PROHIBITED — never copy plan files into target/validation/
```

### source.json schema (normative, matches actual issuer-expiry/run-8/source.json)

```json
{
  "files": {
    "<relative/path/to/file>": "<64-char sha256hex>",
    ...
  },
  "head": "<40-char git commit SHA>",
  "source": "<absolute path to repo root>",
  "target": "<absolute path to CARGO_TARGET_DIR>",
  "toolchain": "rustc <version> (<hash> <date>)"
}
```

`files` is populated by `git ls-files -z --cached --others --exclude-standard`.
This includes tracked files AND non-ignored untracked files; `target/` build
output is excluded because `target/` is gitignored. The SHA-256 of `source.json`
itself is stored as `source_manifest_sha256` in `accepted.json`.

### accepted.json schema (normative, actual fields from issuer-expiry/run-8)

```json
{
  "status": "<scope-specific acceptance string>",
  "accepted_at": "<ISO-8601 timestamp>",
  "scope": "<work-package name>",
  "source_root": "<absolute path to repo root>",
  "source_manifest_sha256": "<sha256hex of source.json>",
  "applied_manifest_sha256": "<sha256hex of applied.json>",
  "owned_paths": ["<relative path>", ...],
  "compiler_input_drift": [],
  "nonembedded_document_drift": [],
  "fresh_rust_passes": [
    {
      "package": "<crate name>",
      "features": "<feature flags>",
      "filter": "<test filter or 'all'>",
      "result": "pass",
      "binary_sha256": "<sha256hex>",
      "binary_filename": "<filename in this run-N/ dir>"
    }
  ],
  "results": [{"case": "<label>", "returncode": 0, "seconds": 0.0, "command": [...]}],
  "logs": {"<label>": "<label>.log"},
  "artifacts": {"<key>": {"path": "<abs path>", "sha256": "<hex>", ...}},
  "strict_lint_log_sha256": "<sha256hex>",
  "build_log_sha256": "<sha256hex>"
}
```

Note: `results` is a **list** (not a dict) in the actual file, matching the
runner's `results = []` array. `artifacts` is a dict of key→object (not key→bare
sha256). `status` is a scope-specific descriptive string, not a bare "accepted"
literal.

---

## 5. Step 5 — Update `original-runner.py` to enforce the forward spec

The current `original-runner.py` in `issuer-expiry-20261026/run-8/` already
implements most of the forward spec. The following changes are required.

### 5.1 Remove `owned-source/` copy logic

The actual copy is in the main `try:` block after the `shutil.copyfile(model_source_path, ...)` call:

```python
for name in report['applied']:
    dest = out/'owned-source'/name
    dest.parent.mkdir(parents=True,exist_ok=True)
    shutil.copyfile(root/name,dest)
```

`report['applied']` is a **dict** (path → sha256), so `for name in report['applied']`
iterates its keys. Delete these four lines. The `applied.json` file already
records every path and its post-apply SHA-256; the physical copies are redundant.

**What must remain:** `report['applied']` is also used by the external post-run
acceptance step to populate `owned_paths`. Do not remove the `report` variable
or the `assert all(files[name] == digest for name, digest in report['applied'].items())`
precondition check — only the four-line file-copy loop is deleted.

### 5.2 Add assertion that no full-tree copy is attempted

Add a guard that asserts the output directory (`out`) does not contain
prohibited full-tree artifacts. The function takes a `pathlib.Path` (matching
`out`'s type throughout the runner):

```python
def assert_no_tree_copy(out_dir) -> None:
    """Fail fast if a full-tree artifact was accidentally written to the run dir."""
    forbidden = ["crates", "docs", "AGENTS.md", "Cargo.toml", "path_to_v3.5.md"]
    for name in forbidden:
        p = Path(out_dir) / name
        if p.exists():
            raise RuntimeError(
                f"Validation output contains prohibited full-tree artifact: {p}\n"
                "The runner must not copy the repo tree into target/validation/."
            )
```

Call it in two places in the main `try:` block:

1. **After the `shutil.copyfile(__file__, out / 'original-runner.py')` line** and
   before the first `command()` call — i.e., right after the setup copies and
   before any Cargo execution:
   ```python
   assert_no_tree_copy(out)
   ```

2. **Immediately before the `(out / 'completed.json').write_text(...)` line** —
   as a final check that no step accidentally wrote prohibited content:
   ```python
   assert_no_tree_copy(out)
   persist(status='completed', ...)
   (out / 'completed.json').write_text(...)
   ```

Note: `preserve()` is called inside `command()`'s JSON-parsing loop and cannot
be individually wrapped at the call site. The two placement points above cover
the entire execution span.

### 5.3 Enforce log file size cap

Add a `truncate_log(path: str, max_bytes: int = 500_000)` call after every log
write. Keep the **tail** (most recent lines), not the head, because build
failures appear at the end:

```python
def truncate_log_tail(path: str, max_bytes: int = 500_000) -> None:
    size = os.path.getsize(path)
    if size <= max_bytes:
        return
    with open(path, "rb") as f:
        f.seek(size - max_bytes)
        tail = f.read()
    # find first newline to start at a clean line boundary
    nl = tail.find(b"\n")
    tail = tail[nl + 1:] if nl >= 0 else tail
    with open(path, "wb") as f:
        f.write(b"[... truncated, showing last 500 KB ...]\n")
        f.write(tail)
```

### 5.4 No other changes to the runner

The runner's Cargo serial lock acquisition, `verify_source()` before/after
each step, `preserve()` binary copy, `progress.json` + `completed.json` write,
`capacity()` disk-space check, `source.json` generation logic, and the
`report['applied']` assertion are all correct and must not be modified.

Note: the runner does **not** write `accepted.json` — that is written by a
separate post-run step that reads `completed.json` and the run directory.
The runner's output for a successful run is: `source.json`, `applied.json`,
`original-runner.py`, `model-source.json` (when applicable), `progress.json`,
`completed.json`, `*.log` files, and the preserved test binaries.

---

## 6. Step 6 — Update `docs/development-policy.md` with the forward retention policy

Add the following paragraph at the end of the section
`## Shared source and central build ownership (2026-10-30)`, immediately before
the `## Reason before compiling` section heading:

```markdown
**Evidence retention format (2026-11-02):** each validation run writes
`source.json` (per-file SHA-256 manifest of all git-tracked files plus HEAD
commit hash), `applied.json` (before/after SHA-256 for changed files),
`original-runner.py`, per-step log files, `accepted.json` or `failed.json`,
and the compiled test binaries for the accepted run. The runner must not copy
the full working tree, `crates/`, `docs/`, root plan files, or changed-file
copies (`owned-source/`) into `target/validation/`. Binary SHA-256 hashes in
`accepted.json` are the immutable artifact identity; the binary is retained for
re-execution capability for the accepted run only. Binaries from failed and
superseded intermediate runs are disposable after the receipt is written.
```

**This is a documentation-only change.** `git diff --check` and a link
consistency review are the only required checks (per `docs/development-policy.md`
§"Choose evidence by the claim", row "Ordinary documentation or development
policy").

---

## 7. Step 7 — Add a `.gitignore` entry for `target/validation/`

`target/validation/` must never be committed to the git repository. The binary
files alone would make the repository enormous. Verify that `target/` is
already listed in `.gitignore` (it is, as is standard for Rust projects).

```bash
grep -n 'target' /Volumes/SSDE/brassclaw/.gitignore
```

If `target/` appears, no additional entry is needed. If only specific
subdirectories are listed, add:

```
target/
```

This is a safety check; no change is expected to be necessary.

---

## 8. Constraints that must not be violated by any step

These rules are drawn from `AGENTS.md` and `docs/development-policy.md` and are
binding regardless of disk pressure:

1. **Never delete `accepted.json` or `failed.json` — under any filename.**
   Old-style artifacts dirs use non-standard names like
   `brassclaw-<pkg>-accepted.json` and `failed-acceptance.json`. All `*.json`
   files in artifacts directories are receipts or inventory files and must be kept.

2. **Never delete a log file whose path is cited in any doc.** Before deleting
   any log, grep `docs/` and all root-level `.md` files for its relative path.

3. **Never delete the binary from the accepted (final) run of a work package**
   without confirming that no doc cites that binary's path (as opposed to its
   SHA-256 hash). If the path is cited, keep the binary.

4. **Never modify any receipt file.** All `*.json` files in `*-artifacts-*`
   directories are immutable historical records. Their SHA-256 hashes may be
   cited inline in validation docs. Any modification would break that evidence chain.

5. **Never restore a full working-tree snapshot over the current checkout.**
   The old-style snapshot directories in `target/validation/` are historical
   frozen states. Their content (including stale plan files like
   `path_to_v3.5.md`) must not be used as the source of truth for any plan or
   architecture decision. The canonical versions are always the files in the
   repository root.

6. **`source.json` must not be deleted** from any run that has a corresponding
   `accepted.json`. Its SHA-256 is stored in `accepted.json` as
   `source_manifest_sha256` and is part of the immutable receipt.

7. **Disk-space checks are mandatory before every Cargo execution.** Per
   `AGENTS.md`: check `df -h /Users/ollama/brassclaw-target` (NVMe build target)
   before compiling. If below 15 GiB or above 90%, clean disposable build
   artifacts from `CARGO_TARGET_DIR` before proceeding.

8. **Never delete an old-style source snapshot directory whose name appears in
   any document under `docs/` or any root `.md` file.** Many such directories
   are cited as "frozen source", "retained source", or "compiled from" in
   `docs/plans/simplified-v3-implementation.md`. Step 1's two-pass filter
   enforces this; the per-directory safety re-check is the enforcement point.

9. **Do not treat `source/` inside `prefix-model-config-ui-*` as `owned-source/`.**
   These packages contain a `source/` subdirectory holding curated model artifacts
   (not a changed-file copy), plus `accepted.json` directly at the package root
   without `run-N/` nesting. Do not apply Step 3's `owned-source` removal to them.

---

## 9. Execution order

Steps must be executed in this order by the leading Root/Auth chat:

| Step | Description | Blocking dependency | Expected duration |
|------|-------------|---------------------|-------------------|
| 1 | Delete uncited old-style source snapshots (two-pass filter) | None — safe immediately | 15–30 min (grep pass + rm) |
| 2 | Remove binaries from failed/superseded artifact dirs | Step 1 complete | 10–20 min (many directories) |
| 3 | Remove `owned-source/` from `auth-dcr-common` and `issuer-expiry` runs | Step 2 complete | < 1 min |
| 4 | Forward runner spec | None (documentation only) | — |
| 5 | Update runner script | Disk pressure resolved (Steps 1–2) | — |
| 6 | Update `docs/development-policy.md` | None | — |
| 7 | Check `.gitignore` | None | — |

Steps 4, 6, and 7 are documentation-only and require no Cargo execution.
Step 5 modifies `original-runner.py` — not a Rust source file — so no
recompilation is required; verify with `git diff --check`.

Steps 1–3 must be completed before the next Cargo execution because
`/Volumes/SSDE` is currently at 97% capacity with only 16 GiB free, which is
below the 15 GiB mandatory threshold.

---

## 10. Verification checklist

After all steps are complete:

- [ ] `df -h /Volumes/SSDE` shows ≥ 30 GiB free (conservative target after cleanup)
- [ ] All directories in `/tmp/cited-absolute.txt` that existed before Step 1 still exist:
  `while IFS= read -r d; do [ -d "$d" ] || echo "MISSING: $d"; done < /tmp/cited-absolute.txt`
- [ ] No uncited directory with `crates/` and `AGENTS.md` remains:
  `for d in /Volumes/SSDE/brassclaw/target/validation/*/; do [ -d "${d}crates" ] && [ -f "${d}AGENTS.md" ] && echo "RESIDUAL: $d"; done`
  (All output from this command should correspond only to cited directories.)
- [ ] Every `*-artifacts-*/` directory that had receipt files before Step 2 still has them.
  Note: early-format dirs from before the JSON-receipt era legitimately have zero `.json` files;
  this is not a failure. Verify no cited artifacts directory is unexpectedly empty:
  `for d in /Volumes/SSDE/brassclaw/target/validation/*-artifacts-*/; do COUNT=$(find "$d" -maxdepth 1 -name "*.json" | wc -l); echo "$COUNT $d"; done | sort -n | head -10`
  Zero-JSON entries are only acceptable for pre-receipt-format runs never cited in docs.
- [ ] `grep -roh "target/validation/[a-zA-Z0-9_-]*" /Volumes/SSDE/brassclaw/docs/ /Volumes/SSDE/brassclaw/*.md | grep -v "\-artifacts" | sed 's|.*target/validation/|/Volumes/SSDE/brassclaw/target/validation/|' | sort -u | while read p; do [ -d "$p" ] || echo "CITED BUT MISSING: $p"; done`
  — must produce no output
- [ ] `prefix-model-config-ui-aolgri5u/source/` and `prefix-model-config-ui-kk0ebg8y/source/`
  still exist (not deleted by Step 3)
- [ ] The modified `original-runner.py` contains no four-line `owned-source` copy loop
- [ ] The modified `original-runner.py` calls `assert_no_tree_copy()` before the first `command()` call
      and again immediately before writing `completed.json`
- [ ] `git diff --check` is clean on all modified files
- [ ] Spot-check: `shasum -a 256 /Volumes/SSDE/brassclaw/target/validation/issuer-expiry-20261026/run-8/accepted.json`
  matches the SHA-256 cited in `docs/validation/2026-10-26-k3-issuer-expiry.md`
