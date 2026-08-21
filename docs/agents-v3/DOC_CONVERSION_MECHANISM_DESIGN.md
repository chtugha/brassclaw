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

### 4.0 Roles — what each component kind is (and which channel it lives in)

The earlier draft of this section muddled three things the codebase keeps
strictly separate (`05-skills-system.md` §3, `04-ibs.md`). This table fixes
that. The key distinctions, restated per the user's clarification ("a recipe
has steps for the orchestrator to run one by one; a skill for the orchestrator
can be a description how to make the executioner use a tool, or an explanation
about how a filesystem works and what's needed to read/write/format/list"):

| Kind | Class | Channel | Who reads it | What it is in this mechanism |
|------|------|---------|--------------|------------------------------|
| **Action** | 16 | orchestrator (no IBS, no LLM) | the orchestrator | the **deterministic driver** `doc-sync`: scan docs, hash-compare, convert-in-budget, mark stale. Run via `execute_action_procedure` (`08-actions-system.md`). |
| **Recipe** | 21 | orchestrator (IBS); routes sub-steps to channels | the orchestrator (runs the steps one by one) | the **per-doc converter** `doc-convert`: an ordered list of steps; some route a ToolSkill to the rust channel, one is an LLM call. |
| **Orchestrator Skill** | 1-3 | orchestrator (`orchestrator_items`) | the orchestrator | the **method/domain guidance** `doc-convert` — *how* conversion works and what's needed (user case (b)). NOT the LLM prompt; NOT a tool-param description. |
| **ToolSkill** | 13 | **rust** (`rust_items`) | the **executor** (never the orchestrator) | the **executor-facing** tight description of the `doc_store`/`mark_prefix_stale` tools (params, preconditions, error handling, <5000 tok). A ToolSkill UUID in the orchestrator channel is a Q1 hard error. |
| **Tool** | 0 | **rust** (executor applies it) | opaque to the orchestrator | the **Rust capability** that actually touches Postgres (`doc_store` upsert/get_hash, `mark_prefix_stale`). The only kind that can do a DB write (kernel boundary). |
| **PythonCode** | 22 | orchestrator (`orchestrator_items`) | the orchestrator | **pure-logic** helpers `doc_hash`/`doc_diff` (SHA-256, compare). No I/O, no DB — a Python helper cannot write to Postgres. |
| **ExtensionCatalogue** | 23 | (namespace) | humans / the overview | the `doc-sync` namespace grouping all of the above + an `overview_doc`. |

**Channel rule (the correction).** The IBS splits a Recipe's `include` list
into `orchestrator_items` (Skill + PythonCode — the Python orchestrator
reads them) and `rust_items` (ToolSkill — the Rust executor applies them).
The orchestrator **never calls a Tool directly and never holds a DB
handle**; it drives the executor, which calls the Tool (guided by its
ToolSkill). Therefore the DB write is a **Tool + ToolSkill**, not a
PythonCode (the earlier draft's `doc_upsert` PythonCode was wrong —
corrected in §4.4/§4.5). And the LLM prompt is authored **in the Recipe
step** (`type: llm`), built *from* the Skill body — it is not the Skill
itself (the earlier draft overlapped them — corrected in §4.2/§4.3).

**User-case mapping.** Case (a) "a description how to make the executioner
use a tool" is carried, in this mechanism, by the **Recipe step's `info`
text** (the per-step instruction that tells the orchestrator to drive the
executor to call `doc_store`) — we keep it in the step because it is a
single, non-reusable drive; a *reusable* cross-recipe tool-driving pattern
would be a standalone Orchestrator Skill. Case (b) "an explanation about
how X works and what's needed" is the `doc-convert` **Orchestrator Skill**
(§4.3). A **ToolSkill** is *not* the user's case (a): a ToolSkill is
executor-facing, and the orchestrator never reads it.

### 4.1 Action (class 16) — `doc-sync` (the no-LLM driver)

