# 11 — Retrieval System

> **Subsystem:** Prior-knowledge component retrieval — the layer that, given a user query + scope,
> returns the set of **validated** agent components (skills, tool-skills, specs, plans, recipes,
> actions, python_code, catalogues, …) the orchestrator assembles into the prompt. There are two
> backends behind one `RetrievalSource` trait: `PostgresSource` (intent-driven, single UNION ALL
> query — **the active production backend**, wired in composition under `skills-db`) and
> `RamSource` (keyword-over-postgres, the engine-internal legacy fallback — **dormant in
> production**; the engine Monty VM path that hosts it has no external callers).
> **Grounded in:** `crates/brassclaw_engine/src/memory/retrieval_source.rs`,
> `crates/brassclaw_reborn_composition/src/retrieval_lookup_impl.rs` (`PgRetrievalLookup`),
> `crates/brassclaw_reborn_composition/src/orchestrator_lookup_impl.rs` (`PgOrchestratorLookup`),
> `crates/brassclaw_reborn_composition/src/runtime.rs:2556-2611` (wiring),
> `crates/brassclaw_engine/src/memory/intent_system.rs` (`resolve_intent`),
> `saved_plan_to_v3.md` Phase E.0 / Phase E / Phase K.3.

## 1. Purpose

Retrieval feeds the orchestrator's prior-knowledge assembly — the step that decides what agent
context (skills, recipes, specs, …) belongs in the prompt for a given query. The match path's
recipe is retrieved here (the intent system routes to a class-21 recipe component); the no-match
path's "selected memories" are the full-scan output of this subsystem.

Two backends, one trait:

- **`PostgresSource`** — the active production backend. Issues a single UNION ALL query across
  all validated component tables (PERF-05 "single-query fetch") and is intent-driven:
  `fetch_for_turn` calls `resolve_intent` first, fetches the specific matched component by ID,
  and falls back to the full scan on no-match. Wired into composition as `PgRetrievalLookup`
  (`runtime.rs:2562-2570`, `skills-db` + pg_pool) — the turns run-profile's `RetrievalLookup`.
- **`RamSource`** — keyword-retrieval over a `Store` (`PgMemoryDocStore` → keyword-over-Postgres;
  the static filesystem fallback was removed — Postgres is mandatory). Ignores the intent system
  and `consumer_tag`. This is the **engine-internal** retrieval backend, wired only into the
  engine `ThreadManager`/`ExecutionLoop` spawn path — which is **dormant in production** (the
  engine Monty VM is not constructed by `build_reborn_runtime`; the active path is the turns
  `PgOrchestratorLookup` bridge). `RamSource` + its keyword helpers are deleted in Phase K.3.

Both backends enforce the SEC-01 validation gate:
`validation_status = 'validated' AND '05:validator' != ALL(consumer_tags)` — only validated
components, and none still pending validator review, reach the prompt.

## 2. Location

- **Engine memory crate:** `crates/brassclaw_engine/src/memory/`
  - `retrieval_source.rs` — the `RetrievalSource` trait, `ComponentItem`, `ComponentScope`,
    `FetchForTurnResult`, `RetrievalSourceError`, `RamSource`, `PostgresSource`
    (`#[cfg(feature = "skills-db")]`), `fetch_component_by_id`, `class_code_to_table`,
    `doc_type_to_class_code`, `estimate_tokens`, `TOKENS_PER_BYTE`, and unit tests.
  - `retrieval_dbless.rs` — legacy keyword helpers (`extract_keywords`, `keyword_match_score`,
    `doc_type_weight(DocType)`); removed in Phase K.3.
  - `RetrievalEngine` (the legacy `retrieve_context` MemoryDoc path) — the second caller of
    `retrieve_context`; dead in production, deleted in Phase K.3.
- **Composition wiring:** `crates/brassclaw_reborn_composition/src/runtime.rs:2556-2611` —
  `PgRetrievalLookup(PostgresSource)` (the `RetrievalLookup` slot) + `PgOrchestratorLookup`
  (the `OrchestratorLookup` slot, wrapping `TierZeroOrchestrator` + `PgThreadEngineStore` +
  `TierZeroEffectExecutorBuilder`). Both `skills-db`-gated; off → `None` → Tier-2 degrade.
- **Composition lookup impls:** `retrieval_lookup_impl.rs` (`PgRetrievalLookup`),
  `orchestrator_lookup_impl.rs` (`PgOrchestratorLookup` — the active Tier-0 deterministic
  bridge; `TierZeroLlmGuard` is the always-erroring `LlmBackend` so a mis-compiled recipe
  surfaces loudly instead of silently calling a model).
