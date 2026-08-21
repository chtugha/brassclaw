# 11 — Retrieval System

> **Subsystem:** Prior-knowledge component retrieval — the layer that, given a
> user query + scope, returns the set of **validated** agent components
> (skills, tool-skills, specs, plans, recipes, actions, etc.) the orchestrator
> assembles into the prompt. There are two backends behind one
> `RetrievalSource` trait: `PostgresSource` (intent-driven, single UNION ALL
> query, the v3 target) and `RamSource` (keyword-over-postgres, the legacy
> fallback that is **active in production today**). The intent system
> (`resolve_intent`) drives `PostgresSource::fetch_for_turn`; `RamSource`
> ignores intent and does keyword scoring over a `Store`.
> **Grounded in:** `crates/brassclaw_engine/src/memory/retrieval_source.rs`
> (trait, `RamSource`, `PostgresSource`, `fetch_component_by_id`,
> `FetchForTurnResult`, `ComponentItem`, `ComponentScope`),
> `crates/brassclaw_engine/src/memory/retrieval_dbless.rs` (legacy keyword
> helpers), `crates/brassclaw_engine/src/runtime/manager.rs:374-400` (wiring),
> `crates/brassclaw_engine/src/executor/orchestrator.rs` (`handle_assemble_prior_knowledge` ~2552, `handle_retrieve_docs` ~2488), `saved_plan_to_v3.md` Phase E.0 (lines 3603-3698) + Phase E (3812) + Phase K.3 (5909-5971).

## 1. Purpose

Retrieval is what feeds the orchestrator's "prior-knowledge assembly"
(`__assemble_prior_knowledge__`) — the step that decides what agent context
(skills, recipes, specs, …) belongs in the prompt for a given query. The
user's Task 3 description frames the no-match path as "an LLM-prompt … made of
a head and a body part … one part is containing the chat message, the chat
history and selected memories" — those *selected memories* are exactly the
output of this subsystem. The match path's "recipe" is also retrieved here
(via the intent system routing to a class-21 recipe component).

Two backends, one trait:

- **`PostgresSource`** — the v3 target. Issues a single UNION ALL query across
  all validated component tables (PERF-05 "single-query fetch") and is
  intent-driven: `fetch_for_turn` calls `resolve_intent` first, fetches the
  specific matched component by ID, and only falls back to the full scan on
  no-match. **Implemented but not wired** today (see §6).
- **`RamSource`** — keyword-retrieval over a `Store`. In production the store
  is `PgMemoryDocStore`, so this is keyword-retrieval **over Postgres**, not a
  postgres-less path. The static filesystem fallback that once supported
  "fully offline / DB-less deployments" has been removed (Postgres is
  mandatory). `RamSource` ignores the intent system and `consumer_tag`; it is
  the **active** backend today and is deleted in v3 Phase K.3.

Both backends enforce the SEC-01 validation gate:
`validation_status = 'validated' AND '05:validator' != ALL(consumer_tags)` —
only validated components, and none still pending validator review, reach the
prompt.

## 2. Location

- **Engine memory crate:** `crates/brassclaw_engine/src/memory/`
  - `retrieval_source.rs` (849 lines) — the `RetrievalSource` trait,
    `ComponentItem`, `ComponentScope`, `FetchForTurnResult`,
    `RetrievalSourceError`, `RamSource`, `PostgresSource` (`#[cfg(feature =
    "skills-db")]`), `fetch_component_by_id`, `doc_type_to_class_code`,
    `estimate_tokens`, `TOKENS_PER_BYTE`, and unit tests.
  - `retrieval_dbless.rs` (144 lines) — legacy keyword helpers
    (`extract_keywords`, `keyword_match_score`, `doc_type_weight(DocType)`).
    `RamSource` does **not** call these directly; they survive for the
    remaining legacy keyword path + unit tests + the §3.12 "try it with AI"
    fallback, and are removed in v3 Phase K.3.
  - `RetrievalEngine` (the legacy `retrieve_context` MemoryDoc path) —
    re-exported from the `memory` module; the second caller of `retrieve_context`.
- **Runtime wiring:** `crates/brassclaw_engine/src/runtime/manager.rs:374-400`
  — `ThreadManager::spawn` builds `RetrievalEngine::new(store)` and
  `RamSource::new(store)`, then calls `exec_loop.with_retrieval(retrieval)` +
  `.with_retrieval_source(retrieval_source)` (line 400). The `TODO(Phase K)`
  marker sits at lines 377-381.
- **Orchestrator handler:** `crates/brassclaw_engine/src/executor/orchestrator.rs`
  - `handle_assemble_prior_knowledge` (~2552) — the production entry; prefers
    `retrieval_source.fetch_for_turn`, falls back to legacy
    `retrieval.retrieve_context` on `None`/error.
  - `handle_retrieve_docs` (~2488) — the legacy `__retrieve_docs__` handler
    (calls `retrieve_context`); its registration is removed in Phase K.3 and
    the body removed with it.
