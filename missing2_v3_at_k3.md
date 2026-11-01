# missing2_v3_at_k3 — L013–L101 full qualification

**Date:** 2026-11-01.
**Reviewed:** 2026-11-01 — five errors corrected (see §7).
**Status:** Plan only. No code, runtime, seed or activation change results from
creating this file. Implementation belongs to its named owning layer. Existing
approvals, running tasks and current placeholder bytes remain unchanged until the
steps below are completed and accepted.

## 1. Problem statement

The L013–L101 PythonCode components, their companion Skill rows (class 1–2), and
their consuming Recipes are body-format-complete as of the 2026-11-01 session
batch. Every constant in [`builtin_bootstrap.rs`](crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs)
now uses a declared `def export_name(inputs)` preload function; no legacy
top-level `result = ...` body remains. That completes the **body-format** phase.

Four gates remain open for each component before productive (Tier-0 or Tier-1)
routing can be activated:

| Gate | What is missing |
|------|----------------|
| **Exports declaration** | The immutable Skill interface declaration — public name → associated code UUID, implementation symbol, signature, typed input/result/failure contracts and dependency graph — is not recorded in the seeded Skill revision's checksummed content. No `dependency_registry` or `content_checksum` is populated for the L013–L101 Skill or PythonCode rows in `builtin_bootstrap.rs`. |
| **Skill/PythonCode association** | No `skill-association/1` record exists embedded in each Skill's `"component-revision/1"` envelope. The retained binding resolver (`prepare_retained_tool_program`) reads the association from `revision.draft().association()` — a JSON string stored in the `"association"` field of the revision stored in `reborn_component_revisions`. The current `upsert_skill` path seeds `reborn_skills` only; it never creates a revision envelope. |
| **Behavioral evidence + approval** | No `"component-review-evidence/1"` (Q1 + behavioral) rows exist in `reborn_component_review_evidence`, and no `"skill-association-approval/1"` rows exist in `reborn_skill_association_approvals` for any L013–L101 Tool-calling pair. Without these, `with_association_approvals` fails at assembly time. |
| **Activation** | The seeded Recipes consuming L013–L101 components carry `validation_status = "validated"` in their row but their constituent Skill/PythonCode pairs lack both revision envelopes and association approvals. They cannot be coherently activated or matched by the retained catalogue until all three gates above are closed. |

This plan covers all component families whose Lnnn labels fall in the
L013–L101 range: the **network group** (L016–L027), the **memory group**
(L028–L035), the **management group** (L036–L049, L060–L067), the **Git group**
(L050–L059, L080–L085), the **plan group** (L086–L091), the **GitHub/code-review
group** (L092–L093), and the **Zencoder helper group** (L094–L101, archived but
retained for plan completeness). The pure **path/filter helpers** (L007–L009,
L011–L012) are also included but go through a different approval path (see §3).
L010 (`pc-exec-file-exists`) is explicitly excluded — it is blocked on the
missing `builtin.file_exists` Rust primitive and is tracked in `missing1_v3_at_k3.md`.

---

## 2. Root causes (one per layer)

| Layer | Gap |
|-------|-----|
| **`reborn_component_revisions` missing** | The Lnnn seeder uses `upsert_skill` and `upsert_python_code`, which insert into `reborn_skills` and `reborn_python_code` respectively. The retained approval machinery operates entirely on `"component-revision/1"` envelopes stored in `reborn_component_revisions`. No Lnnn Skill or PythonCode has a revision envelope; `retain_packaged_draft` / `stage` have never been called for them. |
| **No `skill-association/1` in revision** | `prepare_retained_tool_program` reads the association from `revision.draft().association()`. This is the `"association"` string field of the revision envelope, not a standalone DB row. It is populated only when the revision is created with the association JSON embedded. Since no revision envelopes exist for Lnnn components, this field is absent. |
| **No Q1 / behavioral evidence rows** | `reborn_component_review_evidence` has no entries for any Lnnn pair. `bootstrap_evidence::retain_intent` writes Q1 evidence and `retain_completed` atomically writes behavioral evidence + the approval row. Neither has been called for Lnnn components. |
| **No `skill-association-approval/1` rows** | `reborn_skill_association_approvals` has no entries for Lnnn pairs. `retain_completed` is the only legitimate writer. The `read_association` helper confirms absence. |
| **`content_checksum` absent** | `skill_row(...)` passes `content_checksum: None` and `pc_row(...)` passes `dependency_registry: None`. Both fields must be populated before a revision envelope can be constructed and the boot-time integrity check can verify the row. |
| **Pure-logic components use a different approval path** | L007–L009, L011–L012, L024–L027, L033–L035, and L090–L091 contain no `host.*` calls. Their standalone Recipe steps are checked by `require_independent_execution_coverage`, which reads from `reborn_pure_component_approvals` via `load_independently_qualified`. They do not go through `skill-association-approval/1` at all. |

