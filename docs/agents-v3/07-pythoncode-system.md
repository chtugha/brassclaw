# 07 — PythonCode System

> **Subsystem:** PythonCode (class 22) — the orchestrator-channel component that carries an
> executable Python body the orchestrator runs to drive a recipe. It is the "body" half of the
> recipe step pair: a `type:"component"` step `include`s a PythonCode UUID and the component's
> `content` *is* the orchestrator instruction.
> **Grounded in:** `crates/brassclaw_pg/migrations/V052__reborn_python_code.sql`,
> `crates/brassclaw_pg/migrations/V069__reborn_python_code_includes.sql`,
> `crates/brassclaw_reborn_composition/src/pg_python_code_store.rs`,
> `crates/brassclaw_engine/src/memory/component_validator.rs`,
> `crates/brassclaw_engine/src/memory/retrieval_source.rs`,
> `crates/brassclaw_engine/src/memory/intent_system.rs`,
> `crates/brassclaw_reborn_composition/src/composition.rs` (composer),
> `saved_plan_to_v3.md` (Phase B, C.4.5.2, Phase N, Phase L).
> **Status:** **shipped.** `V052` (table), `V069` (`includes` column), `pg_python_code_store.rs`,
> the class-22 validator arm, the class-22 retrieval arms, and `22 => "python_code"` in
> `class_label` are all landed.

## 1. Purpose

A **PythonCode** (class 22) is the component that turns a recipe step into actual orchestrator
behavior. Where a **ToolSkill** (class 13) teaches the *Rust* executioner how to call one tool, a
PythonCode is the *orchestrator*-channel body: a small Python program (run inside the Monty VM via
`host.run_program`) that calls host functions (`host.<tool>(...)`, `host.resolve_intent`,
`host.compose_orchestrator`, …) to glue tools together, format results, and drive a Tier-0 recipe
to completion **without an LLM round-trip**.

PythonCode is born one of two ways:

1. **Authored directly** in the WebUI as a standalone component (the grain-rule sibling of a
   Skill — see §0.16), or seeded by the C.2 builtin bootstrap (`source='system'`).
2. **Promoted from a `codesnippet`** inside a StepDescription: an author writes inline Python in
   a step's `codesnippet` field, the WebUI *creates* a PythonCode row (class 22, `pending`) on
   save, the row enters the Q1 queue, and once it passes Q1 + Q2 the step is rewritten to
   `type:"component"` referencing the new UUID. Until then the IBS **refuses to assemble** the
   recipe (`IbsError::UnpromotedSnippet`).

The Q1/Q2 graduation gate that *completes* the snippet→component promotion (the rewrite + boot-
integrity check) is a **Phase N** capability. Until Phase N lands, only **system-seeded** or
**operator-validated** PythonCode is usable in `type:"component"` steps — but the table, store,
validator, retrieval, and intent label are all in place today.

## 2. Location

- **Migration (table):** `crates/brassclaw_pg/migrations/V052__reborn_python_code.sql`
  (class 22; the V051 slot is the validation-queue table).
- **Migration (includes):** `crates/brassclaw_pg/migrations/V069__reborn_python_code_includes.sql`
  (C.4.5.2 — adds the `includes` JSONB column the composer inlines).
- **Store:** `crates/brassclaw_reborn_composition/src/pg_python_code_store.rs`
  (+ WebUI facade).
- **Validator dispatch:** `crates/brassclaw_engine/src/memory/component_validator.rs` — the
  `22 =>` arm of `validate_by_class` uses the `Generic(GenericComponent { name, description,
  content, extra })` payload (FINDING E — `GenericComponent` has no `class_code` field; the class
  is implicit from the dispatch arm) and calls `validate_python_code_body` +
  `validate_python_code_placeholders`.
- **Retrieval:** `crates/brassclaw_engine/src/memory/retrieval_source.rs` — the class-22 arm is
  in **both** `fetch_for_consumer` (the UNION ALL fallback, lines ~601–606) and
  `fetch_component_by_id` (the direct UUID lookup, line ~1096); `class_code_to_table` maps
  `(22, "reborn_python_code", "content")` (line ~1663).
- **Intent label:** `crates/brassclaw_engine/src/memory/intent_system.rs` — `22 => "python_code"`
  in `class_label` (line ~289); the legend doc-comment lists `21=recipe, 22=python_code,
  23=extension_catalogue, 50=scaffold` (line ~264).