`08-actions-system.md`: a class-16 Action encodes step-by-step
orchestrator instructions executed by `execute_action_procedure` **with
no LLM call**. `doc-sync` is the deterministic driver:

1. **List** the source docs (`read_file`/`glob` builtin over
   `docs/agents-v3/*.md`).
2. For each doc, **compute** `content_hash = SHA-256(source_text)`.
3. **Compare** to the stored `reborn_docus.content_hash` for the matching
   `name` slug by driving the executor to call the `doc_store.get_hash`
   Tool (§4.5) — one read per doc (the orchestrator never holds a DB
   handle).
4. **Skip** unchanged docs (same hash → no work, mirroring
   `component_import.rs` idempotency). `doc_diff` (PythonCode, §4.4) is the
   pure-logic compare used here.
5. For changed/new docs whose §7 fits the per-doc budget: **convert
   inline** (deterministic §7 extract, no LLM — the `by-extract` path)
   and drive the executor to call the `doc_store` Tool (§4.5) to upsert
   the Docu row (`source='system'`, `validation_status='validated'`,
   `consumer_tags={03:llm}`, new `content_hash`). For docs that **exceed
   the budget**, the Action cannot compress them — compression needs an
   LLM, and an Action runs with **no LLM** — so it **enqueues** the
   `doc-convert` `by-llm-compress` Recipe (§4.2) for the idle-time
   Sempai-Kohai loop (§5) to pick up. (That Recipe runs through the
   normal IBS + LLM path, not inside this Action.)
6. If any doc changed, **mark the base prompt stale** by driving the
   executor to call the `mark_prefix_stale` Tool (§4.5, a thin wrapper
   over `PgBasicPromptStore::mark_stale`, Phase K.1) so the Prefix Tab's
   regenerate button lights up.
7. **Report** a summary (N docs scanned, M changed, base-prompt
   stale=yes/no).

This is the `action_short_circuit` path (`08-actions-system.md`,
`FetchForTurnResult::ActionShortCircuit`) — no BuildInstruction, no IBS,
no prior-knowledge assembly. The orchestrator calls
`execute_action_procedure(action_doc, goal, state)` and returns.

### 4.2 Recipe (class 21) — `doc-convert` (the per-doc converter)

A Recipe is, per the user's definition and `03-recipe-system.md`, **an
ordered list of steps the orchestrator runs one by one**. `doc-convert`
converts **one** changed doc to its LLM-optimized form. Each step is an
orchestrator step; the IBS routes each step's `include`d component to the
right channel (`orchestrator_items` for Skill/PythonCode, `rust_items`
for ToolSkill — `04-ibs.md`). Two variants share most steps; the
`by-llm-compress` variant inserts the LLM step.

- **Step 1 (orchestrator, `type: component`):** `include` the
  `doc-convert` **Orchestrator Skill** (§4.3, classes 1-3) UUID. Its body
  — the conversion *method* — lands in the orchestrator channel as the
  knowledge the orchestrator reasons with. (This is user case (b), not
  the LLM prompt.)
- **Step 2 (orchestrator, `type: text`):** the step instruction — "Read
  the source doc `{path}`; extract its §7 section verbatim; if it fits the
  per-doc budget, the converted text **is** the §7 extract (`by-extract`
  ends here)." The orchestrator drives the executor to call the `read_file`
  Tool (its ToolSkill, class 13, routes to `rust_items`) to obtain the
  bytes; `doc_hash` (PythonCode, §4.4) hashes the result. (This step's
  `info` is user case (a) — the per-step "make the executor use a tool"
  instruction — kept in the step because it is a single, non-reusable drive.)