- **Monty host calls:** `host.resolve_intent` (C.2) returns the intent match to Monty;
  `host.fetch_component(uuid, class_code)` (C.2) wraps `fetch_component_by_id` for direct
  UUID lookup. (See `13-orchestrator.md` / `02-intent-system.md`.)
- **Intent system:** `crates/brassclaw_engine/src/memory/intent_system.rs` — `resolve_intent`,
  `IntentResolution::{Match,Disambiguation,NoMatch}` (see `02-intent-system.md`).
- **Plan:** Phase E.0 (wire `PostgresSource` live), Phase E (`fetch_for_turn` upgrade +
  `FetchForTurnResult::SplitResult`), Phase K.3 (delete `RamSource` + `retrieval_dbless.rs` +
  legacy fallback).

## 3. Data model

### `RetrievalSource` trait

```rust
#[async_trait]
pub trait RetrievalSource: Send + Sync {
    async fn fetch_for_consumer(&self, scope: &ComponentScope, query: &str,
        token_budget: usize, consumer_tag: &str) -> Result<Vec<ComponentItem>, RetrievalSourceError>;
    async fn fetch_for_turn(&self, scope: &ComponentScope, query: &str,
        token_budget: usize, sender_class_code: &str) -> Result<FetchForTurnResult, RetrievalSourceError>;
}
```

- **`fetch_for_consumer`** — the keyword/scan path: validated components matching `consumer_tag`
  in `consumer_tags[]`, ordered by `(class_code, prompt_uid)`, capped at `token_budget`. The
  `consumer_tag` is the numeric class-code prefix of the *caller* (`"02"` for the orchestrator).
  `RamSource` ignores it; `PostgresSource` requires it.
- **`fetch_for_turn`** — the intent-driven path. `PostgresSource` overrides it to call
  `resolve_intent` first; `RamSource` uses the keyword path.

### `ComponentItem` (the normalized row)

```rust
pub struct ComponentItem {
    pub id: uuid::Uuid,
    pub class_code: i32,          // 0–23, 50 (incl. 22 python_code, 23 extension_catalogue)
    pub prompt_uid: i64,          // monotonic assembly ordinal
    pub name: String,
    pub description: String,
    pub effective_content: String, // prior_knowledge_content ?? content/body
    pub override_prompt_creation: bool,
}
```

`effective_content` resolves the §3.13 Solution Override rule: if the row has a non-NULL
`prior_knowledge_content`, that is used verbatim (override); otherwise the table's
`content`/`body`/`description` column is used. `override_prompt_creation` tells the orchestrator
to return the content verbatim as the stable KV-cache base instead of concatenating it.

### `ComponentScope`

`{ tenant_id, user_id, agent_id, project_id }` — must match the 4-part scope tuple on every
component table. The composition path threads the validated identity's full tuple.

### `FetchForTurnResult` (all 4 variants shipped)

```rust
pub enum FetchForTurnResult {
    Components(Vec<ComponentItem>),
    Disambiguation(Vec<IntentCandidate>),
    ActionShortCircuit { component_id: Uuid, name: String },  // class 16, vestigial
    SplitResult { /* two-channel BuildInstruction output */ }, // IBS, see 04-ibs.md
}
```

- **`Components`** — one or more components ready to assemble.
- **`Disambiguation`** — multiple near-equal intent candidates; surfaced to Monty as a clickable
  disambiguation message (§3.12 Q11).
- **`ActionShortCircuit`** — class-16 no-LLM shortcut (vestigial under Q2 — see
  `08-actions-system.md`).
- **`SplitResult`** — two-channel `rust_items`/`orchestrator_items` delivery (the IBS
  `BuildInstruction` output — see `04-ibs.md`).

### `RetrievalSourceError`

`Db(String)` / `Engine(String)` — `EngineError` maps via `From`.

## 4. Behavior

### 4.1 `PostgresSource::fetch_for_consumer` — the single UNION ALL (PERF-05)

One query unions all validated component tables, each sub-select projecting to `(id, class_code,
prompt_uid, name, description, effective_content, override_prompt_creation)`:

| Class(es) | Table | effective_content expr |
|---|---|---|
| 1–3 | `reborn_skills` | `COALESCE(NULLIF(prior_knowledge_content,''), body)` |
| 4–9 | `reborn_extensions_unified` | `COALESCE(prior_knowledge_content, description)` |
| 12 | `reborn_specs` | `COALESCE(NULLIF(prior_knowledge_content,''), content)` |
| 13 | `reborn_tool_skills` | `COALESCE(NULLIF(prior_knowledge_content,''), content)` |
| 14 | `reborn_plans` | `COALESCE(NULLIF(prior_knowledge_content,''), content)` |
| 15 | `reborn_summaries` | `COALESCE(NULLIF(prior_knowledge_content,''), content)` |
| 16 | `reborn_actions` | `COALESCE(prior_knowledge_content, description)` (steps is JSONB) |
| 17 | `reborn_docus` | `COALESCE(NULLIF(prior_knowledge_content,''), content)` |
| 18 | `reborn_lessons` | `COALESCE(NULLIF(prior_knowledge_content,''), content)` |
| 19 | `reborn_issues` | `COALESCE(NULLIF(prior_knowledge_content,''), content)` |
| 20 | `reborn_notes` | `COALESCE(NULLIF(prior_knowledge_content,''), content)` |
| 21 | `reborn_recipes` | `COALESCE(NULLIF(prior_knowledge_content,''), '')` |
| 22 | `reborn_python_code` | `COALESCE(NULLIF(prior_knowledge_content,''), content)` |
| 23 | `reborn_extension_catalogues` | `COALESCE(prior_knowledge_content, description)` |

Each sub-SELECT filters: `tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4 AND
validation_status='validated' AND '05:validator' != ALL(consumer_tags) AND $5 =
ANY(consumer_tags)`. The outer query `ORDER BY class_code ASC, prompt_uid ASC`.

> **Class 22 (`reborn_python_code`, V052)** and **class 23 (`reborn_extension_catalogues`,
> V053)** are both in the UNION and the by-id arms (shipped). **Class 0 (`reborn_tools`) is
> excluded** — tools have no prompt text (reached via ToolSkills, see `06-tools-system.md`).

Token budget: rows accumulate in order until `tokens_used + cost > token_budget` (cost =
`estimate_tokens(byte_len)` = `max(1, byte_len * 0.25)`); **partial rows are not split** — the
whole component is included or the loop breaks (when `items` is non-empty).

### 4.2 `PostgresSource::fetch_for_turn` — intent-first

`resolve_intent(&pool, &intent_scope, query)`:
- **`Match { component_id, component_class_code, step_link, component_name }`** — the score is
  atomically incremented inside `resolve_intent` (PERF-03, SEC-05); the specific component is
  fetched by ID via `fetch_component_by_id` (re-applies the SEC-01 gate; returns empty if demoted
  between intent lookup and fetch) → `Components(items)`.
- **`Disambiguation { candidates }`** → `Disambiguation(candidates)`.
- **`NoMatch | Err`** → fall back to the full UNION ALL keyword scan → `Components(items)`.

### 4.3 `fetch_component_by_id` — SEC-01-gated single-row fetch

Maps `component_class_code` → `(table, content_expr)` and issues a parameterized `SELECT … FROM
{table} WHERE id=$1 AND <scope> AND validation_status='validated' AND '05:validator' !=
ALL(consumer_tags)`. The match arms cover `1..=3`, `4..=9`, `10 | 50`, `12..=21`, `22`
(`reborn_python_code`), `23` (`reborn_extension_catalogues`). Class 0 (tools) and unknown classes
return empty (no prompt text). `class_code_to_table` carries the same map (incl. 22/23).

### 4.4 `RamSource::fetch_for_consumer` — keyword over `Store` (dormant in prod)

Parses `project_id` to a UUID, calls `RetrievalEngine::retrieve_context(project_id, user_id,
query, RAM_MAX_DOCS=200)`, maps each `MemoryDoc` to a `ComponentItem` via
`doc_type_to_class_code`, assigns a synthetic `prompt_uid = items.len()`, sorts by `(class_code,
prompt_uid)`, truncates by token budget. **`consumer_tag` is ignored.** This is the engine-internal
backend; the engine VM path is dormant in production, so `RamSource` is not the live retrieval
path. Deleted in Phase K.3.

### 4.5 Production integration (the turns run-profile)

The live production path is the turns run-profile: `PgRetrievalLookup` (composition,
`PostgresSource`-backed) supplies `fetch_for_turn` results; `PgOrchestratorLookup` runs the
Tier-0 deterministic channel (`TierZeroOrchestrator::run_tier_zero`). Monty reaches retrieval
through the `host.resolve_intent` + `host.fetch_component` host calls (C.2). The engine-internal
`handle_assemble_prior_knowledge` / `handle_retrieve_docs` handlers (engine
`executor/orchestrator.rs`) belong to the dormant engine VM path; `handle_retrieve_docs`
(`__retrieve_docs__`) registration + the legacy `retrieve_context` fallback are deleted in Phase
K.3.