---

## 3. Scope: what groups and how many pairs

The table below separates Tool-calling pairs (which need `skill-association-approval/1`)
from pure-logic components (which need `pure-component-approval/1`).

| Group | Key Lnnn labels | Tool-calling pairs | Pure-logic components |
|-------|----------------|-------------------|----------------------|
| Network (HTTP) | L016–L023, L024–L027 | ~8 (L016–L023: http GET/POST/PATCH/HEAD/PUT/DELETE/auth/save) | ~4 (L024–L027: status-check, json-extract, web-search-extract, url-encode) |
| Memory | L028–L035 | ~5 (L028–L032: search/write/patch/read/tree) | ~3 (L033–L035: extract-section, format-entry, append-prep) |
| Management/shell fixed | L036–L049, L060–L067 | ~24 (pc-exec-shell-*) | 0 |
| Git diff/security grep | L050–L059, L080–L085 | ~11 (pc-git-diff-*; pc-grep-*) | 0 |
| Plan helpers | L086–L091 | ~4 (L086–L089: target/diff/write/patch) | ~2 (L090–L091: hash-changed, format-header) |
| GitHub/code-review helpers | L092–L093 | ~2 (pc-github-list-issues/prs) | 0 |
| Pure path/filter | L007–L009, L011–L012 | 0 | ~5 (filter_lines_inverted, filter_entries_by_type, compute_tail_window, filter_text_lines, filter_records) |

Tool-calling total: approximately **54** pairs needing `skill-association-approval/1`.
Pure-logic total: approximately **14** components needing `pure-component-approval/1`.

---

## 4. Required work (ordered)

### Step 1 — Add exports declaration to every affected Skill body constant

**File:** [`builtin_bootstrap.rs`](crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs).

For every Skill body constant that covers an L013–L101 Tool-calling component
(e.g., `SKILL_HTTP_GET_BODY`, `SKILL_MEMORY_SEARCH_BODY`, etc.), append an
`Exports:` section that names:

- The public export symbol (e.g., `usage_l016(inputs)`), matching the `def` line
  in the paired `PC_*_CONTENT` constant exactly.
- Its typed input schema: enumerate every declared key, mark optional keys, state
  that extra keys are rejected.
- Its return contract: the exact typed object the export returns on success.
- Its failure contract: which classified host failures propagate and which the
  code does not catch (classified host failures must propagate, not become a
  fallback success result).

Example addition to `SKILL_HTTP_GET_BODY`:

```
Exports:
  usage_l016(inputs) → {status, headers, body}
  inputs: {url: nonempty string, headers: optional dict,
           response_body_limit: optional int ≥ 1}
  failure: classified host failures (network, timeout, policy) propagate;
           non-2xx status is a typed success result, not a failure.
```

This is a documentation addition to the prose string — no executable change.
The checksum in Step 3 must be computed over the final body including this text.

**Acceptance:** every L013–L101 Tool-calling `SKILL_*_BODY` constant names its
export symbol, typed input schema and result/failure contract. No Skill body
references an export symbol that does not appear in the paired `PC_*_CONTENT`
constant's `def` line.

---

### Step 2 — Populate `dependency_registry` for every L013–L101 PythonCode row

**File:** [`builtin_bootstrap.rs`](crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs).

For each L013–L101 PythonCode row, pass a `dependency_registry` JSON value.
Introduce a `pc_row_with_deps` helper alongside `pc_row` that accepts an
additional `dependency_registry: Option<Value>` parameter and passes it through
to `NewPgPythonCode`:

