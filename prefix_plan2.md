# Prefix Plan 2 — Bundle Assembly via Recipe (Architecture-Correct v6)

*Audit date: post-codebase-audit. All line numbers verified.*

---

## Ground Truth: How the Execution Model Works

### The two-channel execution model

```
channel: "rust"           → pre-loads a ToolSkill binding into context (does NOT execute)
channel: "orchestrator"   → PythonCode body is collected into assembled_program
```

### The assembled_program

`compose_program()` (`brassclaw_engine/src/memory/composition.rs:235`) joins **all**
orchestrator PythonCode bodies with `"\n\n"`:

```rust
assembled_program: assembled_parts.join("\n\n"),
```

The `assembled_program` is ONE Python script. It runs in a **single Monty execution
context**. Local variables set in the first PythonCode block are visible in the second
and third. There is NO step isolation in the assembled_program — isolation only applies
to individual `host.run_program(code)` calls, not to the orchestrator channel.

### Consequence for recipe design

Multiple orchestrator PythonCode steps in a recipe = one joined script where data flows
naturally between tool calls via Python local variables.

Example of a 3-tool recipe assembled into one script:

```python
# pc-sweep-components body:
bundle_parts = host.sweep_validated_components(user_id="{{vars.slot0}}", project_id="{{vars.slot1}}")

# pc-store-prefix-bundle body (appended with \n\n):
store_result = host.store_prefix_bundle(user_id="{{vars.slot0}}", project_id="{{vars.slot1}}", bundle=bundle_parts["bundle"])

# pc-post-reply body (appended with \n\n):
result = host.post_reply(answer="Bundle assembled. fingerprint=" + store_result["fingerprint"])
```

`bundle_parts` is a Python local variable — visible to the second and third blocks because
all three are joined into one script. No slot tricks or template substitution needed for
inter-step data; `{{vars.slot0}}` / `{{vars.slot1}}` carry only the user_id and
project_id from the turn message.

### How Skills fit in

A Recipe does **not** own Skills directly. A Recipe's `step_descriptions` list step
entries whose `include` array can reference both a PythonCode UUID **and** Skill UUIDs
in the same step. `compose_program()` (lines 161–178 of `composition.rs`) processes
each step's `include` list:

- First UUID whose component is class 22 (PythonCode) → `executable_code` (goes into `assembled_program`)
- Any UUID whose component is class 1–3 (Skill) → appended to `program.skills` (read-only context passed to Monty as narrative)

`program.skills` is the list of Skill bodies Monty can **consult** when deciding how to
call a tool. The PythonCode bodies in `assembled_program` are what actually **execute**.
Skills co-exist in the same step's `include` as the PythonCode they contextualise —
they are NOT separate steps.

### Rust is just providing tools

The orchestrator (Monty VM) is the execution engine. Rust provides tools. The recipe's
`step_descriptions` wire Skills (context) and PythonCode (executors) together so the
IBS can compose a `program` that Monty runs. No monolithic tool is needed. The assembly
logic currently in `do_assemble_bundle` + `do_format_bundle` is split into two focused tools:

- `host.sweep_validated_components` — reads all validated component tables, formats the
  bundle text (including appending CLAUDE.md + AGENTS.md slices). Returns `{bundle}`.
- `host.store_prefix_bundle` — stores the bundle text via `PgBasicPromptStore::store()`.
  Returns `{fingerprint, generation_ms}`.

---

## Goal

1. **Remove** the `05:validator` filter from the DB query in `do_assemble_bundle` —
   all validated components regardless of `consumer_tags` must appear in the bundle.
2. **Include** CLAUDE.md (from `## Architecture`) and AGENTS.md (from
   `## Architecture Mental Model`) in every assembled bundle.
3. **Move** the assembly and store logic into two focused Rust tools behind the
   two-channel recipe model, so the orchestrator is in control of the flow.