## 5. Relations

- **Intent system** (`02-intent-system.md`) — `resolve_intent` drives
  `PostgresSource::fetch_for_turn`; disambiguation candidates are surfaced to Monty via
  `host.resolve_intent`.
- **Component catalog** (`15-component-catalog.md`) — the UNION / `fetch_*` queries read the
  unified class tables (12–23) + skills/extensions/actions/recipes.
- **Validation system** (`14-validation-queue.md`) — owns the `validation_status='validated'` +
  `05:validator` consumer-tag gate both backends enforce; graduation is what makes a component
  retrievable.
- **Orchestrator** (`13-orchestrator.md`) — Monty consumes `FetchForTurnResult` via
  `host.resolve_intent` / `host.fetch_component`; the Solution Override path is decided here.
- **IBS** (`04-ibs.md`) — `FetchForTurnResult::SplitResult` carries the two-channel
  `BuildInstruction` output.
- **Actions** (`08-actions-system.md`) — `FetchForTurnResult::ActionShortCircuit` short-circuits
  the LLM for class 16 (vestigial).
- **Prefix/base-prompt** (`10-prefix-base-prompt.md`) — the base prompt is pre-assembled per-turn
  from validated components via `SystemBundleSource::get_system_bundle` (`do_assemble_bundle`);
  retrieval supplies the per-turn component delta, not the whole base prompt.

## 6. Status — shipped vs. pending

| Aspect | Shipped | Pending |
|---|---|---|
| Active production backend | `PostgresSource` via `PgRetrievalLookup` (`runtime.rs:2562`, `skills-db` + pg_pool) | — |
| Tier-0 deterministic bridge | `PgOrchestratorLookup` (`TierZeroOrchestrator` + `TierZeroLlmGuard`, `runtime.rs:2585`) | engine Monty VM activation (C.5/C.6) |
| `fetch_for_turn` variants | `Components`, `Disambiguation`, `ActionShortCircuit`, `SplitResult` (all 4) | — |
| Class 22 / 23 in UNION + by-id arms | yes (V052 / V053 arms shipped) | — |
| `RamSource` | engine-internal, **dormant** in prod (engine VM not constructed) | **deleted** Phase K.3 |
| Legacy `retrieve_context` fallback | dead in prod | deleted Phase K.3 |
| `handle_retrieve_docs` (`__retrieve_docs__`) | engine-internal (dormant) | registration + body removed Phase K.3 |
| `retrieval_dbless.rs` | present (`extract_keywords`, `keyword_match_score`, `doc_type_weight`) | `extract_keywords` + `keyword_match_score` moved to `retrieval_source.rs`; `doc_type_weight` + frozen `DocType` deleted Phase K.3 |

> **Wire-then-delete.** `PostgresSource` is already wired live (Phase E.0 done), so Phase K.3 is
> **pure deletion** — no "no retrieval backend" window.

## 7. LLM summary (for prompt injection)

Retrieval supplies the validated agent components (skills, tool-skills, specs, plans, recipes,
actions, python_code, catalogues) the orchestrator assembles into the prompt. One
`RetrievalSource` trait, two backends: `PostgresSource` (intent-driven, single UNION ALL across
all validated class tables 1–23 — **the active production backend**, wired via
`PgRetrievalLookup` under `skills-db`) and `RamSource` (keyword-over-postgres, engine-internal,
**dormant in production**, deleted in Phase K.3). Both enforce the SEC-01 gate — only
`validation_status='validated'` components, excluding any still tagged `05:validator`.
`fetch_for_turn` is intent-first: `resolve_intent` routes to a specific component (atomically
incrementing its score), returns `Disambiguation` candidates when intent is ambiguous, and falls
back to the full scan on no-match. Results are ordered by `(class_code, prompt_uid)` and capped
at a token budget (whole rows, never split). All four `FetchForTurnResult` variants are shipped
(`Components` / `Disambiguation` / `ActionShortCircuit` / `SplitResult`); class 22
(`reborn_python_code`) + class 23 (`reborn_extension_catalogues`) arms are in the UNION + by-id
lookup. Monty reaches retrieval via `host.resolve_intent` + `host.fetch_component` (C.2); the
Tier-0 deterministic path runs through `PgOrchestratorLookup` (`TierZeroOrchestrator`).
