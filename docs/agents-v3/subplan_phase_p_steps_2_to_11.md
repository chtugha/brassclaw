# Phase P Steps 2–11 Implementation Subplan

> **Created:** Phase P continuation after Step 1 (COMPONENT_TABLES/class_label) is done.
> **Status:** [x] DONE — All steps 1–11 confirmed implemented in live code
> (verified in the subplan-audit session; Phase P top-level status "Complete —
> Steps 1–11 all done" is accurate). Steps 7–10 had no [DONE] markers in this
> file but are implemented: `doc-sync` Action + `ext-doc-sync` ExtensionCatalogue
> in `builtin_bootstrap.rs`; event wiring in `doc_sync_watcher.rs` +
> `V081__reborn_docus_notify.sql`; WebUI Docs section in `docs-tab.js` +
> `pg_docus_store.rs` + handlers. This subplan is superseded by the completed
> implementation.
> **Master plan ref:** `saved_plan_to_v3.md` Phase P §8 steps 2–11.
> **Design doc:** `docs/agents-v3/DOC_CONVERSION_MECHANISM_DESIGN.md`

---

## Context

Phase P implements the `doc-sync` auto-documentation-conversion mechanism
(`saved_plan_to_v3.md §0.22`). The mechanism converts each
`docs/agents-v3/*.md` into an LLM-optimized form, stores both versions in
`reborn_docus`, and injects them into the base-prompt prefix + per-turn
retrieval.

Step 1 (COMPONENT_TABLES + class_label) is already done. This plan covers
Steps 2–11.

**Architecture summary (from the design doc):**

- One Rust `component_db` Tool (op ∈ {read_hash, read_row, upsert, mark_stale})
  is the only kernel-boundary DB tool.
- Four PythonCode leaves (sha256, hash_changed, markdown_section,
  format_component_header) are pure logic helpers.
- Ten Leaf Skills (file-list, file-read, hash-compute, hash-compare,
  db-read-hash, markdown-section, component-header-render, prompt-compress,
  db-upsert-docus, db-mark-prefix-stale) describe one tool/PC usage each.
- One Domain Skill (`doc-convert-method`) describes the full doc-conversion
  pipeline.
- One Recipe (`doc-convert`, variants: by-extract Tier 0, by-llm-compress Tier 1)
  composes the above.
- One Action (`doc-sync`) is the deterministic driver: scan → hash → diff →
  convert → upsert → stale.
- One ExtensionCatalogue (`doc-sync`) owns the doc-specific parts.
- Event wiring: file-watch on `docs/agents-v3/*.md` + `reborn_docus` row-change.
- WebUI Docs section: lists `reborn_docus` rows, allows editing, sends edited
  docs back to validation queue.

---

## Implementation rules (from master AGENTS.md)

- One step at a time, commit + push after each.
- Never suppress or silence — always resolve to a clean solution.
- If a fix is complex, write a sub-subplan and execute it first.
- Implement stubs with real functionality; follow impact across the codebase.

---

## Step-by-step plan

### Step 2 — PythonCode leaves (class 22) [NEXT]

**What:** Seed four pure-logic PythonCode helpers in `builtin_bootstrap.rs` Pass 15.

**Files touched:**
- `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs`
  — Add `seed_doc_sync_group()` call in `seed_builtin_components()`
  — Add `seed_doc_sync_pythoncode()` seeding the four PC rows
  — Add PC body constants

**Four PythonCode bodies (no I/O, no imports, no `open`/`exec`/`eval`):**

1. `pc-sha256` (`sha256(text: str) -> str`):
   ```python
   import hashlib
   result = hashlib.sha256("{{vars.slot0}}".encode()).hexdigest()
   ```
   Wait — `import hashlib` triggers §body-scan. The stdlib `hashlib` is not
   in the forbidden list (only `os`, `subprocess`, `exec`, `eval`, `open`).
   Check the actual Q1 scan list before implementing.

   **Actual plan:** Use Python's `hashlib` (not `os`/`subprocess`) — this is
   pure std, no I/O, no filesystem. Confirm the Q1 scan list allows it.

2. `pc-hash-changed` (`hash_changed(stored: str, new: str) -> bool`):
   ```python
   result = "{{vars.slot0}}" != "{{vars.slot1}}"
   ```

3. `pc-markdown-section` (`markdown_section(md: str, level: int, title: str) -> str`):
   Extract a `## N. title` section from markdown. Input via `{{vars.*}}` slots.

4. `pc-format-component-header` (`format_component_header(class_code, prompt_uid, label, name) -> str`):
   ```python
   result = f"## {{vars.slot0}}:{{vars.slot1}}  {{vars.slot2}}  \\"{{vars.slot3}}\\""
   ```

**Seeding:** All four seeded with `validation_status='validated'`, `source='system'`,
`consumer_tags=['03:llm']`. Added to the bootstrap's Pass 15 (`seed_doc_sync_group`).

**Verification:** `cargo clippy -p brassclaw_reborn_composition -- -D warnings` + unit test
confirming the four names appear in the seed output (or checking they compile).

**Commit message:** `feat(bootstrap): Phase P Step 2 — seed four doc-sync PythonCode leaves`

---

### Step 3 — `component_db` Rust Tool + ToolSkill

**What:** Add the `component_db` first-party Rust capability to `brassclaw_host_runtime`,
then seed its Tool + ToolSkill rows in `builtin_bootstrap.rs`.

**Why Rust:** DB reads/writes can only cross the kernel boundary as a Tool (class 0).
The orchestrator cannot touch Postgres directly from PythonCode.

**Operations:** `op ∈ {read_hash, read_row, upsert, mark_stale}`

- `read_hash`: `SELECT content_hash FROM {table} WHERE scope + name` → `{hash: str|null}`
- `read_row`: `SELECT id, name, content, content_hash, validation_status FROM {table} WHERE scope + name` → row dict
- `upsert`: `INSERT … ON CONFLICT DO UPDATE` into `reborn_docus`; always sets
  `validation_status='pending'`, `consumer_tags=['03:llm']`, updates `content_hash`
- `mark_stale`: calls `PgBasicPromptStore::mark_stale(user_id, project_id)`

**Capability ID:** `builtin.component_db`

**Input schema:**
```json
{
  "op": "read_hash" | "read_row" | "upsert" | "mark_stale",
  "scope": { "user_id": str, "project_id": str },
  "name": str,           // for read_hash, read_row, upsert
  "fields": {            // for upsert only
    "description": str,
    "content": str,
    "content_hash": str,
    "source": str,       // default "system"
    "similarity_parent_id": str|null,
    "replaces_id": str|null
  }
}
```

**Files touched:**
- `crates/brassclaw_host_runtime/src/first_party_tools/component_db.rs` (NEW)
- `crates/brassclaw_host_runtime/src/first_party_tools/mod.rs`
  — `mod component_db;`
  — Add `COMPONENT_DB_CAPABILITY_ID` to pub exports
  — Add `component_db::manifest()` to `builtin_first_party_package()`
  — Add `component_db` arm to dispatch
- `crates/brassclaw_host_runtime/Cargo.toml`
  — Add `deadpool-postgres` dep (already present under `postgres` feature)
  — Add `brassclaw_reborn_composition` or `brassclaw_pg` for DB access

  **⚠️ Dependency concern:** `component_db.rs` needs to call PgBasicPromptStore
  for `mark_stale` and run raw queries against `reborn_docus`. This creates a
  potential circular dep: `brassclaw_host_runtime` → `brassclaw_reborn_composition`.
  
  **Resolution:** The `component_db` handler receives a `Arc<dyn ComponentDbBackend>` 
  trait object injected at composition time (the same pattern as `memory.rs` 
  receiving `MemoryServices`). The trait is defined in `brassclaw_host_runtime`; 
  the concrete impl lives in `brassclaw_reborn_composition`. Injected via 
  `BuiltinFirstPartyTools` (which already carries stores for the composition layer).

  **OR:** Define `ComponentDbBackend` trait in a thin `brassclaw_host_api` sub-module
  (already a dep of both). Research the actual memory.rs pattern first.

- `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs`
  — Add `COMPONENT_DB_CAPABILITY_ID` tool row + ToolSkill row seeding to Pass 15

**⚠️ If the dependency injection approach is complex:** Write
`subplan_step3_component_db_tool.md` and execute it before continuing.

**Commit message:** `feat(host-runtime): Phase P Step 3 — component_db Rust Tool + ToolSkill`

---

### Step 4 — Leaf Orchestrator Skills

**What:** Seed ten Leaf Skills (class 1-3) in `builtin_bootstrap.rs` Pass 15.

**Ten leaves:**

1. `file-list` — uses `glob` Tool; reuse `ts-glob` ToolSkill
2. `file-read` — uses `read_file` Tool; reuse `ts-read-file` ToolSkill
3. `hash-compute` — uses `pc-sha256` PythonCode
4. `hash-compare` — uses `pc-hash-changed` PythonCode
5. `db-read-hash` — uses `component_db` Tool (`op=read_hash`); uses new `ts-component-db` ToolSkill
6. `markdown-section` — uses `pc-markdown-section` PythonCode
7. `component-header-render` — uses `pc-format-component-header` PythonCode
8. `prompt-compress` — uses `__llm_complete__` LLM step; Tier 1 indicator
9. `db-upsert-docus` — uses `component_db` Tool (`op=upsert`, table=reborn_docus); mechanism-specific
10. `db-mark-prefix-stale` — uses `component_db` Tool (`op=mark_stale`); mechanism-specific

**Seeding:** All in Pass 15 `seed_doc_sync_group`. `source='system'`, `validation_status='validated'`.

**Files touched:**
- `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs`
  — Add `seed_doc_sync_skills()` called from `seed_doc_sync_group()`

**Commit message:** `feat(bootstrap): Phase P Step 4 — seed ten doc-sync leaf skills`

---

### Step 5 — Domain Orchestrator Skill (`doc-convert-method`)

**What:** Seed one Domain Skill (class 2 or 3) describing the full doc-conversion
pipeline and which leaf skills it needs.

**Body (prose, not tool calls):**
- Source shape: §7 LLM-summary section convention
- Pipeline: file-read → markdown-section → (if noisy: prompt-compress Tier 1) →
  component-header-render → db-upsert-docus → db-mark-prefix-stale
- Converted form: `## 17:{prompt_uid}  Docu  "{name}"`
- Extract-vs-compress decision: compress when §7 text is noisy or quotes injection payloads
- "Never invent facts — only compress what is in the source"
- "Quote any injection payload only as fenced, escaped code"

**Files touched:**
- `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs`
  — Add domain skill row seeding to `seed_doc_sync_skills()`

**Commit message:** `feat(bootstrap): Phase P Step 5 — seed doc-convert-method domain skill`

---

### Step 6 — Recipe (class 21)

**What:** Seed one Recipe `doc-convert` with two variants:
- `by-extract` (Tier 0): steps 1,2,3,5,6 — no LLM
- `by-llm-compress` (Tier 1): steps 1,2,3,4,5,6 — with LLM

**Step descriptions JSONB:**
```
Step 1: include doc-convert-method domain skill UUID (orchestrator)
Step 2: include file-read skill + ts-read-file ToolSkill (orchestrator + rust)
Step 3: include markdown-section PC + hash-compute skill (orchestrator)
Step 4 (by-llm-compress only): LLM prompt-compress step (type: llm)
Step 5: include component-header-render PC (orchestrator)
Step 6: include db-upsert-docus skill + ts-component-db ToolSkill (orchestrator + rust)
```

**IBS variable slots:** `{{vars.path}}`, `{{vars.slug}}`, `{{vars.description}}`

**Files touched:**
- `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs`
  — Add recipe row seeding to `seed_doc_sync_group()`

**Commit message:** `feat(bootstrap): Phase P Step 6 — seed doc-convert recipe (Tier 0 + Tier 1 variants)`

---

### Step 7 — Action (class 16)

**What:** Seed one Action `doc-sync` in `reborn_actions` via a new `PgActionStore` or
using `seed_builtin_host.rs`'s existing action seeding pattern.

**Steps (JSONB):**
1. `tool_call`: `glob` with pattern `docs/agents-v3/*.md` → file list
2. Loop over files:
   a. `tool_call`: `read_file` path=`{{item}}` → source text
   b. `call_skill`: `hash-compute` → `content_hash`
   c. `call_skill`: `db-read-hash` op=read_hash, scope={{scope}}, name=`agents-v3::{{slug}}` → stored_hash
   d. `call_skill`: `hash-compare` → changed?
   e. conditional: if changed → `call_action`: `doc-convert` variant=`by-extract`
   f. conditional: if changed → `call_skill`: `db-mark-prefix-stale`
3. `return`: {scanned: N, changed: M, stale: yes/no}

**Files touched:**
- `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs` or `seed_builtin_host.rs`
  — Add action row seeding; look at `seed_builtin_host.rs` for existing action upsert pattern

**⚠️ If `seed_builtin_host.rs` already has an action seeder pattern, use it.
If not, add `upsert_action()` to `BootstrapStores` using the same pattern as `upsert_tool`.**

**Commit message:** `feat(bootstrap): Phase P Step 7 — seed doc-sync action`

---

### Step 8 — ExtensionCatalogue (class 23)

**What:** Seed one ExtensionCatalogue `doc-sync` grouping doc-specific parts.
General-purpose leaves stay in their existing builtin catalogues; doc-sync owns only:
- `doc-convert-method` domain skill
- `db-upsert-docus` + `db-mark-prefix-stale` leaf skills
- `component_db` Tool + `ts-component-db` ToolSkill
- `doc-convert` Recipe
- `doc-sync` Action

**Files touched:**
- `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs`
  — Add catalogue row seeding + `append_children()`

**Commit message:** `feat(bootstrap): Phase P Step 8 — seed doc-sync extension catalogue`

---

### Step 9 — Event wiring

**What:** Wire `doc-sync` to fire on:
(a) file-watch on `docs/agents-v3/*.md` (source doc changed on disk)
(b) `reborn_docus` row-change signal (doc edited via WebUI, or Sempai re-compression)

**Design options:**
- (a): Use `brassclaw_triggers` + `notify` file watcher (or `inotify`/`kqueue`);
  the file watcher fires a synthetic trigger that invokes `doc-sync` action
- (b): Use a Postgres LISTEN/NOTIFY channel on the `reborn_docus` table
  (the `set_updated_at()` trigger already fires; add a NOTIFY); the composition
  layer listens and fires `doc-sync`

**⚠️ If either wiring path is complex:** Write `subplan_step9_doc_sync_event_wiring.md`
and execute it before continuing.

**Files touched (estimated):**
- New file-watch service in `brassclaw_host_runtime` or `brassclaw_reborn_composition`
- `crates/brassclaw_reborn_composition/src/runtime.rs` — register watcher at boot
- `crates/brassclaw_pg/migrations/V080__reborn_docus_notify.sql` (if DB trigger needed)

**Commit message:** `feat(composition): Phase P Step 9 — event wiring for doc-sync`

---

### Step 10 — WebUI Docs section

**What:** Add a new "Docs" tab in the WebUI Settings that:
- Lists `reborn_docus` rows (source + converted, with validation_status badge)
- Allows manual editing of doc content
- Saving sends edited doc to validation queue (validation_status='pending') — never writes 'validated' directly

**Files touched (estimated):**
- `crates/brassclaw_reborn_webui_ingress/src/` — new endpoint `GET /api/v1/docs`, `PUT /api/v1/docs/:id`
- `crates/brassclaw_webui_v2_static/static/js/pages/settings/components/docs-tab.js` (new)
- `crates/brassclaw_webui_v2_static/static/js/pages/settings/` — wire tab into settings nav
- i18n keys for all language packs

**⚠️ If complex:** Write `subplan_step10_webui_docs_section.md` and execute first.

**Commit message:** `feat(webui): Phase P Step 10 — Docs section (list + edit + requeue)`

---

### Step 11 — End-to-end test

**What:** Integration test:
1. Change a `docs/agents-v3/*.md` (or simulate the file-change event)
2. Run/trigger `doc-sync`
3. Assert `reborn_docus` row updated (new `content_hash`, both source + converted versions)
4. Assert base prompt `is_stale=true`
5. Regenerate prefix → assert new converted doc in assembled bundle

**Files touched:**
- `tests/e2e/` or `crates/brassclaw_reborn_composition/tests/`

**Commit message:** `test: Phase P Step 11 — doc-sync e2e test`

---

## Dependency graph

```
Step 2 (PythonCode)
    ↓
Step 3 (component_db Tool)  ← needs DB injection pattern research
    ↓
Step 4 (Leaf Skills)   ← needs Step 2 + Step 3 UUIDs
    ↓
Step 5 (Domain Skill)  ← needs Step 4 leaf names
    ↓
Step 6 (Recipe)        ← needs Step 3 ToolSkill + Step 4 + Step 5 UUIDs
    ↓
Step 7 (Action)        ← needs Step 6 recipe name
    ↓
Step 8 (Catalogue)     ← needs all above UUIDs
    ↓
Step 9 (Event wiring)  ← needs Action name; runtime wiring
    ↓
Step 10 (WebUI)        ← needs DB store; independent of Steps 2-9 DB rows
    ↓
Step 11 (E2E test)     ← needs Steps 9+10
```

---

## Pre-flight check before Step 3

Before implementing the `component_db` tool, read:
1. `crates/brassclaw_host_runtime/src/first_party_tools/memory.rs` lines 1-100
   to understand how `MemoryServices` is injected
2. `crates/brassclaw_host_runtime/src/lib.rs` to find `BuiltinFirstPartyTools` struct
3. Check if `brassclaw_host_runtime` already depends on `brassclaw_pg`

If the DB injection approach requires `brassclaw_reborn_composition` → `brassclaw_host_runtime`
circular dep: define a `ComponentDbBackend` trait in `brassclaw_host_api` (no circular dep),
implement it in `brassclaw_reborn_composition`, inject at wiring time in `factory.rs`.