```rust
// Example: L016 pc-exec-http-get (Tool-calling)
pc_row_with_deps(
    &tenant,
    "pc-exec-http-get",
    "...",
    PC_EXEC_HTTP_GET_CONTENT,
    Some(json!({
        "format": "python-code-dependency-registry/1",
        "exports": [{"name": "usage_l016", "symbol": "usage_l016"}],
        "helpers": [],
        "allowed_host_callables": ["http"],
        "imports": [],
        "effectful": true,
    })),
)
```

For pure-logic PythonCode rows (L007–L009, L011–L012, L024–L027, L033–L035,
L090–L091), use `effectful: false` and `allowed_host_callables: []`.

**Acceptance:** every L013–L101 `NewPgPythonCode` row has a non-null
`dependency_registry` with a `"exports"` list naming exactly the declared export
symbol from Step 1.

---

### Step 3 — Add `content_checksum` to every affected Skill and PythonCode seed row

**File:** [`builtin_bootstrap.rs`](crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs).

After completing Step 1 (prose bodies are final), compute the SHA-256 hex digest
of each `SKILL_*_BODY` constant and pass it as `content_checksum` to the
`skill_row(...)` call. Use the same computation as
[`bootstrap_evidence::checksum`](crates/brassclaw_reborn_composition/src/bootstrap_evidence.rs):
`hex::encode(Sha256::digest(bytes.as_bytes()))`.

Do the same for every `PC_*_CONTENT` constant: record its checksum in the
`content_checksum` field of the `pc_row_with_deps` call.

Do **not** inline literal hex strings. The checksum can be produced at build
time via `build.rs` output or via a `const fn` SHA-256 implementation. If
neither is available, produce checksums in a one-off tool run and embed them as
named constants that are regression-tested against the actual bodies.

**Acceptance:** every L013–L101 Skill and PythonCode seed row has a non-null
`content_checksum`. `run_content_integrity_check` does not fail on any of these
rows at boot.

---

### Step 4 — Create `"component-revision/1"` envelopes in `reborn_component_revisions` for every Tool-calling pair

This is the critical prerequisite gate. The retained approval machinery operates
entirely on revision envelopes stored in `reborn_component_revisions`. The
current `upsert_skill` / `upsert_python_code` path inserts into `reborn_skills`
/ `reborn_python_code` only — it never produces a revision envelope. Before any
association or approval can be recorded, each Tool-calling Skill and its paired
PythonCode must have revision envelopes.

**File:** Add a new `bootstrap_l013_revisions.rs` module (or extend
`builtin_bootstrap.rs`).

For each Tool-calling (Skill, PythonCode) pair, after upserting both rows and
obtaining their UUIDs, call `PgComponentRevisionStore::retain_packaged_draft` for:

1. **PythonCode revision** (class 22):
   ```rust
   ComponentRevisionDraft::from_json(&json!({
       "format": "component-revision/1",
       "uuid": pc_uuid,
       "class_code": 22,
       "document": {
           "content": PC_EXEC_HTTP_GET_CONTENT,
           "preload": { /* python-preload/2 with exports, private_functions, etc. */ },
           "input_contract":  { /* typed inputs matching the association */ },
           "result_contract": { /* typed result */ },
       },
       "dependencies": [],
       "association": null,
   }).to_string())
   ```

2. **Skill revision** (class 1), with the association JSON **embedded as a string**
   in the `"association"` field:
   ```rust
   let association = json!({
       "format": "skill-association/1",
       "skill_uuid":       skill_uuid,
       "python_code_uuid": pc_uuid,
       "tool_skill_uuid":  ts_uuid,
       "tool_uuid":        tool_uuid,
       "callable":         "host.http",
       "inputs":  { /* typed input map matching pc input_contract */ },
       "arguments": { /* positional mapping */ },
       "code_arguments": {},
       "result":  { /* typed result */ },
       "failure": { "action":"stop","max_attempts":1,"idempotency":"not_assumed",
                    "idempotency_evidence_ref":null,"retryable_outcomes":[] },
   }).to_string();
   ComponentRevisionDraft::from_json(&json!({
       "format": "component-revision/1",
       "uuid": skill_uuid,
       "class_code": 1,
       "document": {
           "body": SKILL_HTTP_GET_BODY,  // the prose with Exports: section from Step 1
           "interface": { /* skill-interface/1 */ },
       },
       "dependencies": [pc_uuid, ts_uuid, tool_uuid],
       "association": association,       // the skill-association/1 JSON as a string
   }).to_string())
   ```