- **Composer:** `crates/brassclaw_reborn_composition/src/composition.rs` — inlines each
  `{{component_name}}` placeholder by fetching the matching `includes` UUID's body (C.4.5.17).
- **Validation queue:** `reborn_validation_queue` (V051 — shipped; the Phase N populate/trigger/
  legacy-DROP is still pending).
- **Seeder:** `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs` (C.2 system seeds).

## 3. Data model — `reborn_python_code` (V052, class 22; + `includes` V069)

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
| `prompt_uid` | BIGINT NOT NULL DEFAULT `nextval(reborn_python_code_prompt_uid_seq)` | required for the `fetch_for_consumer` UNION ALL sub-select (casts `prompt_uid::bigint` per arm) — missing it makes the UNION ALL fail at runtime (FIND-AUDIT-15) |
| `consumer_tags` | TEXT[] NOT NULL DEFAULT '{}' | default `{02:orchestrator, 05:validator}` until validated |
| `intent_examples` | JSONB | `[{input, class}]` for the intent system |
| `validation_status` | TEXT NOT NULL DEFAULT 'pending' | `CHECK IN (pending, auto_passed, auto_failed, validated, review_requested, rejected, garbage, upgrade_queued)` — **stays on the component table** (post-validation gate, §0.18) |
| `source` | TEXT NOT NULL DEFAULT 'authored' | `CHECK IN ('authored','extracted','migrated','imported','system')` — **`'system'` allowed from day one** (FIND-P6-02; C.2 seeds with `source='system'`) |
| `content_hash` | TEXT | lineage |
| `similarity_parent_id` | UUID | lineage |
| `replaces_id` | UUID | lineage |
| `parent_version` | TEXT | lineage |
| `last_audit_at` | TIMESTAMPTZ | lineage |
| `audit_failure_count` | SMALLINT NOT NULL DEFAULT 0 | lineage |
| `parent_mission_id` | UUID | lineage (slated for V064-style drop, already done on some tables) |
| `dependency_registry` | JSONB | included at creation (Phase J.2; V055 retrofits the 13 older tables) |
| `includes` | JSONB NOT NULL DEFAULT '[]' | **V069 (C.4.5.2)** — `Vec<Uuid>` of mini-PythonCode components the composer inlines into `{{component_name}}` placeholders at compose time (one function each, like an include) |
| `created_at`,`updated_at` | TIMESTAMPTZ | `set_updated_at()` trigger |

**Deliberately absent** (§0.18 — centralised on `reborn_validation_queue`): `queue_code`,
`review_attempts`, `review_feedback`, `rejected_at`, `validation_errors`. This table carries
**only** `validation_status` (the post-validation gate that stays on the component). V052 is the
**final authoritative shape**: all solution-override columns present at creation (no V046-style
retrofit), the five queue-tracking columns omitted, `prompt_uid` present, and V069 adds `includes`.

Unique: `(scope, name)`. Indexes: scope, scope+status, scope+prompt_uid (UNION ALL assembly
order), `consumer_tags` GIN, `similarity_parent` (partial), `replaces` (partial). Auto
`updated_at` trigger.

### `ComponentPayload` for class 22 (FINDING E)

There is **no** `ComponentPayload::PythonCode` variant. Class-22 validation reuses the existing
`Generic(GenericComponent)` payload, where `GenericComponent` carries `{ name, description,
content, extra }` (`extra` transiently carries the `includes` array on the Q1 save path; the
canonical store is the `includes` column). The class is implicit from the `validate_by_class`
dispatch arm.

## 4. Behavior / flow

### 4.1 Snippet → PythonCode → component promotion (§0.5)

