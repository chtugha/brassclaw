> **Recipe architecture precedence (2026-10-06):** This document preserves an
> earlier plan or implementation record. Its component granularity, input
> substitution, fresh-step state and mutable-version descriptions are superseded
> where they conflict with [recipe.md](recipe.md). One component per Recipe step
> permits internal PythonCode composition; IBS must pin immutable versions,
> including nested components, and preserve typed task input/result flow.
> Do not implement obsolete examples as new requirements.

# Prefix Plan 2 — Bundle Assembly via Recipe (Architecture-Correct v11)

*Audit date: post-codebase-audit v4. Corrective revision v10 applied per v10.md.
Line references from v10 must be re-verified before implementation.*

---

## Ground Truth: How the Execution Model Works

### The two-channel execution model

```
channel: "rust"           → pre-loads a ToolSkill binding into the Monty namespace (does NOT execute)
channel: "orchestrator"   → PythonCode body is run via host.run_program(step["executable_code"])
```

### Step isolation invariant (CRITICAL — runtime behaviour, not an option)

`compose_program()` (`brassclaw_engine/src/memory/composition.rs:235`) builds
`assembled_program` by joining all orchestrator PythonCode bodies with `"\n\n"`.
This string is carried as a convenience field on `ComposedProgram` (documented as
"pre-assembled fallback / single-shot path").

**The primary execution path is NOT `assembled_program`.**

Monty's `pc-host-compose-orchestrator` PythonCode (`seed_builtin_host.rs:676–677`):

```python
for step in program["steplist"]:
    host.run_program(step["executable_code"])
```

`host.run_program` (`orchestrator.rs:1224–1262`) runs each step via a **nested
`execute_code` call with a fresh empty state**:

```rust
let fresh_state = serde_json::json!({});
execute_code(&code, thread, None, effects, leases, policy, &exec_ctx, &[], &fresh_state)
```

`execute_tier_zero_channel` (`orchestrator.rs:2078`) applies the same pattern.
Both paths document this explicitly:

> **ISOLATION INVARIANT: no variables are shared between steps; each step sees
> only the IBS-baked-in literals from `{{vars.slot0}}` substitution.**

**Consequence:** a Python variable assigned in step N is invisible in step N+1.
Any recipe that relies on cross-step variable sharing will produce a `NameError`
at runtime and degrade to Tier 2.

### Consequence for recipe design

Each orchestrator PythonCode step must be **self-contained**. If the output of
one tool call is needed by a subsequent tool call, both calls must be in the
**same PythonCode body** (one step), sharing local scope within one
`host.run_program` execution. This is the Rule 3 dependent-chain exception.

### How Skills fit in

A Recipe's `step_descriptions` `include` array can reference both a PythonCode UUID
and Skill UUIDs in the same step. `compose_program()` (lines 161–178 of
`composition.rs`) processes each step's `include` list:

- First UUID whose component is class 22 (PythonCode) → `executable_code`
  (run via `host.run_program` per step)
- Any UUID whose component is class 1–3 or 13 (Skill / ToolSkill) → appended to
  `program.skills` (read-only narrative context Monty consults, not executed)

### Rust provides primitives; the orchestrator sequences them

The orchestrator is the execution engine. Rust provides **primitives** — the smallest
independently useful operations. A Rust tool must not contain sequencing logic; it
executes one operation and returns a result.

For the prefix bundle:
- `host.sweep_validated_components` sweeps only validated, Q2-cleared component rows, formats
  the bundle string, and returns `{bundle: String, component_count: usize, generation_ms: i64}`.
- `host.store_prefix_bundle` takes a pre-assembled bundle string and stores it in
  `PgBasicPromptStore`, returning `{fingerprint, generation_ms}`.

These two tools are logically separate — the sweep can be called without storing,
and the store can be called with a bundle assembled by any means. They are chained
in **one PythonCode body** because the store call's `bundle` parameter is the direct
runtime output of the sweep call. A single `host.run_program` execution runs both
calls in one Python scope, so `bundle_parts` is available when `host.store_prefix_bundle`
is called. This satisfies the isolation invariant (no cross-step sharing) while keeping
both tools independently reusable. See the Rule 3 dependent-chain exception in AGENTS.md.

---

## Goal

1. **Remove the `05:validator` consumer-tag filter**, but keep `validation_status = 'validated'` as the hard gate everywhere; this intentionally supersedes the legacy `05:validator` consumer-tag exclusion. Pending/rejected/unvalidated components must never enter the base prompt or be called, executed, loaded, or used by any agent subsystem. Every component seeded on installation must already be validated before runtime availability. Consumer tags remain routing metadata, not proof of validation.
2. **Include** CLAUDE.md (from `## Architecture`) and AGENTS.md (from
   `## Architecture Mental Model`) in every assembled bundle.
3. **Move** the assembly and store logic into **two focused Rust tools** behind the
   two-channel recipe model, so the orchestrator controls the flow.
4. **Replace** `regenerate_prefix`'s direct call to `do_assemble_bundle` with a turn
   submission through a trusted internal-turn API, routed by intent to the new recipe with Tier-2 fallback disabled.
5. **Two Rust primitives, chained in one PythonCode body.**
   `host.sweep_validated_components` (DB sweep + format) and
   `host.store_prefix_bundle` (persist to `PgBasicPromptStore`) are two separate
   Rust tools — each reusable independently by other recipes. They are chained
   in a single PythonCode executor body (`pc-host-assemble-prefix-bundle`) so
   that the `bundle` string produced by the sweep is directly passed to the
   store call within the same `host.run_program` execution context. No cross-step
   variable dependency; both calls share local scope inside one orchestrator step.
   See the Rule 3 carve-out in AGENTS.md (dependent sequential chain exception).

---

## Current State (verified)

| Symbol | File | Lines |
|--------|------|-------|
| `COMPONENT_TABLES` const | `interceptor_config_service.rs` | 61–155 |
| `class_label()` fn | `interceptor_config_service.rs` | 158–187 |
| `do_assemble_bundle()` — includes `05:validator` filter | `interceptor_config_service.rs` | 288–411 |
| `do_format_bundle()` | `interceptor_config_service.rs` | 417–443 |
| `regenerate_prefix` calling `do_assemble_bundle` | `interceptor_config_service.rs` | 578–580 |
| First-party capability seeder | `builtin_bootstrap.rs` | `seed_builtin_components()` — Passes 1–16 |
| Host orchestrator seeder | `seed_builtin_host.rs` | Slices 1–12 (ends ~line 2020) — **not touched** |
| `BootstrapStores::seed_recipe()` helper | `builtin_bootstrap.rs` | 637 |
| `BootstrapStores::mark_recipe_tier0()` | `builtin_bootstrap.rs` | 477 |
| `BootstrapStores::audit_builtin_graduation()` | `builtin_bootstrap.rs` | 211 — returns `()`, called without `?` |
| `BootstrapStores::append_children()` | `builtin_bootstrap.rs` | 449 — method name is `append_children`, not `append_child_component_ids` |
| `recipe_row()` factory | `builtin_bootstrap.rs` | 3740 |
| `step_entry()` helper | `builtin_bootstrap.rs` | 3707 |
| Internal turn submission | `runtime.rs` | add/verify a trusted internal-turn entrypoint that prevents Tier-2 fallback |
| `RebornRuntime::new_conversation()` | `runtime.rs` | 912 — `pub` |
| `compose_program()` join | `brassclaw_engine/src/memory/composition.rs` | 235 |
| Interceptor service wiring cfg gate | `webui.rs` | 485 — `#[cfg(all(postgres, root-llm-provider))]` |
| `runtime` type at `webui.rs:485` | `webui.rs` | 48 — `runtime: &RebornRuntime` |