4. **Replace** `regenerate_prefix`'s direct call to `do_assemble_bundle` with a turn
   submission via `RebornRuntime::send_user_message`, routed by intent to the new recipe.
5. **No new monolithic black-box tool.** Sweep = one tool. Store = one tool. Skills
   describe how to use each. The recipe wires them together.

---

## Current State (verified)

| Symbol | File | Lines |
|--------|------|-------|
| `COMPONENT_TABLES` const | `interceptor_config_service.rs` | 61–155 |
| `class_label()` fn | `interceptor_config_service.rs` | 158–187 |
| `do_assemble_bundle()` — includes `05:validator` filter | `interceptor_config_service.rs` | 288–411 |
| `do_format_bundle()` | `interceptor_config_service.rs` | 417–443 |
| `regenerate_prefix` calling `do_assemble_bundle` | `interceptor_config_service.rs` | 578–580 |
| Seeder last slice | `seed_builtin_host.rs` | Slice 12 (ends line 1996) |
| `compute_program()` join | `brassclaw_engine/src/memory/composition.rs` | 235 |

Nothing seeded. No new tools exist. `seed_builtin_host.rs` ends at Slice 12.

---

## New Flow

```
Operator clicks Regenerate (WebUI)
  → POST /api/prefixes/base-prompt/regenerate
    → RebornInterceptorConfigService::regenerate_prefix
        → rate-limit check (unchanged)
        → RebornRuntime::send_user_message(
              conversation,
              "regenerate prefix bundle user={user_id} project={project_id}"
          ).await                              ← normal turn submission

        → turn arrives at Monty
        → intent system matches "regenerate prefix bundle …"
              → routes to host-assemble-prefix-bundle Recipe (class 21)
        → host.compose_orchestrator → assembled_program (one script, 3 PythonCode blocks):

              # Block 1 — pc-host-sweep-components
              bundle_parts = host.sweep_validated_components(
                  user_id="{{vars.slot0}}",
                  project_id="{{vars.slot1}}"
              )

              # Block 2 — pc-host-store-prefix-bundle
              store_result = host.store_prefix_bundle(
                  user_id="{{vars.slot0}}",
                  project_id="{{vars.slot1}}",
                  bundle=bundle_parts["bundle"]
              )

              # Block 3 — pc-host-post-reply (reuses existing pc-host-post-reply)
              result = host.post_reply(
                  answer="Bundle assembled. fingerprint=" + store_result["fingerprint"]
              )

        → host.sweep_validated_components (Rust):
              • queries ALL validated component tables — NO `consumer_tags` filter
              • appends CLAUDE.md from "## Architecture" (include_str! at compile time)
              • appends AGENTS.md from "## Architecture Mental Model" (include_str!)
              • appends Sempai Response Schema footer
              • returns {bundle: "<full text>"}

        → host.store_prefix_bundle (Rust):
              • calls PgBasicPromptStore::store(user_id, project_id, bundle, false, Some(ms))
              • returns {fingerprint, generation_ms}

        → host.post_reply emits the reply to the turn
        → turn completes; send_user_message returns

      ← regenerate_prefix reads PgBasicPromptStore::get_for_scope(user_id, project_id)
      → optional gateway prewarm (unchanged logic)
      → return PrefixRegenerateResponse {fingerprint, assembled_at, generation_ms, …}
```

---

## Changes Required

### Change 1 — New Rust Tool: `host.sweep_validated_components`

**New file**: `crates/brassclaw_host_runtime/src/first_party_tools/sweep_components.rs`

Pattern: identical to `component_db.rs` — backend trait injected from composition.

```rust
pub const SWEEP_COMPONENTS_CAPABILITY_ID: &str = "host.sweep_validated_components";
```

**Backend trait** (`SweepComponentsBackend`, `async_trait`):

