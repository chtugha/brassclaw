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
pointers), §0.16 (builtin bootstrap — revised per Answer 2: builtins go through Q1+Q2, no bypass), §0.18
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
6. **Runs on change events** — a source `docs/agents-v3/*.md` file
   change on disk, or a `reborn_docus` row change in the DB (§5; Answer 4).
   No idle-time loop, no scheduled cadence, no boot trigger.
7. Is built **as v3 agent artifacts** (Recipe / Skills / Tools /
   PythonCode / Action), **not as Rust code** — the agent operates on
   itself through the same component catalog + execution paths it uses
   for every other task.

The companion requirement (item 7, `09-sempai-kohai.md`): the Sempai-Kohai
interceptor runs **idle-time self-optimization** — sending everything that
belongs to a chat/prompt to the Sempai to receive new skills/tools/recipes/
python-code, queued for validation. **The doc-conversion mechanism is NOT
part of that idle-time loop** (Answer 4): it is **event-driven** — it runs
only when a source `docs/agents-v3/*.md` file changes on disk or a
`reborn_docus` row changes in the DB (§5). The Sempai-Kohai system's only
role here is that, when the `by-llm-compress` variant runs and a Sempai is
connected, the Sempai reviews the compression prompt before shipment
(`09-sempai-kohai.md`); without a Sempai, only the deterministic
`by-extract` variant runs.

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
| `content` | per-row text. For the **source row** = the unconverted source markdown; for the **converted row** = the LLM-optimized text (`do_reassemble` reads the converted row into the base prompt). Both versions are stored — see §2.1.1. |
| `prior_knowledge_content` (SCH-02) | optional richer form used by the per-turn retrieval path (`PostgresSource`) instead of `content` |
| `override_prompt_creation` | `false` for normal docs (assembled normally); could be `true` for a doc that must replace standard assembly (Solution Override) |
| `class_code` | `17` (CHECK `= 17`) |
| `prompt_uid` | sequence — the stable base-prompt ordering key |
| `consumer_tags` | `{03:llm}` (and `{02:orchestrator}` where relevant) — **never `05:validator`** on a graduated row, or `do_reassemble` excludes the row |
| `validation_status` | `'validated'` **only after full Q1+Q2 graduation** (Answer 2). Upsert always sets `'pending'`; there is no bypass path. This is the retrieval/base-prompt gate. |
| `source` | provenance label only — e.g. `'system'` for the agent's own docs, `'authored'`/`'migrated'` for others. `reborn_docus` has **no `source` CHECK constraint** (unlike tools/tool_skills/skills), so `'system'` is allowed with no migration. **`source` never gates validation** (Answer 2). |
| `content_hash` | **the staleness key** — SHA-256 of the source `.md` (mirrors `component_import.rs`'s idempotency) |
| lineage (`similarity_parent_id`, `replaces_id`, `parent_version`, `last_audit_at`, `audit_failure_count`) | links the source row to its converted row, and versions the converted doc across regenerations (§2.1.1) |

### 2.1.1 Storing both versions (Answer 1)

The user's Answer 1: **both the unconverted source and the converted form
are stored.** This is done with **two `reborn_docus` rows per doc**, linked
by the existing lineage columns — no schema change needed:

| Row | `name` convention | `content` | `source` | `validation_status` | role |
|-----|-------------------|-----------|----------|---------------------|------|
| **source** | `agents-v3::02-intent-system` (the slug) | unconverted source markdown | `'system'` (agent's own) or `'authored'` | goes through Q1+Q2 → `'validated'` | the auditable original; the `content_hash` staleness key is computed from this |
| **converted** | `agents-v3::02-intent-system::llm` | LLM-optimized text | `'system'` | goes through Q1+Q2 → `'validated'` | what `do_reassemble` reads into the base prompt |

The converted row points at the source row via `similarity_parent_id`
(or `replaces_id` on a re-conversion), carrying `parent_version`. A
re-conversion writes a new converted row version (lineage), keeping
history auditable; the active converted row is the one with
`validation_status='validated'`. Both rows pass Q1+Q2 independently
(§7) — the source row's validation is a structural/injection check on
the original text; the converted row's validation is the same check on
the optimized text.

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
<4k tokens. The converted docs live in the **prefix**. **There is no
per-doc token budget (Answer 5)** — docs are not budget-limited. The
deterministic §7 extract is used verbatim; the LLM-assisted compression
pass (Sempai, when connected) runs for clarity/injection-safety, **not**
to hit a token ceiling. Whether any token budget applies anywhere in the
codebase at all is governed by the **global token-budget kill switch**
added to `saved_plan_to_v3.md` (Answer 5): a WebUI settings toggle that,
when disabled, makes token budgets play no role in any decision or
function.

---

## 4. The v3 artifacts that implement the mechanism

The mechanism is **not a monolith** — it is a composition of mostly
**reusable library parts**, plus exactly **one** doc-specific skill. This
follows v3's core architecture (§4.0): a growing library of small,
one-purpose Skills + Tools that many Recipes compose. Only the
`doc-convert-method` domain skill and the two doc-specific leaf skills
(`db-upsert-docus`, `db-mark-prefix-stale`, both over the one generic
`component_db` Tool — §4.0.1) are mechanism-specific; every other
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
  [compress if noisy] → render → write, using the `file-read`,
  `markdown-section`, `prompt-compress`, `db-upsert-docus` leaves"). A
  domain skill **references** leaves by name; it does **not** duplicate
  their tool instructions. One domain skill per mechanism; do not
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

### 4.0.1 Tool vs Skill — the generic-DB-tool rule (Answer 3)

The user's refinement sharpens the recycling principle for **DB access**:

> **One generic DB-reading (and DB-writing) Tool suffices. The
> per-purpose specificity lives in Skills (one per reading/writing
> approach) and in sub-recipes that tell the orchestrator how to use each
> skill to call Rust to read from / write to the DB in a certain way.**

Rationale and definitions:

- **Tool (class 0)** = the opaque Rust capability that touches Postgres
  (the kernel boundary). Keep it **maximally generic**: one Tool exposes
  a small, uniform surface (e.g. `component_db { op: read_hash | read_row
  | upsert | mark_stale, table, scope, name, … }`) and is recycled by
  every future "sync" recipe. Do **not** mint a new Tool per read/write/
  stale-mark shape — that was the earlier draft's mistake
  (`component_get_content_hash` + `docu_upsert` + `mark_prefix_stale` as
  three separate Tools).
- **Skill (classes 1-3, leaf)** = the description of **one** way to use
  the generic DB Tool for a specific purpose: `db-read-hash` ("read a
  stored `content_hash` for a component row"), `db-upsert-docus`
  ("upsert a `reborn_docus` row"), `db-mark-prefix-stale` ("mark the
  base-prompt prefix stale"). Each leaf binds to the **one** DB Tool and
  carries the purpose-specific param shaping / preconditions / error
  handling. This is the unit of reuse across recipes.
- **Sub-recipe** = a small Recipe (class 21) that composes one or more
  leaf skills and tells the orchestrator **the exact sequence** to call
  Rust to read/write the DB in a certain way (e.g. a `db-read-hash`
  sub-recipe = "load the stored hash for slug X via `db-read-hash`, return
  it for comparison"). The `doc-convert` Recipe (§4.3) composes these
  sub-recipes; it does not call the DB Tool directly.

So the earlier three mechanism-specific Tools collapse to **one** generic
DB Tool + three leaf skills (+ their sub-recipes). The Tool is the
recyclable substrate; the skills/sub-recipes are the recyclable method —
exactly the v3 library paying off.

### 4.1 The reusable library — leaf skills, pythoncode, atomic tools

Each entry is **one-purpose and reusable beyond `doc-convert`**. Reuse
status: **builtin** (exists / Phase L bootstrap), **new-leaf** (new but
general-purpose → bootstrap candidate), **mechanism-specific** (only this
mechanism needs it). Per §4.0.1, the three DB-access shapes are **one
generic Tool + three leaf skills** (not three Tools).

**Leaf Orchestrator Skills (classes 1-3), one tool / pythoncode each — user case (a):**

| Skill | Binds to | Reuse | What it teaches |
|-------|----------|-------|-----------------|
| `file-list` | `glob` Tool | builtin | list files matching a glob |
| `file-read` | `read_file` Tool | builtin | read one file; handle not-found |
| `hash-compute` | `sha256` PythonCode | new-leaf | hash text for staleness/integrity |
| `hash-compare` | `hash_changed` PythonCode | new-leaf | decide "changed?" from two hashes |
| `db-read-hash` | `component_db` Tool (`op=read_hash`) | new-leaf | read a stored `content_hash` for any component row (the staleness probe) |
| `db-upsert-docus` | `component_db` Tool (`op=upsert`, table=`reborn_docus`) | mechanism-specific | upsert a `reborn_docus` row (source + converted; `validation_status='pending'`, never `validated` — §7) |
| `db-mark-prefix-stale` | `component_db` Tool (`op=mark_stale`) | mechanism-specific | mark the base-prompt prefix stale |
| `markdown-section` | `markdown_section` PythonCode | new-leaf | extract a `## N. title` section from any markdown |
| `component-header-render` | `format_component_header` PythonCode | new-leaf | render the `## CC:UID  LABEL  "name"` base-prompt line |
| `prompt-compress` | `__llm_complete__` (LLM step) | new-leaf | compress a text block for a base prompt (keep cited facts, drop prose, never invent, escape injection) — reusable for ANY compression, not just docs. **No token budget** (Answer 5); runs for clarity/injection-safety, not to hit a ceiling. |

(`token-estimate` is **removed** — there is no per-doc token budget,
Answer 5.)

**PythonCode (class 22) — pure logic, one concern each, no I/O:**

| Helper | Signature | Reuse |
|--------|-----------|-------|
| `sha256` | `(text: str) -> str` | new-leaf (general) |
| `hash_changed` | `(stored: str, new: str) -> bool` | new-leaf (general) |
| `markdown_section` | `(md: str, level: int, title: str) -> str` | new-leaf (general) |
| `format_component_header` | `(class_code: int, prompt_uid: int, label: str, name: str) -> str` | new-leaf (general) |

(`token_estimate` is **removed** — no token budget, Answer 5. A Python
helper cannot write to Postgres; only a Tool can, behind the kernel
boundary. The earlier draft's `doc_upsert` PythonCode was wrong;
corrected.)

**Tools (class 0) — one concern each, Rust, capability-gated:**

| Tool | Signature | Reuse | Notes |
|------|-----------|-------|-------|
| `read_file`, `glob`, `memory_*` | — | builtin | reused as-is |
| `component_db` | `(op, table, scope, name, fields?) -> result` | new-leaf | **the one generic DB Tool** (§4.0.1). `op ∈ {read_hash, read_row, upsert, mark_stale}`. `upsert` does `INSERT … ON CONFLICT (scope,name) DO UPDATE` and **always sets `validation_status='pending'`** (never `validated` — the row goes to the Q1+Q2 queue, §7). `mark_stale` wraps `PgBasicPromptStore::mark_stale` (Phase K.1). Recycled by every future "sync" recipe. |

The single `component_db` Tool gets **one ToolSkill (class 13)** —
executor-facing param schema / preconditions / error handling for the
uniform `op` surface. The ToolSkill UUID is what a Recipe step `include`s
to route the call to `rust_items`; a ToolSkill UUID in the orchestrator
channel is a Q1 hard error (`04-ibs.md`). The per-`op` nuances
(`read_hash` vs `upsert` vs `mark_stale`) are carried by the **leaf
skills** (`db-read-hash`, `db-upsert-docus`, `db-mark-prefix-stale`) and
their sub-recipes — not by splitting the Tool.

**The split is the point.** A future "config-sync" or "skill-sync" recipe
reuses `file-list`/`file-read`/`hash-compute`/`hash-compare`/`db-read-hash`
unchanged, plus the **same** `component_db` Tool (just a different `op`/
`table`); only its domain skill differs. That is the v3 library paying
off.

### 4.2 The one doc-specific Orchestrator Skill — `doc-convert-method` (case b)

A Classic Claude-style skill (`05-skills-system.md` item 5.1: DB-stored
frontmatter + body, no `SKILL.md` file, WebUI-exportable), used in its
**domain-Skill role** (user case (b)): an explanation of *how doc-conversion
works and which leaf skills it needs* — it **references** the §4.1 leaves by
name, it does **not** re-describe their tool usage. Its body:

- the §7 source shape and why it is machine-convertible;
- the pipeline: `file-read` → `markdown-section` → (needs compression?
  `prompt-compress` LLM step — for clarity/injection-safety, **not** a
  token budget, Answer 5) → `component-header-render` → `db-upsert-docus`;
- the converted-form render (`do_reassemble`'s `## 17:{prompt_uid} Docu "{name}"`);
- the extract-vs-compress decision rule (compress when the §7 extract is
  noisy or quotes injection payloads; **no token budget** — Answer 5);
- "never invent facts — only compress what is in the source";
- "quote any injection payload only as fenced, escaped code" (so Q1 passes, §7).

This is the knowledge the Sempai-Kohai system may optimize over time (item
7): when a Sempai is connected and the `by-llm-compress` variant runs, the
deltas can feed back into a better conversion method. Stored `source='system'`
(provenance only); like every component it **goes through Q1+Q2** — no bypass
(§7, Answer 2). **This is the only doc-specific skill** — everything else in
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
  (PythonCode) → extract §7; `include` `hash-compute` leaf → `content_hash`.
  (No `token-estimate` — there is no token budget, Answer 5.)
- **Step 4 (orchestrator, `type: llm`) — `by-llm-compress` variant only:**
  `__llm_complete__` using the `prompt-compress` leaf Skill's rubric; the
  prompt is assembled here from the domain skill (step 1) + the §7 text
  (step 3). Sempai reviews before shipment (`09-sempai-kohai.md`) when a
  Sempai is connected. (The Skill is the reusable rubric; the prompt is
  assembled per-call — they are not the same thing.)
- **Step 5 (orchestrator, `type: component`):** `include` `component-header-render`
  (PythonCode) → render the `## 17:…` header.
- **Step 6 (rust, `type: component`):** `include` the `db-upsert-docus` leaf
  Skill + the `component_db` ToolSkill (`op=upsert`, rust) → the executor
  writes **both** `reborn_docus` rows (§2.1.1): the source row and the
  converted row, each with `validation_status='pending'` (they go to the
  Q1+Q2 queue — never `'validated'` on write, §7), `consumer_tags={03:llm}`,
  new `content_hash`, and the converted row linked to the source row via
  lineage. Values flow in via IBS `{{vars.*}}` substitution (`04-ibs.md`).

Variants: `by-extract` = steps 1,2,3,5,6 (no LLM — Tier 0); `by-llm-compress`
= steps 1,2,3,4,5,6 (Tier 1). The `doc-sync` Action (§4.4) runs `by-extract`
inline; it enqueues `by-llm-compress` for docs whose §7 extract needs
compression (no budget gate — Answer 5; an Action has no LLM).

### 4.4 Action (class 16) — `doc-sync` (composes the leaves)

`08-actions-system.md`: a class-16 Action is step-by-step orchestrator
instructions run by `execute_action_procedure` **with no LLM call** — the
`action_short_circuit` path (no BuildInstruction, no IBS, no prior-knowledge
assembly). `doc-sync` composes the §4.1 leaves:

1. `file-list` leaf + `glob` Tool → list `docs/agents-v3/*.md`.
2. Per doc: `file-read` leaf + `read_file` Tool → source text.
3. `hash-compute` leaf → `content_hash = SHA-256(source_text)`.
4. `db-read-hash` leaf + `component_db` Tool (`op=read_hash`) → stored
   hash; `hash-compare` leaf → changed? Skip unchanged (mirrors
   `component_import.rs` idempotency).
5. Changed: run `doc-convert` `by-extract` (§4.3) inline (no LLM) →
   `db-upsert-docus` writes both rows (`pending`). If the §7 extract needs
   compression: **enqueue** `doc-convert` `by-llm-compress` (run when a
   Sempai is connected; not idle-time — §5 is event-driven) — an Action
   cannot run the LLM step itself. (No budget gate — Answer 5.)
6. If any changed: `db-mark-prefix-stale` leaf + `component_db` Tool
   (`op=mark_stale`) → light up the Prefix Tab regenerate button (Phase K.1).
7. Report (N scanned, M changed, stale=yes/no).

### 4.5 ExtensionCatalogue (class 23) — `doc-sync`

`15-component-catalog.md` §0.2: one ExtensionCatalogue grouping the
`doc-sync` Action (16), `doc-convert` Recipe (21), the `doc-convert-method`
domain Skill (1-3), the §4.1 leaf Skills + PythonCode + Tools/ToolSkills —
under the `doc-sync` namespace, with an `overview_doc` describing the
mechanism and how the parts fit (the bigger picture; it never re-documents
the components). `source='system'` (provenance); **goes through Q1+Q2 like
every component — no bypass** (Answer 2; revises the §0.16 /
`17-webui-prefix-tab.md` bootstrap-bypass pattern). **Note:** the
general-purpose leaves (`file-read`, `hash-compute`, `markdown-section`,
`prompt-compress`, …) belong in the matching *builtin* catalogue
(`builtin-filesystem` / `builtin-memory` / `builtin-management`,
`05-skills-system.md` §4.7) and are only *referenced* by `doc-sync`; the
`doc-sync` catalogue owns only the doc-specific parts (the domain skill,
the `db-upsert-docus` / `db-mark-prefix-stale` leaf skills over the one
`component_db` Tool, the Recipe, the Action).

---

## 5. The refresh loop — event-driven (file / DB change only)

The mechanism runs **only on change events** — there is **no auto-refresh,
no idle-time loop, no scheduled cadence, no boot trigger** (Answer 4). The
Kohai/Sempai system is **not** in the refresh loop.

1. **File-change trigger.** A watcher on the `docs/agents-v3/*.md` source
   tree fires `doc-sync` when a source doc changes on disk (the agent's
   own docs are edited). This is the primary ingress for the agent's own
   documentation.
2. **DB-change trigger.** A `reborn_docus` row change inside the DB (a
   doc edited via the WebUI Docs section, §7, or a Sempai-proposed
   re-compression graduating) fires `doc-sync` for the affected slug — so
   a doc edited through the WebUI re-runs the deterministic §7 extract and
   (if needed) the LLM compression pass against the new source text.
3. **Staleness-driven, not cadence-driven.** `doc-sync` does O(N) hash
   compares (one read per doc in the changed set) and only converts the
   changed subset — so a no-op run is cheap. `content_hash` is the
   staleness key (same as `component_import.rs`). There is no periodic
   full sweep.
4. **Base-prompt invalidation.** When any doc changes, the Action calls
   the `db-mark-prefix-stale` leaf (the one `component_db` Tool,
   `op=mark_stale` — §4.0.1/§4.1) on the scope. The Prefix Tab
   (`17-webui-prefix-tab.md`) shows the base prompt as stale; the operator
   (or an automation) regenerates it, which re-runs `do_reassemble` — now
   reading the
   freshly-converted Docu rows (prerequisite §2.2) — and re-prewarms the
   LLM KV cache.
5. **No per-turn cost.** The conversion is an event-time action, not a
   per-turn action. Per-turn prompts only carry the `base-prompt`
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

Per `14-validation-queue.md` and the user's Answer 2 — **nothing ever
bypasses Q1+Q2**:

- **Every converted doc goes through the full Q1 + Q2 queue.**
  `validation_status='pending'` on upsert → Q1 (Gate 1) → Q2 (review) →
  `'validated'`. This includes the agent's own `docs/agents-v3/*.md`
  conversions, system-authored docs, and any Sempai-proposed
  re-compression. There is **no `source='system'` Q2-bypass path**;
  `source` is a provenance label only and never gates validation. (The
  earlier draft's "system bypass Q2" pattern is removed — Answer 2.)
- **Q1 (Gate 1, automatic)** runs on every converted doc
  (`component_validator.rs`): structural check (name/description/non-empty
  content) + injection scan on the converted text. A converted doc that
  accidentally contains an injection pattern (e.g. the source doc
  documents prompt-injection and the §7 summary quotes a payload) fails
  Q1 — the converter must sanitize (the `doc-convert-method` domain skill
  and the `prompt-compress` leaf both carry "quote injection payloads only
  as fenced, escaped code").
- **Q2 (review)** graduates the doc. For system-authored/builtin docs
  this is an **automated-but-auditable** Q2 graduation recorded in the
  queue (never a silent skip) — this requires the validation-system
  extension flagged in §10; until that extension exists, system-authored
  conversions cannot be marked `validated` and will not reach the base
  prompt. For operator-authored or Sempai-proposed conversions, Q2 is the
  human reviewer. Q2 approval → the `db-mark-prefix-stale` leaf (the one
  `component_db` Tool, `op=mark_stale`) → Prefix Tab regenerate.
- **WebUI Docs section (Answer 2).** A new WebUI section lists the
  `reborn_docus` rows (source + converted, with validation status) and
  allows manual editing. **Saving an edited doc sends it to the
  validation queue again** (`validation_status='pending'`, enqueued to
  `reborn_validation_queue`) — it never writes `validated` directly. This
  mirrors the existing validation-queue tab pattern
  (`./crates/brassclaw_webui_v2_static/static/js/pages/settings/components/validation-queue-tab.js`).
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
   `hash_changed`, `markdown_section`, `format_component_header` —
   **pure logic, one concern each, no I/O** (the snippet→component path of
   `07-pythoncode-system.md`). Q1-scan for shell-injection. (`token_estimate`
   is dropped — no token budget, Answer 5.) Like every component these
   **go through Q1+Q2** (§7); `source='system'` is provenance only, never a
   bypass. General-purpose → bootstrap candidates.
3. **The one generic DB Tool + its ToolSkill (class 0 + 13):** author the
   **Rust** capability `component_db` (`op ∈ {read_hash, read_row, upsert,
   mark_stale}` — §4.0.1/§4.1) and its single **executor-facing ToolSkill**
   (class 13) for the uniform `op` surface. `upsert` does the
   `INSERT … ON CONFLICT … DO UPDATE` into `reborn_docus` and **always sets
   `validation_status='pending'`** (§7). `mark_stale` wraps
   `PgBasicPromptStore::mark_stale`. `read_file`/`glob`/`memory_*` are
   reused as-is. This is the only part that touches Postgres (kernel
   boundary); the orchestrator drives the executor to call it via the leaf
   skills + sub-recipes. (Host Rust code — see §9.3.) One generic Tool, not
   three — so every future "sync" recipe recycles it with a different `op`.
4. **Reusable + DB leaf Orchestrator Skills (classes 1-3):** author the
   one-tool-each leaves — `file-list`, `file-read`, `hash-compute`,
   `hash-compare`, `db-read-hash`, `markdown-section`,
   `component-header-render`, `prompt-compress`, plus the doc-specific
   `db-upsert-docus` and `db-mark-prefix-stale` (§4.1). The DB leaves bind
   to the one `component_db` Tool (different `op`); the rest bind to their
   own tool/pythoncode (user case (a)). All **go through Q1+Q2** (§7) — no
   bypass. Most are general-purpose → bootstrap candidates.
5. **The one domain Orchestrator Skill (classes 1-3):** author
   `doc-convert-method` (§4.2) — the doc-specific overview that *references*
   the §4.1 leaves by name (user case (b); NOT the LLM prompt, NOT a
   tool-param description). Goes through Q1+Q2 (§7) — no bypass.
6. **Recipe (class 21):** author `doc-convert` (variants `by-extract`
   Tier 0, `by-llm-compress` Tier 1) with `step_descriptions` JSONB. Its
   steps `include` the leaf UUIDs from step 4 + the domain skill from
   step 5 + the `component_db` ToolSkill UUID from step 3 — so steps 3-5
   must exist first. Goes through Q1+Q2 (§7) — no bypass.
7. **Action (class 16):** author `doc-sync` (`execute_action_procedure`,
   no LLM) — the scan/decide/extract/upsert/mark-stale driver that
   *composes* the §4.1 leaves; enqueues `by-llm-compress` for docs whose
   §7 extract needs compression (no budget gate — Answer 5). Goes through
   Q1+Q2 (§7) — no bypass.
8. **ExtensionCatalogue (class 23):** register `doc-sync` owning only the
   doc-specific parts (domain skill, the `db-upsert-docus`/
   `db-mark-prefix-stale` leaf skills over the one `component_db` Tool,
   Recipe, Action); the general-purpose leaves live in the matching
   builtin catalogue and are referenced. With `overview_doc`.
9. **Event wiring (Answer 4):** wire `doc-sync` to fire on (a) a
   file-watch on `docs/agents-v3/*.md` (source doc changed on disk), and
   (b) a `reborn_docus` row-change signal (doc edited via the WebUI Docs
   section, §7, or a re-compression graduating). No idle-time loop, no
   scheduled cadence, no boot trigger (§5).
10. **End-to-end test:** change a `docs/agents-v3/*.md`, run `doc-sync`,
    assert the `reborn_docus` row updated (new `content_hash`), the base
    prompt is `is_stale`, and a Prefix Tab regenerate pulls the new
    converted doc into the assembled bundle.

**Host-Rust prerequisites** (the unavoidable exceptions to "not code per
se"): step 1 (the `COMPONENT_TABLES`/`class_label` const edit) and step 3
(the one generic `component_db` Tool + its single ToolSkill). Both are
host code because (a) the base-prompt assembler is a Rust const, and (b) a
DB read/write/stale-mark can only be a Rust capability — the agent cannot
touch Postgres from PythonCode (kernel boundary). Steps 2, 4-9 are pure v3
artifacts (DB rows + the agent operating on itself); many leaves are
Phase-L bootstrap candidates. No new migrations are required (V040 already
has every column; `reborn_docus.source` has no CHECK to relax;
`reborn_basic_prompt_store` V056 is the Phase K.1 prerequisite the design
depends on).

---

## 9. Resolved decisions (user-approved with revisions)

The user answered all five open decisions. The design above is revised
to match; the decisions are recorded here as the authoritative spec for
implementation.

1. **Storage class — reuse class 17, store BOTH versions.** `reborn_docus`
   (class 17, Docu) is reused (no new migration / `class_label` /
   `COMPONENT_TABLES` / retrieval arms — the table already has
   `content_hash` + lineage + SCH-02). **Both the unconverted source doc
   AND the LLM-optimized converted form are stored** (see §2.1.1). The two
   are linked by the existing lineage columns; both pass Q1+Q2.
2. **Nothing ever bypasses Q1+Q2.** Every doc — including the agent's own
   `docs/agents-v3/*.md` conversions and any system-authored/builtin
   seed — goes through the **full Q1 + Q2 validation queue**
   (`validation_status='pending'` → Q1 → Q2 → `'validated'`). There is no
   `source='system'` Q2-bypass path. `source` is a provenance label only;
   it never gates validation. This requires extending the validation
   system so system-authored components still run Q1 and record a Q2
   graduation (automated-but-auditable, never silently skipped) — flagged
   as a required follow-up implementation (§10). A **WebUI Docs section**
   is added: docs are listed there and can be edited manually; **saving
   an edited doc sends it to the validation queue again**.
3. **Tool vs Skill rethink — ONE generic DB tool, MANY skills, sub-recipes
   to compose them.** A Tool is the opaque Rust capability that touches
   Postgres; it should be as generic as practical. **One generic DB-reading
   (and DB-writing) Tool suffices** for this mechanism — not a per-purpose
   Tool per read/write/stale-mark. The per-purpose specificity lives in
   **Skills** (one skill per reading/writing approach — "read a stored
   content_hash", "upsert a docus row", "mark the prefix stale") and in
   **sub-recipes** that tell the orchestrator how to use each skill to
   call Rust to read from / write to the DB in a certain way (see §4.0.1,
   §4.1). This maximizes reuse: the single DB Tool is recycled by every
   future "sync" recipe; only the skills/sub-recipes differ.
4. **No auto-refresh — event-only.** Conversion/update is triggered
   **only** by (a) a source `docs/agents-v3/*.md` file change on disk, or
   (b) a `reborn_docus` row change inside the DB. There is **no** Kohai
   idle-time refresh, **no** scheduled cadence, **no** boot trigger (see
   §5). The Kohai/Sempai system is not in the refresh loop.
5. **No token budget for docs.** Docs are not budget-limited; the
   deterministic §7 extract and the LLM compression pass run without a
   per-doc token ceiling (see §3.3). Separately, a **global token-budget
   kill switch** is added to `saved_plan_to_v3.md`: a WebUI settings
   button that, when disabled, makes token budgets play no role anywhere
   in the code — not used in any decision or function.

---

## 10. Status — approved with revisions; follow-up implementations

**This remains a design document.** The user approved the approach with
the five revisions in §9. No code or v3 artifacts have been created for
the mechanism yet; implementation proceeds as the steps in §8, one at a
time, commit + push to `origin/main` after each.

**Required follow-up implementations surfaced by the §9 decisions and
the validation-bypass audit** (each needs its own subplan, per the task's
"subplan complicated changes" rule; none are done here):

- **Validation-system extension (Answer 2):** add a path so
  system-authored/builtin components graduate through Q1+Q2 with an
  automated-but-auditable Q2 — no silent bypass. This revises
  `saved_plan_to_v3.md` §0.16 / Phase L (which currently lets builtins
  skip the queue).
- **On-disk system-skills bypass (audit finding 1):** migrate
  `SYSTEM_SKILLS_ROOT` `SKILL.md` skills (`brassclaw_skills::management`)
  into `reborn_skills` DB rows that pass Q1+Q2; remove the disk-loaded
  `SkillSource::System` bypass. Also satisfies v3 goal 5.1 (skills are
  DB-stored, no `SKILL.md`).
- **Global token-budget kill switch (Answer 5):** added to
  `saved_plan_to_v3.md` (separate commit) — a WebUI settings toggle that
  disables every token budget in the whole codebase.