Nothing seeded. No new tools exist.

### Known bugs in current code that must NOT be ported

**`COMPONENT_TABLES` — phantom entries and mislabelled rows:**

The current `COMPONENT_TABLES` at lines 144–154 lists two entries that reference
tables that **do not exist**:

```rust
("reborn_orchestrators", 10, "COALESCE(NULLIF(prior_knowledge_content,''), body)"),
("reborn_scaffolds",     50, "COALESCE(NULLIF(prior_knowledge_content,''), body)"),
```

CLAUDE.md is explicit: *"Classes 10 and 50 are NOT separate tables — Orchestrator
and Scaffold rows live in `reborn_skills`, filtered by `class_code`."* The
`information_schema` existence check silently skips both entries. These rows are dead
code — remove them in `pg_sweep_validated_components_backend.rs`.

The `reborn_skills` entry (class_code hardcoded `1`) actually returns **all** rows
from that table regardless of class_code (the SQL has no `class_code` filter). This
means class 10 (Orchestrator) and class 50 (Scaffold) rows from `reborn_skills` do
appear in the bundle, but are mislabelled as `"1:xxx Skill"` in the bundle header
instead of `"10:xxx Orchestrator"` / `"50:xxx Scaffold"`.

**Fix in the new backend:** add `class_code` to the SELECT for `reborn_skills` and
use the actual DB value in the tuple, instead of the hardcoded `1`. For all other
tables, the hardcoded value remains correct (each has a single class code). The tuple
type widens to `(u16, u32, String, String)` where the `u16` for `reborn_skills` comes
from the DB row rather than the COMPONENT_TABLES constant.

Concretely, the new backend does not use `COMPONENT_TABLES` at all. It queries each
known table directly. For `reborn_skills` the query is:

```sql
SELECT class_code, prompt_uid, name,
       COALESCE(NULLIF(prior_knowledge_content,''), body) AS content
FROM reborn_skills
WHERE validation_status = 'validated'
  AND NOT ('05:validator' = ANY(COALESCE(consumer_tags, ARRAY[]::text[])))
ORDER BY prompt_uid ASC
LIMIT 1000
```

For every other table the `class_code` is the known constant for that table (no
`class_code` column needed in the SELECT).

**`COMPONENT_TABLES` unit tests** at `interceptor_config_service.rs:717–744` assert
that `("reborn_docus", 17)`, `("reborn_python_code", 22)`, and
`("reborn_extension_catalogues", 23)` are present. These tests are for the **existing
`do_assemble_bundle`** path, not the new backend. They must be left in place. The new
backend has its own tests.

### Seeder architecture (critical)

**`seed_builtin_host.rs`** (`HostStores`) seeds only Step 27 `host.*` orchestrator
infrastructure (resolve_intent, compose_orchestrator, post_reply, etc.). It has **no**
`mark_recipe_tier0`, no `audit_builtin_graduation`, no `seed_recipe()` helper.

**`builtin_bootstrap.rs`** (`BootstrapStores`) is the Phase L seeder for all
first-party capability tools (Passes 1–16). It has `seed_recipe()`, `mark_recipe_tier0()`,
and the Phase P.0 audit trail (`audit_builtin_graduation`). Called from `webui.rs:200`
via `seed_builtin_components()`.

The new `host.sweep_validated_components` and `host.store_prefix_bundle` are
**capability tools** — they belong in `builtin_bootstrap.rs` as Pass 17, not in
`seed_builtin_host.rs`.

---

## New Flow

**Trusted-scope rule:** the HTTP handler validates `WebUiAuthenticatedCaller` and the requested target scope, creates a random one-use `scope_ticket` bound to that caller and the new conversation, and passes only that ticket in an internal turn. The two tools accept no raw user/project identifiers. Their shared backend resolves the ticket to the authorized scope; storing consumes it atomically. A regular user-authored message cannot mint or use such a ticket. The internal turn is marked in trusted request metadata; recipe miss or execution failure must stop before Tier 2 (no general LLM fallback).

```
Operator clicks Regenerate (WebUI)
  → POST /api/prefixes/base-prompt/regenerate
    → RebornInterceptorConfigService::regenerate_prefix
        → rate-limit check (unchanged)
        → RebornRuntime::new_conversation().await                                 ← fresh thread
        → validate authenticated caller and requested scope; create one-use scope ticket + request UUID bound to this conversation
        → runtime.send_internal_user_message(
              conversation,
              "regenerate prefix bundle ticket={scope_ticket}",
              trusted internal metadata: hard-fail recipe miss; disable Tier 2
          ).await                                                                  ← internal Tier-0 turn

        → turn arrives at Monty
        → intent system matches "regenerate prefix bundle …"
              → routes to host-assemble-prefix-bundle Recipe (class 21)
        → host.compose_orchestrator → program (3 PythonCode steps):

              # Step 3 — pc-host-assemble-prefix-bundle (single body, two chained calls)
              bundle_parts = host.sweep_validated_components(
                  scope_ticket="{{vars.slot0}}"
              )
              # bundle_parts = {bundle: String, component_count: usize, generation_ms: i64}
              result = host.store_prefix_bundle(
                  scope_ticket="{{vars.slot0}}",
                  bundle=bundle_parts["bundle"],
                  generation_ms=bundle_parts["generation_ms"]
              )
              # result = {fingerprint, generation_ms, generation_id}

              # Step 5 — pc-host-post-reply (existing, reused)
              result = host.post_reply(answer="Bundle assembled.")

        → host.sweep_validated_components (Rust):
              • queries only rows with validation_status = 'validated'; consumer_tags do not override status
              • uses actual class_code from DB for reborn_skills rows
              • appends CLAUDE.md from "## Architecture"
                (constant from crate::builtin_bootstrap::CLAUDE_MD_SEED)
              • appends AGENTS.md from "## Architecture Mental Model"
                (constant from crate::builtin_bootstrap::AGENTS_MD_SEED)
              • appends Sempai Response Schema footer
              • returns {bundle: String, component_count: usize, generation_ms: i64}

        → host.store_prefix_bundle (Rust):
              • resolves the one-use scope ticket against trusted conversation context; atomically consumes it and obtains authorized scope + request UUID
              • calls PgBasicPromptStore::store for that scope with the request UUID
              • returns {fingerprint, generation_ms, generation_id}

        → host.post_reply emits the reply to the turn
        → turn completes; internal reply status is checked

      ← regenerate_prefix reads PgBasicPromptStore::get_for_scope(authorized user_id, project_id)
      → exact generation_id guard (cold-start-safe; unrelated/concurrent writes cannot satisfy it)
      → optional gateway prewarm (unchanged logic)
      → return PrefixRegenerateResponse {fingerprint, assembled_at, generation_ms, …}
```

**Cold-start note:** On first-ever boot `pre_assembled_at` will be `None`.
The request UUID comparison succeeds on first assembly and cannot be satisfied by an unrelated or concurrent write.

---

## Changes Required

### Cross-cutting invariants (must be implemented before the recipe is enabled)