```rust
async fn sweep_and_format(
    &self,
    user_id: &str,
    project_id: &str,
) -> Result<SweepComponentsResult, SweepComponentsError>;

pub struct SweepComponentsResult {
    pub bundle: String,          // full formatted bundle text
    pub component_count: usize,
}
```

**`handle(state, params)`**: extracts `user_id`, `project_id`; calls backend;
returns `json!({"bundle": result.bundle, "component_count": result.component_count})`.

**`manifest()`**: `EffectKind::DispatchCapability` (read-only DB sweep).

**Registration** in `mod.rs`: `mod sweep_components;` +
`pub use sweep_components::SWEEP_COMPONENTS_CAPABILITY_ID;` + register manifest.

---

### Change 2 — New Rust Tool: `host.store_prefix_bundle`

**New file**: `crates/brassclaw_host_runtime/src/first_party_tools/store_prefix_bundle.rs`

```rust
pub const STORE_PREFIX_BUNDLE_CAPABILITY_ID: &str = "host.store_prefix_bundle";
```

**Backend trait** (`StorePrefixBundleBackend`, `async_trait`):

```rust
async fn store(
    &self,
    user_id: &str,
    project_id: &str,
    bundle: &str,
    generation_ms: Option<i64>,
) -> Result<StorePrefixBundleResult, StorePrefixBundleError>;

pub struct StorePrefixBundleResult {
    pub fingerprint: String,
    pub generation_ms: i64,
}
```

**`handle(state, params)`**: extracts `user_id`, `project_id`, `bundle`;
calls backend; returns `json!({"fingerprint": ..., "generation_ms": ...})`.

**`manifest()`**: `EffectKind::ExternalWrite` (writes `reborn_basic_prompt_store`).

**Registration** in `mod.rs`: `mod store_prefix_bundle;` +
`pub use store_prefix_bundle::STORE_PREFIX_BUNDLE_CAPABILITY_ID;` + register manifest.

---

### Change 3 — New Composition Backends

**New file**: `crates/brassclaw_reborn_composition/src/pg_sweep_components_backend.rs`

Implements `SweepComponentsBackend`. Logic moved verbatim from `do_assemble_bundle` +
`do_format_bundle` (minus the `store` call at the end):

1. Query `information_schema.tables` — discover existing component tables.
2. For each table: `WHERE validation_status = 'validated'` —
   **NO `consumer_tags` filter** (drop `AND NOT ('05:validator' = ANY(...))` entirely).
3. Collect `(class_code, prompt_uid, name, content)`, sort `(class_code ASC, prompt_uid ASC)`.
4. Format each row: `\n\n## {cc}:{uid}  {label}  "{name}"\n\n{content}`.
5. Append CLAUDE.md slice:
   ```rust
   static CLAUDE_MD: &str = include_str!("../../../CLAUDE.md");
   let pos = CLAUDE_MD.find("\n## Architecture\n").unwrap_or(0);
   buf.push_str("\n\n---\n\n## CLAUDE.md — Architecture & Design Reference\n\n");
   buf.push_str(&CLAUDE_MD[pos..]);
   ```
6. Append AGENTS.md slice:
   ```rust
   static AGENTS_MD: &str = include_str!("../../../AGENTS.md");
   let pos = AGENTS_MD.find("\n## Architecture Mental Model\n").unwrap_or(0);
   buf.push_str("\n\n---\n\n## AGENTS.md — Routing & Design Reference\n\n");
   buf.push_str(&AGENTS_MD[pos..]);
   ```
7. Append Sempai Response Schema footer (moved verbatim from `do_format_bundle`).
8. Record wall-clock time for `component_count` + log line.
9. Return `SweepComponentsResult { bundle, component_count }`.

**`include_str!` path**: from `brassclaw_reborn_composition/src/` → 3 levels up →
workspace root. `"../../../CLAUDE.md"` and `"../../../AGENTS.md"` ✓

**New file**: `crates/brassclaw_reborn_composition/src/pg_store_prefix_bundle_backend.rs`

