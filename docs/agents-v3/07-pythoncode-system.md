
> **Subsystem:** PythonCode (class 22) — the orchestrator-channel component that carries an
> executable Python body the orchestrator runs (in the Monty VM) to drive a recipe. It is the
> "body" half of the recipe step pair: a `type:"component"` step `include`s a PythonCode UUID and
> the component's `content` *is* the orchestrator instruction.
> **Grounded in:** `saved_plan_to_v3.md` Phase B (V052), §0.5 (StepDescription authoring layer),
> §0.18 (validation queue), §0.16 (grain rule), Phase A.5 (V051 queue), Phase N (V059 gate),
> Phase L (seeder).
> **Status:** **v3-only — does not exist in the current codebase.** `V052__reborn_python_code.sql`,
> `pg_python_code_store.rs`, the class-22 validator arm, and the class-22 retrieval arms are all
> Phase B work that has not landed.

## 1. Purpose

A **PythonCode** (class 22) is the component that turns a recipe step into actual orchestrator
behavior. Where a **ToolSkill** (class 13) teaches the *Rust* execution layer how to call one tool,
a PythonCode is the *orchestrator*-channel body: a small Python program (run inside the Monty VM)
that calls host functions (`__execute_action__`, `__check_budget__`, …) to glue tools together,
format results, and drive a Tier-0 recipe to completion **without an LLM round-trip**.

PythonCode is born one of two ways:

1. **Authored directly** in the WebUI as a standalone component (the grain-rule sibling of a
   Skill — see §0.16).
2. **Promoted from a `codesnippet`** inside a StepDescription: an author writes inline Python in
   a step's `codesnippet` field, the WebUI *creates* a PythonCode row (class 22, `pending`) on
   save, the row enters the Q1 queue, and once it passes Q1 + Q2 the step is rewritten to
   `type:"component"` referencing the new UUID. Until then the IBS **refuses to assemble** the
   recipe (`IbsError::UnpromotedSnippet`).

The Q1/Q2 graduation gate that completes the snippet→component promotion is a **Phase N**
capability (V059). Until Phase N lands, only **system-seeded** (Phase L) or
**operator-validated** PythonCode is usable in `type:"component"` steps.

## 2. Location

- **Migration (new, Phase B):** `crates/brassclaw_pg/migrations/V052__reborn_python_code.sql`
  (class 22; **was V051 before Decision 2** — the V051 slot is now the validation-queue table).
- **Store (new, Phase B):** `crates/brassclaw_reborn_composition/src/pg_python_code_store.rs`
  (+ WebUI facade).
- **Validator dispatch (modify, Phase B):** `crates/brassclaw_engine/src/memory/component_validator.rs`
  — adds a `22 =>` arm to `validate_by_class` using the existing `Generic(GenericComponent {
  name, description, content })` payload (FIND-P10-03 / FINDING E — `GenericComponent` has no
  `class_code` field; the class is implicit from the dispatch arm).
- **Retrieval (modify, Phase B):** `crates/brassclaw_engine/src/memory/retrieval_source.rs`
  — adds a class-22 arm to **both** `fetch_for_consumer` (the UNION ALL fallback) and
  `fetch_component_by_id` (the direct UUID lookup). They are separate functions; both must grow
  the arm (FINDING C — today class 22 silently returns nothing on *both* paths).
- **Intent label (modify, Phase B):** `crates/brassclaw_engine/src/memory/intent_system.rs`
  — adds `22 => "python_code"` to `class_label` (today the match stops at `21 => "recipe"` and
  falls through to `_ => "component"`); the class-code legend doc-comment (lines ~250–253) is
  extended to include `22=python_code`.
- **Validation queue (Phase A.5 + Phase N):** `reborn_validation_queue` (V051, DDL only; V059
  populate + trigger + legacy-column DROP).
- **Seeder (Phase L):** `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs`.
- **Plan:** §0.5 (`codesnippet`/`snippet` promotion), §0.18 (validation queue), §0.16 (grain
  rule), Phase B, Phase A.5, Phase N, Phase L (V057).

## 3. Data model — `reborn_python_code` (V052, class 22)