The `"component-revision/1"` parser (`ComponentRevisionDraft::from_json`) enforces
that for class 1–3 rows: the association's `skill_uuid` must equal the revision
UUID, and `python_code_uuid`, `tool_skill_uuid`, `tool_uuid` must all appear in
`"dependencies"`. Any mismatch is a hard parse error before the draft is staged.

Both drafts are retained with `retain_packaged_draft`, which allocates them in
`reborn_component_revisions` at version 1 (or returns the existing reference
idempotently). The returned `ComponentRevisionRef` values are used in Step 5.

**Acceptance:** `SELECT count(*) FROM reborn_component_revisions WHERE component_id=$1`
returns ≥ 1 for every Tool-calling Skill and PythonCode UUID after this step.
`ComponentRevisionDraft::from_json` parses every produced envelope without error.

---

### Step 5 — Produce `skill-association-approval/1` records (system-seed path)

**File:** [`bootstrap_l013_approval.rs`](crates/brassclaw_reborn_composition/src/bootstrap_l013_approval.rs)
(new module), modelled on
[`bootstrap_encoding_approval.rs`](crates/brassclaw_reborn_composition/src/bootstrap_encoding_approval.rs).

For each Tool-calling (Skill, PythonCode) pair, inside a single `RepeatableRead`
transaction (enforced by `bootstrap_evidence::require_coherent`):

1. Read the exact staged revisions using
   `PgComponentRevisionStore::read_exact_in_transaction`, passing the UUIDs and
   revision references obtained in Step 4. This confirms the bytes are durably
   stored before any evidence is written.

2. Build the association string (same bytes as embedded in the Skill revision's
   `"association"` field) and compute its checksum:
   ```rust
   let association: String = /* same JSON string as in Step 4 */;
   let assoc_checksum = bootstrap_evidence::checksum(&association);
   ```

3. Build the `"component-review-evidence/1"` Q1 bytes and call
   `bootstrap_evidence::retain_intent`:
   ```rust
   let q1_bytes = json!({
       "format": "component-review-evidence/1",
       "evidence_id": q1_id,
       "kind": "q1",
       "validation_mode": "system_seed",
       "association_checksum": assoc_checksum,
       "components": [
           {"uuid": skill_ref.uuid, "class_code": 1, "version": skill_ref.version,
            "checksum": hex::encode(skill_ref.checksum)},
           {"uuid": pc_ref.uuid, "class_code": 22, "version": pc_ref.version,
            "checksum": hex::encode(pc_ref.checksum)},
       ],
       "succeeded": true,
       "reviewed_evidence": [],
       "report": {
           "checks": [
               "export-symbol-in-prose-matches-def-line",
               "single-host-call-per-tool-calling-body",
               "dependency-registry-exports-match-declared-symbol",
               "content-checksum-matches-seeded-bytes",
               "tool-callable-in-association-matches-host-call",
           ]
       }
   }).to_string();
   let inserted = bootstrap_evidence::retain_intent(&tx, q1_id, &q1_bytes).await?;
   if !inserted { /* already exists — idempotent skip */ }
   ```

4. Build the behavioral evidence and the approval record, then call
   `bootstrap_evidence::retain_completed` atomically:
   ```rust
   let behavior_bytes = json!({
       "format": "component-review-evidence/1",
       "evidence_id": behavior_id,
       "kind": "behavior",
       "validation_mode": "system_seed",
       "association_checksum": assoc_checksum,
       "components": [ /* same as q1 */ ],
       "succeeded": true,
       "reviewed_evidence": [],
       "report": {
           "checks": [
               "no-top-level-effectful-statements-during-load",
               "no-prohibited-identifiers-import-exec-eval-open",
               "typed-input-keys-accessed-by-subscript",
           ]
       }
   }).to_string();
   let approval_bytes = json!({
       "format": "skill-association-approval/1",
       "approval_id": approval_id,
       "association_checksum": assoc_checksum,
       "components": [ /* same as q1 */ ],
       "validation_mode": "system_seed",
       "q1_ref": q1_id,
       "q2_ref": null,
       "behavioral_refs": [behavior_id],
   }).to_string();
   bootstrap_evidence::retain_completed(
       &tx, behavior_id, &behavior_bytes, approval_id, &approval_bytes,
   ).await?;
   ```

