# 10 — Prefix / Base-Prompt (vLLM Prefix Caching) — f6

> **Subsystem:** Prefix caching for the agent's LLMs — **f6: the LLM-prompt
> creation/composition and the Prefix-System**. A *prefix* is a large, pre-assembled prompt
> **pre-warmed into the serving engine's KV-cache** (vLLM automatic prefix caching — APC) so
> that, on every turn, only the small *delta* (chat message + history + selected memory +
> orchestrator patch) is computed as new tokens. The first and largest prefix is the **base
> prompt** — the complete, compiled documentation/system prompt of the agent. **Shipped
> mechanism:** the bundle text is stored verbatim in `reborn_basic_prompt_store` (V063) and
> prepended to the prompt each turn by `SystemBundleSource::get_system_bundle` (stored bundle
> → KV-cache hit; stale/absent → `minimal_base_prompt_fallback`). **§0.13 spec refinement
> (planned):** a single line `base-prompt` is inserted as a placeholder during composition and
> resolved with the real content at the very end of prompt creation, just before tokenization.
> **Grounded in:** `crates/brassclaw_interceptor/` (`config_store.rs`, `packet.rs`,
> `pg_store.rs`), `crates/brassclaw_reborn_composition/src/interceptor_config_service.rs`
> (`do_assemble_bundle`, `do_format_bundle`, `get_system_bundle`, `list_prefix_entries`,
> `regenerate_prefix`) + `pg_basic_prompt_store.rs` (`PgBasicPromptStore`),
> `crates/brassclaw_reborn/src/loop_driver_host.rs` (`SystemBundleSource` per-turn prepend),
> `crates/brassclaw_reborn_composition/src/runtime.rs` (wiring) + `validation_queue.rs`
> (`mark_stale` on Q2 `approve`), `crates/brassclaw_pg/migrations/V063__reborn_basic_prompt_store.sql`,
> `crates/brassclaw_webui_v2_static/.../interceptor-tab.js`, `saved_plan_to_v3.md` §0.13 + §0.23.7/§0.23.8.

## 1. Purpose

The Prefix-System makes the agent's large system prompt cheap to reuse across turns. Without
prefix caching, every turn would re-process the full agent documentation (potentially many
thousands of tokens) at ~200 tokens/s — prohibitively slow for a 7B–14B model serving
interactive chat. With a pre-compiled prefix resident in the KV-cache, only the per-turn delta
is computed as new tokens.

The user's task (item 8) calls for **vLLM prefix caching with multiple precompiled prompts**,
the first being the "base prompt", with:

- a **single-line `base-prompt` placeholder** added during prompt composition, replaced by real
  content by the Sempai-Kohai system at the end of prompt creation (the §0.13 spec refinement;
  the shipped mechanism prepends the stored bundle via `SystemBundleSource` — see §4.2);
- a **Prefix Tab** under Settings in the WebUI that lists the prefixes and has a button to
  **generate / regenerate** each one — on click the prefix is assembled and sent to the LLM for
  compilation (KV-cache pre-warm);
- the existing single-prefix "base prompt" implementation **shifted into the Prefix Tab**
  section (rather than duplicated).

> **Two base prompts — read carefully.** The codebase has a **Sempai base prompt** (the
> reviewer's audit prompt, Part A) and a **Kohai base prompt** (the answer LLM's system prompt).
> They are distinct (whose KV-cache they warm) but share the **same assembly mechanism** and the
> **same store** (`reborn_basic_prompt_store`, V063 — `PgBasicPromptStore` serves both the
> Sempai and Kohai prefix bundle). The user's "base prompt" is the **Kohai** base prompt; the
> "already foreseen such an implementation" that was **shifted to the Prefix Tab** is the
> **Sempai** base prompt's Reassemble/Pre-warm UI in the Interceptor tab. Its *assembly
> mechanism* (`do_reassemble`, walking the component tables) is exactly what the Kohai base
> prompt reuses; its *UI shape* (a card with last-assembled timestamp + size + a Reassemble
> button + a Pre-warm button) is the template for the Prefix Tab.

## 2. Location

- **Crate:** `crates/brassclaw_interceptor/` — config keys + packet flag.
  - `src/config_store.rs` — `InterceptorConfig` (`sempai_base_prompt`,
    `sempai_base_prompt_assembled_at`, `sempai_persona`, `sempai_prewarm_last_at`) +
    `InterceptorConfigStore` trait (`save_base_prompt`, `save_persona`, `save_prewarm_last_at`,
    `load`). Keys live in the `brassclaw_config` Postgres table — no dedicated migration.
  - `src/packet.rs` — `TokenAccountingSnapshot.kv_cache_optimised: bool` (whether
    KV-cache-optimised prompt ordering was applied for the captured turn), nested in
    `CapturedPrompt.token_accounting`.