- **Validation gate:** all component reads used for the prefix require `validation_status = 'validated'`; do not use `consumer_tags` as a substitute for validation state. No pending/rejected/unvalidated component may enter the bundle or be loaded, called, executed, or otherwise used by an agent subsystem. Apply this invariant to every runtime lookup and capability-dispatch path, not only prefix assembly.
- **Installation seeding:** every component seeded by installation/bootstrap (all passes, including Pass 17 tools, ToolSkills, PythonCode, Skills, Recipe, and catalogue) is persisted with `source = 'system'` and `validation_status = 'validated'` in its seed/upsert operation. Do not insert installation-seeded built-ins as `pending` and patch their status afterward. Update seeder APIs that currently force pending status so they accept or derive the validated status for trusted system seeds. Never mark user-authored pending components validated. Add a global fresh-install/reseed invariant test over all seeded built-ins, checking the status written by the seed operation.
- **Scope authority:** raw scope IDs are not recipe inputs. Use a random short-lived one-use scope ticket bound to the authenticated caller, exact conversation, target scope, and a request UUID. Validate caller access before issuance and consume the ticket atomically on store. Acquire a per-scope regeneration lease before submitting the turn and hold it through response verification/prewarm so concurrent requests serialize and each request can verify its own generation ID.
- **Request success/concurrency:** persist a `generation_id` (UUID) with each prefix-store write and return it in the entry. Verify the exact request UUID, not a timestamp delta, after the turn. Add the migration and update `PgBasicPromptStore` DTO/signatures. Propagate the initial store read error; do not convert it to `None`.
- **Fail closed:** internal regeneration turns must hard-fail on recipe miss, validation failure, or tool error before invoking the ordinary Tier-2 no-match path. Check `AssistantReply` terminal status as well as the stored `generation_id`.

### Change 1 — Two New Rust Tools

#### Change 1a — `host.sweep_validated_components`

**New file**: `crates/brassclaw_host_runtime/src/first_party_tools/sweep_validated_components.rs`

Pattern: identical to `component_db.rs` — backend trait injected from composition.

**Capability ID:** `host.*` prefix matches the orchestrator-infrastructure tools seeded in
`seed_builtin_host.rs` (e.g. `host.post_reply`, `host.resolve_intent`). Capability tools
seeded in `builtin_bootstrap.rs` as Pass 17 use the same `host.*` prefix because they are
host-runtime-backed tools invoked by the Monty VM, not `builtin.*` pure-sandbox primitives.

```rust
pub const SWEEP_VALIDATED_COMPONENTS_CAPABILITY_ID: &str = "host.sweep_validated_components";
```

**Backend trait** (`SweepValidatedComponentsBackend`, `async_trait`):

The sweep validates the one-use ticket against trusted conversation context but does not consume it; the paired store operation consumes it atomically. This lets both primitives bind to the same authorized scope and prevents ordinary turns from invoking either tool.

```rust
async fn sweep_and_format(
    &self,
    scope_ticket: &str,
) -> Result<SweepValidatedComponentsResult, SweepValidatedComponentsError>;

pub struct SweepValidatedComponentsResult {
    pub bundle: String,
    pub component_count: usize,
    pub generation_ms: i64,
}
```

**`handle(state, params)`:** extracts `user_id`, `project_id`; calls backend;
returns `json!({"bundle": result.bundle, "component_count": result.component_count, "generation_ms": result.generation_ms})`.

**`manifest()`:** `vec![EffectKind::ExternalWrite, EffectKind::DispatchCapability]`.
This tool reads from 14+ Postgres tables across the component catalog — DB access
crosses the sandbox boundary and must be declared as `ExternalWrite` (the same reason
`component_db.rs:53` uses `ExternalWrite` even for read-only ops). `DispatchCapability`
covers the pure computation path (formatting, section slicing). Do **not** use
`EffectKind::Read` here — it is insufficient for cross-boundary DB reads and the
capability policy will deny the tool at runtime.

#### Change 1b — `host.store_prefix_bundle`

**New file**: `crates/brassclaw_host_runtime/src/first_party_tools/store_prefix_bundle.rs`

```rust
pub const STORE_PREFIX_BUNDLE_CAPABILITY_ID: &str = "host.store_prefix_bundle";
```

**Backend trait** (`StorePrefixBundleBackend`, `async_trait`):

```rust
async fn store(
    &self,
    scope_ticket: &str,
    bundle: &str,
    generation_ms: i64,
) -> Result<StorePrefixBundleResult, StorePrefixBundleError>;

pub struct StorePrefixBundleResult {
    pub fingerprint: String,
    pub generation_ms: i64,
    pub generation_id: Uuid,
}
```

**`handle(state, params)`:** extracts `scope_ticket`, `bundle`, `generation_ms`; backend atomically consumes the ticket and uses its authorized scope/request UUID; returns
`json!({"fingerprint": result.fingerprint, "generation_ms": result.generation_ms, "generation_id": result.generation_id})`.

**`manifest()`:** `EffectKind::ExternalWrite` (writes `reborn_basic_prompt_store`).

#### Change 1c — `mod.rs` updates

In `first_party_tools/mod.rs`:
```rust
mod sweep_validated_components;
mod store_prefix_bundle;
pub use sweep_validated_components::SWEEP_VALIDATED_COMPONENTS_CAPABILITY_ID;
pub use store_prefix_bundle::STORE_PREFIX_BUNDLE_CAPABILITY_ID;
// + register both manifests
```

---

### Change 2 — New Composition Backends + `include_str!` seed constants

#### Change 2a — Seed constants in `builtin_bootstrap.rs`

Add to the top of the seed constants section in `builtin_bootstrap.rs` (after the
existing `DIRECTION_*_SEED` constants around line 116):

```rust
/// Architecture reference embedded for prefix bundle assembly.
/// The ONLY place CLAUDE.md is embedded via include_str!.
pub(crate) const CLAUDE_MD_SEED: &str = include_str!("../../../CLAUDE.md");

/// Agent routing reference embedded for prefix bundle assembly.
/// The ONLY place AGENTS.md is embedded via include_str!.
pub(crate) const AGENTS_MD_SEED: &str = include_str!("../../../AGENTS.md");
```

No entry needed in `EXPECTED_CHECKSUMS` — CLAUDE.md and AGENTS.md are not
component bodies with content integrity checks (they are reference docs, not
seeded as DB rows with `source='system'`).

**`include_str!` path**: from `brassclaw_reborn_composition/src/` → 3 levels up →
workspace root. `"../../../CLAUDE.md"` and `"../../../AGENTS.md"` ✓

#### Change 2b — `pg_sweep_validated_components_backend.rs`

**New file:** `crates/brassclaw_reborn_composition/src/pg_sweep_validated_components_backend.rs`

Implements `SweepValidatedComponentsBackend`. Logic ported from `do_assemble_bundle` +
`do_format_bundle` with all bugs fixed and new features:

1. Query each known component table directly (do **not** copy `COMPONENT_TABLES` —
   remove phantom `reborn_orchestrators`/`reborn_scaffolds` entries, fix `reborn_skills`
   label bug).
2. `reborn_skills` query:
   ```sql
   SELECT class_code, prompt_uid, name,
          COALESCE(NULLIF(prior_knowledge_content,''), body) AS content
   FROM reborn_skills
   WHERE validation_status = 'validated'
   ORDER BY prompt_uid ASC
   LIMIT 1000
   ```
   Use the **actual `class_code` from each row** in the bundle header.
