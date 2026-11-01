# missing1_v3_at_k3 — L010 typed `not_found` contract

**Date:** 2026-11-01.
**Reviewed:** 2026-11-01 — two errors corrected (see §6).
**Status:** Plan only. No code, runtime, seed or activation change results from
creating this file. Implementation belongs to its named owning layer. Existing
approvals, running tasks and current placeholder bytes remain unchanged until the
steps below are completed and accepted.

## 1. Problem statement

`L010` (`pc-exec-file-exists`) is blocked on a **typed `not_found` contract**.
The placeholder body at
[`builtin_bootstrap.rs` ~4438](crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs)
branches on `outcome["kind"] == "not_found"`, but that branch can never be
reached from any current Tool output. The gap has three concrete sources:

1. **No existence-probe Tool exists.** There is no registered capability with
   the contract `{kind: "success" | "not_found", path}`. The six coding
   capabilities — `builtin.read_file`, `builtin.write_file`, `builtin.list_dir`,
   `builtin.glob`, `builtin.grep`, `builtin.apply_patch` — do not expose a
   stat-or-not-found result. Adding an `operation` sub-field to an existing
   tool is not possible: the dispatch is by `CodingCapabilityKind` enum variant,
   one per `capability_id` constant, not by an input parameter.

2. **`FilesystemError::NotFound` maps to `OperationFailed` at the boundary.**
   [`filesystem_error()` in `paths.rs` line 299](crates/brassclaw_first_party_extensions/src/coding/paths.rs)
   maps `FilesystemError::NotFound` to `RuntimeDispatchErrorKind::OperationFailed` —
   the same kind as access-denied, binary files, size overflows and transport
   failures. A missing file is indistinguishable from any other operation failure
   through the existing read path. A failed read does not prove nonexistence.

3. **`stat_optional` already distinguishes absence.** The helper
   [`stat_optional()` in `paths.rs` line 127](crates/brassclaw_first_party_extensions/src/coding/paths.rs)
   maps `FilesystemError::NotFound` to `Ok(None)` and every other error to
   `Err(…)`. This is the correct, safe building block — it just is not exposed
   to the Orchestrator through any Tool callable.

Consequence: the placeholder PythonCode body is dead code. The `"not_found"`
branch is permanently unreachable given current Tool outputs.

The existing Recipe YAML at `RECIPE_FILE_EXISTS_YAML` compounds the problem:
its step-1 references `<uuid:ts-read-file>` — the `builtin.read_file` ToolSkill —
which is the wrong Tool for an existence probe.

---

## 2. Root causes (one per layer)

| Layer | Gap |
|-------|-----|
| **Rust capability** | No registered `builtin.file_exists` capability exists. `CodingCapabilityKind` has no `FileExists` variant; `CODING_CAPABILITIES` has no corresponding entry; the manifest does not declare it. |
| **Tool row (class 0)** | No `NewPgTool` seed row exists for a `file_exists` tool with `capability_id: "builtin.file_exists"`. |
| **ToolSkill (class 13)** | No `ts-file-exists` ToolSkill binding descriptor names `tool_name: "file_exists"`. The existing `RECIPE_FILE_EXISTS_YAML` incorrectly references `<uuid:ts-read-file>`. |
| **PythonCode (L010)** | The body is pure classification logic over a hypothetical `outcome` input — no `host.*` call. It must remain `BLOCKED` until the upstream Tool and ToolSkill deliver the required typed outcome. |
| **Skill (`skill-file-exists`)** | Prose still says "attempting to read line 1" and the proposed export `use_skill_file_exists(inputs)` is BLOCKED for the same reason. |
| **Recipe (`file-exists`, `RCP025`)** | Step-1 names the wrong ToolSkill; step-2 names the blocked PythonCode. The Recipe cannot be activated. |

---

## 3. Required work (ordered)

### Step 1 — Add `FileExists` to `CodingCapabilityKind` and register the capability

**Files:** `crates/brassclaw_first_party_extensions/src/coding/mod.rs`
and `crates/brassclaw_host_runtime/src/first_party_tools/mod.rs`.

