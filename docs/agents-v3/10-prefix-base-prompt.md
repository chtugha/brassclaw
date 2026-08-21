# 10 — Prefix / Base-Prompt (vLLM Prefix Caching)

> **Subsystem:** Prefix caching for the agent's LLMs. A *prefix* is a large,
> pre-assembled, pre-compiled prompt that is **pre-warmed into the serving
> engine's KV-cache** (vLLM/LMCache) so that, on every turn, only the small
> *delta* (chat message + history + selected memory + orchestrator patch) has
> to be computed as new tokens. The first and largest prefix is the
> **base prompt** — the complete, compiled documentation/system prompt of the
> agent. More prefixes will be added in the future. During prompt composition a
> **single line** containing the text `base-prompt` is inserted as a
> placeholder; the Sempai-Kohai interceptor replaces that line with the real
> base-prompt content **at the very end of prompt creation, just before
> tokenization**. If no base prompt has been pre-compiled yet, a short
> minimal-info prompt-part is synthesized instead (the LLM can only compute
> ~200 tokens/s, so the full base prompt cannot be recompiled inline).
> **Grounded in:** `crates/brassclaw_interceptor/` (`config_store.rs`,
> `packet.rs`, `pg_store.rs`), `crates/brassclaw_reborn_composition/src/
> interceptor_config_service.rs` (`do_reassemble`, `reassemble_base_prompt`),
> `crates/brassclaw_webui_v2_static/.../interceptor-tab.js`,
> `saved_plan_to_v3.md` §0.13 (lines 1487-1499) + Phase K / K.1 (lines
> 5856-5882) + §2 migration table (line 26).

## 1. Purpose

The user's task description (item 8) calls for **vLLM prefix caching with
multiple precompiled prompts**, the first being the "base prompt", with:

- a **single-line `base-prompt` placeholder** added during prompt
  composition, replaced by real content by the Sempai-Kohai system at the end
  of prompt creation;
- a **Prefix Tab** under Settings in the WebUI that lists the prefixes and
  has a button to **generate / regenerate** each one — on click the prefix is
  assembled and sent to the LLM for compilation (KV-cache pre-warm);
- the existing single-prefix "base prompt" implementation that was "already
  foreseen somewhere" **shifted into the Prefix Tab** section (rather than
  duplicated).

This subsystem exists to make the agent's large system prompt cheap to reuse
across turns. Without prefix caching, every turn would re-process the full
agent documentation (potentially many thousands of tokens) at ~200 tokens/s —
prohibitively slow for a 7B-14B model serving interactive chat. With a
pre-compiled prefix resident in the KV-cache, only the per-turn delta is
computed as new tokens.