3. All other tables: `SELECT prompt_uid, name, ({content_expr}) AS content FROM {table} WHERE validation_status = 'validated' ORDER BY prompt_uid ASC LIMIT 1000` with the **known constant** `class_code` for that table.
4. Do not filter by `consumer_tags`; `validation_status = 'validated'` is authoritative. Never include pending/rejected/unvalidated rows, even when they are system-seeded or tagged for an otherwise valid consumer.
5. Check `information_schema.tables` before querying (unchanged pattern).
6. Collect `(class_code, prompt_uid, name, content)`, sort `(class_code ASC, prompt_uid ASC)`.
7. Format: `\n\n## {cc}:{uid}  {label}  "{name}"\n\n{content}`.
8. Append CLAUDE.md slice — **using the seed constant, not `include_str!` here**:
   ```rust
   let claude_md = crate::builtin_bootstrap::CLAUDE_MD_SEED;
   // unwrap_or(len) → &str[len..] = "" — appends nothing if the section marker
   // is absent, rather than silently dumping the entire file from byte 0.
   let pos = claude_md.find("\n## Architecture\n").unwrap_or(claude_md.len());
   buf.push_str("\n\n---\n\n## CLAUDE.md — Architecture & Design Reference\n\n");
   buf.push_str(&claude_md[pos..]);
   ```
9. Append AGENTS.md slice:
   ```rust
   let agents_md = crate::builtin_bootstrap::AGENTS_MD_SEED;
   // Same guard — fall back to empty append if section marker moves or is absent.
   let pos = agents_md.find("\n## Architecture Mental Model\n").unwrap_or(agents_md.len());
   buf.push_str("\n\n---\n\n## AGENTS.md — Routing & Design Reference\n\n");
   buf.push_str(&agents_md[pos..]);
   ```
10. Append Sempai Response Schema footer (moved verbatim from `do_format_bundle`).
11. Measure `generation_ms` here (wall-clock assembly time) and include in result.
12. Return `SweepValidatedComponentsResult { bundle, component_count, generation_ms }`.

**`// TODO(scaling)` note:** `LIMIT 1000` per table is carried from the existing
`do_assemble_bundle`. With 14+ tables this caps at ~14k components before truncation.
Safe for current library size — increase to `LIMIT 10000` or remove cap as the
library grows.

#### Change 2c — `pg_store_prefix_bundle_backend.rs`

**New file:** `crates/brassclaw_reborn_composition/src/pg_store_prefix_bundle_backend.rs`

Implements `StorePrefixBundleBackend`. Takes the pre-assembled bundle string and
`generation_ms` (measured by the sweep backend, passed through by PythonCode body).
Resolves the one-use permit to `(user_id, project_id, generation_id)` and calls `PgBasicPromptStore::store(user_id, project_id, &bundle, false, Some(generation_ms), generation_id)`. Returns `StorePrefixBundleResult { fingerprint, generation_ms, generation_id }`.

#### Change 2d — `lib.rs` additions

```rust
pub(crate) mod pg_sweep_validated_components_backend;
pub(crate) mod pg_store_prefix_bundle_backend;
```

---

### Change 3 — Pass 17 in `builtin_bootstrap.rs`: Two Tools, Two ToolSkills, PythonCode, Two Skills, Recipe, Catalogue

Seeded in **`builtin_bootstrap.rs`** as Pass 17, inside `seed_builtin_components()`
via a new `seed_prefix_bundle_group(&stores)` function. Called after Pass 16
(Zencoder) in `seed_builtin_components()`.

```rust
// Pass 17 — prefix-bundle group: sweep + store tools + recipe.
seed_prefix_bundle_group(&stores).await?;
```

#### 3a. Two Tool rows (class 0) — seed FIRST

**Every capability tool requires a `NewPgTool` row (class 0) in `reborn_tools` before its
ToolSkill can be seeded.** All other passes follow this pattern: `upsert_tool(...)` first,
then `upsert_tool_skill(...)`. Without the Tool rows, `tool_sweep_id` / `tool_store_id`
are undefined and the `append_children` call at §3e will fail to compile.

`NewPgTool` rows for the two new tools — seed both with `stores.upsert_tool(...)`:

```
name:              "host.sweep_validated_components"
description:       "Sweep components whose validation_status is validated, regardless of consumer tags; format the bundle string, append
                    CLAUDE.md architecture sections and AGENTS.md routing sections.
                    Returns {bundle, component_count, generation_ms}."
effect_type:       "external_write"  // matches ExternalWrite + DispatchCapability in manifest()
consumer_tags:     ["00:rusty", "05:validator"]
source:            "system"
validation_status: "validated"
capability_id:     "host.sweep_validated_components"
```

```
name:              "host.store_prefix_bundle"
description:       "Persist a pre-assembled bundle string to PgBasicPromptStore.
                    Returns {fingerprint, generation_ms, generation_id}."
effect_type:       "external_write"
consumer_tags:     ["00:rusty", "05:validator"]
source:            "system"
validation_status: "validated"
capability_id:     "host.store_prefix_bundle"
```

Pattern for the seeder function (matches `tool_read_file_row` / `tool_http_row` etc.):
```rust
let tool_sweep_id = stores.upsert_tool(
    NewPgTool { /* fields above */ capability_id: "host.sweep_validated_components".into(), .. },
    "host.sweep_validated_components",
).await?;
let tool_store_id = stores.upsert_tool(
    NewPgTool { /* fields above */ capability_id: "host.store_prefix_bundle".into(), .. },
    "host.store_prefix_bundle",
).await?;
```

These UUIDs are used in `append_children` at §3e.

#### 3b. Two ToolSkills

**`ts-host-sweep-validated-components`** (class 13):
```
name:         "ts-host-sweep-validated-components"
description:  "Binding for host.sweep_validated_components. Queries all validated
               component rows with validation_status=validated regardless of consumer_tags, formats the bundle string,
               appends CLAUDE.md architecture sections and AGENTS.md routing sections,
               and returns {bundle, component_count, generation_ms}."
content:      "Call host.sweep_validated_components(scope_ticket=<one-use ticket>).
               Returns {bundle, component_count, generation_ms}."
tool_name:    "host.sweep_validated_components"
param_schema: [{name:"scope_ticket", type:"string", required:true}]
consumer_tags: ["00:rusty", "02:orchestrator"]
source: "system" / validation_status: "validated"
content_checksum: None
```

**`ts-host-store-prefix-bundle`** (class 13):
```
name:         "ts-host-store-prefix-bundle"
description:  "Binding for host.store_prefix_bundle. Persists a pre-assembled bundle
               string to PgBasicPromptStore and returns {fingerprint, generation_ms, generation_id}."
content:      "Call host.store_prefix_bundle(scope_ticket=<one-use ticket>,
               bundle=<bundle_string>, generation_ms=<ms>).
               Returns {fingerprint, generation_ms, generation_id}."
tool_name:    "host.store_prefix_bundle"
param_schema: [{name:"scope_ticket", type:"string", required:true},
               {name:"bundle",       type:"string",  required:true},
               {name:"generation_ms",type:"integer", required:true}]
consumer_tags: ["00:rusty", "02:orchestrator"]
source: "system" / validation_status: "validated"
content_checksum: None
```

#### 3c. PythonCode executor (the chained body)

**`pc-host-assemble-prefix-bundle`** (class 22):

```python
# Channel: orchestrator | Class: 22 | No I/O, no imports.
# IBS bakes {{vars.slot0}} = a server-issued, one-use scope_ticket. The ticket is not an ID or authorization claim.
# Dependent sequential chain — two host calls in one body (Rule 3 exception):
# host.store_prefix_bundle requires bundle_parts["bundle"] from sweep.
bundle_parts = host.sweep_validated_components(
    scope_ticket="{{vars.slot0}}"
)
result = host.store_prefix_bundle(
    scope_ticket="{{vars.slot0}}",
    bundle=bundle_parts["bundle"],
    generation_ms=bundle_parts["generation_ms"]
)
```