- **Composition (assembly + store + per-turn):** `crates/brassclaw_reborn_composition/src/interceptor_config_service.rs`
  — `RebornInterceptorConfigService` (impl of the `InterceptorConfigService` trait):
  - `PREFIX_NAME_BASE_PROMPT = "base-prompt"` — the well-known name of the default bundle.
  - `COMPONENT_TABLES: &[(table_name, class_code)]` — the component tables walked during
    assembly (`reborn_skills`/`reborn_tools`/`reborn_actions`/`reborn_specs`/`reborn_summaries`/
    `reborn_lessons`/`reborn_issues`/`reborn_notes`/`reborn_recipes`/`reborn_tool_skills`/
    `reborn_plans`/`reborn_extensions_unified`/`reborn_orchestrators`/`reborn_scaffolds`).
  - `class_label(class_code)` — human-readable heading per class.
  - `do_assemble_bundle()` — the **assembly mechanism**: discovers which component tables
    exist via `information_schema.tables`, then per table selects `prompt_uid, name, content`
    where `validation_status = 'validated'` and `'05:validator'` is **not** in `consumer_tags`,
    ordered by `prompt_uid ASC` (LIMIT 1000 per table), sorts all parts by
    `(class_code, prompt_uid)`, formats via `do_format_bundle`, and stores the result via
    `PgBasicPromptStore::store`. Per-table/per-row errors are logged at `debug!` and skipped
    (graceful degradation against not-yet-deployed tables).
  - `do_format_bundle()` — pure formatter: `## {class_code}:{prompt_uid}  {class_label}  "{name}"`
    per part, plus the appended `Sempai Response Schema` JSON block.
  - `get_system_bundle()` — the service's per-turn entry point (delegates to the store's
    `SystemBundleSource::get_system_bundle`).
  - `list_prefix_entries()` / `regenerate_prefix()` — the trait methods backing the Prefix Tab
    (list one row; regenerate = assemble + store + pre-warm via `sempai_gateway.stream_model`
    with the `sempai_model` profile, rate-limited 60 s/caller).
- **Durable store (shipped, V063):** `crates/brassclaw_reborn_composition/src/pg_basic_prompt_store.rs`
  — `PgBasicPromptStore` (`get_for_scope`, `store`, `mark_stale`, `delete`) backing
  `reborn_basic_prompt_store`; impls `brassclaw_loop_support::SystemBundleSource`.
- **Per-turn prepend (shipped):** `crates/brassclaw_reborn/src/loop_driver_host.rs` — the loop
  driver holds an `Option<Arc<dyn SystemBundleSource>>` and calls
  `source.get_system_bundle(user_id, project_id)` to prepend the bundle to the prompt; wired in
  `crates/brassclaw_reborn_composition/src/runtime.rs` (`PgBasicPromptStore` as
  `SystemBundleSource`). `mark_stale` is invoked from `validation_queue::approve` on Q2
  graduation.
- **WebUI (Interceptor tab → Prefix Tab):**
  `crates/brassclaw_webui_v2_static/static/js/pages/settings/components/interceptor-tab.js` —
  `StatusCard` (assembled-at + size + prewarm-last-at) + `ControlCard` (Reassemble + Pre-warm
  buttons). The dedicated Prefix Tab is `17-webui-prefix-tab.md`.

## 3. Data model

### `reborn_basic_prompt_store` (V063 — shipped)

```sql
CREATE TABLE reborn_basic_prompt_store (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       TEXT        NOT NULL DEFAULT '',
    user_id         TEXT        NOT NULL DEFAULT '',
    agent_id        TEXT        NOT NULL DEFAULT '',
    project_id      TEXT        NOT NULL DEFAULT '',
    bundle_json     JSONB       NOT NULL DEFAULT '""',  -- assembled bundle text as a JSON string
    fingerprint     TEXT        NOT NULL DEFAULT '',    -- sha256(bundle_text)
    is_stale        BOOLEAN     NOT NULL DEFAULT false,
    assembled_at    TIMESTAMPTZ,
    prewarm_last_at TIMESTAMPTZ,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT reborn_basic_prompt_store_scope_unique
        UNIQUE (tenant_id, user_id, agent_id, project_id)
);
```