1. Add `FileExists` to the `CodingCapabilityKind` enum in `mod.rs`:

   ```rust
   pub enum CodingCapabilityKind {
       ReadFile,
       WriteFile,
       ListDir,
       Glob,
       Grep,
       ApplyPatch,
       FileExists,   // new
   }
   ```

2. Add a `FILE_EXISTS_CAPABILITY_ID` constant in
   `crates/brassclaw_host_runtime/src/first_party_tools/mod.rs` alongside the
   other six:

   ```rust
   pub const FILE_EXISTS_CAPABILITY_ID: &str = "builtin.file_exists";
   ```

3. Add an entry to `CODING_CAPABILITIES`:

   ```rust
   CodingCapabilityMetadata {
       id: FILE_EXISTS_CAPABILITY_ID,
       kind: CodingCapabilityKind::FileExists,
       description: "Probe whether a scoped file exists. Returns \
                     {kind: \"success\" | \"not_found\", path}. \
                     Absence is FilesystemError::NotFound only; other \
                     failures surface as classified errors.",
       effects: &[EffectKind::ReadFilesystem],
   },
   ```

4. Add the `FileExists` arm to the `dispatch` match in
   `crates/brassclaw_first_party_extensions/src/coding/mod.rs`:

   ```rust
   CodingCapabilityKind::FileExists => file::file_exists(request)
       .await
       .map(CodingCapabilityOutput::new),
   ```

**Acceptance:** `cargo check -p brassclaw_first_party_extensions` and
`cargo check -p brassclaw_host_runtime` both clean; no existing arm is
disturbed.

---

### Step 2 — Implement `file_exists` in the coding handler

**File:** `crates/brassclaw_first_party_extensions/src/coding/file.rs`.

Add `pub(super) async fn file_exists`:

- Accepts only `path` (string); reject any other input key.
- Calls `resolve_required_path(request, "path", FilesystemOperation::Stat)`.
  (`Stat` is the correct operation: `operation_allowed` maps it to
  `permissions.read || permissions.list`, so list-only mounts can also probe
  existence. `ReadFile` would require `permissions.read` exclusively and would
  wrongly deny existence probes on list-only mounts.)
- Calls `stat_optional(request, &resolved.virtual_path).await?` — the existing
  helper that already maps `FilesystemError::NotFound → Ok(None)` and every
  other error to a classified `Err`.
- On `Ok(Some(_stat))`: returns
  `json!({"kind": "success", "path": resolved.scoped_path.as_str()})`.
- On `Ok(None)` (absence): returns
  `json!({"kind": "not_found", "path": resolved.scoped_path.as_str()})`.
- The `?` on `stat_optional` propagates all other classified errors
  (`FilesystemDenied`, `Backend`, etc.) as host-side failures — they do
  **not** become `not_found`.

Do **not** inspect `stat.file_type` or `stat.sensitive` — a `stat_optional`
success means the entry exists at that path; type and sensitivity checks belong
to callers that need them (e.g. `read_file`).

**Unit tests** (in `file.rs` `#[cfg(test)]` module or the coding `tests`
module):

- Existing path → `{"kind": "success", "path": …}`.
- Missing path → `{"kind": "not_found", "path": …}`.
- Path outside mount → host-classified failure, **not** `not_found`.

**Acceptance:** `cargo check` and `cargo clippy -p brassclaw_first_party_extensions --all-targets -- -D warnings` clean; all three test cases pass.

---

### Step 3 — Seed the Tool row (`class 0`) and ToolSkill row (`class 13`)

**File:** `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs`
inside `seed_filesystem_group`.

**Tool row** — add a `tool_file_exists_row` constructor and call
`stores.upsert_tool(tool_file_exists_row(&tenant), "file_exists")`:

```rust
fn tool_file_exists_row(tenant: &str) -> NewPgTool {
    NewPgTool {
        name: "file_exists".to_string(),
        description: "Probe whether a file exists at a scoped workspace path. \
                      Returns {kind: \"success\" | \"not_found\", path}. \
                      Does not read file content. Absence is distinguished from \
                      access-denied and other failures."
            .to_string(),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Scoped workspace path to probe"}
            },
            "required": ["path"],
            "additionalProperties": false
        })),
        param_template: Some(json!({"path": "{{path}}"})),
        effect_type: "read".to_string(),
        preconditions: Some("Path must resolve within a scoped mount.".into()),
        error_handling: Some(
            "FilesystemDenied: path outside mounts or permission denied. \
             Backend/Resource: stat failure. not_found is a typed result, not an error."
                .into(),
        ),
        consumer_tags: vec!["00:rusty".into()],
        source: "system".into(),
        validation_status: "validated".into(),
        capability_id: "builtin.file_exists".into(),
        // … other tenant/user/agent/project fields follow the existing pattern
    }
}
```

**ToolSkill row** — add a `ts_file_exists_row` constructor and call
`stores.upsert_tool_skill(ts_file_exists_row(&tenant), "ts-file-exists")`:

```rust
fn ts_file_exists_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        name: "ts-file-exists".to_string(),
        description: "Binding descriptor for builtin.file_exists. \
                      Probes whether a path exists; returns typed kind result."
            .to_string(),
        content: "Call `host.file_exists(path=<scoped path>)` to probe existence. \
                  Returns {kind: \"success\", path} if the entry exists, \
                  {kind: \"not_found\", path} if absent. Other errors surface as \
                  classified host failures — they do not imply nonexistence."
            .to_string(),
        tool_name: Some("file_exists".to_string()),
        param_schema: Some(json!([
            {"name": "path", "param_type": "string", "required": true,
             "description": "Workspace-relative scoped path to probe"}
        ])),
        param_template: Some(json!({"path": "{{path}}"})),
        consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
        source: "system".into(),
        validation_status: "validated".into(),
        // … other fields follow the existing ToolSkill pattern
    }
}
```

Both seeds must be idempotent (upsert pattern). Bind the returned UUIDs to
`tool_file_exists` and `ts_file_exists` local variables for use in the Recipe
wiring and child-list updates below.

**Acceptance:** `cargo check -p brassclaw_reborn_composition --features postgres,skills-db` clean; two runs produce no duplicate rows.

---

### Step 4 — Replace `PC_EXEC_FILE_EXISTS_CONTENT` and fix its description

**File:** `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs`.

Replace the blocked constant with a real one-Tool-call body:

```rust
// L010: one host.file_exists call; classifier over typed kind outcome.
// Preloadable export: use_file_exists(inputs).
const PC_EXEC_FILE_EXISTS_CONTENT: &str = r#"def use_file_exists(inputs):
    outcome = host.file_exists(path=inputs["path"])
    if outcome["kind"] == "success":
        result = {"exists": True, "path": outcome["path"]}
    elif outcome["kind"] == "not_found":
        result = {"exists": False, "path": outcome["path"]}
    else:
        raise ValueError("existence indeterminate")
    return result
"#;
```

Also update the seed row description for `pc-exec-file-exists` (the string
passed to `pc_row`) from the old catch-all read description to:

> "Preloadable `use_file_exists(inputs)`: probes file existence via
> `host.file_exists`. Typed input: `path` (nonempty string). Returns
> `{exists: bool, path: string}`. Absence is `kind: not_found`; classified
> failures (denied, backend) raise ValueError."

Remove the `BLOCKED` comment above the constant.

**Do not:**
- Use `except` to catch any failure and return `exists: False` — that is the
  original bug this plan fixes.
- Read `outcome["path"]` from `inputs["path"]` — the Tool returns the scoped
  canonical path, which may differ and is the authoritative value.
- Omit the `else: raise ValueError` branch — classified failures must not
  silently collapse into `exists: False`.

**Acceptance:** constant has exactly one `host.*` call; `BLOCKED` comment is
removed; description no longer references "attempting to read line 1"; `cargo
check` clean.

---

### Step 5 — Fix `RECIPE_FILE_EXISTS_YAML` to reference the new ToolSkill

**File:** `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs`.

The existing `RECIPE_FILE_EXISTS_YAML` has step-1 pointing at
`<uuid:ts-read-file>` — the wrong ToolSkill. Replace it with the new one:

```rust
const RECIPE_FILE_EXISTS_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-file-exists>"],
    "label":   "Bind ts-file-exists ToolSkill for builtin.file_exists"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-file-exists>"],
    "label":   "PythonCode calls host.file_exists; returns {exists: bool, path}"
  }
]
"#;
```

**Acceptance:** YAML references `ts-file-exists` in step-1 and
`pc-exec-file-exists` in step-2; both are valid seeded component names; no
`ts-read-file` reference remains in this constant.

---

### Step 6 — Update `SKILL_FILE_EXISTS_BODY` prose and Skill/PythonCode association

**File:** `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs`.

Replace `SKILL_FILE_EXISTS_BODY` with accurate prose that:

- Names `host.file_exists` (not a read attempt).
- States the export is `use_file_exists(inputs)`.
- Describes the `{exists: bool, path}` result and that `kind: not_found` means
  absent (not a failure).
- States that classified failures (denied, backend) surface as errors and do
  not set `exists: False`.

Example prose:

```
Use `use_file_exists(inputs)` (via pc-exec-file-exists / builtin.file_exists)
to check whether a file exists at a given path before attempting a full read or
write. Returns {exists: true, path} when the entry is present, {exists: false,
path} when absent. A classified host failure (FilesystemDenied, Backend, etc.)
raises ValueError — it does not mean the file is absent. Use this before
skill-read-file when existence is uncertain to avoid surfacing a "file not
found" error to the user. Also use before skill-write-file-replace to determine
whether to overwrite or create-new.
```

The Skill association (explicit reviewed link between the prose row and the
L010 PythonCode row) must name the export `use_file_exists(inputs)`, matching
the code. No discrepancy between the host callable named in the prose and the
one in `PC_EXEC_FILE_EXISTS_CONTENT` is acceptable.

**Acceptance:** Skill prose no longer references "read line 1"; host callable
`host.file_exists` is named; export name `use_file_exists` is consistent with
the code body.

---

### Step 7 — Confirm the new capability is exercised by the boot-time integration path

There is no separate `expected_tools` list file to update. The file
`builtin_bootstrap_seed.rs` referenced in an earlier draft **does not exist**.
The actual integrity mechanism is `EXPECTED_CHECKSUMS` in
[`builtin_bootstrap.rs`](crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs)
(a `LazyLock<HashMap<&'static str, String>>` keyed by component *name*, not
capability ID), and it covers only the class-10 prompt rows (orchestrator,
preamble, postamble, subagent directions, etc.). Tool capability IDs are not
tracked there. `run_content_integrity_check` verifies `source='system'` rows
whose names appear in `EXPECTED_CHECKSUMS`; it is not a registry of expected
`capability_id` values.

The required action for Step 7 is therefore:

1. **Check that `seed_filesystem_group` is called unconditionally** in the boot
   sequence that runs `seed_builtin_components`. Confirm that the new
   `upsert_tool` and `upsert_tool_skill` calls inside it succeed and are
   exercised by the existing boot-integration test path. No new list entry is
   required.

2. **Add the `builtin.file_exists` capability to the existing capability-level
   test that enumerates seeded Tools** — if one exists in
   `crates/brassclaw_reborn_composition/src` — to catch accidental removals.
   If no such enumeration test exists today, add a targeted `cargo test -p
   brassclaw_reborn_composition --features integration` case that asserts
   `builtin.file_exists` is present after seeding.

**Acceptance:** `cargo check -p brassclaw_reborn_composition --features
postgres,skills-db` clean; a targeted integration test confirms the tool row is
present after `seed_builtin_components` runs.

---

### Step 8 — Update `docs/implementation-status.md`

Add a new dated entry recording completion of L010. The entry must:

- List each commit that implements Steps 1–7.
- Confirm that the typed `not_found` contract is established via `stat_optional`
  behind the new `builtin.file_exists` capability.
- Note that `RECIPE_FILE_EXISTS_YAML` step-1 now references `ts-file-exists`
  (the correct ToolSkill) and that the `BLOCKED` comment on
  `PC_EXEC_FILE_EXISTS_CONTENT` is removed.