| Column | Type | Notes |
|--------|------|-------|
| `id` | UUID PK | `gen_random_uuid()` |
| `tenant_id`,`user_id`,`agent_id`,`project_id` | TEXT NOT NULL | scope tuple |
| `name` | TEXT NOT NULL | `length(name) BETWEEN 1 AND 256`; unique per scope |
| `description` | TEXT NOT NULL DEFAULT '' | `<= 1024` |
| `content` | TEXT NOT NULL DEFAULT '' | **the executable Python body** — *is* the orchestrator instruction |
| `prior_knowledge_content` | TEXT | Solution-Override (§3.13/§3.14, SCH-02); present at creation, no retrofit |
| `override_prompt_creation` | BOOL NOT NULL DEFAULT false | enables the Solution-Override LLM path |
| `class_code` | SMALLINT NOT NULL DEFAULT 22 | `CHECK = 22` |
| `prompt_uid` | BIGINT | `nextval(reborn_python_code_prompt_uid_seq)`; **required** for the `fetch_for_consumer` UNION ALL sub-select (casts `prompt_uid::bigint` per arm) — missing it makes the UNION ALL fail at runtime (FIND-AUDIT-15) |
| `consumer_tags` | TEXT[] NOT NULL DEFAULT '{}' | default `{02:orchestrator, 05:validator}` until validated |
| `intent_examples` | JSONB | `[{input, class}]` for the intent system |
| `validation_status` | TEXT NOT NULL DEFAULT 'pending' | `CHECK IN (pending, auto_passed, auto_failed, validated, review_requested, rejected, garbage, upgrade_queued)` — **stays on the component table** (post-validation gate, §0.18) |
| `source` | TEXT NOT NULL DEFAULT 'authored' | `CHECK IN ('authored','extracted','migrated','imported','system')` — **`'system'` allowed from day one** (FIND-P6-02; Phase L seeds with `source='system'`; V057 only alters the *older* tables) |
| `content_hash` | TEXT | lineage |
| `similarity_parent_id` | UUID | lineage |
| `replaces_id` | UUID | lineage |
| `parent_version` | TEXT | lineage |
| `last_audit_at` | TIMESTAMPTZ | lineage |
| `audit_failure_count` | SMALLINT NOT NULL DEFAULT 0 | lineage |
| `parent_mission_id` | UUID | lineage |
| `dependency_registry` | JSONB | included at creation (Phase J.2; V055 retrofits the 13 older tables, new tables have it from day one) |
| `created_at`,`updated_at` | TIMESTAMPTZ | `set_updated_at()` trigger |

**Deliberately absent** (§0.18 — centralised on `reborn_validation_queue`): `queue_code`,
`review_attempts`, `review_feedback`, `rejected_at`, `validation_errors`. This table carries
**only** `validation_status` (the post-validation gate that stays on the component).

Unique: `(scope, name)`. Indexes: scope, scope+status, scope+prompt_uid (UNION ALL assembly
order), `consumer_tags` GIN, `similarity_parent` (partial), `replaces` (partial). Auto
`updated_at` trigger.

### Why this shape (FIND-AUDIT-15)

Earlier drafts said only "same column shape as `V036__reborn_specs.sql`". That was dangerous:
`V036` was retrofitted by `V046` to add `prior_knowledge_content`/`override_prompt_creation`.
`V052` is the **final authoritative shape**: all solution-override columns present at creation
(no `V046`-style retrofit), the five queue-tracking columns omitted (§0.18), and `prompt_uid`
present (required by the UNION ALL sub-select).

### `ComponentPayload` for class 22 (FINDING E)

There is **no** `ComponentPayload::PythonCode` variant today. Class-22 validation reuses the
existing `Generic(GenericComponent<'a>)` payload, where `GenericComponent` has exactly three
fields — `{ name, description, content }` (FIND-P10-03: it does **not** carry `class_code`; the
class is implicit from the `validate_by_class` dispatch arm). A dedicated `PythonCode` variant
may be added later if richer validation is needed, but the simpler path is `Generic`.

## 4. Behavior / flow

### 4.1 Snippet → PythonCode → component promotion (§0.5)

```
Author types inline Python in a StepDescription step's `codesnippet` field
        │
        ▼  (WebUI save; requires authenticated `component:write` session — ACL is first line of defense)
[WebUI] creates a `reborn_python_code` row (class 22, validation_status='pending')
        │
        ▼  (Phase B save path MUST call ValidationQueueStore::submit(scope, component_id, 22))
[reborn_validation_queue] state=1 (Q1 queue)   ← queue row created here from day one
        │
        ▼  (Gate 1 / Q1 runs the shell-injection scan + name/content/token-budget checks)
[Q1 pass] queue state → 2 (Q1_passed; awaiting Q2 manual review)
        │  (Q1 fail → snippet field cleared, PythonCode row removed)
        ▼
[Q2 manual approval] queue row DELETED; component validation_status → 'validated'
        │  (Q2 reject → state=3, counter++, author revises)
        ▼
[WebUI rewrites the step] type:"snippet" → type:"component" with the new PythonCode UUID;
        parent Recipe re-queued to Q1
```