```
name:          "pc-host-assemble-prefix-bundle"
description:   "Sweep, format, and store the full prefix bundle (two chained host calls)."
consumer_tags: ["01:monty", "02:orchestrator"]
source: "system"
content_checksum: None
```

`BootstrapStores::upsert_python_code` currently inserts `pending` and updates status afterward. Change this seeder API/path so trusted installation-seeded `NewPgPythonCode` rows are inserted/upserted with `validation_status = "validated"` directly. Do not seed them as pending and follow with `update_validation_status`; preserve the pending workflow for user-authored/untrusted PythonCode. Apply the same direct validated-status seeding rule to every built-in component class.

#### 3d. Two leaf Skills

**`skill-sweep-validated-components`** (class 1):
```
body: "Use host.sweep_validated_components(scope_ticket) to query only validated
      component rows regardless of consumer_tags, format the bundle string, append
      CLAUDE.md architecture sections and AGENTS.md routing sections, and return
      {bundle, component_count, generation_ms}. Use before host.store_prefix_bundle."
consumer_tags: ["02:orchestrator", "05:validation"]
content_checksum: None
```

**`skill-store-prefix-bundle`** (class 1):
```
body: "Use host.store_prefix_bundle(scope_ticket, bundle, generation_ms) to
      persist a pre-assembled bundle string to PgBasicPromptStore. Call only after
      host.sweep_validated_components. Returns {fingerprint, generation_ms, generation_id}."
consumer_tags: ["02:orchestrator", "05:validation"]
content_checksum: None
```

#### 3e. Recipe: `host-assemble-prefix-bundle` (class 21)

**Do NOT use `seed_recipe()` here.** `seed_recipe()` calls `recipe_row()` which
synthesizes `variable_patterns: []` and cannot be overridden. This recipe requires
`variable_patterns` with slot-capture rules. Build `NewPgRecipe` directly, call
`stores.upsert_recipe()` + `stores.mark_recipe_tier0()` +
`stores.audit_builtin_graduation()` in sequence. `audit_builtin_graduation` signature:
`async fn audit_builtin_graduation(&self, component_id: Uuid, class_code: i32, name: &str)`.
The `class_code` parameter is **`i32`** (not `u16`). Call without `?` — it returns `()`
and suppresses errors internally as non-fatal.

**Step entries** (1-based stepnumbers — matches `step_link: "0:1-0:E"`):

```rust
let steps = vec![
    step_entry(1, "rust",         "Pre-load sweep tool binding",        "component", &[ts_sweep_id]),
    step_entry(2, "rust",         "Pre-load store tool binding",        "component", &[ts_store_id]),
    step_entry(3, "orchestrator", "Sweep and store prefix bundle",      "component", &[pc_assemble_id, skill_sweep_id, skill_store_id]),
    step_entry(4, "rust",         "Pre-load post-reply tool binding",   "component", &[ts_post_reply_id]),
    step_entry(5, "orchestrator", "Emit confirmation reply",            "component", &[pc_post_reply_id]),
];
```

Step 3 includes both leaf skills in its `include` array alongside the PythonCode UUID.
Per `compose_program()`, the first class-22 UUID becomes `executable_code`; the class-1
UUIDs become `program.skills` (narrative context for Monty — not executed).

`ts_post_reply_id` / `pc_post_reply_id` resolved via
`stores.tool_skill.get_id_by_name(..., "ts-host-post-reply")` /
`stores.python_code.get_by_name(..., "pc-host-post-reply")` — guaranteed present
because `seed_builtin_host_components()` (Slice 4) runs before `seed_builtin_components()`.

**`variable_patterns`:**
```json
[
  {"name": "slot0", "pattern": "ticket=([A-Fa-f0-9-]{36})", "description": "server-issued one-use scope_ticket"}
]
```

The pattern captures only the opaque ticket. The request path marks this as an internal turn in trusted metadata; a user-authored message cannot establish that metadata. A recipe match without a valid permit is rejected before any tool side effect and before Tier-2 fallback.

**Full `NewPgRecipe` construction** (intent examples + `consumer_tags` + `step_link`):

```rust
let recipe_name = "host-assemble-prefix-bundle";
let intent_bare = vec![
    // Internal route examples only; every example includes the captured ticket slot.
    // Runtime dispatch also requires trusted internal-turn metadata and a valid permit.
    "regenerate prefix bundle ticket=00000000-0000-0000-0000-000000000000",
    "assemble base prompt ticket=00000000-0000-0000-0000-000000000000",
    "rebuild prefix cache ticket=00000000-0000-0000-0000-000000000000",
    "regenerate base-prompt ticket=00000000-0000-0000-0000-000000000000",
    "refresh prefix bundle ticket=00000000-0000-0000-0000-000000000000",
    "generate base prompt bundle ticket=00000000-0000-0000-0000-000000000000",
    "re-assemble prefix knowledge base ticket=00000000-0000-0000-0000-000000000000",
    "trigger prefix bundle rebuild ticket=00000000-0000-0000-0000-000000000000",
    "update prefix knowledge base ticket=00000000-0000-0000-0000-000000000000",
    "rebuild kohai prefix ticket=00000000-0000-0000-0000-000000000000",
];
let intent_examples_top: Vec<Value> = intent_bare
    .iter()
    .map(|s| json!({"input": s, "class": 0}))
    .collect();

let row = NewPgRecipe {
    // ... standard fields ...
    consumer_tags: vec!["02:orchestrator".into(), "05:validator".into()],
    step_descriptions: Some(json!([{
        "desc_idx": 0,
        "label": "Assemble prefix bundle",
        "yaml_source": "",
        "steps": steps,  // note: key is "steps", matches recipe_row() factory
    }])),
    variants: Some(json!([{
        "variant_key": recipe_name,
        "step_link": "0:1-0:E",
        "description": "Tier-0: assemble full prefix bundle — no LLM.",
        "intent_examples": intent_bare,
        "variable_patterns": [
            {"name": "slot0", "pattern": "ticket=([A-Fa-f0-9-]{36})", "description": "server-issued one-use scope_ticket"}
        ],
    }])),
    // ...
};

let recipe_id = stores.upsert_recipe(row, recipe_name).await?;
stores.mark_recipe_tier0(recipe_id).await?;
// audit_builtin_graduation: class_code is i32 (not u16). Returns () — no ? needed.
stores.audit_builtin_graduation(recipe_id, 21i32, recipe_name).await;
```

#### 3f. ExtensionCatalogue

```rust
let cat_id = stores.upsert_catalogue(NewPgExtensionCatalogue {
    // ... standard fields ...
    name: "builtin-prefix-bundle".to_string(),
    description: "Prefix bundle assembly — sweep and store tools, recipe, and skills for
                  assembling the Kohai base-prompt bundle from all validated components."
        .to_string(),
    source: "system".into(),
    validation_status: "validated".into(),
    consumer_tags: vec!["02:orchestrator".into()],
    child_component_ids: vec![],
    dependency_registry: None,
}, "builtin-prefix-bundle").await?;

stores.append_children(cat_id, &[
    tool_sweep_id, tool_store_id,
    ts_sweep_id, ts_store_id,
    pc_assemble_id,
    skill_sweep_id, skill_store_id,
    recipe_id,
]).await?;
```

Note: method name is `append_children` (wraps `append_child_component_ids` internally).

---

### Change 4 — `regenerate_prefix`: turn submission + cold-start-safe guard