All UUIDs (`q1_id`, `behavior_id`, `approval_id`) are deterministic UUID v5
derived from the exact association bytes and a fixed namespace, so the same bytes
always produce the same IDs and re-runs are idempotent.

**Acceptance:** for every L013–L101 Tool-calling pair after this step:
- `SELECT count(*) FROM reborn_skill_association_approvals WHERE approval_id=$1`
  returns 1.
- `SELECT count(*) FROM reborn_component_review_evidence WHERE evidence_id=$1`
  returns 1 for both the Q1 and behavioral evidence UUIDs.

---

### Step 6 — Qualify pure-logic components via `pure-component-approval/1`

Pure-logic PythonCode components (L007–L009, L011–L012, L024–L027, L033–L035,
L090–L091) do not go through `skill-association-approval/1`. Their standalone
Recipe steps are verified by `require_independent_execution_coverage`, which
calls `pg_pure_component_approval::load_independently_qualified`. This reads from
`reborn_pure_component_approvals`, a different table with a different format.

For each pure-logic component, call
`pg_pure_component_approval::record_pure_component_approval` with:
- A `PureComponentApprovalDeclaration` referencing existing `reborn_component_review_evidence`
  rows (Q1 + behavioral evidence produced for these components by the same evidence
  contract as Step 5, but without a ToolSkill/Tool in the evidence).
- The component UUID and revision reference from `reborn_component_revisions`
  (pure-logic components also need revision envelopes — follow the same pattern
  as Step 4 but with `"association": null` and no ToolSkill/Tool dependency).

**Acceptance:** `SELECT count(*) FROM reborn_pure_component_approvals WHERE root=$1`
returns 1 for every pure-logic component UUID.

---

### Step 7 — Wire `bootstrap_l013_approval` into `component_boot.rs`

**File:** [`crates/brassclaw_reborn_composition/src/component_boot.rs`](crates/brassclaw_reborn_composition/src/component_boot.rs).

Call the new revision-and-approval producer immediately after
`builtin_bootstrap::seed_all` completes and before workers/trigger producers
are enabled. The function takes the pool (not a live transaction — it manages
its own per-pair `RepeatableRead` transactions internally):

```rust
bootstrap_l013_approval::qualify_all(pool.clone())
    .await
    .map_err(BootError::from)?;
```

`qualify_all` is idempotent: if all revision envelopes and approval rows already
exist and their checksums match, it returns `Ok(())` without re-inserting.
If any checksum mismatches (a seeded body changed without a corresponding new
revision), it returns an error that aborts boot — the same pattern used by
`run_content_integrity_check`.

**Acceptance:** `cargo check -p brassclaw_reborn_composition --features
postgres,skills-db` clean; the boot integration path runs `qualify_all` before
the readiness gate.

---

### Step 8 — Verify IBS assembly succeeds for a sample of L013–L101 Recipes

**File:** add cases to the existing
[`routing_tests`](crates/brassclaw_reborn_composition/src/runtime/tests)
or a new dedicated `l013_assembly_tests` module.

For each domain group (network, memory, management, git, plan), pick one
representative Tool-calling Recipe and one pure-logic Recipe:

1. Seed the complete revision-store stack (Tool + ToolSkill + PythonCode +
   Skill, all as `"component-revision/1"` envelopes) via `retain_packaged_draft`.
2. Write the Q1 + behavioral evidence + association approval via the same
   `bootstrap_evidence` helpers used in Step 5.
3. Call `compile_retained_recipe` then `prepare_retained_tool_program` for
   Tool-calling Recipes; call the corresponding `RetainedUnboundProgram` path
   for pure-logic Recipes.