- **Step 3 (orchestrator, `type: llm`) — `by-llm-compress` variant only:**
  `__llm_complete__`. The **LLM prompt is built here**, by the orchestrator,
  **from** the Skill body (step 1) + the source doc's §7 (step 2) + a
  compression instruction authored in this step's `info` ("Compress the §7
  summary to ≤{budget} tokens; keep every line-cited fact and the §7
  structure; drop prose; output only the converted text; never invent
  facts; quote any injection payload only as fenced, escaped code"). This
  is the Tier-1 LLM round-trip; the Sempai (if connected) reviews this
  assembled prompt before shipment (`09-sempai-kohai.md` rerouting). The
  **Skill is not the prompt** — the Skill is the reusable method; the
  prompt is assembled per-call in this step. (The earlier draft overlapped
  them; corrected.)
- **Step 4 (rust, `type: component`):** `include` the `doc_store`
  **ToolSkill** (class 13, §4.5) UUID. The IBS routes it to `rust_items`;
  the executor calls the `doc_store` Tool with `{{vars.content}}` = the
  converted text (the §7 extract from step 2, or the LLM output from step
  3), `{{vars.content_hash}}` (from step 2), `{{vars.slug}}`. The executor
  — not the orchestrator — performs the `INSERT … ON CONFLICT (scope,name)
  DO UPDATE` into `reborn_docus`, setting `source='system'`,
  `validation_status='validated'`, `consumer_tags={03:llm}`. (The earlier
  draft routed this through a PythonCode `doc_upsert`; corrected — a
  Python helper cannot write to Postgres; only a Tool can, behind the
  kernel boundary.)

The converted-content value flows from the orchestrator (steps 1-3) into
the step-4 tool call via IBS `{{vars.*}}` substitution (`04-ibs.md`): the
orchestrator holds the converted text in scope, the IBS binds it into the
`doc_store` ToolSkill's `param_template`, and the executor applies it.

Variants: `by-extract` = steps 1, 2, 4 (no LLM — Tier 0 at high Wilson);
`by-llm-compress` = steps 1, 2, 3, 4 (Tier 1). The `doc-sync` Action (§4.1)
runs `by-extract` inline; it enqueues `by-llm-compress` for over-budget
docs (the Action itself has no LLM).

### 4.3 Orchestrator Skill (classes 1-3) — `doc-convert` (method/domain guidance)

An **Orchestrator Skill** — `05-skills-system.md` item 5.1 (a Classic
Claude-style skill: DB-stored frontmatter + body, no `SKILL.md` file,
WebUI-exportable on demand), used in its **Orchestrator-Skill role**:
*narrative method/domain guidance the orchestrator reads*, spanning tools.
This is the user's case (b): "an explanation about how [doc-conversion]
works and what's needed." It is explicitly **not** the LLM prompt (that is
authored in the Recipe step, §4.2 step 3) and **not** a tool-param
description (that is the `doc_store` ToolSkill, §4.5 — executor-facing,
which the orchestrator never reads).