- **Scope tuple** — one stored bundle per `(tenant_id, user_id, agent_id, project_id)`; the
  `UNIQUE` constraint enforces one-row-per-scope.
- **`bundle_json`** — the assembled bundle stored as a **JSONB string value** (not an object);
  default `'""'` (empty JSON string) until the first assembly. Stored verbatim so every turn
  sends the **exact same bytes → byte-identical tokens → vLLM APC KV-cache hit**.
- **`fingerprint`** — `sha256(bundle_text)`. Detects whether a re-assembly produced identical
  output (skip the redundant DB write / pre-warm round-trip when unchanged).
- **`is_stale`** — `false` after `store`/regeneration; flipped to `true` by `mark_stale(scope)`
  whenever any component passes **Q2** (`validation_status` → `'validated'`). A stale bundle is
  still served (a stale prefix is cheaper than no prefix and usually still largely correct) until
  an operator regenerates it from the Prefix Tab.
- **`assembled_at` / `prewarm_last_at` / `updated_at`** — provenance for the Prefix Tab display.

V063 also adds `brassclaw_forensic_packets.component_uuid` (§0.23.7) and the **validation-improve
idle-time settings** on `reborn_monty_vm_settings` (§0.23.8): `validation_idle_threshold_minutes`
(default 120), `validation_improve_start_hour` (default 15), `validation_improve_enabled`
(default true) — the shipped hook for the Sempai idle-time self-optimization loop (see `09` §6).

### `PgBasicPromptStore` facade (shipped)

| Method | Behavior |
|---|---|
| `get_for_scope(user_id, project_id)` | Return the stored `BasicPromptEntry` for the scope (whether or not stale); `None` if never assembled. |
| `store(...)` | Upsert the bundle for the scope; recompute `fingerprint`; clear `is_stale`; stamp `assembled_at`/`updated_at`. Called by `do_assemble_bundle`/`regenerate_prefix` on operator demand. |
| `mark_stale(scope)` | Set `is_stale = true` (idempotent). Called on every component `validated` transition. |
| `delete(scope)` | Remove the row for the scope. |

Per-turn usage pattern (shipped):
```text
get_for_scope() → Some(entry) if !is_stale → entry.bundle → prepend to LLM call (KV-cache hit)
                → None or stale → minimal_base_prompt_fallback()
```

### Sempai base-prompt config keys (shipped, in `brassclaw_config`)

| Key | Meaning |
|---|---|
| `interceptor.sempai_base_prompt` | assembled Sempai base prompt (Part A); `None` if never assembled |
| `interceptor.sempai_base_prompt_assembled_at` | ISO-8601 last-assembly timestamp |
| `interceptor.sempai_persona` | Sempai persona text (Part B); falls back to compiled-in default |
| `interceptor.sempai_prewarm_last_at` | ISO-8601 last successful pre-warm |

### `TokenAccountingSnapshot.kv_cache_optimised` (shipped)

A boolean on `CapturedPrompt.token_accounting` recording whether KV-cache-optimised prompt
ordering was applied for the captured turn — the per-turn hook by which the forensic packet
reports whether the base-prompt prefix was reused from cache. (`brassclaw_forensic_packets.component_uuid`,
added by the same V063 migration per §0.23.7, is a DB column on the packet table, not a struct
field.)

## 4. Behavior

### 4.1 Assembly (the shared mechanism)

`InterceptorConfigService::do_reassemble()` is the canonical assembly routine, reused for both
the Sempai and Kohai base prompts. Assembly steps:

1. **Discover tables** — query `information_schema.tables` for the intersection of
   `COMPONENT_TABLES` and existing base tables, so tables from not-yet-deployed phases are
   skipped gracefully.
2. **Per-table select** — `SELECT prompt_uid, name, COALESCE(content,'') AS content FROM {table}
   WHERE validation_status = 'validated' AND NOT ('05:validator' = ANY(COALESCE(consumer_tags,
   ARRAY[]::text[]))) ORDER BY prompt_uid ASC LIMIT 1000`. Only **validated** components, and
   only those **not** still tagged for the validator, are included.
3. **Global sort** — all parts sorted by `(class_code ASC, prompt_uid ASC)` for deterministic
   assembly order (the same ordering composition uses — see `04-ibs.md`).
4. **Concatenate** — each part becomes `## {class_code}:{prompt_uid}  …  "{name}"\n\n{content}`;
   the whole is the assembled base-prompt body, stored via `PgBasicPromptStore::store`.