- The step stays **greyed out** in the WebUI while the PythonCode is pending.
- The **Q1/Q2 gate logic that performs the `snippet`→`component` rewrite is a Phase N
  capability (V059)**: the boot-integrity check + the gate logic complete the promotion flow
  when Phase N lands. Until then, only **system-seeded (Phase L) or operator-validated**
  PythonCode is usable in `type:"component"` steps (§0.5 caveat).
- **IBS guard:** if `build_instruction` encounters a step with `type:"snippet"` it returns
  `IbsError::UnpromotedSnippet { step_id }` and refuses to assemble the `BuildInstruction`
  (§0.5 step-type table; assembly algorithm step 3). So a recipe containing an un-promoted
  snippet can never reach the runtime two-channel split.

### 4.2 Q1 shell-injection scan (FIND-AUDIT-12)

The class-22 validator scans the raw `content` string (simple substring search — no AST) before
execution. The scan is **additive** to Q1 checks; it does not replace them.

**Hard errors (Q1 fail):**
- `import os` — direct OS module (use `__execute_action__` instead)
- `import subprocess` — bypasses capability dispatch
- `import sys` — interpreter manipulation risk
- `import socket` — direct network socket (bypasses host network controls)
- `import ctypes` — native escape
- `import importlib` — import-whitelist bypass
- `__import__(` — built-in dynamic import
- `exec(` — nested unsandboxed exec
- `eval(` — unsafe expression evaluation
- `open(` — direct filesystem access (use `__execute_action__("read_file", …)`)
- `compile(` — code-object injection
- `__builtins__` — builtins manipulation
- `globals()` / `locals()` — scope inspection for injection

**Warnings (Q1 soft — flag, do not block):**
- `print(` — stdout writes (the VM captures stdout, not the host terminal)
- `input(` — interactive prompt (hangs in the VM; likely a copy-paste error)

False-positive rate is low because PythonCode bodies are authored to call host functions, not
OS/subprocess. Correct usage — `__execute_action__("read_file", {"path": path})` — passes Q1.

### 4.3 At turn time (v3, after Phase E + Phase N)

A recipe `Match` → the IBS emits `orchestrator_steps[]`; for each `type:"component"` step whose
`include` UUID is a PythonCode, `fetch_for_turn` calls `fetch_component_by_id` (class-22 arm),
and `handle_assemble_prior_knowledge` renders the body into `orchestrator_content` under a
`## [PythonCode: {name}]` heading (the `StepContextSpec` for class 22 — computed at fetch time
from `class_code`, never set by the author). The orchestrator then runs that Python in the
Monty VM. In a **Tier-0** recipe the PythonCode body (not a Skill — see the S7-extension guard)
is what drives the executor **without an LLM call**.

### 4.4 The S7-extension guard (Tier 0)

For a Tier-0 recipe (`llm_call_required == false`) whose `rust_steps` carry `tool_bindings`,
the IBS requires `orchestrator_steps` to contain **≥1 PythonCode UUID (class 22)** — **not** a
Skill UUID, because Skill bodies need an LLM interpreter (§0.4.1 / assembly-algorithm step 4).
A Tier-0 recipe with `tool_bindings` and empty `orchestrator_steps` is a Q1 hard error
(§tier0-orchestrator-channel Rule 2). This is the structural reason PythonCode exists: it is
the orchestrator-channel component that can run without an LLM.

## 5. Relations

- **StepDescription / IBS** (`03`/`04`): `codesnippet` creates a PythonCode; the `snippet` step
  type blocks IBS assembly until promotion to `component`; `StepContextSpec` for class 22 =
  `PythonCode` (`## [PythonCode: {name}]`).
- **Skills** (`05`): the grain-rule sibling — Skill = capability spanning tools (narrative);
  PythonCode = sub-orchestrator utility helper. Both live in the orchestrator channel.
- **Validation Queue** (`14`): every authored PythonCode gets a `reborn_validation_queue` row
  on creation (`submit(scope, id, 22)`); Q1 (state 1→2) + Q2 (queue row deleted,
  `validation_status='validated'`) graduate it; `source='system'` (Phase L) bypasses Q2.