Implements `StorePrefixBundleBackend`:

1. Records `assembly_start` before the call (or accepts an elapsed if provided).
2. Calls `PgBasicPromptStore::store(user_id, project_id, bundle, false, generation_ms)`.
3. Returns `StorePrefixBundleResult { fingerprint, generation_ms }`.

**Wire backends** in `webui.rs`: inject `PgSweepComponentsBackend` and
`PgStorePrefixBundleBackend` into their respective tool states — same pattern as
`PgComponentDbBackend` injected into `ComponentDbState`.

---

### Change 4 — Slice 13: ToolSkills, PythonCode, Skills, Recipe

Seeded in `seed_builtin_host.rs` as **Slice 13**
(`seed_host_assemble_prefix_bundle()`), appended to the `child_ids` vector.

#### 4a. ToolSkill: `ts-host-sweep-components` (class 13)

```
name:         "ts-host-sweep-components"
description:  "Executor binding for host.sweep_validated_components. Sweeps all
               validated components (no consumer_tags filter), appends CLAUDE.md
               architecture sections and AGENTS.md routing sections, and returns
               the full formatted bundle text."
content:      "Call host.sweep_validated_components(user_id=<uid>, project_id=<pid>).
               Returns {bundle, component_count}. The bundle contains every validated
               component formatted as ## CC:UID LABEL \"name\" headers."
tool_name:    "host.sweep_validated_components"
param_schema: [{name:"user_id",    type:"string", required:true},
               {name:"project_id", type:"string", required:true}]
consumer_tags: ["00:rusty", "02:orchestrator"]
source: "system" / validation_status: "validated"
```

#### 4b. ToolSkill: `ts-host-store-prefix-bundle` (class 13)

```
name:         "ts-host-store-prefix-bundle"
description:  "Executor binding for host.store_prefix_bundle. Stores the assembled
               bundle text in PgBasicPromptStore for the given scope."
content:      "Call host.store_prefix_bundle(user_id=<uid>, project_id=<pid>,
               bundle=<bundle_text>). Returns {fingerprint, generation_ms}."
tool_name:    "host.store_prefix_bundle"
param_schema: [{name:"user_id",    type:"string", required:true},
               {name:"project_id", type:"string", required:true},
               {name:"bundle",     type:"string", required:true}]
consumer_tags: ["00:rusty", "02:orchestrator"]
source: "system" / validation_status: "validated"
```

#### 4c. PythonCode: `pc-host-sweep-components` (class 22)

```python
# Channel: orchestrator | Class: 22 | No I/O, no imports.
# IBS bakes {{vars.slot0}} = user_id, {{vars.slot1}} = project_id.
bundle_parts = host.sweep_validated_components(
    user_id="{{vars.slot0}}",
    project_id="{{vars.slot1}}"
)
```

```
name:          "pc-host-sweep-components"
description:   "Sweep all validated components and return the formatted bundle text."
consumer_tags: ["01:monty", "02:orchestrator"]
source: "system" / validation_status: "validated"
```

#### 4d. PythonCode: `pc-host-store-prefix-bundle` (class 22)

```python
# Channel: orchestrator | Class: 22 | No I/O, no imports.
# Uses bundle_parts set by pc-host-sweep-components (same assembled_program).
store_result = host.store_prefix_bundle(
    user_id="{{vars.slot0}}",
    project_id="{{vars.slot1}}",
    bundle=bundle_parts["bundle"]
)
```

```
name:          "pc-host-store-prefix-bundle"
description:   "Store the assembled bundle text in PgBasicPromptStore."
consumer_tags: ["01:monty", "02:orchestrator"]
source: "system" / validation_status: "validated"
```

#### 4e. Leaf Skill: `skill-sweep-validated-components` (class 1)