> **Why `05:validator` exclusion matters:** the base prompt must not contain components still
> pending human validation. A component graduates out of `05:validator` only when it passes Q2
> (see `14-validation-queue.md`). So the assembly reading
> `validation_status = 'validated' AND NOT 05:validator` is the same gate that `mark_stale`
> reacts to: the moment a component joins the base-prompt set, the old bundle is flagged stale.

### 4.2 Per-turn prepend (shipped) and the `base-prompt` placeholder (§0.13 spec)

**Shipped mechanism.** Each turn the loop driver
(`brassclaw_reborn::loop_driver_host`) calls `SystemBundleSource::get_system_bundle(user_id,
project_id)` and prepends the returned text as the System-message prefix of the prompt
(`PgBasicPromptStore` is wired as the `SystemBundleSource` in composition `runtime.rs`).
`get_system_bundle` then resolves to one of:

- a **non-stale, non-empty** stored bundle → return `entry.bundle` (one cheap single-row DB
  fetch; vLLM APC cache hit — only the *delta* after it is computed as new tokens);
- a **stale, empty, or absent** row (or a DB error) → return `minimal_base_prompt_fallback()`
  (a short stand-in; the ~200-tokens/s rationale — the full base prompt cannot be recomputed
  inline, so a compact stand-in carries only the most necessary information).

Storing the bundle text verbatim is what makes APC fire: every turn sends the exact same bytes
→ byte-identical tokens → KV-cache hit. The same `get_system_bundle` serves both the Kohai
prompt path and the Sempai `run_sempai_review` path.

**§0.13 spec refinement (planned, not yet shipped).** The plan calls for a single line whose
content is exactly `base-prompt` to be inserted as a **placeholder** during composition and
resolved with the real content at the very end of prompt creation, just before tokenization
(keeping composition cheap and order-stable while deferring the large content to the end). The
shipped `SystemBundleSource` prepend realizes the same KV-cache benefit today; the literal
placeholder-substitution is the planned refinement of that path.

> **`basic_prompt_section_refs` (§0.13).** The orchestrator patch that accompanies the base
> prompt must **not repeat** content already in the stored base prompt. Instead it carries
> navigation hints — pointers, not content — e.g. `"→ see §ls-skill in basic-prompt"`, because
> the LLM already has the body from the KV-cache. Target patch size **< 4k tokens**. Within the
> bundle priority scheme: orchestrator snippets are **PRIORITY 2**, memory snippets
> **PRIORITY 3**, and **Rust context is delivered directly by the Executioner on `host.*` calls
> — not in the prompt bundle at all**.

### 4.3 Staleness and regeneration

- **Stale trigger:** any component `validated` transition calls `mark_stale(scope)`. A stale
  bundle is still served, but the Prefix Tab shows it as stale and offers Regenerate.
- **Regeneration (manual trigger only):** the base prompt is **never** auto-regenerated on the
  hot path. An operator clicks **Generate / Regenerate** on the Prefix Tab → `store(...)`
  re-runs assembly, recomputes the fingerprint, clears `is_stale`, stamps timestamps → the
  Prefix Tab then triggers a **pre-warm** (send the assembled prefix to the LLM for compilation
  into the KV-cache).
- **Fingerprint:** `store` computes `fingerprint = sha256(bundle)` and persists it for change
  detection; a future optimization will skip the DB write / pre-warm round-trip when a
  re-assembly matches the stored fingerprint (Phase K.1 always writes today).

### 4.4 Pre-warm (the "compile into KV-cache" step)

`regenerate_prefix` pre-warms by streaming the assembled bundle as a System message to the
Sempai gateway (`sempai_gateway.stream_model`, model profile `sempai_model`) so vLLM APC
populates the KV-cache; on success it re-stores the row with `with_prewarm = true` to stamp
`prewarm_last_at`. After pre-warm succeeds, subsequent turns that begin with the identical
prefix get a cache hit. The Prefix Tab records `prewarm_last_at` per prefix (mirroring the
Interceptor-tab field). The pre-warm endpoint is rate-limited to one call per caller per 60 s.

### 4.5 Multiple prefixes (future)

The base prompt is the **first** prefix. The design is explicitly extensible: additional
prefixes (e.g. a per-domain context prefix, a tools-surface prefix) will be added later. The
`reborn_basic_prompt_store` table is named generically and keyed by scope, but extending to
*multiple named prefixes per scope* will require either a `prefix_name TEXT` column added to the
`UNIQUE` tuple or a separate `reborn_prefix_store` table — **not decided in the current plan**;
the Prefix Tab UI is specified to "list the prefixes", anticipating this (see
`17-webui-prefix-tab.md`).

## 5. Relations