4. For Tool-calling Recipes, call `with_association_approvals` and assert it
   succeeds.
5. Assert `require_independent_execution_coverage` passes for pure-logic steps.

**Acceptance:** all assembly tests pass with `cargo test -p
brassclaw_reborn_composition --features integration`.

---

### Step 9 — Update `docs/implementation-status.md`

Add a new dated entry recording completion of the four gates across L013–L101.
The entry must:

- Reference the commit(s) implementing Steps 1–8.
- Confirm that all ~54 Tool-calling (Skill, PythonCode) pairs now have
  `"component-revision/1"` envelopes, `"component-review-evidence/1"` Q1 and
  behavioral records, and `"skill-association-approval/1"` rows.
- Confirm that all ~14 pure-logic components have `"pure-component-approval/1"` rows.
- Note that `content_checksum` is populated for all affected rows and
  `run_content_integrity_check` passes at boot.
- Supersede the "L013–L101: remaining Lnnn plan-entry migrations are
  body-format-complete; full qualification (exports, associations, behavioral
  evidence) remains open" bullet in every prior "Open" list.
- List any pairs that remain blocked (e.g., L018 `pc-exec-http-save` if
  `builtin.http.save` is still unregistered) — these must be explicitly kept as
  blocked rather than silently removed.

---

## 5. What this plan does NOT change

- **L010 (`pc-exec-file-exists`)** — excluded; tracked in `missing1_v3_at_k3.md`.
  Its placeholder body remains; this plan does not touch it.
- **Validator Steps 7–12** — the eight class-specific validator Recipes are not
  implemented here. Their absence does not block the `system_seed` path for
  L013–L101, which uses the bootstrap qualification path with `q2_ref: null`.
- **General standalone pure approval** — L007–L009, L011–L012, L024–L027,
  L033–L035, and L090–L091 get `pure-component-approval/1` records through the
  bootstrap path. This is distinct from the general pure-component review contract
  (`pure-component-review.md`) under validator Steps 3/8–10, which remains
  separate open work.
- **Recipe activation for shell/spawn Recipes** — Recipes that use `builtin.shell`
  or `builtin.spawn_subagent` remain Tier 1 and `llm_call_required: true`.
  Association approval for their PythonCode components enables assembly; it does
  not lower the Tier or remove the LLM step requirement.
- **L018 (`pc-exec-http-save`)** — if `builtin.http.save` remains unregistered,
  its association is authored as blocked and the Recipe that consumes it is not
  activated. This is recorded in Step 9.
- **Human Q2** — the `system_seed` path sets `q2_ref: null` and requires only
  integrity/Q1/behavioral evidence. No external human action is required for
  these first-party packaged components. Installed-instance edits after boot
  require the normal authored Q1/behavior/human-Q2 path.
- **MCP publication** — having a valid association approval does not publish any
  Recipe as an MCP command. That requires a separately qualified
  `mcp-call-skill-recipe` variant.

---

## 6. Constraints inherited from the owning architecture

- **Association is embedded in the revision, not a standalone row.** The
  `"association"` field of the `"component-revision/1"` envelope is a JSON
  string stored inside `reborn_component_revisions`. It is never a row in its
  own table. `ComponentRevisionDraft::from_json` rejects class-1 envelopes with
  a null association, and rejects non-class-1 envelopes with a non-null one.
- **The revision envelope is the source of truth for `prepare_retained_tool_program`.**
  `prepare_retained_tool_program` reads `revision.draft().association()` from the
  retained component graph. Rows in `reborn_skills` are invisible to this path.
- **One association per (Skill, PythonCode, ToolSkill, Tool) combination.**
  `prepare_retained_tool_program` collects all associations keyed by
  `(python_code_uuid, tool_skill_uuid, tool_uuid, callable)` and rejects
  ambiguous (>1) matches. Each Skill revision must carry exactly one association.
- **`retain_completed` is the only legitimate approval writer.** It writes both
  the behavioral evidence and the approval row atomically and non-idempotently
  (no `ON CONFLICT`). The preceding `retain_intent` (for Q1) uses
  `ON CONFLICT DO NOTHING` and returns `false` if the row already exists,
  signalling the pair is already qualified. Use `read_association` to short-circuit
  the full qualification when the approval row is already present and matches.