`RebornInterceptorConfigService` gains a `runtime` field.
Add/use an internal turn submission method (or equivalent trusted options) that marks the request as internal and makes recipe lookup fail closed before Tier 2. Do not treat an ordinary `send_user_message` string as trusted metadata.

Add fields:

```rust
runtime: Option<Arc<RebornRuntime>>,
scope_permits: Arc<dyn PrefixScopePermitStore>,
```

`PrefixScopePermitStore` must support ticket creation bound to authenticated caller, authorized scope, conversation ID, request UUID, and an expiring per-scope lease; lookup for the sweep is non-consuming, while the store tool atomically consumes the permit and writes its request UUID.

Add builder:

```rust
pub fn with_runtime(mut self, r: Arc<RebornRuntime>) -> Self {
    self.runtime = Some(r);
    self
}
```

Replace lines 578–580 in `regenerate_prefix`:

```rust
let runtime = self.runtime.as_ref()
    .ok_or(InterceptorConfigServiceError::Unavailable)?;

let conversation = runtime.new_conversation().await
    .map_err(|_| InterceptorConfigServiceError::Unavailable)?;
let request_id = Uuid::new_v4();
let scope_ticket = self.scope_permits.create(
    caller, user_id, project_id, &conversation, request_id,
).await.map_err(|_| InterceptorConfigServiceError::Unavailable)?;

let reply = runtime
    .send_internal_user_message(
        &conversation,
        &format!("regenerate prefix bundle ticket={scope_ticket}"),
        InternalTurnOptions { hard_fail_on_recipe_miss: true, allow_tier_two: false },
    )
    .await
    .map_err(|_| InterceptorConfigServiceError::Unavailable)?;
if !reply.is_success() {
    return Err(InterceptorConfigServiceError::Unavailable);
}

// Cold-start-safe request-ID guard:
// Store entry generation_id == this request UUID → success (also on cold start).
// Missing/mismatched generation_id → this request did not store; fail.
#[cfg(feature = "postgres")]
{
    let new_entry = self.pg_basic_prompt_store.as_ref()
        .ok_or(InterceptorConfigServiceError::Unavailable)?
        .get_for_scope(user_id, project_id).await
        .map_err(|_| InterceptorConfigServiceError::Unavailable)?;
    let new_generation_id = new_entry.as_ref().and_then(|e| e.generation_id);
    let succeeded = new_generation_id == Some(request_id);
    if !succeeded {
        tracing::warn!(
            user_id, project_id,
            "regenerate_prefix: this request did not store the expected generation_id"
        );
        return Err(InterceptorConfigServiceError::Unavailable);
    }
}
// Bundle is now in PgBasicPromptStore — read it back for the response DTO.
```

`fingerprint`, `generation_ms`, and `generation_id` are then read from
`PgBasicPromptStore::get_for_scope(user_id, project_id)` — same pattern already
used for `assembled_at`/`prewarm_last_at` at lines 649–663.

**Prewarm block — bundle variable no longer exists after the refactor.**
The existing prewarm block (lines 583–645 in the current code) passes `bundle` (a local
variable returned by `do_assemble_bundle`) to `gateway.stream_model`. After the refactor,
`do_assemble_bundle` is deleted and `bundle` is never a local variable in
`regenerate_prefix`. The prewarm block is **not** "unchanged" — it must be rewritten to
read the bundle from `PgBasicPromptStore` before calling the gateway. After the
`generation_id` guard passes, the entry is already in the store. Read it once and use it
for both the prewarm and the response DTO:

```rust
// After the request generation_id guard passes — read the stored entry once.
#[cfg(feature = "postgres")]
let stored_entry = if let Some(store) = &self.pg_basic_prompt_store {
    store.get_for_scope(user_id, project_id).await
        .ok()
        .flatten()
} else {
    None
};

// Prewarm the Sempai gateway — uses stored_entry.bundle (not a removed local var).
#[cfg(feature = "postgres")]
let mut with_prewarm = false;
if let Some(gateway) = &self.sempai_gateway {
    if let Some(ref entry) = stored_entry {
        // ... existing HostManagedModelRequest construction using entry.bundle ...
        // with_prewarm = true on success, debug log on failure (unchanged logic)
    }
}
```

`fingerprint` and `generation_ms` are likewise read from `stored_entry` (not from a
removed local variable).

**Delete** from `interceptor_config_service.rs`:
- `COMPONENT_TABLES` const (lines 61–155)
- `class_label()` fn (lines 158–187)
- `do_assemble_bundle()` (lines 288–411)
- `do_format_bundle()` (lines 417–443)
- The `do_format_bundle` unit tests at lines 748–760 and the `COMPONENT_TABLES` assertion tests at 717–744 (test deleted functions; new backend has its own tests)

---

### Change 5 — Wire backends and runtime

#### 5a — Backend injection: `factory.rs`

The `component_db` backend is wired at line 1025. The new backends follow the
identical pattern — built from the pool + tenant, passed into `BuiltinFirstPartyTools`:

```rust
#[cfg(feature = "postgres")]
let sweep_backend: Option<Arc<dyn brassclaw_host_runtime::SweepValidatedComponentsBackend>> = { ... };

#[cfg(feature = "postgres")]
let store_backend: Option<Arc<dyn brassclaw_host_runtime::StorePrefixBundleBackend>> = { ... };
```

Chain `with_sweep_validated_components()` and `with_store_prefix_bundle()` onto
`BuiltinFirstPartyTools::default()` — same pattern as `with_component_db()`.

#### 5b — Runtime injection: `webui.rs`

The `runtime` parameter at `webui.rs:48` is `&RebornRuntime`.

**`RebornRuntime` does NOT implement `Clone`** — it contains `JoinHandle`, `Mutex`,
and other non-`Clone` fields. `Arc::new(runtime.clone())` will not compile.

The `with_runtime` builder must accept `Arc<RebornRuntime>`. Since `webui.rs` receives
`runtime: &RebornRuntime` (a plain reference, not an `Arc`), the caller cannot construct
`Arc<RebornRuntime>` from a reference without owning the value. Two correct approaches:

**Option A (preferred) — change `build_webui_services` to accept `Arc<RebornRuntime>`:**
Update `build_webui_services(runtime: &RebornRuntime, ...)` to
`build_webui_services(runtime: Arc<RebornRuntime>, ...)` so it can pass
`Arc::clone(&runtime)` to `with_runtime`. This requires updating callers in the CLI
and any other entrypoints that call `build_webui_services`.

**Option B — wrap the runtime reference in a newtype / weak Arc upstream:**
If changing the signature is too disruptive, `RebornRuntime` can be wrapped in `Arc` at
construction time in `build_reborn_runtime` and the `Arc<RebornRuntime>` threaded through.

Before coding, verify which approach is consistent with how `runtime` is used at other
`webui.rs` call sites. The `with_runtime` field on the service must be `Arc<RebornRuntime>`.
The `runtime.clone()` call in the BEFORE/AFTER snippets below must be replaced with
`Arc::clone(&runtime)` once the type change is applied.

Wire inside the `#[cfg(feature = "root-llm-provider")]` block (after the `#[cfg]` gate
is decoupled per §5c):

```rust
// webui.rs — inside #[cfg(feature = "root-llm-provider")] block:
// runtime is Arc<RebornRuntime> here (after Option A is applied).
interceptor_svc = interceptor_svc.with_runtime(Arc::clone(&runtime));
```

#### 5c — WebUI prefix tab: decouple `#[cfg]` gate (live bug fix)