- Supersede the "L010 (`pc-exec-file-exists`) blocked on typed `not_found`
  contract" bullet that appears in every prior "Remaining open items" list.

---

## 6. Review corrections summary (2026-11-01)

The plan was reviewed against the live source immediately after authoring. Two
errors were found and corrected. No code was changed by the review; all
corrections are to plan text only.

| # | Severity | Error in original plan | Correction |
|---|----------|----------------------|------------|
| 1 | **Critical** | Step 7 named `builtin_bootstrap_seed.rs` and an `expected_tools` list as the place to register the new `builtin.file_exists` capability ID. That file does not exist. The actual integrity mechanism is `EXPECTED_CHECKSUMS` in `builtin_bootstrap.rs` — a `LazyLock<HashMap>` keyed by component *name*, covering only class-10 prompt rows. It tracks no Tool `capability_id` values. | Rewrote Step 7 to remove the false file reference. The required action is confirming the new upsert calls are exercised by the boot path, and adding a targeted integration test that asserts the seeded tool row is present. |
| 2 | **Minor** | Step 2 called `resolve_required_path(request, "path", FilesystemOperation::ReadFile)` with the comment "Read permission is sufficient". The `Stat` variant exists in `FilesystemOperation` and is the semantically correct choice: `operation_allowed` maps `Stat` to `permissions.read \|\| permissions.list`, allowing existence probes on list-only mounts. `ReadFile` maps only to `permissions.read` and would wrongly deny probes on mounts that grant list but not read. | Changed `FilesystemOperation::ReadFile` to `FilesystemOperation::Stat` in Step 2 and updated the comment to explain the permission semantics. |

---

## 4. What this plan does NOT change

- `read_file`, `write_file`, `list_dir`, `glob`, `grep`, `apply_patch` —
  unchanged. No existing capability is extended with an `operation` input field.
- `RuntimeDispatchErrorKind` — no new variant. The existing `OperationFailed`
  is sufficient for other errors; `not_found` is a **typed success result**, not
  an error kind.
- `stat_optional` in `paths.rs` — reused as-is. No change to its
  `Ok(None)` / `Err(…)` semantics.
- `L011`–`L091` and their blocked/open statuses — unaffected.
- Intent examples for the `file-exists` Recipe — already seeded; no new rows
  needed unless the Recipe review calls for additional coverage.
- No documentation files other than `docs/implementation-status.md` (Step 8)
  are changed. `monty.composition.md`, `monty.composition.implementation.md` and
  the coverage audit correctly describe the gap and serve as acceptance
  references.

---

## 5. Constraints inherited from the owning architecture

- **One Tool per capability ID.** The dispatch in
  `crates/brassclaw_host_runtime/src/first_party_tools/mod.rs` routes by
  `capability_id` string to a `CodingCapabilityKind` enum variant. Each new
  operation is a new capability, not a parameter of an existing one.
- **One Tool call per PythonCode step.** The dependent-chain exception does not
  apply; existence probe is a single atomic operation.
- **Rust ToolSkill binding step immediately precedes the PythonCode step.**
  The two steps cannot be merged into one.
- **Loading the PythonCode defines the export; it does not invoke the Tool.**
  The `host.file_exists` call happens only when `use_file_exists(inputs)` is
  invoked on demand after its matching ToolSkill binding.
- **Live kernel policy** is checked before every dispatch, independently of
  component approval. Policy denial after preload surfaces as a classified
  failure, not as `not_found`.
- **`stat_optional` returns `Ok(None)` only for `FilesystemError::NotFound`.**
  `FilesystemError::PermissionDenied`, `Backend`, `MountNotFound` and others
  remain classified errors; the probe contract must honour these distinctions.
- **Immutable approved versions.** The corrected body replaces the placeholder
  only after the full Q1/behavior/human-Q2 acceptance path for an authored
  change, or via the system-seed bootstrap path for a first-party packaged
  change. This plan does not claim either approval. The Recipe is not activated
  until its constituent steps pass their own Q1/behavior requirements.