- **Retrieval** (`11`): `fetch_for_consumer` (UNION ALL) + `fetch_component_by_id` both need the
  class-22 arm (Phase B); ordering is `(class_code ASC, prompt_uid ASC)`.
- **Tools** (`06`): a PythonCode reaches a Tool indirectly — it calls `__execute_action__`,
  which dispatches through `CapabilityHost` to the Tool's registered handler.
- **Component Catalog** (`15`): `reborn_python_code` is the class-22 component table; `DocType`
  is frozen (no `PythonCode` variant — §0.11 FINDING B).

## 6. Status — today vs. v3

**Today:**
- `reborn_python_code` **does not exist** (no `V052` migration; the `V051`/`V052`/`V059` slots
  are all uncreated).
- `pg_python_code_store.rs` does not exist.
- `class_label` in `intent_system.rs` stops at `21 => "recipe"` and falls through to
  `_ => "component"` — class 22 is **not** `"python_code"` today.
- `fetch_for_consumer` and `fetch_component_by_id` have **no class-22 arm** — class 22
  silently returns nothing on both paths (FINDING C).
- `ComponentPayload` has no `PythonCode` variant (would reuse `Generic`).
- `reborn_validation_queue` does not exist (V051 not created); the snippet→Q1→Q2 promotion flow
  has no queue to enter.
- No PythonCode rows, system or otherwise (Phase L not done).

**v3 plan adds:**
- **Phase A.5 (V051):** `reborn_validation_queue` table (DDL + indexes only) +
  `ValidationQueueStore` (submit/approve/reject; `approve` is one transaction: UPDATE component
  table then DELETE queue row, dispatch on `component_class` — FIND-P9-05).
- **Phase B (V052):** `reborn_python_code` (final authoritative shape, FIND-AUDIT-15) +
  `pg_python_code_store.rs`; the Phase-B WebUI-save path calls
  `ValidationQueueStore::submit(scope, id, 22)` on creation (queue row from day one); class-22
  validator arm (`Generic(GenericComponent)`, FINDING E) with the FIND-AUDIT-12 shell-injection
  scan; class-22 `fetch_for_consumer` + `fetch_component_by_id` arms (FINDING C);
  `22 => "python_code"` in `class_label` + the legend doc-comment.
- **Phase N (V059):** the gate logic that completes the `snippet`→`component` rewrite +
  boot-integrity check; populate `reborn_validation_queue` from existing component tables +
  the `last_graduation_at` trigger + DROP the five per-table queue columns from the older
  tables (`reborn_python_code` never had them).
- **Phase L (V057):** `builtin_bootstrap.rs` seeds 4–5 PythonCode helpers
  (`source='system'`, `validated`, Q2 bypassed) across the 5 domain groups.
- **Phase E:** `fetch_for_consumer` UNION ALL gains the class-22 arm; `class_code_to_table`
  includes the `10 | 50` arm; ordering is automatic via `class_code ASC, prompt_uid ASC`.

## 7. LLM-relevant summary

A PythonCode (class 22, `reborn_python_code` V052/Phase B) is the orchestrator-channel
component whose `content` column **is** the executable orchestrator instruction (Python run in
the Monty VM, calling `__execute_action__`). It is born authored directly or promoted from a
StepDescription `codesnippet` (on WebUI save → class-22 `pending` row → Q1 queue → Q1+Q2 pass
→ step rewritten `snippet`→`component` with the new UUID; IBS refuses un-promoted snippets with
`IbsError::UnpromotedSnippet`). The Q1/Q2 promotion gate is a **Phase N (V059)** capability —
until then only system-seeded (Phase L) or operator-validated PythonCode is usable. Q1 runs a
shell-injection scan (hard-fail on `import os/subprocess/sys/socket/ctypes/importlib`,
`__import__`, `exec`, `eval`, `open`, `compile`, `__builtins__`, `globals`/`locals`; warn on
`print`/`input`). The table omits the five queue-tracking columns (centralised on
`reborn_validation_queue`, V051) and keeps only `validation_status`; `source` allows `'system'`
from day one (Phase L); `prompt_uid` is required for the UNION ALL. Class-22 validation reuses
`Generic(GenericComponent { name, description, content })` (no `class_code` field; no
`PythonCode` payload variant yet). `class_label` gains `22 => "python_code"`; both retrieval
functions gain a class-22 arm. PythonCode is the Tier-0 orchestrator-channel body the S7 guard
requires (`tool_bindings` + `llm_call_required=false` ⇒ ≥1 class-22 UUID, never a Skill).
`DocType` stays frozen (no PythonCode variant).