The interceptor service is currently gated on `all(feature = "postgres", feature =
"root-llm-provider")`, which means the prefix tab returns 503 when built without
`root-llm-provider`. The base service (prefix list + reassemble) needs only `postgres`.

**Fix in `webui.rs` lines 482–498** (exact diff):

```rust
// BEFORE (broken):
#[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
if let Some(pool) = services.pg_pool.clone() {
    let tenant_id = runtime.webui_tenant_id().to_string();
    let mut interceptor_svc =
        crate::interceptor_config_service::RebornInterceptorConfigService::new(pool, tenant_id);
    if let Some(mode) = runtime.interceptor_mode() {
        interceptor_svc = interceptor_svc.with_interceptor_mode(mode);
    }
    if let Some(gateway) = runtime.sempai_gateway() {
        interceptor_svc = interceptor_svc.with_sempai_gateway(gateway);
    }
    api = api.with_interceptor_config_service(Arc::new(interceptor_svc)
        as Arc<dyn brassclaw_product_workflow::InterceptorConfigService>);
}

// AFTER (correct):
#[cfg(feature = "postgres")]
if let Some(pool) = services.pg_pool.clone() {
    let tenant_id = runtime.webui_tenant_id().to_string();
    let mut interceptor_svc =
        crate::interceptor_config_service::RebornInterceptorConfigService::new(pool, tenant_id);
    #[cfg(feature = "root-llm-provider")]
    if let Some(mode) = runtime.interceptor_mode() {
        interceptor_svc = interceptor_svc.with_interceptor_mode(mode);
    }
    #[cfg(feature = "root-llm-provider")]
    if let Some(gateway) = runtime.sempai_gateway() {
        interceptor_svc = interceptor_svc.with_sempai_gateway(gateway);
    }
    #[cfg(feature = "root-llm-provider")]
    {
        // runtime is Arc<RebornRuntime> here (after Option A in §5b is applied).
        // RebornRuntime does NOT implement Clone — use Arc::clone, not runtime.clone().
        interceptor_svc = interceptor_svc.with_runtime(Arc::clone(&runtime));
    }
    api = api.with_interceptor_config_service(Arc::new(interceptor_svc)
        as Arc<dyn brassclaw_product_workflow::InterceptorConfigService>);
}
```

---

## Files to Create / Modify

| File | Change |
|------|--------|
| `crates/brassclaw_host_runtime/src/first_party_tools/sweep_validated_components.rs` | **NEW** — `SWEEP_VALIDATED_COMPONENTS_CAPABILITY_ID`, `SweepValidatedComponentsBackend` trait, handler, manifest (`vec![EffectKind::ExternalWrite, EffectKind::DispatchCapability]`) |
| `crates/brassclaw_host_runtime/src/first_party_tools/store_prefix_bundle.rs` | **NEW** — `STORE_PREFIX_BUNDLE_CAPABILITY_ID`, `StorePrefixBundleBackend` trait, handler, manifest (`EffectKind::ExternalWrite`) |
| `crates/brassclaw_host_runtime/src/first_party_tools/mod.rs` | `mod sweep_validated_components;` + `mod store_prefix_bundle;` + re-exports + manifests |
| `crates/brassclaw_reborn_composition/src/pg_sweep_validated_components_backend.rs` | **NEW** — implements `SweepValidatedComponentsBackend`: fixed DB sweep, CLAUDE_MD_SEED / AGENTS_MD_SEED via `crate::builtin_bootstrap`, Sempai footer |
| `crates/brassclaw_reborn_composition/src/pg_store_prefix_bundle_backend.rs` | **NEW** — implements `StorePrefixBundleBackend`: calls `PgBasicPromptStore::store(..., generation_id)` |
| `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs` | Add `CLAUDE_MD_SEED` + `AGENTS_MD_SEED` `pub(crate) const` seed constants (`include_str!` only here) |
| `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs` | **Pass 17**: new `seed_prefix_bundle_group()` fn + call after Pass 16 |
| `crates/brassclaw_reborn_composition/src/interceptor_config_service.rs` | Delete `COMPONENT_TABLES`, `class_label`, `do_assemble_bundle`, `do_format_bundle` + their tests; add runtime + scope-permit dependencies; validate caller/scope; replace assembly with trusted internal turn + request-ID verification; rewrite prewarm block to read bundle from `PgBasicPromptStore` |
| `crates/brassclaw_reborn_composition/src/webui.rs` | Wire `runtime` into interceptor service (5b — requires signature change to `Arc<RebornRuntime>`); decouple `#[cfg]` gate (5c) |
| `crates/brassclaw_reborn_composition/src/factory.rs` | Wire both new backends alongside `component_db` backend |
| `crates/brassclaw_reborn_composition/src/runtime.rs` | Add trusted internal turn submission options/path; reject no-match before Tier 2. |
| Scope permit store | Create, validate, expire, and atomically consume caller/conversation-bound one-use scope tickets and hold a per-scope lease through regeneration completion. |
| `crates/brassclaw_reborn_composition/src/pg_basic_prompt_store.rs` + next migration | Persist/read request-specific `generation_id`. |
| `crates/brassclaw_reborn_composition/src/lib.rs` | `pub(crate) mod pg_sweep_validated_components_backend;` + `pub(crate) mod pg_store_prefix_bundle_backend;` |

**Also modify**:
- `pg_basic_prompt_store.rs` and a new forward migration after the current schema head — persist/read `generation_id`; thread it through the store backend and response entry.
- Runtime internal-turn submission path — carry trusted conversation metadata and fail closed before Tier 2 for this internal route.
- `seed_builtin_host.rs` — Slices 1–12, no touch at all.
- `pc-host-post-reply` / `ts-host-post-reply` (Slice 4) — looked up by name in Pass 17, not re-seeded.
- `doc-sync` / `doc-convert` / `host-assemble-prior-knowledge` — untouched.
- `list_prefix_entries` — untouched.

---

## How data flows through the step execution

| Step | Channel | Content | Action |
|------|---------|---------|--------|
| 1 | rust | `ts-host-sweep-validated-components` | Pre-load sweep tool binding into Monty namespace |
| 2 | rust | `ts-host-store-prefix-bundle` | Pre-load store tool binding into Monty namespace |
| 3 | orchestrator | `pc-host-assemble-prefix-bundle` + both leaf skills | Run chained PythonCode: sweep → store. `bundle_parts` is a local variable. Two `host.*` calls share one `host.run_program` context — Rule 3 dependent-chain exception. |
| 4 | rust | `ts-host-post-reply` | Pre-load post-reply tool binding |
| 5 | orchestrator | `pc-host-post-reply` | Emit confirmation reply |

Step 3's `include` array: `[pc_assemble_id, skill_sweep_id, skill_store_id]`.
`compose_program()` selects the first class-22 UUID as `executable_code` (run via
`host.run_program`); the class-1 UUIDs go into `program.skills` (narrative context
for Monty, not executed). The result of step 3 (`result` containing `{fingerprint,
generation_ms, generation_id}`) is local to that `host.run_program` call and does not cross into
step 5. The caller reads the DTO from `PgBasicPromptStore` after its own `generation_id` guard passes.

`step_link: "0:1-0:E"` means desc_idx 0, steps 1 through End — matches the 1-based
`stepnumber` values in `step_entry()`. Steps 1–5 inclusive, all executed.

---

## variable_patterns / slot capture

The internal message is `"regenerate prefix bundle ticket={scope_ticket}"`. `capture_variables()` binds only the opaque ticket to `slot0`; it does not accept user/project IDs. Trusted internal-turn metadata and the one-use permit remain mandatory in addition to intent matching.