- **Sempai-Kohai interceptor** (`09`) — owns the placeholder substitution (§4.2) and is the
  chokepoint where the stored bundle is prepended before LLM shipment.
- **Composition / IBS** (`04`) — the assembly ordering `(class_code, prompt_uid)` matches
  composition's; the orchestrator patch obeys the §0.13 `basic_prompt_section_refs`
  non-repetition rule.
- **Validation system** (`14`) — Q2 graduation is the `mark_stale` trigger and the
  `validated` + `NOT 05:validator` filter in assembly.
- **Component catalog** (`15`) — `COMPONENT_TABLES` and the class-code/prompt_uid ordering used
  by `do_reassemble`.
- **Retrieval** (`11`) — the base-prompt body is *not* retrieved per turn; it is pre-assembled
  from validated components and served from cache. Per-turn retrieval supplies only the delta
  (memory, orchestrator patch).
- **WebUI Prefix Tab** (`17`) — the UI surface for listing prefixes, showing
  staleness/last-assembled/last-pre-warmed, and the Generate/Regenerate + Pre-warm buttons
  (shifted from the Interceptor tab).
- **Orchestrator** (`13`) — Monty's prompt assembly is where the `base-prompt` placeholder line
  is inserted during composition.

## 6. Shipped vs. pending

| Aspect | Shipped | Pending |
|---|---|---|
| Base-prompt store | `reborn_basic_prompt_store` (V063) + `PgBasicPromptStore` (`get_for_scope`/`store`/`mark_stale`/`delete`) | — |
| Per-turn bundle prepend (`SystemBundleSource::get_system_bundle`) | in place (cache-hit inline / stale-or-absent → `minimal_base_prompt_fallback`) | literal `base-prompt` placeholder-substitution (§0.13) |
| Assembly mechanism (`do_assemble_bundle` + `do_format_bundle`) | exists; reused for Sempai + Kohai; writes the V063 row | — |
| `mark_stale` on component `validated` | yes (Q2 graduation) | — |
| `TokenAccountingSnapshot.kv_cache_optimised` + forensic `component_uuid` | flag on `CapturedPrompt.token_accounting` + `component_uuid` column on `brassclaw_forensic_packets` (§0.23.7) | flag driven by a real per-turn ordering decision |
| WebUI prefix surface | Interceptor tab: Reassemble + Pre-warm | dedicated **Prefix Tab** listing all prefixes (Sempai Reassemble/Pre-warm shifted in) |
| Idle-time self-optimization | validation-improve settings on `reborn_monty_vm_settings` (§0.23.8) | background Sempai-driven refresh loop |
| Multiple prefixes | one (base prompt) | more added later (UI already lists) |

## 7. LLM summary (for prompt injection)

The **base prompt** is the agent's complete, pre-compiled system prompt, kept resident in the
serving LLM's KV-cache via **vLLM automatic prefix caching (APC)** so each turn computes only
the small delta as new tokens. **Shipped:** the bundle text is stored verbatim in
`reborn_basic_prompt_store` (V063) and prepended each turn by `SystemBundleSource::get_system_bundle`
(wired from the loop driver) — a non-stale stored bundle is a KV-cache hit; a stale/absent/error
row yields the short `minimal_base_prompt_fallback` stand-in (the LLM cannot recompute the full
base prompt inline at ~200 tokens/s). Storing the text verbatim ensures every turn sends the
exact same bytes → byte-identical tokens → cache hit; the same bundle serves the Kohai and Sempai
paths. The bundle is assembled by `do_assemble_bundle` from **validated** components (excluding
any still tagged `05:validator`), ordered by `(class_code, prompt_uid)` via `do_format_bundle`,
with a SHA-256 `fingerprint` and one row per `(tenant, user, agent, project)` scope. Any
component passing Q2 calls `mark_stale` (wired from `validation_queue::approve`); an operator
regenerates and pre-warms it (`regenerate_prefix` → `sempai_gateway.stream_model`) from the WebUI
Prefix Tab. **§0.13 spec refinement (planned):** insert a single `base-prompt` placeholder line
during composition and resolve it at the very end of prompt creation — a literalization of the
shipped prepend. The orchestrator patch that accompanies the base prompt must not repeat its
content — it uses `basic_prompt_section_refs` navigation pointers and targets < 4k tokens; Rust
context is delivered by the Executioner on `host.*` calls, not in the bundle. The base prompt is
the first of multiple planned prefixes; the validation-improve idle-time settings (§0.23.8) are
the shipped hook for the Sempai idle-time self-optimization loop.
