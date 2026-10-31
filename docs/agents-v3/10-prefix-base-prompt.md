# 10 — Prefix / Base-Prompt (vLLM Prefix Caching) — f6

## Current prefix contract; historical implementation below — 2026-10-26

Follow the [shared selection/pair-design contract](../reborn/model-token-limits.md#prefix-selection-pair-design-and-assembly--binding-target-2026-10-26)
and [Prefix implementation owner](../plans/prefix_v3_upgrade.md). A Recipe,
Orchestrator or role selects a specific prefix; without one, base-prompt is the
regular default, including normal No-Match, not a Tier-2-only restriction.
Unavailable explicit selections/conflicts block rather than silently becoming
base. Shared knowledge/composites require an explicit content contract.

Design for formats/limits belongs to the provider/model pair; role content rules
and finished role prefixes are distinct. Same-pair Kohai/Sempai use the same
accepted design as the generator. Generate the fixed prefix specifically for
that pair. Kohai first formats only the dynamic Orchestrator prompt for the final
target pair. A connected/enabled Sempai corrects that already formatted prompt,
using its own generated reviewer prefix/target-pair design, and returns the final
dynamic prompt. Kohai then inserts the intended finished prefix unchanged exactly
once before it, without reformatting the return, and checks the complete request.
Every manual limit editor links to the existing provider revision/CAS endpoint
and shared query. Historical snapshots are not current mutable settings.
Provider facts may be retrieved through supported APIs/documentation or supplied
manually with provenance; no provider host/template/service edits are an option.

The old store, placeholder, server-template and prewarm walkthrough below describes
historical implementation/evidence where it differs. It is not current serving
qualification. Generation and prefill are separate; cached tokens are not promised
free, permanently resident or reusable without current backend-specific evidence.
Runtime cutover/remaining acceptance stays open in the implementation index.

For construction calls, use the
[visible construction-selection successor](../reborn/model-token-limits.md#open-prefixless-successor--visible-makerbaseprefixless-construction-selection).
It scopes the preceding ordinary role/Base/reviewer rules and the old
[generation-dialog section](../reborn/model-token-limits.md#generation-prompts-missing-selection-and-model-use)
for new workflow revisions. The current UI target must show the editable context
before model work and retain the actual submitted choice, not silently apply a
runtime preference. Saved Recipe profile selection is separate and has no Generate
button. Existing native/UI support remains an open owner task; the historical
walkthrough below is not proof of this successor's uptake. Preserve original Use/
job identity, explicit readiness and generation-versus-activation boundaries.

## Binding Recipe architecture (v3)

Read [recipe.md](../../recipe.md) before authoring or changing components. This contract
supersedes older examples below where they conflict; it specifies the target,
not completed runtime or database functionality.

- Rust Tools supply primitives; many ToolSkills describe their IBS bindings;
  many Skills explain one Tool usage and have associated executable PythonCode;
  many small PythonCode components provide reusable executable building blocks.
  Recipes tell the orchestrator how to use them to fulfill task goals. Prefer
  explicit reusable steps, not fewer steps or specialized Rust workflow Tools.
- Each Recipe component step references exactly one stable component UUID.
  PythonCode may internally compose smaller PythonCode components; this is not
  a multi-component Recipe step. Keep all independent Tool calls in separate
  execution steps; the existing direct dependent-chain exception still applies.
- IBS/composition reads the newest activated, approved versions at task start
  from one consistent catalogue snapshot and pins exact UUID/version/checksum
  references in BuildInstruction, including nested dependencies. Recipes carry
  no version numbers. Execution, child steps, waits and resumption retain that
  selection; do not look up latest again during the task.
- Approved versions are immutable. Changes create new versions; authored
  versions pass Q1 and human Q2 before activation. Replacement neither deletes
  nor invalidates originals used by running/suspended tasks. Current global
  Tool policy is checked independently before every dispatch.
- Inputs and results are typed data. Use the exact input-reference grammar and
  step-local binding convention in recipe.md. Runtime values never become
  Python source. Monty owns each task's intermediate results; unrelated tasks
  and attempts stay isolated, including across child execution and waits.
- Rust-channel ToolSkill binding executes nothing and grants no permission.
  Orchestrator-channel PythonCode calls `host.<tool>(...)`. Only an actual
  No-Match enters Tier 2; errors or begun Recipe failures never replay there.

The legacy composer still contains source-substitution and historical step-body
paths. Ordinary PostgreSQL/skills-db execution uses immutable retained selections,
typed inputs, checked single-component steps and preloaded exports on the global
Monty service. Full-library migration, general pure approval, dynamic occurrences
and durable recovery remain open; see the current implementation status index.

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
  still served on that historical path (this is not the current compatibility rule) until
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

<a id="4-behavior"></a>

## 4. Historical implementation behavior

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

<a id="42-per-turn-prepend-shipped-and-the-base-prompt-placeholder-013-spec"></a>

### 4.2 Historical per-turn prepend and placeholder (§0.13; superseded target)

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

<a id="44-pre-warm-the-compile-into-kv-cache-step"></a>

### 4.4 Historical prewarm operation (not generation or residency proof)

`regenerate_prefix` pre-warms by streaming the assembled bundle as a System message to the
Sempai gateway (`sempai_gateway.stream_model`, model profile `sempai_model`) so vLLM APC
populates the KV-cache; on success it re-stores the row with `with_prewarm = true` to stamp
`prewarm_last_at`. After pre-warm succeeds, subsequent turns that begin with the identical
prefix get a cache hit. The Prefix Tab records `prewarm_last_at` per prefix (mirroring the
Interceptor-tab field). The pre-warm endpoint is rate-limited to one call per caller per 60 s.

<a id="45-multiple-prefixes-future"></a>

### 4.5 Historical single-prefix storage and former multiple-prefix proposal

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

<a id="6-shipped-vs-pending"></a>

## 6. Historical shipped/pending snapshot

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

<a id="7-llm-summary-for-prompt-injection"></a>

## 7. Current LLM-facing summary

This summary describes the binding target, not completed serving integration:

- Recipe, Orchestrator or role specifies a prefix; otherwise select `base-prompt`
  for the target provider/model pair. This applies beyond Tier 2; No-Match normally
  selects base. An unavailable explicit prefix or unresolved conflict blocks the
  affected call rather than substituting base.
- Formats/limit references belong to one saved accepted provider/model design.
  Same-pair Kohai/Sempai share that design; their knowledge/content rules and finished
  role prefixes are separate. Share source knowledge explicitly, not by compulsory
  insertion of the whole base prefix into every role.
- Orchestrator → Kohai formats only the dynamic prompt for its target pair →
  optional connected Sempai corrects it and returns the final dynamic prompt →
  Kohai inserts the intended finished prefix unchanged once, without reformatting
  the return → actual complete-request guard →
  send. Reviewer output cannot change fixed prefix/design/limits. No second runtime
  prefix render or provider server/template/service modification.
- All manually editable prefix/prompt limits link to the canonical provider record,
  endpoint and shared settings query. Historical immutable pins remain evidence;
  they are not another mutable settings source. Hard limits survive disabled budgets.
- Missing/incompatible selected prefix offers OK/Cancel for supported generation.
  Successful provider/model Use also asks to generate/regenerate base. Cancel after
  Use preserves the committed selection and exposes readiness. Existing durable
  jobs, validation, activation and pre-dispatch waits apply; no duplicate call or
  original-message replay and no unreviewed automatic activation.
- Retain full source/effect/proposal evidence and current Q1/behavior/human-Q2 or
  verified shipped-bootstrap contracts. Cache prefill is separate from generation;
  cache residency, token price, quality and performance need their own evidence.
- Older placeholder, minimal/stale fallback, server-template and fixed patch-cap
  descriptions in the historical walkthrough are not current architecture.
  Remaining runtime integration is owned by the [Prefix plan](../plans/prefix_v3_upgrade.md)
  and [implementation index](../implementation-status.md); follow the
  [shared contract](../reborn/model-token-limits.md#prefix-selection-pair-design-and-assembly--binding-target-2026-10-26).

## Open K3-2 consumer successor — prefix selection and construction requests

For new affected consumers, use the operator-confirmed
[shared K3-2 selection/lifecycle contract](../reborn/model-token-limits.md#open-k3-2-successor--retained-prefix-selection-and-prefix-free-construction-workflows)
and [Exact §5.2](../plans/Exact_V3_definition_update_at_K3-2.md#52-bestehende-retained-prefixbindung-um-drei-auswahlzustände-erweitern).
This explicit successor replaces earlier unconditional Use→Base and required
Prefix-Maker-prefix instructions for construction workflows, without rewriting
historical behavior or claiming current runtime uptake.

Default follows declared role/Recipe rules; explicit Selected never silently
falls back; PrefixFreeWorkflow intentionally omits generated prefixes throughout
required child/review work. Independent ordinary chat/Sempai reviews keep their
normal selection. FinalBoundary defers insertion and does not mean prefix-free.
Generation includes all Prefix-Q1 checks; publication leaves human Q2 activation
outstanding. A future v3.5 Prefix-Maker prefix is not a present dependency.