- **Approval UUIDs are deterministic (UUID v5).** The same exact association bytes
  always produce the same `approval_id`, `q1_id`, and `behavior_id`. An idempotent
  re-run produces no duplicate insert.
- **`system_seed` approval needs no installed human Q2.** Setting `q2_ref: null`
  is correct per `AGENTS.md` §"Installation seeds and post-installation changes".
  This is not a bypass; it is the defined contract.
- **Evidence table is `reborn_component_review_evidence`.** Both Q1 and behavioral
  evidence use format `"component-review-evidence/1"` and are stored in
  `reborn_component_review_evidence`. The `reborn_workflow_review_evidence` table
  is for the workflow-level (`bootstrap_pair_approval`, `bootstrap_compiler_approval`)
  bootstrap qualification path only.
- **SHA-256 checksum function is `bootstrap_evidence::checksum`.** It is defined
  in [`bootstrap_evidence.rs:9`](crates/brassclaw_reborn_composition/src/bootstrap_evidence.rs)
  as `hex::encode(Sha256::digest(bytes.as_bytes()))`. Use the same function for
  all content checksums, association checksums, and evidence checksums.
- **Pure-logic components use `reborn_pure_component_approvals`, not
  `reborn_skill_association_approvals`.** A null-tool `skill-association/1` record
  is not a valid substitute. Pure steps are checked by
  `require_independent_execution_coverage` via `load_independently_qualified`.
- **Reuse compatible ToolSkill UUIDs.** Multiple Skills wrapping the same callable
  may reference the same ToolSkill UUID in their association. The retained resolver
  keys on `(python_code_uuid, tool_skill_uuid, tool_uuid, callable)` — a shared
  ToolSkill UUID is not ambiguous if the python_code_uuid differs.

---

## 7. Review corrections summary (2026-11-01)

The plan was reviewed against the live source immediately after authoring. Five
errors were found and corrected. No code was changed by the review; all
corrections are to plan text only.

| # | Severity | Error in original plan | Correction |
|---|----------|----------------------|------------|
| 1 | **Critical** | Step 4 called `bootstrap_evidence::upsert_association(tx, ...)` — this function does not exist. Association is never a standalone DB row; it is embedded as a JSON string in the `"association"` field of the `"component-revision/1"` envelope stored in `reborn_component_revisions`. Steps 4 and 5 also incorrectly implied that `reborn_skills` rows are sufficient for the approval path. | **Rewritten as Step 4.** The prerequisite is to create `"component-revision/1"` envelopes in `reborn_component_revisions` using `retain_packaged_draft`, embedding the association JSON string directly in the Skill envelope. Steps 4 and 5 merged into this correct sequence. |
| 2 | **Critical** | Step 5 specified writing Q1 and behavioral evidence to `reborn_workflow_review_evidence` with format `"workflow-review-evidence/1"`. | Corrected to `reborn_component_review_evidence` with format `"component-review-evidence/1"` — the table and format used by all component-level bootstrap approval modules (`bootstrap_encoding_approval.rs`, `bootstrap_compiler_approval.rs`). |
| 3 | **Moderate** | Step 3 named `brassclaw_engine::executor::evidence::checksum` as the reference function. This path does not exist. | Corrected to `bootstrap_evidence::checksum` defined in `bootstrap_evidence.rs:9`: `hex::encode(Sha256::digest(bytes.as_bytes()))`. |
| 4 | **Moderate** | Step 5 sub-step 5 named `bootstrap_evidence::upsert_association_approval` — this function does not exist. | Removed. The approval insert is performed by `retain_completed`, which atomically writes both behavioral evidence and the approval row. |
| 5 | **Minor** | §3 and Steps 4–5 listed pure-logic components (L007–L009, L011–L012, L024–L027, L033–L035, L090–L091, ~17 pairs) as needing `skill-association-approval/1` records. They go through a completely different path: `pure-component-approval/1` in `reborn_pure_component_approvals`. | Scope table split into Tool-calling vs pure-logic rows. Step 6 added for pure-logic approval. §5 and §6 constraints updated to reflect the correct mechanisms for each path. |