- **Intent system:** `crates/brassclaw_engine/src/memory/intent_system.rs` —
  `resolve_intent`, `IntentResolution::{Match,Disambiguation,NoMatch}`,
  `IntentCandidate`, `IntentScope` (see `02-intent-system.md`).
- **Plan:** Phase E.0 (wire `PostgresSource` live, lines 3603-3698), Phase E
  (`fetch_for_turn` upgrade + `FetchForTurnResult::SplitResult`, 3812),
  Phase K.3 (delete `RamSource` + `retrieval_dbless.rs` + legacy fallback
  block, 5909-5971).

## 3. Data model

### `RetrievalSource` trait

```rust
#[async_trait]
pub trait RetrievalSource: Send + Sync {
    async fn fetch_for_consumer(
        &self, scope: &ComponentScope, query: &str,
        token_budget: usize, consumer_tag: &str,
    ) -> Result<Vec<ComponentItem>, RetrievalSourceError>;

    async fn fetch_for_turn(
        &self, scope: &ComponentScope, query: &str,
        token_budget: usize, sender_class_code: &str,
    ) -> Result<FetchForTurnResult, RetrievalSourceError> {
        // default impl: delegate to fetch_for_consumer (RamSource uses this)
        let items = self.fetch_for_consumer(scope, query, token_budget, sender_class_code).await?;
        Ok(FetchForTurnResult::Components(items))
    }
}
```

- **`fetch_for_consumer`** — the keyword/scan path: return validated
  components matching `consumer_tag` in `consumer_tags[]`, ordered by
  `(class_code, prompt_uid)`, capped at `token_budget` tokens. `consumer_tag`
  is the numeric class-code prefix of the *caller* (e.g. `"02"` for the
  orchestrator). `RamSource` ignores it; `PostgresSource` requires it.
- **`fetch_for_turn`** — the intent-driven path (live turn). Default impl
  delegates to `fetch_for_consumer`; `PostgresSource` overrides it to call
  `resolve_intent` first.

### `ComponentItem` (the normalized row)

```rust
pub struct ComponentItem {
    pub id: uuid::Uuid,
    pub class_code: i32,          // 0–21, 50
    pub prompt_uid: i64,          // monotonic assembly ordinal
    pub name: String,
    pub description: String,
    pub effective_content: String, // prior_knowledge_content ?? content/body
    pub override_prompt_creation: bool,
}
```

`effective_content` resolves the §3.13 Solution Override rule: if the row has
a non-NULL `prior_knowledge_content`, that is used verbatim (override);
otherwise the table's `content`/`body`/`description` column is used. The
`override_prompt_creation` flag tells the orchestrator to return the content
verbatim as the stable KV-cache base instead of concatenating it.

### `ComponentScope`

```rust
pub struct ComponentScope {
    pub tenant_id: String,
    pub user_id: String,
    pub agent_id: String,
    pub project_id: String,
}
```

Must match the 4-part scope tuple on every component table. Today the
orchestrator stubs `tenant_id = thread.user_id` and `agent_id = "default"`
(`orchestrator.rs:2586-2591`) because `Thread` does not carry the full tuple in
Phase 1; v3 Phase F tightens this (the prior `tenant_id: "default"` /
`agent_id: ""` hardcoded stub was the H2 scope-bug fix from the Task-2 audit).

### `FetchForTurnResult`

```rust
pub enum FetchForTurnResult {
    Components(Vec<ComponentItem>),
    Disambiguation(Vec<IntentCandidate>),
}
```

- **`Components`** — one or more components ready to assemble.
- **`Disambiguation`** — multiple near-equal intent candidates; the
  orchestrator surfaces a clickable disambiguation message (§3.12 Q11).
- **v3 adds** `SplitResult` (two-channel `rust_items`/`orchestrator_items`
  delivery — see `04-ibs.md`) and `ActionShortCircuit` (class-16 no-LLM
  shortcut — see `08-actions-system.md`) per Phase E / Phase E/G.

### `RetrievalSourceError`

`Db(String)` / `Engine(String)` — `EngineError` maps via `From`.

## 4. Behavior

### 4.1 `PostgresSource::fetch_for_consumer` — the single UNION ALL (PERF-05)

One query unions all validated component tables, each sub-select projecting
to `(id, class_code, prompt_uid, name, description, effective_content,
override_prompt_creation)`:

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

Each sub-SELECT filters:
`tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4 AND
validation_status='validated' AND '05:validator' != ALL(consumer_tags) AND
$5 = ANY(consumer_tags)`. The outer query `ORDER BY class_code ASC,
prompt_uid ASC`.

