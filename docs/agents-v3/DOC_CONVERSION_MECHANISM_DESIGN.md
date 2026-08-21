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

The mechanism is **not a monolith** — it is a composition of mostly
**reusable library parts**, plus exactly **one** doc-specific skill. This
follows v3's core architecture (§4.0): a growing library of small,
one-purpose Skills + Tools that many Recipes compose. Only the
`doc-convert-method` domain skill and the two doc-specific Tools
(`docu_upsert`, `mark_prefix_stale`) are mechanism-specific; every other
part is a reusable leaf that future recipes can — and should — reuse. One
ExtensionCatalogue (class 23) groups the doc-specific parts.

### 4.0 Recycling — the v3 composition principle (read this first)

**This is the foundational v3 design rule, and it applies to every recipe
any future agent authors — not just this mechanism.** State it plainly:

> The library is the asset. **Skills should be as small as practical — at
> best, the description of ONE tool usage — so they can be reused in many
> recipes. Tools too: one concern each.** A Recipe is a *composition* of
> already-existing Skills + Tools; prefer reusing a library part over
> authoring a new one. When a genuinely new capability is needed, add it as
> a small leaf so the next recipe can reuse it too. **Never bake a whole
> procedure into one fat skill** — split it into leaves the library can
> recycle.

Two skill grains coexist (`05-skills-system.md` §3, and the user's two
cases):

- **Leaf Orchestrator Skill (one tool / one pythoncode) — the reusable
  building block.** Describes how to drive the executor to use ONE tool
  (user case (a)): e.g. `file-read`, `hash-compute`, `markdown-section`,
  `prompt-compress`. This is the unit of reuse. **Author these.**