---

## Validation

1. `cargo clippy -p brassclaw_host_runtime -p brassclaw_reborn_composition --all-targets -- -D warnings` — zero warnings.
2. `cargo test -p brassclaw_reborn_composition` — existing tests pass; retain coverage for the validation gate and add tests for permits, replay, concurrent requests, internal-turn no-fallback, and providerless route behavior.
3. Fresh-install and reseed assertions: every installation-seeded built-in is written with `source=system` and `validation_status=validated` by its seed/upsert operation (including PythonCode); the recipe is Tier 0 (`tier='mature'`, `wilson_lower=1.0`). Also assert unvalidated rows never enter the prefix or become callable/executable.
4. Integration: POST `/api/prefixes/base-prompt/regenerate` → assert `reborn_basic_prompt_store`
   bundle contains `"## Architecture"` (CLAUDE.md) and `"## Architecture Mental Model"` (AGENTS.md).
5. Validation-gate test: pending/rejected/unvalidated rows never appear or become callable/executable; a validated row appears regardless of `consumer_tags`; every installation-seeded built-in is written as validated at seed time.
6. `include_str!` constants compile: `crate::builtin_bootstrap::CLAUDE_MD_SEED` and
   `crate::builtin_bootstrap::AGENTS_MD_SEED` are accessible from
   `pg_sweep_validated_components_backend.rs`; paths `"../../../CLAUDE.md"` and
   `"../../../AGENTS.md"` from `builtin_bootstrap.rs` resolve correctly at build time.
7. `reborn_skills` class_code test: a class-10 (Orchestrator) row in `reborn_skills` appears
   in the bundle with header `"10:xxx Orchestrator"`, not `"1:xxx Skill"`.
8. WebUI feature tests: with `postgres` only, GET `/api/webchat/v2/prefixes` returns 200; regeneration is explicitly unavailable (503) unless trusted runtime submission is wired. With runtime submission available, POST succeeds without a Tier-2 fallback.
9. Request integrity tests: failed/non-terminal internal turn, recipe miss, tool failure, expired/replayed ticket, or mismatched stored `generation_id` all return `Unavailable` without Tier-2 LLM work. Two concurrent requests for one scope serialize on the per-scope lease; each succeeds only after its own UUID is written and verified.
10. Cold-start: on a fresh DB with no prior bundle, POST succeeds and returns a valid
    `PrefixRegenerateResponse` when the stored `generation_id` equals this request UUID.

---

## Risk / Open Points

| Risk | Resolution |
|------|-----------|
| Internal turn API | Must carry trusted metadata and hard-fail before Tier 2 on recipe miss; a plain text prefix is not an authorization signal. |
| Cross-step variable sharing | Resolved by dependent-chain exception (Rule 3 carve-out): both host calls share one PythonCode body / one `host.run_program` scope. `bundle_parts["bundle"]` is a Python local variable — no step boundary is crossed. |
| `include_str!` path depth | `CLAUDE_MD_SEED` and `AGENTS_MD_SEED` are `pub(crate)` constants in `builtin_bootstrap.rs`. Path `"../../../CLAUDE.md"` from that file's directory = workspace root. ✓ Backend uses `crate::builtin_bootstrap::CLAUDE_MD_SEED` — no `include_str!` in the backend. |
| Validation status | `validation_status = 'validated'` is the only inclusion/availability gate; consumer tags do not substitute for status. All installation-seeded built-ins must be written as validated by their seed/upsert operation. |
| Recipe variant key is recipe name, not `"default"` | Intent system matches by `step_link` value, not `variant_key` — no routing impact. |
| `content_checksum` field on `NewPgToolSkill` / `NewPgSkill` / `NewPgPythonCode` | Set `content_checksum: None` on every initializer — consistent with all existing Pass 1–16 rows. |
| PythonCode seed status | Update `BootstrapStores::upsert_python_code` to persist trusted installation seeds as `validated` directly; retain pending status for user-authored/untrusted rows. Apply direct validated seeding to every built-in class. |
| `ts-host-post-reply` / `pc-host-post-reply` availability at Pass 17 seed time | `seed_builtin_host_components()` (Slice 4) runs before `seed_builtin_components()` — dependency always satisfied. |
| Backend injection order in `factory.rs` | Must be wired before `builtin_first_party_registry_with_trigger_create_hook()` is called — same ordering constraint as `component_db` at line 1025. |
| Internal recipe reachability | Only trusted internal turns with a valid, conversation-bound permit can execute the recipe; normal user messages and no-match turns fail closed before Tier 2. |
| Scope input trust | Raw user/project IDs never come from message text. A short-lived one-use scope ticket is server-minted, conversation-bound, authorized, and atomically consumed. |
| `audit_builtin_graduation` call | Signature: `(Uuid, i32, &str)` — `class_code` is `i32`, not `u16`. Returns `()` — call without `?`. Errors suppressed internally as non-fatal. |
| `append_children` method name | Confirmed `append_children` (wraps `append_child_component_ids`) — not `append_child_component_ids` directly. |
| `with_runtime` injection type | `RebornRuntime` does NOT implement `Clone` (contains `JoinHandle`, `Mutex`). `Arc::new(runtime.clone())` will not compile. Must change `build_webui_services` to accept `Arc<RebornRuntime>` (Option A in §5b) then use `Arc::clone(&runtime)`. See §5b for full options. |
| Prewarm block uses removed `bundle` variable | **Must be rewritten** — see Change 4. After refactor, `bundle` no longer exists as a local variable. Read `entry.bundle` from `PgBasicPromptStore::get_for_scope` after the `generation_id` guard passes; use that entry for both the prewarm gateway call and the response DTO. |
| `generation_ms` flows correctly | Sweep measures wall-clock time, returns it in result. PythonCode body passes `bundle_parts["generation_ms"]` to store call. Store writes it to `PgBasicPromptStore`. |
| `PrefixRegenerateResponse` fields after turn completes | Read from `stored_entry` (one `get_for_scope` call after the guard) — same pattern as existing lines 649–663, but with `bundle` also read from there for the prewarm. |
| `generation_id` guard on non-postgres builds | Regeneration is not wired without Postgres; never compile a handler that skips request-ID verification. |
| LIMIT 1000 per table | Carried from `do_assemble_bundle`. With 14+ tables = ~14k components before truncation. Safe for current library size. Add `// TODO(scaling)` comment; increase to `LIMIT 10000` or remove cap as library grows. |
| Turn latency / HTTP timeout | the internal turn submission runs synchronously. Do not assume a sub-second bound; retain rate limiting and establish an explicit request timeout/cancellation behavior for the HTTP caller. |
| Conversation history for background regeneration | `new_conversation()` creates a DB-tracked conversation. The background turn will appear in history and be reviewed by Sempai. Desirable: Sempai may propose recipe improvements. Orphaned history row is a known side effect with no harmful consequence. |
| Silent recipe failure / Tier 2 fallback | Internal regeneration fails closed before no-match LLM handling; terminal turn status and request-specific `generation_id` are both checked. |
| WebUI feature gates | With `postgres` only, prefix listing is wired and GET returns 200. Regeneration requires the runtime/turn path; without the required provider/runtime feature, POST reports 503 and the UI disables or explains regeneration. Test GET and POST separately. |
| `reborn_orchestrators`/`reborn_scaffolds` phantom entries | **Resolved** — new backend queries tables directly without copying `COMPONENT_TABLES`. |