> **Class 0 (`reborn_tools`) is excluded** — tools have no prompt text (they
> are reached via ToolSkills, see `06-tools-system.md`). **Class 22
> (`reborn_python_code`) is not in the UNION today** — the table does not exist
> yet (Phase B/V052); a class-22 arm will be added with that phase (FINDING C
> from the prior session).

Token budget: rows are accumulated in the returned order until
`tokens_used + cost > token_budget` (cost = `estimate_tokens(byte_len)` =
`max(1, byte_len * 0.25)`); **partial rows are not split** — the whole
component is included or the loop breaks (when `items` is non-empty).

### 4.2 `PostgresSource::fetch_for_turn` — intent-first

```rust
async fn fetch_for_turn(&self, scope, query, token_budget, sender_class_code) {
    match resolve_intent(&self.pool, &intent_scope, query).await {
        Match { component_id, component_class_code } =>
            // score already incremented inside resolve_intent (PERF-03, SEC-05)
            fetch_component_by_id(&self.pool, scope, component_id, component_class_code).await
                -> Components(items)
        Disambiguation { candidates } => Disambiguation(candidates)
        NoMatch | Err(_) => self.fetch_for_consumer(scope, query, token_budget, sender_class_code)
                -> Components(items)
    }
}
```

- **Match** — `resolve_intent` atomically increments the matched row's score
  *before* returning (PERF-03, SEC-05), so no separate increment call. The
  specific component is then fetched by ID via `fetch_component_by_id`
  (re-applies the SEC-01 gate; returns empty if the component was demoted
  between the intent lookup and the fetch).
- **Disambiguation** — surfaced to Python as `result.disambiguation = true`
  + a `candidates` JSON array (`component_id`, `component_class_code`,
  `class_label`, `score`).
- **NoMatch / Err** — fall back to the full UNION ALL keyword scan.

### 4.3 `fetch_component_by_id` — SEC-01-gated single-row fetch

Maps `component_class_code` → `(table, content_expr)` and issues a
parameterized `SELECT … FROM {table} WHERE id=$1 AND <scope> AND
validation_status='validated' AND '05:validator' != ALL(consumer_tags)`. The
match arms cover `1..=3`, `4..=9`, `10 | 50`, `12..=21` (recipes 21). **Class
22 is absent** (`_ => None` → empty vec) until Phase B/V052 adds it. Class 0
(tools) and unknown classes return empty (no prompt text).

### 4.4 `RamSource::fetch_for_consumer` — keyword over `Store`

Parses `project_id` to a UUID, calls
`RetrievalEngine::retrieve_context(project_id, user_id, query, RAM_MAX_DOCS=200)`,
maps each `MemoryDoc` to a `ComponentItem` via `doc_type_to_class_code`
(`Skill→3, Spec→12, ToolSkill→13, Plan→14, Summary→15, Lesson→18, Issue→19,
Note→20, Recipe→21`), assigns a synthetic `prompt_uid = items.len()` (MemoryDoc
has no prompt_uid), sorts by `(class_code, prompt_uid)`, and truncates by
token budget. **`consumer_tag` is ignored** (`_consumer_tag`). When the store
returns nothing, retrieval is simply empty (no filesystem fallback — Postgres
is mandatory).

### 4.5 Orchestrator integration (`handle_assemble_prior_knowledge`)

`args = [goal, token_budget=100000, sender_class_code="02"]`. If
`retrieval_source` is `Some` (always true in production — wired at
`manager.rs:400`), it calls `fetch_for_turn`:

- `Components(items)` → `assemble_from_component_items(&items)` → returns
  `{content, formatted_content, override_prompt_creation,
  matched_component_ids}` to Python (Python uses `formatted_content` for
  `working_messages`; raw `content` is for Rust dispatch + KV fingerprint).
- `Disambiguation(candidates)` → returns `{disambiguation:true, candidates,
  override_prompt_creation:false, content:"", formatted_content:"",
  matched_component_ids:[]}` for Python to surface the UX message.
- `Err` → `debug!` log, fall through to the **legacy fallback**.

**Legacy fallback** (`retrieval.retrieve_context`, lines 2631-2648,
`LEGACY_MAX_DOCS=20`) — the MemoryDoc-based `RetrievalEngine::retrieve_context`.
This block is **dead in production** (the `Some(source)` arm at line 2574 is
always taken) and is deleted in Phase K.3. `handle_retrieve_docs` is the *other*
caller of `retrieve_context` and is also removed in K.3, after which
`retrieve_context` has no callers and is deleted.

## 5. Relations

- **Intent system** (`02-intent-system.md`) — `resolve_intent` drives
  `PostgresSource::fetch_for_turn`; disambiguation candidates are surfaced
  through the orchestrator.
