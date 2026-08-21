# Auto-Documentation-Conversion Mechanism — Design & Approach (item 4)

> **Status:** DESIGN — presented for approval, **NOT implemented**. This
> document proposes how repeat item 4 will be built. Per the user's
> instruction, the mechanism is to be created **as v3 agent artifacts**
> (Recipe + Skills + Tools + PythonCode + Action component), **not as
> Rust code** ("not code per se, but as a recipe, skills, tools,
> python-code, action component"), and the approach must be presented
> before implementation begins.
> **Grounded in:** the 17 per-system docs `docs/agents-v3/01..17-*.md`
> (each carrying a §7 "LLM-summary (machine-convertible)" section),
> `15-component-catalog.md` (class-code taxonomy, `reborn_docus`
> class 17),
`crates/brassclaw_pg/migrations/V040__reborn_docus.sql` (Docu schema),
`crates/brassclaw_reborn_composition/src/interceptor_config_service.rs`
(`COMPONENT_TABLES:47`, `class_label:65`, `do_reassemble:204` — the base
prompt assembly that reads validated components),
`crates/brassclaw_reborn_composition/src/component_import.rs`
(`content_hash` idempotency pattern),
`09-sempai-kohai.md` (idle-time self-optimization loop, item 7),
`10-prefix-base-prompt.md` + `17-webui-prefix-tab.md` (prefix caching,
`reborn_basic_prompt_store` V056, `mark_stale`-on-graduation),
`14-validation-queue.md` (Q1/Q2), `08-actions-system.md` (class-16 Action
no-LLM `execute_action_procedure`), `03-recipe-system.md` (Tier-0/1/2),
`07-pythoncode-system.md` (class 22), `05-skills-system.md` / `06-tools-system.md`,
and `saved_plan_to_v3.md` §0.13 (KV-cache / `basic_prompt_section_refs`
pointers), §0.16 (builtin bootstrap `source='system'` + Q2 bypass), §0.18
(validation queue), Phase K.1 (BasicPromptStore).

---

## 1. Goal & scope (item 4, restated)

The user's repeat item 4 asks for a mechanism that:

1. **Converts** each documentation file (`docs/agents-v3/*.md`) into an
   **LLM-optimized form** (token-efficient, prompt-ready).
2. **Inserts** the converted form **into the DB**.
3. **Automatically updates** the stored converted docs (keeps them fresh
   when the source docs change).
4. Makes the converted docs **injectable into an LLM prompt** if needed
   or selected.
5. Makes the **prefix prompts** (the base prompt) **contain** these
   converted + optimized docs.
6. **Runs automatically**.
7. Is built **as v3 agent artifacts** (Recipe / Skills / Tools /
   PythonCode / Action), **not as Rust code** — the agent operates on
   itself through the same component catalog + execution paths it uses
   for every other task.

The companion requirement (item 7, `09-sempai-kohai.md`): the Sempai-Kohai
interceptor runs **idle-time self-optimization** — sending everything that
belongs to a chat/prompt to the Sempai to receive new skills/tools/recipes/
python-code, queued for validation. The doc-conversion mechanism is a
**concrete instance** of that idle-time loop: it is the recipe the Sempai
produces (and the Kohai schedules) to keep the agent's own documentation
inside its base prompt.

---

## 2. Storage target — `reborn_docus` (class 17, Docu)

### 2.1 Why class 17

The component catalog (`15-component-catalog.md`) reserves **class 17
= Docu** for reference documentation. `reborn_docus` is the
first-class "documentation" component table — the natural home for
"the agent's own documentation, converted for LLM use." It is **not** a
Spec (12), Note (20), or ExtensionCatalogue (23 — that is the
*namespace/catalogue* class, not the doc-text class).

`reborn_docus` (`V040__reborn_docus.sql`) already has every column the
mechanism needs:

| Column | Use in this mechanism |
|--------|------------------------|
| `name` | stable key = the source doc slug, e.g. `agents-v3::02-intent-system` (unique per scope: `UNIQUE(tenant,user,agent,project,name)`) |
| `description` | one-line human summary (≤1024 chars) |
| `content` | the **LLM-optimized converted text** (what `do_reassemble` reads into the base prompt) |
| `prior_knowledge_content` (SCH-02) | optional richer form used by the per-turn retrieval path (`PostgresSource`) instead of `content` |
| `override_prompt_creation` | `false` for normal docs (assembled normally); could be `true` for a doc that must replace standard assembly (Solution Override) |
| `class_code` | `17` (CHECK `= 17`) |
| `prompt_uid` | sequence — the stable base-prompt ordering key |
| `consumer_tags` | `{03:llm}` (and `{02:orchestrator}` where relevant) — **never `05:validator`**, or `do_reassemble` excludes the row |
| `validation_status` | `'validated'` for system-authored converted docs (the retrieval/base-prompt gate) |
| `source` | `'system'` — and crucially `reborn_docus` has **no `source` CHECK constraint** (unlike tools/tool_skills/skills), so `'system'` is already allowed with no migration |
| `content_hash` | **the staleness key** — SHA-256 of the source `.md` (mirrors `component_import.rs`'s idempotency) |
| lineage (`similarity_parent_id`, `replaces_id`, `parent_version`, `last_audit_at`, `audit_failure_count`) | versioning the converted doc across regenerations |

### 2.2 The one prerequisite (composition, not a migration)

`reborn_docus` is **not** in the `COMPONENT_TABLES` const
(`interceptor_config_service.rs:47`) and `class_label` (`:65`) has no
`17` arm (it falls to `_ => "Component"`). Therefore `do_reassemble`
(the base-prompt assembler, `:204`) **does not read Docu rows today**.
For converted docs to flow into the base prompt (item 4.5), the
mechanism's wiring step adds:

- `("reborn_docus", 17)` to `COMPONENT_TABLES`;
- `17 => "Docu"` to `class_label`.

This is a **Rust const edit in composition**, not a SQL migration —
additive, no data movement, no row breakage. It is the single piece of
"host" code the mechanism needs; everything else is v3 artifacts. (This
is consistent with how `reborn_orchestrators`/`reborn_scaffolds` are
already listed in `COMPONENT_TABLES` and "gracefully skipped when
absent" — `reborn_docus` is present, so it would simply start being
read.)

---

## 3. The conversion — source doc → LLM-optimized form

### 3.1 Source shape

Each `docs/agents-v3/*.md` follows the 7-section convention
(`README.md` "How each document is structured"):

1. Purpose · 2. Location · 3. Data model · 4. Behavior · 5. Relations ·
6. Today vs v3 · **7. LLM-summary (machine-convertible)**.

Section 7 was authored *specifically* to be machine-convertible — a
compressed, bullet-form, line-cited summary of the subsystem. This makes
the conversion largely **deterministic** (extract §7 + the header
metadata) with an **optional LLM-assisted compression** pass for docs
that exceed the per-doc token budget.

### 3.2 The converted form

The LLM-optimized form stored in `reborn_docus.content` is:

```
## 17:{prompt_uid}  Docu  "{name}"

<doc-slug> · <one-line description>
<§7 LLM-summary verbatim, or LLM-compressed to fit the per-doc budget>
```

This matches the `do_reassemble` render format exactly
(`## {class_code}:{prompt_uid}  {label}  "{name}"\n\n{content}`), so a
Docu row's `content` is already in base-prompt shape — `do_reassemble`
just concatenates it with the other validated components, ordered by
`(class_code, prompt_uid)`.

### 3.3 Token budget

Per §0.13, the base prompt is the cached prefix; per-turn patches are
<4k tokens. The converted docs live in the **prefix**, so their total
budget is the prefix budget (operator-controlled via the Prefix Tab, with
a `prior_knowledge_token_budget`-style ceiling on
`reborn_monty_vm_settings`). Per-doc target: each converted doc ≤ ~600
tokens (the §7 summary is already ~200-400 tokens for most docs). If the
deterministic §7 extract exceeds the per-doc budget, the LLM-assisted
pass compresses it (Sempai).

---

## 4. The v3 artifacts that implement the mechanism

The mechanism is a **self-contained "doc-sync" extension** — one
ExtensionCatalogue (class 23) grouping the components below. Every part
is a DB-stored, validated, retrievable component; the agent runs the
mechanism through the same execution paths it uses for any other task.

### 4.1 Action (class 16) — `doc-sync` (the no-LLM driver)

`08-actions-system.md`: a class-16 Action encodes step-by-step
orchestrator instructions executed by `execute_action_procedure` **with
no LLM call**. `doc-sync` is the deterministic driver:

1. **List** the source docs (`read_file`/`glob` builtin over
   `docs/agents-v3/*.md`).
2. For each doc, **compute** `content_hash = SHA-256(source_text)`.
3. **Compare** to the stored `reborn_docus.content_hash` for the matching
   `name` slug (one PK-style read per doc).
4. **Skip** unchanged docs (same hash → no work, mirroring
   `component_import.rs` idempotency).
5. For changed/new docs, **dispatch** the `doc-convert` Recipe (§4.2) —
   the LLM-assisted conversion — and **upsert** the resulting Docu row
   (`source='system'`, `validation_status='validated'`, `consumer_tags={03:llm}`,
   new `content_hash`).
6. If any doc changed, **mark the base prompt stale**:
   `PgBasicPromptStore::mark_stale(scope)` (Phase K.1) so the Prefix Tab's
   regenerate button lights up.
7. **Report** a summary (N docs scanned, M changed, base-prompt
   stale=yes/no).

This is the `action_short_circuit` path (`08-actions-system.md`,
`FetchForTurnResult::ActionShortCircuit`) — no BuildInstruction, no IBS,
no prior-knowledge assembly. The orchestrator calls
`execute_action_procedure(action_doc, goal, state)` and returns.

### 4.2 Recipe (class 21) — `doc-convert` (the LLM-assisted converter)

A Tier-1 Recipe (`llm_call_required: true`, `03-recipe-system.md`) that
converts **one** changed doc to its LLM-optimized form when the
deterministic §7 extract is too large or needs compression:

- **Step 1 (knowledge: orchestrator, type: text):** the conversion
  instruction (use the `doc-convert` Skill, §4.3) + the source doc path.
- **Step 2 (knowledge: orchestrator, type: component):** include the
  `doc-convert` Skill (class 1-3) UUID.
- **Step 3 (knowledge: orchestrator, type: llm):** `__llm_complete__`
  with a compression prompt: "Compress the following subsystem doc to
  ≤{budget} tokens for inclusion in an LLM base prompt. Keep all
  line-cited facts and the §7 structure. Drop prose. Output only the
  converted text." The Sempai (if connected) reviews this prompt before
  shipment (`09-sempai-kohai.md` rerouting).
- **Step 4 (knowledge: rust, type: component):** the PythonCode helper
  (`doc_upsert`, §4.4) writes the result to `reborn_docus`.

Variants: `by-extract` (deterministic §7 only, no LLM — Tier 0 at high
Wilson) and `by-llm-compress` (the LLM pass — Tier 1). The `doc-sync`
Action picks the variant per doc (extract first; fall back to compress
if over budget).

### 4.3 Skill (class 1-3) — `doc-convert` (Classic, DB-stored)

A Classic Claude-style Skill (`05-skills-system.md` item 5.1): DB-stored
parts (name, description, body, activation criteria), no `SKILL.md` file,
exportable on demand via the WebUI. Its body is the conversion rubric:
what sections to keep (§7 verbatim; §2 location only if cited), what to
drop (prose, repetition), the `do_reassemble` render format, the token
budget, and the rule "never invent facts — only compress what is in the
source." This is the prompt the Sempai-Kohai system optimizes over time
(item 7).

### 4.4 PythonCode (class 22) — `doc_upsert`, `doc_hash`, `doc_diff`

`07-pythoncode-system.md`: utility helpers used inside the Recipe's
orchestrator channel, not standalone capabilities:

- `doc_hash(source_text) -> sha256` — the staleness key.
- `doc_upsert(scope, slug, content, content_hash, budget)` — the
  `INSERT ... ON CONFLICT (scope, name) DO UPDATE` into `reborn_docus`
  (mirrors `PgMontyVmSettingsStore::upsert`'s `INSERT … ON CONFLICT … DO
  UPDATE`, `16-kernel-composition.md`). Sets `source='system'`,
  `validation_status='validated'`, `consumer_tags={03:llm}`.
- `doc_diff(scope, slug, new_hash) -> bool` — "has this doc changed?"
  (the skip/convert decision).

These are the snippet→component-promotion path of `07-pythoncode-system.md`:
authored as PythonCode, Q1-scanned for shell-injection, and only promoted
to a real component after the gate.

### 4.5 Tool (class 0) — reuse builtins, plus `mark_prefix_stale`

The mechanism reuses existing builtin tools (`06-tools-system.md`,
`16-kernel-composition.md` first-party tools): `read_file`/`glob` to
list and read source docs, `memory_write`/`memory_search` for any
scratch state. One **new** tool may be needed: `mark_prefix_stale(scope)`
— a thin wrapper over `PgBasicPromptStore::mark_stale` so the Action can
signal the Prefix Tab to regenerate. Alternatively, the Action calls the
existing interceptor/prewarm-adjacent path; the design prefers the
explicit tool so it is auditable and capability-gated.

### 4.6 ExtensionCatalogue (class 23) — `doc-sync`

`15-component-catalog.md` §0.2: one ExtensionCatalogue grouping all the
above (Action + Recipe + Skill + PythonCode + Tool) under the
`doc-sync` namespace, with an `overview_doc` describing the mechanism.
`source='system'`, `validation_status='validated'` (the bootstrap pattern
of §0.16 / `17-webui-prefix-tab.md`).

---

## 5. The automatic refresh loop

The mechanism "runs automatically" via the **idle-time Sempai-Kohai loop**
(item 7, `09-sempai-kohai.md`):

1. **Idle detection.** The Kohai (always-on) detects an idle chat window
   (no pending turn, no recent user message). On idle, it schedules
   self-optimization work. The `doc-sync` Action is one such work item
   (the Sempai proposes it; the Kohai queues it).
2. **Trigger alternative.** A scheduled trigger (`trigger_create` builtin,
   `06-tools-system.md`) can also fire `doc-sync` on a cadence (e.g. on
   boot, or every N minutes) — for deployments with no Sempai connected.
   The Kohai works **with and without** a Sempai (item 7): without a
   Sempai, the deterministic `by-extract` variant runs (no LLM); with a
   Sempai, the `by-llm-compress` variant is available and the Sempai
   reviews the compression prompts.
3. **Staleness-driven.** `doc-sync` does O(N) hash compares (one read
   per doc) and only converts the changed subset — so a no-op run is
   cheap. `content_hash` is the staleness key (same as
   `component_import.rs`).
4. **Base-prompt invalidation.** When any doc changes, step 6 of the
   Action calls `mark_prefix_stale(scope)`. The Prefix Tab
   (`17-webui-prefix-tab.md`) shows the base prompt as stale; the operator
   (or an automation) regenerates it, which re-runs `do_reassemble` —
   now reading the freshly-converted Docu rows (prerequisite §2.2) — and
   re-prewarms the LLM KV cache.
5. **No per-turn cost.** The conversion is an operator/idle-time action,
   not a per-turn action. Per-turn prompts only carry the `base-prompt`
   placeholder + a <4k patch (§0.13); the converted docs are already in
   the cached prefix.

---

## 6. Injection paths (how converted docs reach a prompt)

The converted docs reach LLM prompts through **three** paths, all
already in the architecture:

1. **Base prompt (bulk) — the prefix.** Once `reborn_docus` is in
   `COMPONENT_TABLES` (§2.2), `do_reassemble` reads every validated Docu
   row (`WHERE validation_status='validated' AND NOT '05:validator'`)
   into the base-prompt bundle. The Prefix Tab compiles it
   (`17-webui-prefix-tab.md`). This satisfies item 4.5 ("the prefix prompts
   will contain these converted and optimized Documentations").
2. **Per-turn retrieval (selected).** `PostgresSource::fetch_for_turn`
   (`11-retrieval-system.md`, `15-component-catalog.md`) returns
   intent-relevant components — including Docu rows — for a turn's
   prior-knowledge assembly. So a question about "the intent system"
   retrieves the `02-intent-system` Docu row's `prior_knowledge_content`
   even if the whole base prompt is not loaded. This satisfies item 4.4
   ("can be added to an llm-prompt if needed or selected").
3. **Section pointers (navigational).** For docs **not** in the base
   prompt (budget-capped), `basic_prompt_section_refs` (§0.13) carries
   pointers like `→ see §intent-system in base-prompt` — the LLM already
   has the body from the KV cache, so the per-turn patch only references
   it.

---

## 7. Validation (the regenerated docs go through the queue)

Per `14-validation-queue.md` and the user's "validation queue for
regenerated docs" requirement:

- **System-authored docs bypass Q2** (the §0.16 / `17-webui-prefix-tab.md`
  bootstrap pattern): `source='system'`, `validation_status='validated'`,
  Q1 runs **internally inside the seeder/converter** (a Q1 error in
  converted content is a bug in the conversion Recipe/Skill, not a runtime
  failure). This keeps the agent's own docs from blocking on a human
  reviewer before they reach the base prompt.
- **Q1 still runs** (`component_validator.rs`, `14-validation-queue.md`
  Gate 1): structural check (name/description/non-empty content + token
  budget) + injection scan on the converted text. A converted doc that
  accidentally contains an injection pattern (e.g. the source doc
  documents prompt-injection and the §7 summary quotes a payload) fails
  Q1 — the converter must sanitize (the `doc-convert` Skill's rubric
  includes "quote injection payloads only as fenced, escaped code").
- **Operator-authored or Sempai-proposed conversions** (a human edits a
  doc in the WebUI, or the Sempai proposes a re-compression) go through
  the **full Q1 + Q2 queue** (`validation_status='pending'`, enqueued to
  `reborn_validation_queue`). Q2 approval graduates the doc →
  `mark_stale` the base prompt → Prefix Tab regenerate.
- **Regenerated docs and lineage.** A re-conversion that changes content
  writes a new row version via the lineage columns (`replaces_id`,
  `parent_version`, `similarity_parent_id`) so the history is auditable;
  the active row is the one with `validation_status='validated'`.

---

## 8. Proposed implementation sequence (presented, not done)

This is the order I would implement **after approval**. Each step is
small and ships independently; the plan would be broken into these as
Zenflow plan steps (one at a time, commit + push after each, per the
task rules):

1. **Prerequisite (composition):** add `("reborn_docus", 17)` to
   `COMPONENT_TABLES` and `17 => "Docu"` to `class_label` in
   `interceptor_config_service.rs`. Add a unit test asserting
   `do_reassemble` includes a validated Docu row. (Small Rust edit; no
   migration.)
2. **PythonCode helpers (class 22):** author `doc_hash`, `doc_diff`,
   `doc_upsert` as PythonCode components (the snippet→component path of
   `07-pythoncode-system.md`). Q1-scan for shell-injection. Store as
   `source='system'`, `validated`.
3. **Skill (class 1-3):** author the `doc-convert` Classic Skill (the
   conversion rubric). Q1 + (system bypass Q2). Store `validated`.
4. **Recipe (class 21):** author the `doc-convert` Recipe (variants
   `by-extract` Tier 0, `by-llm-compress` Tier 1) with
   `step_descriptions` JSONB. Store `validated`.
5. **Action (class 16):** author the `doc-sync` Action
   (`execute_action_procedure`, no LLM) — the 7-step driver. Store
   `validated`.
6. **Tool (class 0):** author `mark_prefix_stale` (or confirm the
   existing interceptor path suffices) + register the `doc-sync`
   ExtensionCatalogue (class 23).
7. **Idle-time wiring:** register `doc-sync` as a Kohai idle-time work
   item (`09-sempai-kohai.md`) and/or a scheduled trigger. This is the
   "runs automatically" step.
8. **End-to-end test:** change a `docs/agents-v3/*.md`, run `doc-sync`,
   assert the `reborn_docus` row updated (new `content_hash`), the base
   prompt is `is_stale`, and a Prefix Tab regenerate pulls the new
   converted doc into the assembled bundle.

Steps 1 is the only "host" Rust change; steps 2-8 are pure v3 artifacts
(DB rows + the agent operating on itself). No new migrations are
required (V040 already has every column; `reborn_docus.source` has no
CHECK to relax; `reborn_basic_prompt_store` V056 is the Phase K.1
prerequisite the design depends on).

---

## 9. Open decisions for the user

Before I implement, I need the user to confirm:

1. **Storage class.** I propose `reborn_docus` (class 17, Docu). An
   alternative is a brand-new class (e.g. "agent-doc") — but that needs
   a new migration + `class_label` + `COMPONENT_TABLES` + retrieval arms,
   versus reusing the existing Docu table which already has
   `content_hash` + lineage + SCH-02. **Recommend: reuse class 17.**
2. **System-authored vs full queue.** I propose the agent's own
   `docs/agents-v3/*.md` conversions are `source='system'` (Q2 bypassed,
   Q1 internal), while operator/Sempai-proposed conversions go through
   full Q1+Q2. Confirm this matches intent (else every regeneration
   blocks on a human reviewer).
3. **The one Rust prerequisite** (§2.2 / step 1): adding `reborn_docus`
   to `COMPONENT_TABLES` is a host-code edit, not a v3 artifact. The
   user said "not code per se" — but this single additive const edit is
   unavoidable for the converted docs to reach the base prompt. Confirm
   this exception is acceptable (the alternative is a new class + its
   own migration, which is more code, not less).
4. **Idle-time vs trigger.** Should the auto-refresh be Kohai idle-time
   (needs a Sempai for the compress variant; deterministic extract runs
   without), a scheduled trigger, or both? **Recommend: both** — trigger
   on boot + cadence for the deterministic path; Kohai idle-time for the
   Sempai-assisted compression.
5. **Token budget.** Per-doc ~600 tokens, total = sum of §7 summaries
   (currently ~17 docs × ~300-400 tokens ≈ 5-7k tokens of prefix). Is
   that within the intended base-prompt budget, or should only a subset
   of docs go into the prefix (with the rest available via per-turn
   retrieval + section pointers)?

---

## 10. STOP — approval required

**This is a design document only. No code or v3 artifacts have been
created for the mechanism.** Per the task instruction ("present how you
would do it before you actually start with it"), I am stopping here for
the user to review this approach and approve it (and answer the open
decisions in §9) before implementation begins.

If approved, the work proceeds as the 8 steps in §8, one at a time,
commit + push to `origin/main` after each — exactly as the prior
documentation steps were done.