```
name:  "skill-sweep-validated-components"
body:  "Use host.sweep_validated_components(user_id, project_id) to collect all
        validated components from every component table, with no consumer_tags
        filtering. The tool appends CLAUDE.md architecture sections and AGENTS.md
        routing sections to the bundle. Returns {bundle, component_count}."
class_code: 1
consumer_tags: ["02:orchestrator"]
```

#### 4f. Leaf Skill: `skill-store-prefix-bundle` (class 1)

```
name:  "skill-store-prefix-bundle"
body:  "Use host.store_prefix_bundle(user_id, project_id, bundle) to persist the
        assembled bundle text in PgBasicPromptStore for the given scope. Call this
        after sweep_validated_components with the bundle text it returned. Returns
        {fingerprint, generation_ms}."
class_code: 1
consumer_tags: ["02:orchestrator"]
```

#### 4g. Recipe: `host-assemble-prefix-bundle` (class 21)

Seeded as Slice 13 in `seed_builtin_host.rs`.

**`steps` JSONB** (Tier-0 — no LLM):

```json
{
  "llm_call_required": false,
  "tier": 0,
  "rust_steps": [
    {"tool": "host.sweep_validated_components",
     "tool_skill": "ts-host-sweep-components"},
    {"tool": "host.store_prefix_bundle",
     "tool_skill": "ts-host-store-prefix-bundle"},
    {"tool": "host.post_reply",
     "tool_skill": "ts-host-post-reply"}
  ],
  "orchestrator_steps": [
    {"python_code": "pc-host-sweep-components"},
    {"python_code": "pc-host-store-prefix-bundle"},
    {"python_code": "pc-host-post-reply"}
  ]
}
```

> **Note:** `pc-host-post-reply` is already seeded (Slice 4). The recipe reuses it.
> The last block's `result = host.post_reply(answer=…)` terminates the assembled_program.

**`step_descriptions`** (`desc_idx=0`):

Skills are co-located **in the same step's `include` array** as the PythonCode they
contextualise. `compose_program()` separates them automatically: PythonCode UUID →
`executable_code` (into `assembled_program`); Skill UUIDs → `program.skills`
(narrative context Monty receives). Skills are never separate steps.

```json
[
  {"desc_idx": 0, "label": "Assemble prefix bundle", "yaml_source": "", "steps": [
    {"stepnumber": 1, "knowledge": "rust", "goal": "Pre-load sweep tool binding",
     "content": "ts-host-sweep-components", "type": "component",
     "include": ["<uuid:ts-host-sweep-components>"]},
    {"stepnumber": 2, "knowledge": "orchestrator",
     "goal": "Sweep all validated components",
     "content": "pc-host-sweep-components", "type": "component",
     "include": [
       "<uuid:pc-host-sweep-components>",
       "<uuid:skill-sweep-validated-components>"
     ]},
    {"stepnumber": 3, "knowledge": "rust", "goal": "Pre-load store tool binding",
     "content": "ts-host-store-prefix-bundle", "type": "component",
     "include": ["<uuid:ts-host-store-prefix-bundle>"]},
    {"stepnumber": 4, "knowledge": "orchestrator",
     "goal": "Store bundle in PgBasicPromptStore",
     "content": "pc-host-store-prefix-bundle", "type": "component",
     "include": [
       "<uuid:pc-host-store-prefix-bundle>",
       "<uuid:skill-store-prefix-bundle>"
     ]},
    {"stepnumber": 5, "knowledge": "rust", "goal": "Pre-load post-reply tool binding",
     "content": "ts-host-post-reply", "type": "component",
     "include": ["<uuid:ts-host-post-reply>"]},
    {"stepnumber": 6, "knowledge": "orchestrator", "goal": "Emit confirmation reply",
     "content": "pc-host-post-reply", "type": "component",
     "include": ["<uuid:pc-host-post-reply>"]}
  ]}
]
```

**Single variant**:

```json
{
  "variant_key":  "default",
  "step_link":    "0:1-0:E",
  "description":  "Tier-0: assemble full prefix bundle — no LLM.",
  "intent_examples": [
    "regenerate prefix bundle",
    "assemble base prompt",
    "rebuild prefix cache",
    "regenerate base-prompt",
    "refresh prefix bundle",
    "assemble bundle for user",
    "rebuild the prefix for this project",
    "generate the base prompt bundle",
    "re-assemble prefix knowledge base",
    "trigger prefix bundle rebuild",
    "regenerate prefix bundle user=foo project=bar",
    "update prefix knowledge base",
    "rebuild kohai prefix",
    "freshen the base prompt"
  ],
  "variable_patterns": [
    {"name": "slot0", "pattern": "user=(\\S+)",
     "description": "user_id from user=<value>"},
    {"name": "slot1", "pattern": "project=(\\S+)",
     "description": "project_id from project=<value>"}
  ]
}
```

**Recipe metadata**:

```
name:          "host-assemble-prefix-bundle"
description:   "Assemble and store the full prefix bundle (all validated components,
                no consumer_tags filter, + CLAUDE.md + AGENTS.md). Tier-0, no LLM."
consumer_tags: ["02:orchestrator"]
source:        "system" / validation_status: "validated"
```

After insert: `stores.recipe.mark_mature(recipe_id)` — sets `tier='mature'`,
`wilson_lower=1.0` so the recipe is Tier-0 eligible from first boot.

---

### Change 5 — `regenerate_prefix`: turn submission, not direct assembly

`RebornInterceptorConfigService` gains a `runtime` field (already `pub(crate)` in the
composition crate).

Add field:

```rust
runtime: Option<Arc<RebornRuntime>>,
```

Add builder:

```rust
pub fn with_runtime(mut self, r: Arc<RebornRuntime>) -> Self {
    self.runtime = Some(r);
    self
}
```

Replace lines 578–580 in `regenerate_prefix`:

```rust
// OLD:
let (bundle, fingerprint, generation_ms) =
    self.do_assemble_bundle(user_id, project_id, false).await?;

// NEW:
let runtime = self.runtime.as_ref()
    .ok_or(InterceptorConfigServiceError::Unavailable)?;

let conversation = runtime
    .ensure_system_conversation(user_id, project_id)
    .await
    .map_err(|_| InterceptorConfigServiceError::Unavailable)?;

runtime
    .send_user_message(
        &conversation,
        &format!("regenerate prefix bundle user={user_id} project={project_id}"),
    )
    .await
    .map_err(|_| InterceptorConfigServiceError::Unavailable)?;
// The recipe has stored the bundle in PgBasicPromptStore.
// Read it back for the response DTO.
```

`fingerprint` and `generation_ms` are then read from
`PgBasicPromptStore::get_for_scope(user_id, project_id)` — same pattern already used
for `assembled_at`/`prewarm_last_at` at lines 649–663.

**`ensure_system_conversation`**: a new thin helper on `RebornRuntime` that gets or
creates a stable internal conversation for system/operator-triggered turns in a given
`(user_id, project_id)` scope.

**Delete** from `interceptor_config_service.rs`:
- `COMPONENT_TABLES` const (lines 61–155)
- `class_label()` fn (lines 158–187)
- `do_assemble_bundle()` (lines 288–411)
- `do_format_bundle()` (lines 417–443)
- `get_system_bundle` on `RebornInterceptorConfigService` (delegates to store; keep)
- The `do_format_bundle` unit tests at lines 748–780

The remaining `regenerate_prefix` body handles rate-limiting, the new turn submission,
reading back the stored entry, and the unchanged gateway prewarm block.

---

### Change 6 — Wire backends and runtime into services (`webui.rs`)

Two backend injections in the interceptor service construction block (around line 403):