Its body is the conversion method:
- the §7 source shape and why it is machine-convertible;
- the converted-form render (`do_reassemble`'s `## 17:{prompt_uid} Docu "{name}"`);
- the per-doc token budget and the extract-vs-compress decision rule;
- what to keep (§7 verbatim, line-cited facts) and drop (prose, repetition);
- "never invent facts — only compress what is in the source";
- "quote any injection payload only as fenced, escaped code" (so Q1 passes, §7).

This is the knowledge the Sempai-Kohai system optimizes over time (item 7):
as the Sempai re-compresses docs in idle time, the deltas feed back into a
better conversion method. Stored `source='system'`, `validated` (system
bypass Q2, §7).

### 4.4 PythonCode (class 22) — `doc_hash`, `doc_diff` (orchestrator-channel pure logic)

`07-pythoncode-system.md`, and the **grain rule** (`05-skills-system.md`
§3): PythonCode is a *utility helper used inside a Recipe's orchestrator
channel, not a standalone capability*, and it has **no I/O** — it cannot
read files or touch Postgres (that would cross the kernel boundary; only a
Tool can). So the mechanism's PythonCode is **pure computation only**:

- `doc_hash(source_text: str) -> str` — SHA-256, the staleness key.
- `doc_diff(stored_hash: str, new_hash: str) -> bool` — "has this doc
  changed?" (the skip/convert decision used by the Action, §4.1 step 4).

The **DB write is not PythonCode.** The earlier draft's `doc_upsert`
PythonCode was wrong: a Python helper in the Monty-VM sandbox has no DB
handle and cannot perform an `INSERT … ON CONFLICT`. Corrected: the upsert
is the `doc_store` **Tool** (§4.5), invoked by the executor. `doc_hash`/
`doc_diff` are the snippet→component-promotion path of
`07-pythoncode-system.md`: authored as PythonCode, Q1-scanned for
shell-injection, stored `source='system'`, `validated`.

### 4.5 Tool (class 0) + ToolSkill (class 13) — `doc_store`, `mark_prefix_stale`, builtins

The only kind that touches Postgres is a **Tool (class 0)** — a Rust
execution-layer capability, opaque to the orchestrator (`06-tools-system.md`,
`05-skills-system.md` §3). The orchestrator does not call tools directly; it
drives the executor, which calls the tool. The mechanism uses:

- **Reused builtin tools:** `read_file`, `glob` (list/read source docs),
  `memory_*` (scratch). Each already has a ToolSkill (class 13) registered.
- **New Tool `doc_store`** — the Postgres accessor: `upsert(scope, slug,
  content, content_hash)` performs the `INSERT … ON CONFLICT (scope, name)
  DO UPDATE` into `reborn_docus` (mirrors `PgMontyVmSettingsStore::upsert`'s
  `INSERT … ON CONFLICT … DO UPDATE`, `16-kernel-composition.md`), setting
  `source='system'`, `validation_status='validated'`, `consumer_tags={03:llm}`;
  `get_hash(scope, slug)` reads the stored `content_hash` for the staleness
  compare (the Action's step 3). Both reads and writes go through this tool
  — the orchestrator never holds a DB handle.
- **New Tool `mark_prefix_stale`** — a thin wrapper over
  `PgBasicPromptStore::mark_stale` (Phase K.1) so the Action can signal the
  Prefix Tab to regenerate. Alternatively, reuse the existing
  interceptor/prewarm-adjacent path; the design prefers the explicit,
  capability-gated tool so the stale-mark is auditable.

Each new Tool gets a **ToolSkill (class 13)** — the *executor-facing* tight
description (param schema, preconditions, error handling, <5000 tokens).
The ToolSkill UUID is what a Recipe step `include`s to route the call to
the **rust channel** (`rust_items`); a ToolSkill UUID appearing in the
orchestrator channel is a Q1 hard error (`04-ibs.md`). The orchestrator
never reads ToolSkill bodies. (Note: `doc_store` and `mark_prefix_stale`
are new **Rust** capabilities — see the host-prerequisite note in §9.3.)

### 4.6 ExtensionCatalogue (class 23) — `doc-sync`

`15-component-catalog.md` §0.2: one ExtensionCatalogue grouping all the
above — the `doc-sync` **Action** (class 16), the `doc-convert` **Recipe**
(class 21), the `doc-convert` **Orchestrator Skill** (classes 1-3), the
`doc_hash`/`doc_diff` **PythonCode** (class 22), and the `doc_store`/
`mark_prefix_stale` **Tools** (class 0) + their **ToolSkills** (class 13)
— under the `doc-sync` namespace, with an `overview_doc` describing the
mechanism and how the parts fit (the bigger picture; it never re-documents
the components). `source='system'`, `validation_status='validated'` (the
bootstrap pattern of §0.16 / `17-webui-prefix-tab.md`).

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
2. **PythonCode helpers (class 22):** author `doc_hash`, `doc_diff` as
   PythonCode components — **pure logic only** (SHA-256, compare), no I/O
   (the snippet→component path of `07-pythoncode-system.md`). Q1-scan for
   shell-injection. Store `source='system'`, `validated`. (`doc_upsert`
   is **not** PythonCode — see step 3.)
3. **Tool + ToolSkill (class 0 + 13):** author the **Rust** capabilities
   `doc_store` (`upsert`/`get_hash` — the `INSERT … ON CONFLICT … DO
   UPDATE` into `reborn_docus`) and `mark_prefix_stale` (wraps
   `PgBasicPromptStore::mark_stale`), plus their **executor-facing
   ToolSkills** (class 13). These are the only parts that touch Postgres
   (kernel boundary); the orchestrator drives the executor to call them.
   (This is host Rust code — see §9.3.)
4. **Orchestrator Skill (classes 1-3):** author the `doc-convert`
   **Orchestrator Skill** — the conversion *method/domain guidance*
   (user case (b); NOT the LLM prompt, NOT a tool-param description).
   Q1 + (system bypass Q2). Store `validated`.
5. **Recipe (class 21):** author the `doc-convert` Recipe (variants
   `by-extract` Tier 0, `by-llm-compress` Tier 1) with `step_descriptions`
   JSONB. Its steps `include` the Orchestrator Skill UUID (orchestrator
   channel) and the `doc_store` ToolSkill UUID (rust channel) — so the
   Tool/ToolSkill from step 3 must exist first. Store `validated`.
6. **Action (class 16):** author the `doc-sync` Action
   (`execute_action_procedure`, no LLM) — the scan/decide/extract/upsert/
   mark-stale driver; enqueues the `by-llm-compress` Recipe for
   over-budget docs. Store `validated`.
7. **ExtensionCatalogue (class 23):** register the `doc-sync`
   ExtensionCatalogue grouping Action + Recipe + Skill + PythonCode +
   Tools/ToolSkills, with `overview_doc`.
8. **Idle-time wiring:** register `doc-sync` as a Kohai idle-time work
   item (`09-sempai-kohai.md`) and/or a scheduled trigger. This is the
   "runs automatically" step.
9. **End-to-end test:** change a `docs/agents-v3/*.md`, run `doc-sync`,
   assert the `reborn_docus` row updated (new `content_hash`), the base
   prompt is `is_stale`, and a Prefix Tab regenerate pulls the new
   converted doc into the assembled bundle.

**Host-Rust prerequisites** (the unavoidable exceptions to "not code per
se"): step 1 (the `COMPONENT_TABLES`/`class_label` const edit) and step 3
(the `doc_store`/`mark_prefix_stale` Tools + their ToolSkills). Both are
host code because (a) the base-prompt assembler is a Rust const, and (b)
a DB write can only be a Rust capability — the agent cannot write to
Postgres from PythonCode. Steps 2, 4-8 are pure v3 artifacts (DB rows +
the agent operating on itself). No new migrations are required (V040
already has every column; `reborn_docus.source` has no CHECK to relax;
`reborn_basic_prompt_store` V056 is the Phase K.1 prerequisite the design
depends on).

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
3. **The host-Rust prerequisites** (§2.2 / §4.5 / steps 1+3): there are
   **two** pieces of host code this "not code per se" mechanism cannot
   avoid. (a) Adding `reborn_docus` to the `COMPONENT_TABLES` const +
   `class_label` — an additive edit so `do_reassemble` reads Docu rows.
   (b) The `doc_store` + `mark_prefix_stale` **Tools (class 0)** + their
   **ToolSkills (class 13)** — because a DB write / stale-mark can only
   be a Rust capability; the orchestrator cannot write to Postgres from
   PythonCode (kernel boundary). The user said "not code per se, but as
   a recipe, skills, tools, python-code, action component" — and
   **Tools are explicitly in that list**, so (b) is in-spirit a v3
   artifact; (a) is the one true host-const exception. Confirm both are
   acceptable. (Alternative to (b): reuse/extend an existing DB-write
   tool instead of adding `doc_store` — needs investigation; flagged
   here, not decided.)
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