- **Domain Orchestrator Skill (spans tools — user case (b)) — the bigger
  picture.** An explanation of how a task area works and *which leaf skills
  it needs* (e.g. "how doc-conversion works: read → extract §7 →
  budget-check → [compress] → render → write, using the `file-read`,
  `markdown-section`, `token-estimate`, `prompt-compress`, `docu-write`
  leaves"). A domain skill **references** leaves by name; it does **not**
  duplicate their tool instructions. One domain skill per mechanism; do not
  proliferate.

The ExtensionCatalogue (class 23) is the level above the domain skill — the
namespace + `overview_doc` + `task_groups` — and likewise never re-documents
its children (`05-skills-system.md` §3 ExtensionCatalogue).

**Why this matters here — and the correction.** The previous draft built ONE
monolithic `doc-convert` skill bundling the whole conversion method + ONE
monolithic `doc_store` tool bundling get_hash + upsert. That is the opposite
of reusable: no other recipe could reuse any part. **Corrected:** the method
is split into ~11 leaf skills + ~5 pythoncode + ~4 atomic tools (§4.1), only
the `doc-convert-method` domain skill (§4.2) is doc-specific, and the
`doc-convert` Recipe (§4.3) + `doc-sync` Action (§4.4) merely **compose**
them. Most leaves are candidates for the Phase L builtin bootstrap
(`05-skills-system.md` §4.7: the bootstrap seeds ~23 Tools + 23 ToolSkills +
12–15 Skills + 4–5 PythonCode into 5 catalogues) — i.e. they are
general-purpose, not doc-specific.

**Roles table (the kinds; the specific reusable parts are in §4.1):**

| Kind | Class | Channel | Who reads it | What it is in this mechanism |
|------|------|---------|--------------|------------------------------|
| **Action** | 16 | orchestrator (no IBS, no LLM) | the orchestrator | the deterministic driver `doc-sync` — **composes** leaves (§4.4). |
| **Recipe** | 21 | orchestrator (IBS); routes sub-steps to channels | the orchestrator (runs steps one by one) | the per-doc converter `doc-convert` — **composes** leaves (§4.3). |
| **Orchestrator Skill (leaf)** | 1-3 | orchestrator (`orchestrator_items`) | the orchestrator | ONE tool-usage description (case a) — the reusable unit. |
| **Orchestrator Skill (domain)** | 1-3 | orchestrator (`orchestrator_items`) | the orchestrator | the one doc-specific overview `doc-convert-method` (case b); references leaves. |
| **ToolSkill** | 13 | **rust** (`rust_items`) | the **executor** (never the orchestrator) | executor-facing param/precondition description, one per Tool, <5000 tok. |
| **Tool** | 0 | **rust** (executor applies it) | opaque to the orchestrator | the Rust capability; one concern each; the only kind that touches Postgres. |
| **PythonCode** | 22 | orchestrator (`orchestrator_items`) | the orchestrator | pure-logic helper, one concern each; no I/O, no DB. |
| **ExtensionCatalogue** | 23 | (namespace) | humans / overview | the `doc-sync` namespace + `overview_doc`. |

**Channel rule.** The IBS splits a Recipe's `include` list into
`orchestrator_items` (Skill + PythonCode) and `rust_items` (ToolSkill). The
orchestrator never calls a Tool directly and never holds a DB handle; it
drives the executor, which calls the Tool (guided by its ToolSkill). So a DB
write is always a **Tool + ToolSkill**, never a PythonCode (the earlier
draft's `doc_upsert` PythonCode was wrong). And the LLM prompt is authored
**in the Recipe's `type: llm` step**, built *from* the Skill body — the
Skill is the reusable method, not the prompt (the earlier draft overlapped
them).

### 4.1 The reusable library — leaf skills, pythoncode, atomic tools

Each entry is **one-purpose and reusable beyond `doc-convert`**. Reuse
status: **builtin** (exists / Phase L bootstrap), **new-leaf** (new but
general-purpose → bootstrap candidate), **mechanism-specific** (only this
mechanism needs it).

**Leaf Orchestrator Skills (classes 1-3), one tool each — user case (a):**

| Skill | Binds to | Reuse | What it teaches |
|-------|----------|-------|-----------------|
| `file-list` | `glob` Tool | builtin | list files matching a glob |
| `file-read` | `read_file` Tool | builtin | read one file; handle not-found |
| `hash-compute` | `sha256` PythonCode | new-leaf | hash text for staleness/integrity |
| `hash-compare` | `hash_changed` PythonCode | new-leaf | decide "changed?" from two hashes |
| `component-hash-read` | `component_get_content_hash` Tool | new-leaf | read a stored `content_hash` for any component row |
| `markdown-section` | `markdown_section` PythonCode | new-leaf | extract a `## N. title` section from any markdown |
| `token-estimate` | `token_estimate` PythonCode | new-leaf | ≈4 chars/token estimate |
| `component-header-render` | `format_component_header` PythonCode | new-leaf | render the `## CC:UID  LABEL  "name"` base-prompt line |
| `prompt-compress` | `__llm_complete__` (LLM step) | new-leaf | compress a text block to a token budget for a base prompt (keep cited facts, drop prose, never invent, escape injection) — reusable for ANY compression, not just docs |
| `docu-write` | `docu_upsert` Tool | mechanism-specific | upsert a `reborn_docus` row |
| `prefix-stale-mark` | `mark_prefix_stale` Tool | mechanism-specific | mark the base-prompt prefix stale |

**PythonCode (class 22) — pure logic, one concern each, no I/O:**

| Helper | Signature | Reuse |
|--------|-----------|-------|
| `sha256` | `(text: str) -> str` | new-leaf (general) |
| `hash_changed` | `(stored: str, new: str) -> bool` | new-leaf (general) |
| `markdown_section` | `(md: str, level: int, title: str) -> str` | new-leaf (general) |
| `token_estimate` | `(text: str) -> int` | new-leaf (general) |
| `format_component_header` | `(class_code: int, prompt_uid: int, label: str, name: str) -> str` | new-leaf (general) |

(`doc_upsert` is **not** here — a Python helper cannot write to Postgres;
only a Tool can, behind the kernel boundary. The earlier draft's
`doc_upsert` PythonCode was wrong; corrected.)

**Tools (class 0) — one concern each, Rust, capability-gated:**

| Tool | Signature | Reuse | Notes |
|------|-----------|-------|-------|
| `read_file`, `glob`, `memory_*` | — | builtin | reused as-is |
| `component_get_content_hash` | `(table, scope, name) -> str?` | new-leaf | read-only staleness probe for ANY component table |
| `docu_upsert` | `(scope, name, content, content_hash, …) -> ()` | mechanism-specific | `INSERT … ON CONFLICT (scope,name) DO UPDATE` into `reborn_docus`; sets `source='system'`, `validated`, `consumer_tags={03:llm}` |
| `mark_prefix_stale` | `(scope) -> ()` | mechanism-specific | wraps `PgBasicPromptStore::mark_stale` (Phase K.1) |

Each new Tool gets a **ToolSkill (class 13)** — executor-facing param
schema / preconditions / error handling (<5000 tok). The ToolSkill UUID is
what a Recipe step `include`s to route the call to `rust_items`; a ToolSkill
UUID in the orchestrator channel is a Q1 hard error (`04-ibs.md`).

**The split is the point.** A future "config-sync" or "skill-sync" recipe
reuses `file-list`/`file-read`/`hash-compute`/`hash-compare`/
`component-hash-read` unchanged; only its domain skill + write tool differ.
That is the v3 library paying off.

### 4.2 The one doc-specific Orchestrator Skill — `doc-convert-method` (case b)

A Classic Claude-style skill (`05-skills-system.md` item 5.1: DB-stored
frontmatter + body, no `SKILL.md` file, WebUI-exportable), used in its
**domain-Skill role** (user case (b)): an explanation of *how doc-conversion
works and which leaf skills it needs* — it **references** the §4.1 leaves by
name, it does **not** re-describe their tool usage. Its body:

- the §7 source shape and why it is machine-convertible;
- the pipeline: `file-read` → `markdown-section` → `token-estimate` →
  (over budget? `prompt-compress` LLM step) → `component-header-render` →
  `docu-write`;
- the converted-form render (`do_reassemble`'s `## 17:{prompt_uid} Docu "{name}"`);
- the per-doc token budget and the extract-vs-compress decision rule;
- "never invent facts — only compress what is in the source";
- "quote any injection payload only as fenced, escaped code" (so Q1 passes, §7).

This is the knowledge the Sempai-Kohai system optimizes over time (item 7):
as the Sempai re-compresses docs in idle time, the deltas feed back into a
better conversion method. Stored `source='system'`, `validated` (system
bypass Q2, §7). **This is the only doc-specific skill** — everything else in
§4.1 is a reusable leaf.

### 4.3 Recipe (class 21) — `doc-convert` (composes the leaves)

A Recipe is **an ordered list of steps the orchestrator runs one by one**
(`03-recipe-system.md`). `doc-convert` converts **one** doc; its steps
`include` the leaf UUIDs from §4.1 + the domain skill from §4.2. Two
variants share most steps; `by-llm-compress` inserts the LLM step.

- **Step 1 (orchestrator, `type: component`):** `include` the
  `doc-convert-method` domain Skill (§4.2) — the overview that names the
  leaves and the order.
- **Step 2 (orchestrator, `type: component`):** `include` the `file-read`
  leaf Skill + its `read_file` ToolSkill (rust) → read `{path}`.
- **Step 3 (orchestrator, `type: component`):** `include` `markdown-section`
  (PythonCode) → extract §7; `include` `hash-compute` + `token-estimate`
  leaves → `content_hash` + token count.
- **Step 4 (orchestrator, `type: llm`) — `by-llm-compress` variant only:**
  `__llm_complete__` using the `prompt-compress` leaf Skill's rubric; the
  prompt is assembled here from the domain skill (step 1) + the §7 text
  (step 3) + the budget. Sempai reviews before shipment
  (`09-sempai-kohai.md`). (The Skill is the reusable rubric; the prompt is
  assembled per-call — they are not the same thing.)
- **Step 5 (orchestrator, `type: component`):** `include` `component-header-render`
  (PythonCode) → render the `## 17:…` header.
- **Step 6 (rust, `type: component`):** `include` the `docu-write` leaf
  Skill + the `docu_upsert` ToolSkill (rust) → the executor writes the
  `reborn_docus` row (`source='system'`, `validated`, `consumer_tags={03:llm}`,
  new `content_hash`). Values flow in via IBS `{{vars.*}}` substitution
  (`04-ibs.md`): the orchestrator holds the converted text, the IBS binds it
  into the ToolSkill's `param_template`, the executor applies it.

Variants: `by-extract` = steps 1,2,3,5,6 (no LLM — Tier 0); `by-llm-compress`
= steps 1,2,3,4,5,6 (Tier 1). The `doc-sync` Action (§4.4) runs `by-extract`
inline; it enqueues `by-llm-compress` for over-budget docs (an Action has no
LLM).

### 4.4 Action (class 16) — `doc-sync` (composes the leaves)

`08-actions-system.md`: a class-16 Action is step-by-step orchestrator
instructions run by `execute_action_procedure` **with no LLM call** — the
`action_short_circuit` path (no BuildInstruction, no IBS, no prior-knowledge
assembly). `doc-sync` composes the §4.1 leaves:

1. `file-list` leaf + `glob` Tool → list `docs/agents-v3/*.md`.
2. Per doc: `file-read` leaf + `read_file` Tool → source text.
3. `hash-compute` leaf → `content_hash = SHA-256(source_text)`.
4. `component-hash-read` leaf + `component_get_content_hash` Tool → stored
   hash; `hash-compare` leaf → changed? Skip unchanged (mirrors
   `component_import.rs` idempotency).
5. Changed & in-budget: run `doc-convert` `by-extract` (§4.3) inline (no
   LLM) → `docu-write` writes the row. Over-budget: **enqueue**
   `doc-convert` `by-llm-compress` for the idle-time Sempai-Kohai loop (§5)
   — an Action cannot run the LLM step itself.
6. If any changed: `prefix-stale-mark` leaf + `mark_prefix_stale` Tool →
   light up the Prefix Tab regenerate button (Phase K.1).
7. Report (N scanned, M changed, stale=yes/no).

### 4.5 ExtensionCatalogue (class 23) — `doc-sync`

`15-component-catalog.md` §0.2: one ExtensionCatalogue grouping the
`doc-sync` Action (16), `doc-convert` Recipe (21), the `doc-convert-method`
domain Skill (1-3), the §4.1 leaf Skills + PythonCode + Tools/ToolSkills —
under the `doc-sync` namespace, with an `overview_doc` describing the
mechanism and how the parts fit (the bigger picture; it never re-documents
the components). `source='system'`, `validation_status='validated'` (the
bootstrap pattern of §0.16 / `17-webui-prefix-tab.md`). **Note:** the
general-purpose leaves (`file-read`, `hash-compute`, `markdown-section`,
`prompt-compress`, …) belong in the matching *builtin* catalogue
(`builtin-filesystem` / `builtin-memory` / `builtin-management`,
`05-skills-system.md` §4.7) and are only *referenced* by `doc-sync`; the
`doc-sync` catalogue owns only the doc-specific parts (the domain skill,
`docu_upsert`/`mark_prefix_stale`, the Recipe, the Action).

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
  Q1 — the converter must sanitize (the `doc-convert-method` domain skill
  and the `prompt-compress` leaf both carry "quote injection payloads only
  as fenced, escaped code").
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
2. **Reusable PythonCode leaves (class 22):** author `sha256`,
   `hash_changed`, `markdown_section`, `token_estimate`,
   `format_component_header` — **pure logic, one concern each, no I/O**
   (the snippet→component path of `07-pythoncode-system.md`). Q1-scan for
   shell-injection. Store `source='system'`, `validated`. These are
   general-purpose → bootstrap candidates.
3. **Reusable + mechanism-specific Tools + ToolSkills (class 0 + 13):**
   author the **Rust** capabilities `component_get_content_hash`
   (read-only staleness probe for any component table — reusable),
   `docu_upsert` (the `INSERT … ON CONFLICT … DO UPDATE` into
   `reborn_docus` — mechanism-specific), and `mark_prefix_stale` (wraps
   `PgBasicPromptStore::mark_stale` — mechanism-specific), plus their
   **executor-facing ToolSkills** (class 13). `read_file`/`glob`/`memory_*`
   are reused as-is. These are the only parts that touch Postgres (kernel
   boundary); the orchestrator drives the executor to call them. (Host
   Rust code — see §9.3.) The split (read probe vs write vs stale-mark) is
   deliberate so other "sync" recipes can reuse `component_get_content_hash`.
4. **Reusable leaf Orchestrator Skills (classes 1-3):** author the
   one-tool-each leaves — `file-list`, `file-read`, `hash-compute`,
   `hash-compare`, `component-hash-read`, `markdown-section`,
   `token-estimate`, `component-header-render`, `prompt-compress`,
   `docu-write`, `prefix-stale-mark` (§4.1). Each binds to ONE tool /
   pythoncode (user case (a)). Q1 + (system bypass Q2). Store `validated`.
   Most are general-purpose → bootstrap candidates.
5. **The one domain Orchestrator Skill (classes 1-3):** author
   `doc-convert-method` (§4.2) — the doc-specific overview that *references*
   the §4.1 leaves by name (user case (b); NOT the LLM prompt, NOT a
   tool-param description). Q1 + (system bypass Q2). Store `validated`.
6. **Recipe (class 21):** author `doc-convert` (variants `by-extract`
   Tier 0, `by-llm-compress` Tier 1) with `step_descriptions` JSONB. Its
   steps `include` the leaf UUIDs from step 4 + the domain skill from
   step 5 + the ToolSkill UUIDs from step 3 — so steps 3-5 must exist
   first. Store `validated`.
7. **Action (class 16):** author `doc-sync` (`execute_action_procedure`,
   no LLM) — the scan/decide/extract/upsert/mark-stale driver that
   *composes* the §4.1 leaves; enqueues `by-llm-compress` for over-budget
   docs. Store `validated`.
8. **ExtensionCatalogue (class 23):** register `doc-sync` owning only the
   doc-specific parts (domain skill, `docu_upsert`/`mark_prefix_stale`,
   Recipe, Action); the general-purpose leaves live in the matching
   builtin catalogue and are referenced. With `overview_doc`.
9. **Idle-time wiring:** register `doc-sync` as a Kohai idle-time work
   item (`09-sempai-kohai.md`) and/or a scheduled trigger. This is the
   "runs automatically" step.
10. **End-to-end test:** change a `docs/agents-v3/*.md`, run `doc-sync`,
    assert the `reborn_docus` row updated (new `content_hash`), the base
    prompt is `is_stale`, and a Prefix Tab regenerate pulls the new
    converted doc into the assembled bundle.

**Host-Rust prerequisites** (the unavoidable exceptions to "not code per
se"): step 1 (the `COMPONENT_TABLES`/`class_label` const edit) and step 3
(the `component_get_content_hash`/`docu_upsert`/`mark_prefix_stale` Tools
+ their ToolSkills). Both are host code because (a) the base-prompt
assembler is a Rust const, and (b) a DB read/write/stale-mark can only be
a Rust capability — the agent cannot touch Postgres from PythonCode
(kernel boundary). Steps 2, 4-9 are pure v3 artifacts (DB rows + the agent
operating on itself); many leaves are Phase-L bootstrap candidates. No new
migrations are required (V040 already has every column; `reborn_docus.source`
has no CHECK to relax; `reborn_basic_prompt_store` V056 is the Phase K.1
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
3. **The host-Rust prerequisites** (§2.2 / §4.1 / §4.5 / steps 1+3):
   there are **two** pieces of host code this "not code per se" mechanism
   cannot avoid. (a) Adding `reborn_docus` to the `COMPONENT_TABLES` const
   + `class_label` — an additive edit so `do_reassemble` reads Docu rows.
   (b) The `component_get_content_hash` + `docu_upsert` + `mark_prefix_stale`
   **Tools (class 0)** + their **ToolSkills (class 13)** — because a DB
   read/write/stale-mark can only be a Rust capability; the orchestrator
   cannot touch Postgres from PythonCode (kernel boundary). The user said
   "not code per se, but as a recipe, skills, tools, python-code, action
   component" — and **Tools are explicitly in that list**, so (b) is
   in-spirit a v3 artifact; (a) is the one true host-const exception.
   Confirm both are acceptable. (Alternative to (b): reuse/extend an
   existing DB-write tool instead of adding `docu_upsert` — needs
   investigation; flagged here, not decided.)
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

If approved, the work proceeds as the 10 steps in §8, one at a time,
commit + push to `origin/main` after each — exactly as the prior
documentation steps were done.