> **Two different "base prompts" — read carefully.** The codebase has a
> **Sempai base prompt** (the reviewer's audit prompt, Part A) and a planned
> **Kohai base prompt** (the answer LLM's system prompt). They are distinct:
>
> | | Sempai base prompt (reviewer) | Kohai base prompt (answer LLM) |
> |---|---|---|
> | Whose KV-cache | the Sempai provider | the Kohai provider |
> | Stored today? | **yes** — `brassclaw_config` keys | **no** — Phase K.1 |
> | Storage in v3 | `brassclaw_config` (unchanged) | `reborn_basic_prompt_store` (V056) |
> | Assembled by | `InterceptorConfigService::do_reassemble` | (v3) `PgBasicPromptStore` + same assembly |
> | UI today | Interceptor tab "Reassemble" + "Pre-warm" | — (v3: Prefix Tab) |
> | Placeholder mechanism | none (no placeholder; reassembled string) | `base-prompt` line replaced by interceptor |
>
> The user's "base prompt" (item 8) and "the base prompt that should already
> have been precompiled and is sitting in the kv-cache" (Task 3 §2.2) is the
> **Kohai** base prompt. The "already foreseen such an implementation" that
> must be **shifted to the Prefix Tab** is the **Sempai** base prompt's
> Reassemble/Pre-warm UI in the Interceptor tab — its *assembly mechanism*
> (`do_reassemble`, walking the component tables) is exactly what the Kohai
> base prompt will reuse, and its *UI shape* (a card with last-assembled
> timestamp + size + a Reassemble button + a Pre-warm button) is the template
> for the Prefix Tab.

## 2. Location

### Today (Sempai base prompt only)

- **Crate:** `crates/brassclaw_interceptor/` — config keys + packet flag.
  - `src/config_store.rs` — `InterceptorConfig` struct
    (`sempai_base_prompt: Option<String>`,
    `sempai_base_prompt_assembled_at: Option<String>`,
    `sempai_persona`, `sempai_prewarm_last_at`) and the
    `InterceptorConfigStore` trait (`save_base_prompt`, `save_persona`,
    `save_prewarm_last_at`, `load`). Keys live in the `brassclaw_config`
    Postgres table — **no new migration**.
  - `src/packet.rs:96-97` — `CapturedPrompt` carries a
    `kv_cache_optimised: bool` flag (whether KV-cache-optimised prompt
    ordering was applied for the captured turn).
- **Composition:** `crates/brassclaw_reborn_composition/src/
  interceptor_config_service.rs` — `InterceptorConfigService`.
  - `KEY_BASE_PROMPT = "interceptor.sempai_base_prompt"`,
    `KEY_BASE_PROMPT_ASSEMBLED_AT`,
    `KEY_PREWARM_LAST_AT` (lines 34-37).
  - `COMPONENT_TABLES` — the list of `(table_name, class_code)` pairs walked
    during assembly.
  - `do_reassemble()` (line 204) — the **assembly mechanism**: discovers
    which component tables exist via `information_schema.tables`, then for
    each table selects `prompt_uid, name, content` where
    `validation_status = 'validated'` and `'05:validator'` is **not** in
    `consumer_tags`, ordered by `prompt_uid ASC` (LIMIT 1000 per table),
    sorts all parts by `(class_code, prompt_uid)`, and concatenates
    `## {class_code}:{prompt_uid}  …  "{name}"\n\n{content}` into one string.
    Per-table/per-row errors are logged at `debug!` and skipped (graceful
    degradation against not-yet-deployed tables).
  - `reassemble_base_prompt()` (line 356) — the public entry point, with a
    per-caller rate limit (`reassemble_rate_limit`) and a `prewarm` path
    (`prewarm_rate_limit`).
- **WebUI (Interceptor tab):**
  `crates/brassclaw_webui_v2_static/static/js/pages/settings/components/
  interceptor-tab.js/interceptor-tab.js`.
  - `StatusCard` (line 69) — shows `base_prompt_assembled_at` +
    `base_prompt_size_chars`, and `prewarm_last_at`.
  - `ControlCard` (line 179) — **Reassemble** button (`onReassemble`,
    disabled while mutating) and **Pre-warm** button (`onPrewarm`,
    disabled until a base prompt exists), with i18n strings
    `interceptor.prewarm{,Desc,Ok,Error,ing}`.
- **Plan:** `saved_plan_to_v3.md` §0.13 (lines 1487-1499) and Phase K.1
  (lines 5860-5882); §2 migration table line 26
  (`V052 | V055 | V056 | Phase K.1`).

### v3 (Kohai base prompt + Prefix Tab)

- **Migration:** `V056__reborn_basic_prompt_store.sql` — **does not exist
  today** (no `V054`-`V059` migration files are present; the highest deployed
  migration is below Phase J/K). V056 is **was V055 before Decision 2** (the
  V-number shift: Phase B/C migrations moved to V052/V053, pushing the
  basic-prompt store from V055 to V056).
- **Facade:** `PgBasicPromptStore` (new) — `get_for_scope`, `store`,
  `mark_stale`, `delete`.
- **Interceptor wiring:** prepend the stored bundle before LLM shipment;
  on any component `validated` transition call `mark_stale(scope)`.
- **WebUI:** a dedicated **Prefix Tab** (see `17-webui-prefix-tab.md`).

> **Known stale inline reference:** §0.13 line 1490 still reads
> "`reborn_basic_prompt_store` (V055)". After Decision 2 this table is **V056**
> (confirmed by §2 table line 26 and Phase K.1 line 5862). This is a residual
> stale V-number inside §0.13's prose; the authoritative number is V056. (The
> Task-2 plan audit corrected V055→V056 everywhere else; this one inline
> instance in §0.13 was not swept.)

## 3. Data model

### `reborn_basic_prompt_store` (v3, V056)

```sql
CREATE TABLE reborn_basic_prompt_store (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id     TEXT NOT NULL,
    user_id       TEXT,
    agent_id      TEXT,
    project_id    TEXT,
    fingerprint   TEXT NOT NULL,   -- SHA-256 of bundle content
    bundle_json   JSONB NOT NULL,
    is_stale      BOOLEAN NOT NULL DEFAULT false,
    assembled_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(tenant_id, user_id, agent_id, project_id)
);
```

- **Scope tuple** (`tenant_id`, `user_id`, `agent_id`, `project_id`) — one
  stored bundle per scope; `user_id`/`agent_id`/`project_id` are nullable to
  allow a tenant-global base prompt. The `UNIQUE` constraint enforces
  one-row-per-scope.
- **`fingerprint`** — SHA-256 of the bundle content. Detects whether
  re-assembly would produce an identical bundle (skip the pre-warm round-trip
  when the fingerprint is unchanged) and supports staleness reasoning.
- **`bundle_json`** — the pre-assembled `InstructionBundle` (the same bundle
  shape the IBS produces — see `04-ibs.md`), serialized to JSONB. This is the
  full base-prompt content the LLM pre-compiles into its KV-cache.
- **`is_stale`** — `false` immediately after `store`/regeneration; flipped to
  `true` by `mark_stale(scope)` whenever any component passes **Gate 2**
  (`validation_status` → `'validated'`). A stale bundle is still served
  (better a stale prefix than no prefix) until an operator regenerates it
  from the Prefix Tab.
- **`assembled_at` / `updated_at`** — provenance for the Prefix Tab's
  "last assembled" display.

### `PgBasicPromptStore` facade (v3)

| Method | Behavior |
|---|---|
| `get_for_scope(scope)` | Return the non-deleted bundle for the scope (whether or not stale); `None` if never assembled. |
| `store(scope, bundle)` | Upsert the bundle for the scope; recompute `fingerprint`; clear `is_stale`; stamp `assembled_at`/`updated_at`. |
| `mark_stale(scope)` | Set `is_stale = true` (idempotent). Called on every component `validated` transition. |
| `delete(scope)` | Remove the row for the scope. |

### Sempai base-prompt config keys (today, unchanged in v3)

Persisted in `brassclaw_config` (no dedicated table):

| Key | Meaning |
|---|---|
| `interceptor.sempai_base_prompt` | assembled Sempai base prompt (Part A); `None` if never assembled |
| `interceptor.sempai_base_prompt_assembled_at` | ISO-8601 last-assembly timestamp |
| `interceptor.sempai_persona` | Sempai persona text (Part B); falls back to compiled-in default |
| `interceptor.sempai_prewarm_last_at` | ISO-8601 last successful pre-warm |

### `CapturedPrompt.kv_cache_optimised` (today)

A boolean on the forensic packet (`packet.rs:96-97`) recording whether
KV-cache-optimised prompt ordering was applied for the captured turn. It is
set `true` in the test/default packet construction (`packet.rs:278`) but is
not yet driven by a real ordering decision in production — it is the hook by
which the interceptor will report, per turn, whether the base-prompt prefix
was reused from cache.

## 4. Behavior

### 4.1 Assembly (the mechanism "already foreseen")

`InterceptorConfigService::do_reassemble()` is the canonical assembly
routine. In v3 the **Kohai** base prompt reuses the same logic against the
same `COMPONENT_TABLES`, but writes the result into
`reborn_basic_prompt_store.bundle_json` (as an `InstructionBundle`) with a
fresh fingerprint, instead of into a `brassclaw_config` text key.

Assembly steps:

1. **Discover tables** — query `information_schema.tables` for the
   intersection of `COMPONENT_TABLES` and existing base tables, so tables
   from not-yet-deployed phases are skipped gracefully (no hard dependency on
   later migrations).
2. **Per-table select** — `SELECT prompt_uid, name, COALESCE(content,'') AS
   content FROM {table} WHERE validation_status = 'validated' AND NOT
   ('05:validator' = ANY(COALESCE(consumer_tags, ARRAY[]::text[]))) ORDER BY
   prompt_uid ASC LIMIT 1000`. Only **validated** components, and only those
   **not** still tagged for the validator (`05:validator`), are included.
   `LIMIT 1000` per table bounds the assembled size.
3. **Global sort** — all parts sorted by `(class_code ASC, prompt_uid ASC)`
   for deterministic assembly order (the same ordering the IBS uses — see
   `04-ibs.md`).
4. **Concatenate** — each part becomes
   `## {class_code}:{prompt_uid}  …  "{name}"\n\n{content}`; the whole is the
   assembled base-prompt body.

> **Why `05:validator` exclusion matters here:** the base prompt must not
> contain components still pending human validation. A component graduates
> out of `05:validator` only when it passes Gate 2 (the validation system —
> see `14-validation-queue.md`). So the assembly reading
> `validation_status = 'validated' AND NOT 05:validator` is the same gate
> that `mark_stale` reacts to: the moment a component joins the base-prompt
> set, the old bundle is flagged stale.

### 4.2 The `base-prompt` placeholder and its substitution (v3)

This is the user's core mechanism (item 8) and Task 3 §2.2. It is **not
implemented today** (no literal `base-prompt` string exists in any
composition/orchestrator code — confirmed by search; the placeholder appears
only in design docs).

v3 flow:

1. **Composition inserts a placeholder line.** While the prompt is being
   composed (the IBS `build_instruction`, see `04-ibs.md`, plus the Python
   orchestrator's `working_messages` assembly in `default.py`), a single line
   whose content is exactly `base-prompt` is added where the base prompt
   belongs. This is a **placeholder, not content** — it keeps the composition
   cheap and order-stable while deferring the large content to the end.
2. **The Sempai-Kohai interceptor resolves it at the end.** At the very end
   of prompt creation, just before tokenization, the interceptor (see
   `09-sempai-kohai.md`) replaces the `base-prompt` line with the real
   content:
   - if `PgBasicPromptStore::get_for_scope` returns a non-stale bundle →
     inline that bundle's content (the LLM already has it in KV-cache, so
     this is a cache hit — only the *delta* after it is computed as new
     tokens);
   - if the bundle is stale or absent → the interceptor synthesizes a
     **short minimal-info prompt-part** instead (the user's "around 200
     tokens/s" rationale: the full base prompt cannot be recomputed inline,
     so a compact stand-in carries only the most necessary information).
3. **Result shipped to the Kohai** — the final message list has the base
   prompt expanded in place; the KV-cache prefix is reused; only the delta
   is new tokens.

> **`basic_prompt_section_refs` (§0.13).** The BuildInstruction patch that
> accompanies the base prompt must **not repeat** content already in the
> stored base prompt. Instead it carries navigation hints — pointers, not
> content — e.g. `"→ see §ls-skill in basic-prompt"`, because the LLM already
> has the body from the KV-cache. Target patch size **< 4k tokens** (fast
> new-token computation). Within the `InstructionBundle` priority scheme
> (see `04-ibs.md`): orchestrator snippets are **PRIORITY 2**, memory
> snippets **PRIORITY 3**, and **Rust context is delivered directly by
> `RecipeStage` — not in the bundle at all**.

### 4.3 Staleness and regeneration

- **Stale trigger:** any component `validated` transition calls
  `mark_stale(scope)`. A stale bundle is still served (a stale prefix is
  cheaper than no prefix and usually still largely correct), but the Prefix
  Tab will show it as stale and offer Regenerate.
- **Regeneration (manual trigger only):** the base prompt is **never**
  auto-regenerated on the hot path. An operator clicks **Generate /
  Regenerate** on the Prefix Tab → `store(scope, bundle)` re-runs assembly,
  recomputes the fingerprint, clears `is_stale`, stamps timestamps → the
  Prefix Tab then triggers a **pre-warm** (send the assembled prefix to the
  LLM for compilation into the KV-cache).
- **Fingerprint short-circuit:** if regeneration produces a bundle whose
  SHA-256 matches the stored `fingerprint`, the pre-warm round-trip is
  skipped (nothing changed).

### 4.4 Pre-warm (the "compile into KV-cache" step)

Pre-warm sends the assembled prefix to the LLM provider with the
instruction to compile it into the KV-cache without generating a completion
(the same notion as today's Sempai `prewarm` path,
`KEY_PREWARM_LAST_AT`). After pre-warm succeeds, subsequent turns that begin
with the identical prefix get a cache hit. The Prefix Tab records
`prewarm_last_at` per prefix (mirroring today's Interceptor-tab field).

### 4.5 Multiple prefixes (future)

The base prompt is the **first** prefix. The design is explicitly
extensible: additional prefixes (e.g. a per-domain context prefix, a
tools-surface prefix) will be added later. The `reborn_basic_prompt_store`
table is named generically (`basic_prompt_store`) and keyed by scope, but
extending to *multiple named prefixes per scope* will require either a
`prefix_name TEXT` column added to the `UNIQUE` tuple or a separate
`reborn_prefix_store` table — **not decided in the current plan**; the
Prefix Tab UI is specified to "list the prefixes", anticipating this. (See
`17-webui-prefix-tab.md`.)

## 5. Relations

- **Sempai-Kohai interceptor** (`09-sempai-kohai.md`) — owns the placeholder
  substitution (§4.2) and is the chokepoint where the stored bundle is
  prepended before LLM shipment (Phase K.1 wiring).
- **IBS** (`04-ibs.md`) — produces the `InstructionBundle` shape that
  `bundle_json` stores, and the patch that obeys the §0.13
  `basic_prompt_section_refs` non-repetition rule.
- **Validation system** (`14-validation-queue.md`) — Gate 2 graduation is
  the `mark_stale` trigger and the `validated` + `NOT 05:validator` filter
  in assembly.
- **Component catalog** (`15-component-catalog.md`) — `COMPONENT_TABLES` and
  the class-code/prompt_uid ordering used by `do_reassemble`.
- **Retrieval** (`11-retrieval-system.md`) — the base-prompt body is *not*
  retrieved per turn; it is pre-assembled from validated components and
  served from cache. Per-turn retrieval supplies only the delta (memory,
  orchestrator patch).
- **WebUI Prefix Tab** (`17-webui-prefix-tab.md`) — the UI surface for
  listing prefixes, showing staleness/last-assembled/last-pre-warmed, and
  the Generate/Regenerate + Pre-warm buttons (shifted from the Interceptor
  tab).
- **Agent loop / orchestrator** (`12-agent-loop.md`, `13-orchestrator-default-py.md`)
  — `default.py`'s `working_messages` assembly is where the `base-prompt`
  placeholder line is inserted during composition (v3).

## 6. Today vs. v3

| Aspect | Today | v3 (Phase K.1) |
|---|---|---|
| Kohai base prompt stored? | **No** — no table, no facade, no placeholder | `reborn_basic_prompt_store` (V056) + `PgBasicPromptStore` |
| `base-prompt` placeholder line in composition? | **No** — no literal `base-prompt` in any prompt code | Yes — inserted during composition, replaced by interceptor at end |
| Placeholder substitution (cache-hit inline / stale short fallback)? | **No** | Yes — §4.2 |
| Sempai base prompt stored? | **Yes** — `brassclaw_config` keys, `do_reassemble` | Unchanged (same mechanism, same keys) |
| Assembly mechanism (`do_reassemble`) | exists, serves Sempai base prompt | reused for Kohai base prompt, writes to V056 table |
| `mark_stale` on component `validated`? | n/a (no Kohai store) | Yes |
| `CapturedPrompt.kv_cache_optimised` | flag exists, not driven by real decision | driven by whether the base-prompt prefix was reused |
| WebUI prefix surface | Interceptor tab: Reassemble + Pre-warm (Sempai only) | dedicated **Prefix Tab** listing all prefixes; Sempai Reassemble/Pre-warm shifted in |
| Multiple prefixes | one (Sempai) | first = base prompt; more added later (UI already lists) |

## 7. LLM summary (for prompt injection)

The **base prompt** is the agent's complete, pre-compiled system prompt,
resident in the serving LLM's KV-cache via vLLM prefix caching so that each
turn computes only the small delta as new tokens. During prompt composition a
single `base-prompt` line is inserted as a placeholder; the Sempai-Kohai
interceptor replaces it with the real content at the very end of prompt
creation — inlining the stored bundle on a cache hit, or synthesizing a
short minimal-info stand-in if the bundle is stale/absent (the LLM cannot
recompute the full base prompt inline at ~200 tokens/s). The bundle is
assembled from **validated** components (excluding any still tagged
`05:validator`), ordered by `(class_code, prompt_uid)`, and stored in
`reborn_basic_prompt_store` (V056) with a SHA-256 fingerprint and a per-scope
row. Any component passing validation Gate 2 marks the bundle stale; an
operator regenerates and pre-warms it from the WebUI Prefix Tab. The patch
that accompanies the base prompt must not repeat its content — it uses
`basic_prompt_section_refs` navigation pointers and targets < 4k tokens.
The base prompt is the first of multiple planned prefixes.