```rust
// Sweep backend (reads component tables, formats bundle)
let sweep_backend = Arc::new(PgSweepComponentsBackend::new(
    Arc::clone(&pool),
    tenant_id.clone(),
));
// Store backend (writes PgBasicPromptStore)
let store_backend = Arc::new(PgStorePrefixBundleBackend::new(
    Arc::clone(&pool),
    tenant_id.clone(),
));
```

Register with tool states (same pattern as `PgComponentDbBackend`):

```rust
tools.sweep_components.with_backend(sweep_backend);
tools.store_prefix_bundle.with_backend(store_backend);
```

Wire runtime:

```rust
if let Some(runtime) = services.runtime.clone() {
    interceptor_svc = interceptor_svc.with_runtime(runtime);
}
```

---

## Files to Create / Modify

| File | Change |
|------|--------|
| `crates/brassclaw_host_runtime/src/first_party_tools/sweep_components.rs` | **NEW** — `SWEEP_COMPONENTS_CAPABILITY_ID`, `SweepComponentsBackend` trait, `SweepComponentsState`, manifest, handler |
| `crates/brassclaw_host_runtime/src/first_party_tools/store_prefix_bundle.rs` | **NEW** — `STORE_PREFIX_BUNDLE_CAPABILITY_ID`, `StorePrefixBundleBackend` trait, `StorePrefixBundleState`, manifest, handler |
| `crates/brassclaw_host_runtime/src/first_party_tools/mod.rs` | `mod sweep_components;` + `mod store_prefix_bundle;` + re-exports + manifests |
| `crates/brassclaw_reborn_composition/src/pg_sweep_components_backend.rs` | **NEW** — implements `SweepComponentsBackend`: DB sweep (no `05:validator` filter), CLAUDE.md + AGENTS.md `include_str!`, Sempai schema footer |
| `crates/brassclaw_reborn_composition/src/pg_store_prefix_bundle_backend.rs` | **NEW** — implements `StorePrefixBundleBackend`: calls `PgBasicPromptStore::store()` |
| `crates/brassclaw_reborn_composition/src/seed_builtin_host.rs` | Slice 13: `seed_host_assemble_prefix_bundle()` — 2 ToolSkills + 2 PythonCode + 2 leaf Skills + Recipe |
| `crates/brassclaw_reborn/src/runtime.rs` (or composition runtime) | Add `ensure_system_conversation()` helper |
| `crates/brassclaw_reborn_composition/src/interceptor_config_service.rs` | Delete `COMPONENT_TABLES`, `class_label`, `do_assemble_bundle`, `do_format_bundle`; add `runtime` field + `with_runtime` builder; replace assembly call with turn submission |
| `crates/brassclaw_reborn_composition/src/webui.rs` | Wire `RebornRuntime` + two backends into services |
| `crates/brassclaw_reborn_composition/src/lib.rs` | `pub(crate) mod pg_sweep_components_backend;` + `pub(crate) mod pg_store_prefix_bundle_backend;` |

**Unchanged**:
- `pg_basic_prompt_store.rs` — `store`, `get_for_scope`, `mark_stale`, etc. untouched.
- Gateway prewarm block in `regenerate_prefix` — untouched (reads stored bundle from
  `PgBasicPromptStore` exactly as the fingerprint/timestamp block does).
- Slices 1–12 in `seed_builtin_host.rs` — untouched.
- `pc-host-post-reply` (Slice 4) — reused by the new recipe, not re-seeded.
- `doc-sync` / `doc-convert` / `host-assemble-prior-knowledge` — untouched.
- `list_prefix_entries` — untouched.

---

## How data flows through the assembled_program

`compose_program()` ([`composition.rs:235`](crates/brassclaw_engine/src/memory/composition.rs:235))
builds `assembled_program` by joining the `executable_code` of each orchestrator step:

```
pc-host-sweep-components body
\n\n
pc-host-store-prefix-bundle body
\n\n
pc-host-post-reply body
```