- **Component catalog** (`15-component-catalog.md`) — the union/`fetch_*`
  queries read the unified class tables (12–20) + skills/extensions/actions/
  recipes.
- **Validation system** (`14-validation-queue.md`) — owns the
  `validation_status='validated'` + `05:validator` consumer-tag gate that both
  backends enforce; graduation is what makes a component retrievable.
- **Orchestrator** (`13-orchestrator-default-py.md`) — consumes
  `FetchForTurnResult` via `__assemble_prior_knowledge__`; the
  `override_prompt_creation` / Solution Override path is decided here.
- **IBS** (`04-ibs.md`) — v3 `FetchForTurnResult::SplitResult` carries the
  two-channel `BuildInstruction` output.
- **Actions** (`08-actions-system.md`) — v3
  `FetchForTurnResult::ActionShortCircuit` short-circuits the LLM for class 16.
- **Prefix/base-prompt** (`10-prefix-base-prompt.md`) — the base prompt is
  pre-assembled from validated components (`do_reassemble`); retrieval supplies
  only the per-turn delta, never the whole base prompt.

## 6. Today vs. v3

| Aspect | Today | v3 |
|---|---|---|
| Active backend | `RamSource` (`manager.rs:383`) | `PostgresSource` (wired by Phase E.0) |
| `PostgresSource` wired into composition? | **No** — `TODO(Phase K)` at `manager.rs:377`; `ThreadManager` has no `pg_pool` field; composition never calls `with_retrieval_source(PostgresSource)` | **Yes** — Phase E.0 adds `pg_pool` to `ThreadManager` (feature-gated `#[cfg(feature="skills-db")]`) and builds `PostgresSource::new(pool)` in the spawn path when the pool is present |
| `fetch_for_turn` variants | `Components`, `Disambiguation` | + `SplitResult` (Phase E) + `ActionShortCircuit` (Phase E/G) |
| Class 22 (`reborn_python_code`) in UNION/by-ID arms? | **No** (table absent, arms stop at 21) | Yes — Phase B/V052 adds the arm |
| Legacy `retrieve_context` fallback block | present (`orchestrator.rs:2631-2648`), dead in prod | deleted (Phase K.3) |
| `handle_retrieve_docs` (`__retrieve_docs__`) | registered + body present | registration + body removed (Phase K.3); the dead step-0 shim in `default.py` is also removed |
| `retrieval_dbless.rs` | present (`extract_keywords`, `keyword_match_score`, `doc_type_weight(DocType)`) | `extract_keywords` + `keyword_match_score` **moved** to `retrieval_source.rs` as private helpers; `doc_type_weight` (and the frozen `DocType` enum once `RamSource` is gone) **deleted** (Phase K.3) |
| `RamSource` | active production backend | **deleted** (Phase K.3 — pure deletion *after* E.0 wires `PostgresSource`) |
| Scope tuple | stubbed `tenant_id=user_id`, `agent_id="default"` (H2 fix) | full 4-tuple threaded through `Thread` (Phase F) |

> **Ordering hazard (resolved by the plan's wire-then-delete split).** Deleting
> `RamSource` before `PostgresSource` is wired would leave the engine with *no*
> retrieval backend — every turn's `__assemble_prior_knowledge__` would return
> empty. Phase E.0 is explicitly the **zeroth step of the E family**, pulled
> forward, so that by Phase K.3 `RamSource` is no longer the active backend and
> K.3 is **pure deletion** — no wiring race, no "no retrieval backend" window.
> The K.3 review note records this as "RESOLVED by Phase E.0."

## 7. LLM summary (for prompt injection)

Retrieval supplies the validated agent components (skills, tool-skills,
specs, plans, recipes, actions) the orchestrator assembles into the prompt.
One `RetrievalSource` trait, two backends: `PostgresSource` (intent-driven,
single UNION ALL across all validated class tables, the v3 target) and
`RamSource` (keyword-over-postgres, the active legacy backend today). Both
enforce the SEC-01 gate — only `validation_status='validated'` components,
excluding any still tagged `05:validator`. `fetch_for_turn` is intent-first:
`resolve_intent` routes to a specific component (atomically incrementing its
score), returns `Disambiguation` candidates when intent is ambiguous, and
falls back to the full scan on no-match. Results are ordered by
`(class_code, prompt_uid)` and capped at a token budget (whole rows, never
split). Today `RamSource` runs in production and `PostgresSource` is dormant
(unwired); Phase E.0 wires `PostgresSource` live and Phase K.3 deletes
`RamSource` + the legacy `retrieve_context` fallback + `retrieval_dbless.rs`
(keyword helpers), a wire-then-delete split so there is never a window with
no retrieval backend. v3 adds `SplitResult` and `ActionShortCircuit` result
variants and a class-22 (`reborn_python_code`) arm.