```
Author types inline Python in a StepDescription step's `codesnippet` field
        │
        ▼  (WebUI save; requires authenticated `component:write` session — ACL is first line of defense)
[WebUI] creates a `reborn_python_code` row (class 22, validation_status='pending')
        │
        ▼  (save path calls ValidationQueueStore::submit(scope, component_id, 22))
[reborn_validation_queue] state=1 (Q1 queue)   ← queue row created here
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
- The **Q1/Q2 gate logic that performs the `snippet`→`component` rewrite + the boot-integrity
  check is a Phase N capability**: until Phase N lands, only system-seeded or operator-validated
  PythonCode is usable in `type:"component"` steps (§0.5 caveat).
- **IBS guard:** if `build_instruction` encounters a step with `type:"snippet"` it returns
  `IbsError::UnpromotedSnippet { step_id }` and refuses to assemble the `BuildInstruction`.
  So a recipe containing an un-promoted snippet can never reach the runtime two-channel split.

### 4.2 Q1 shell-injection scan (FIND-AUDIT-12, shipped)

`validate_python_code_body` scans the raw `content` string (simple substring search — no AST)
before execution. The scan is **additive** to Q1 checks; it does not replace them.

**Hard errors (Q1 fail):** `import os`, `import subprocess`, `import sys`, `import socket`,
`import ctypes`, `import importlib`, `__import__(`, `exec(`, `eval(`, `open(`, `compile(`,
`__builtins__`, `globals()`, `locals()`. The error message directs authors to the new host-call
model: *"use host tool calls (`host.<tool>(...)`) for host access instead"* (the retired
`__execute_action__` meta-primitive was removed in C.1).

**Warnings (Q1 soft — flag, do not block):** `print(` (stdout is VM-captured, not the host
terminal), `input(` (hangs in the VM; likely a copy-paste error).

False-positive rate is low because PythonCode bodies are authored to call host functions, not
OS/subprocess. Correct usage — `host.read_file(path=...)` — passes Q1.

### 4.3 Placeholder + includes validation (C.4.5.2, shipped)

`validate_python_code_placeholders` runs two structural checks (Q1 never bakes; the composer
C.4.5.17 is the sole baker):

- **Placeholder grammar** (shared `validate_placeholder_grammar`, C.4.5.3): every `{{ ... }}` in
  `content` must have balanced braces and a recognised kind — `vars.NAME` (identifier),
  `vars.slotN` (non-negative integer), `user_input`, or `component_name`. Variables
  (`{{vars.NAME}}` / `{{user_input}}`) flow from the recipe/caller; PythonCode carries no
  `variable_patterns` field.
- **Includes non-nil** (Fork 2-B=B): each UUID in the `includes` array (carried in `extra` on the
  save path) must parse and be non-nil. Referential placeholder↔include matching (each
  `{{component_name}}` resolves to a real fetched component, and every include is consumed) is
  deferred to Phase I/N (requires a pool); Q1 checks structure only.

### 4.4 At turn time (shipped host-call layer; VM activation pending C.5/C.6)

A recipe `Match` → Monty calls `host.compose_orchestrator(component_id, step_link, user_input)`;
the composition system fetches each `type:"component"` step's PythonCode by UUID (class-22
retrieval arm), inlines its `{{component_name}}` includes, and returns the concrete per-step
Python in the orchestrator program. Monty runs each step via `host.run_program`. In a **Tier-0**
recipe the PythonCode body (not a Skill — see the S7-extension guard) is what drives the Rust
executioner **without an LLM call**. (The engine Monty VM that hosts this loop is dormant in
production today — activation is the C.5/C.6 driver; the Tier-0 deterministic path is active via
the turns `PgOrchestratorLookup` bridge.)

### 4.5 The S7-extension guard (Tier 0)

For a Tier-0 recipe (`llm_call_required == false`) whose `rust_steps` carry `tool_bindings`,
the IBS requires `orchestrator_steps` to contain **≥1 PythonCode UUID (class 22)** — **not** a
Skill UUID, because Skill bodies need an LLM interpreter (§0.4.1 / assembly-algorithm step 4).
A Tier-0 recipe with `tool_bindings` and empty `orchestrator_steps` is a Q1 hard error
(§tier0-orchestrator-channel Rule 2). This is the structural reason PythonCode exists: it is the
orchestrator-channel component that can run without an LLM.

## 5. Relations

- **StepDescription / IBS** (`03`/`04`): `codesnippet` creates a PythonCode; the `snippet` step
  type blocks IBS assembly until promotion to `component`; `StepContextSpec` for class 22 =
  `PythonCode` (`## [PythonCode: {name}]`).
- **Composition system** (`composition.rs` / `pg_composition_port.rs`): the composer inlines
  `{{component_name}}` includes and returns the per-step Python to Monty.
- **Skills** (`05`): the grain-rule sibling — Skill = capability spanning tools (narrative);
  PythonCode = sub-orchestrator utility helper. Both live in the orchestrator channel.
- **Validation Queue** (`14`): every authored PythonCode gets a `reborn_validation_queue` row
  on creation (`submit(scope, id, 22)`); Q1 (state 1→2) + Q2 (queue row deleted,
  `validation_status='validated'`) graduate it; `source='system'` (C.2) bypasses Q2.
- **Retrieval** (`11`): `fetch_for_consumer` (UNION ALL) + `fetch_component_by_id` both carry the
  class-22 arm; ordering is `(class_code ASC, prompt_uid ASC)`.
- **Tools** (`06`): a PythonCode reaches a Tool indirectly — it calls `host.<tool>(...)`, which
  the Rust executioner services (the retired `__execute_action__` dispatcher was removed in C.1).
- **Component Catalog** (`15`): `reborn_python_code` is the class-22 component table; `DocType`
  is frozen (no `PythonCode` variant — §0.11 FINDING B).

## 6. Status — shipped vs. pending

**Shipped:**
- `reborn_python_code` (V052) — the final authoritative shape (FIND-AUDIT-15): solution-override
  columns at creation, the five queue-tracking columns omitted (centralised on
  `reborn_validation_queue` V051), `prompt_uid` present, `source` CHECK allows `'system'`.
- `includes` JSONB column (V069, C.4.5.2) — the composer-include list.
- `pg_python_code_store.rs` (+ WebUI facade).
- Class-22 validator arm (`Generic(GenericComponent)`, FINDING E) with
  `validate_python_code_body` (FIND-AUDIT-12 scan) + `validate_python_code_placeholders`
  (C.4.5.2 placeholder grammar + includes non-nil); 6 unit tests.
- Class-22 `fetch_for_consumer` + `fetch_component_by_id` + `class_code_to_table` arms (FINDING C
  resolved).
- `22 => "python_code"` in `class_label` + the legend doc-comment.
- `reborn_validation_queue` (V051) DDL + `ValidationQueueStore` (submit/approve/reject).
- C.2 builtin bootstrap seeds system PythonCode helpers (`source='system'`, `validated`).
- The composer (C.4.5.17) inlines `{{component_name}}` includes at compose time.

**Pending:**
- **Phase N:** the gate logic that completes the `snippet`→`component` rewrite + the boot-
  integrity check; populate `reborn_validation_queue` from existing component tables + the
  `last_graduation_at` trigger (`reborn_python_code` never had the five legacy queue columns, so
  no DROP is needed here).
- **Phase I/N:** referential placeholder↔include matching (each `{{component_name}}` resolves to
  a real fetched component; every include is consumed) — requires a pool.
- **C.5/C.6 driver:** activate the engine Monty VM so the `host.run_program` loop runs in
  production (today the Tier-0 deterministic path is active via the turns `PgOrchestratorLookup`
  bridge; the engine VM host-call path is wired but dormant).
- **Future:** a WebUI PythonCode-authoring route.

## 7. LLM-relevant summary

A PythonCode (class 22, `reborn_python_code` V052) is the orchestrator-channel component whose
`content` column **is** the executable orchestrator instruction (Python run in the Monty VM via
`host.run_program`, calling `host.<tool>(...)`). It is born authored directly, seeded by the C.2
builtin bootstrap (`source='system'`), or promoted from a StepDescription `codesnippet` (on WebUI
save → class-22 `pending` row → Q1 queue → Q1+Q2 pass → step rewritten `snippet`→`component` with
the new UUID; IBS refuses un-promoted snippets with `IbsError::UnpromotedSnippet`). The Q1/Q2
promotion *gate logic* is a **Phase N** capability — until then only system-seeded or operator-
validated PythonCode is usable. Q1 runs a shell-injection scan (hard-fail on `import
os/subprocess/sys/socket/ctypes/importlib`, `__import__`, `exec`, `eval`, `open`, `compile`,
`__builtins__`, `globals`/`locals`; warn on `print`/`input`) and the C.4.5.2 placeholder-grammar +
includes-non-nil checks. The table omits the five queue-tracking columns (centralised on
`reborn_validation_queue`, V051) and keeps only `validation_status`; V069 adds the `includes`
JSONB column the composer inlines. Shipped: V052/V069/store/validator/retrieval/class_label/C.2
seed/composer. Pending: Phase N gate logic, Phase I/N referential matching, the C.5/C.6 VM driver,
a WebUI authoring route.