This is **one Python script**. The Monty VM runs it start to finish in one execution
context. `bundle_parts` is a Python local variable — by the time
`host.store_prefix_bundle(bundle=bundle_parts["bundle"])` executes, `bundle_parts` was
already assigned by the earlier `host.sweep_validated_components(...)` call in the same
script.

`program.skills` (the Skill bodies co-located in each step's `include`) are the
narrative context blocks Monty receives alongside the program. They describe *how*
each tool works — but the PythonCode already encodes the exact call, so for Tier-0
the skills are reference material that confirms the intent, not instructions the
orchestrator must interpret.

The `{{vars.slot0}}` / `{{vars.slot1}}` substitutions are IBS-baked into both
`pc-host-sweep-components` and `pc-host-store-prefix-bundle` bodies before execution —
they resolve to the `user_id` and `project_id` extracted from the turn message
`"regenerate prefix bundle user=X project=Y"`.

---

## variable_patterns / slot capture

The message: `"regenerate prefix bundle user={user_id} project={project_id}"`

```
slot0 pattern: "user=(\S+)"   → captures user_id
slot1 pattern: "project=(\S+)" → captures project_id
```

These are applied by `capture_variables()` in `instruction_builder.rs:824` on the
matched intent row's `input_text` template. The key=value encoding avoids fragile
positional splitting.

---

## Validation

1. `cargo clippy -p brassclaw_host_runtime -p brassclaw_reborn_composition --all-targets -- -D warnings` — zero warnings.
2. `cargo test -p brassclaw_reborn_composition` — existing tests pass; deleted `do_format_bundle` tests removed.
3. New seeder smoke test: `host-assemble-prefix-bundle` row present with `tier='mature'`,
   `wilson_lower=1.0`, `llm_call_required=false`.
4. Integration: POST `/api/prefixes/base-prompt/regenerate` → assert `reborn_basic_prompt_store`
   bundle contains `"## Architecture"` (CLAUDE.md) and `"## Architecture Mental Model"` (AGENTS.md).
5. Assert no `consumer_tags` filter: a component with `consumer_tags=['05:validator']` appears
   in the assembled bundle.
6. `include_str!` paths compile: `"../../../CLAUDE.md"` and `"../../../AGENTS.md"` from
   `brassclaw_reborn_composition/src/` resolve correctly at build time.
7. `compose_program` test: two PythonCode orchestrator steps produce a joined
   `assembled_program` where `bundle_parts` assigned in step 1 is accessible in step 2.

---

## Risk / Open Points

| Risk | Resolution |
|------|-----------|
| `send_user_message` is `pub(crate)` — accessible from `RebornInterceptorConfigService` | Confirmed — both live in `brassclaw_reborn_composition`. No visibility change needed. |
| `ensure_system_conversation` does not exist yet | One new helper on `RebornRuntime`; thin wrapper over `SessionThreadService::ensure_thread` with a fixed system conversation key. |
| Intent matching precision | 14+ intent examples + key=value encoding; the `%` template `"regenerate prefix bundle user=% project=%"` unambiguously captures both slots. |
| `variable_patterns` regex correctness | `user=(\S+)` is standard and already used in similar recipes. Verify against `capture_variables` in `instruction_builder.rs:824`. |
| `include_str!` path depth | From `brassclaw_reborn_composition/src/` = 3 levels up = `"../../../"` ✓ |
| `PrefixRegenerateResponse` fields after turn completes | `PgBasicPromptStore::get_for_scope` reads them back; same pattern already used at lines 649–663. |
| Turn latency | Acceptable: Regenerate was already slow; rate limit (1/min) remains. |
| `pc-host-post-reply` body expected format | Must assign `result = host.post_reply(...)` at end; already seeded in Slice 4 with that pattern. |
| Rust tool registration order | `sweep_components` and `store_prefix_bundle` must be registered before `seed_builtin_host_components` runs; wiring order in `webui.rs` must be checked. |
