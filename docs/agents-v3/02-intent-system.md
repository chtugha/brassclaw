# 02 — Intent System — f1

> **Subsystem:** **f1 — the Intent-Matching-System.** Classifies a user message and resolves it
> to a component (typically a recipe) by an exact, DB-backed lookup against
> `reborn_intent_inputs`, or falls back to keyword retrieval / the non-matching LLM path. It is
> the entry point of the "match → recipe" branch of the message flow: the Orchestrator (Monty)
> calls `host.resolve_intent(user_input)` and dispatches on the returned status.
> **Grounded in:** `crates/brassclaw_engine/src/memory/intent_system.rs`
> (`classify_query`, `match_order`, `resolve_intent`, `record_disambiguation_choice`,
> `seed_intent_input`, `purge_component_inputs`, `class_label`, `IntentResolution`,
> `InputClass`, `IntentSource`), `crates/brassclaw_engine/src/executor/orchestrator.rs`
> (`handle_resolve_intent` — the `host.resolve_intent` arm), `crates/brassclaw_engine/src/memory/retrieval_source.rs`
> (`PostgresSource::fetch_for_turn`, `FetchForTurnResult`),
> `crates/brassclaw_reborn_composition/src/retrieval_lookup_impl.rs` +
> `pg_intent_inputs_store.rs` (seeding call sites),
> `crates/brassclaw_pg/migrations/V028__reborn_intent_inputs.sql` +
> `V054__reborn_intent_inputs_step_link.sql`, `saved_plan_to_v3.md` (Phases D, E, E.0, J).

## 1. Purpose

The intent system replaces the legacy intent-detection helpers (`signals_tool_intent`,
`signals_execution_intent`, `score_skill`, `extract_explicit_skills`, …) with a single
DB-backed routing lookup. Given a user message, it classifies the query, looks it up in
`reborn_intent_inputs`, and returns one of: an unambiguous **match** (a component id + class
code + `step_link` + name — typically a recipe), a **disambiguation** request (multiple
near-equal candidates), or **no match** (fall back to keyword retrieval / the non-matching LLM
path). Monty calls `host.resolve_intent(user_input)` and dispatches:
`match` (with `step_link`) → `host.compose_orchestrator(component_id, step_link, user_input)`;
`disambiguation` → surface candidates to the user; `no_match` → Non-Matching-Mode
(`host.non_match_llm_answer`).

## 2. Location

- **Rust (pure helpers + DB):** `crates/brassclaw_engine/src/memory/intent_system.rs` —
  pure-Rust helpers (`classify_query`, `match_order`, `class_label`) always compiled; DB
  functions (`resolve_intent`, `record_disambiguation_choice`, `seed_intent_input`,
  `purge_component_inputs`, `increment_score`) behind `#[cfg(feature = "skills-db")]`.
- **Host call:** `crates/brassclaw_engine/src/executor/orchestrator.rs::handle_resolve_intent`
  (the `host.resolve_intent` arm) — calls `resolve_intent` and returns a JSON status to Monty.
- **Retrieval integration:** `crates/brassclaw_engine/src/memory/retrieval_source.rs`
  (`RetrievalSource::fetch_for_turn`, `PostgresSource::fetch_for_turn`, `FetchForTurnResult`).
- **Seeding call sites:** `crates/brassclaw_reborn_composition/src/retrieval_lookup_impl.rs`
  + `pg_intent_inputs_store.rs` (the `seed_intent_input` callers — auto-seeding on component
  validation + the WebUI intent-input store).
- **Migrations:** `V028__reborn_intent_inputs.sql` (table + indexes; requires `pg_trgm`),
  `V054__reborn_intent_inputs_step_link.sql` (adds `step_link TEXT` for variant-aware recipe
  dispatch — Phase D).
- **Spec:** §3.12 (design), §4 (table), §6.1 (SEC-05 / PERF-01..04), §7 Q10/Q11/Q12/Q16/Q18.

## 3. Data model

### `reborn_intent_inputs` (V028 + V054)

One row per `(scope, input_text, input_class, component_id)`:

| Column | Type | Notes |
|--------|------|-------|
| `id` | UUID PK | `gen_random_uuid()` |
| `tenant_id`,`user_id`,`agent_id`,`project_id` | TEXT NOT NULL | the 4-part scope tuple |
| `input_text` | TEXT NOT NULL | `CHECK length BETWEEN 1 AND 2048` |
| `input_class` | SMALLINT NOT NULL | 1–4 (see below); `CHECK BETWEEN 1 AND 4` |
| `component_id` | UUID NOT NULL | the component this input maps to |
| `component_class_code` | INT NOT NULL | class of the mapped component |
| `score` | INT NOT NULL DEFAULT 1 | `CHECK BETWEEN 1 AND 100` (SEC-05 hard cap) |
| `source` | TEXT NOT NULL DEFAULT 'seeded' | `seeded`/`learned_user`/`learned_llm`/`learned_fallback` |
| `needs_review` | BOOLEAN NOT NULL DEFAULT false | `true` for `learned_llm` (SEC-05) |
| `step_link` | TEXT | **V054** — the Recipe variant `step_link` formula; `None` for non-variant/non-Recipe intents |

- **Unique:** `(tenant_id, user_id, agent_id, project_id, input_text, input_class, component_id)`.
- **Indexes:** `scope_text_class_idx` (B-tree, PERF-01 exact match), `scope_text_idx`
  (disambiguation + learning), `scope_component_idx` (purge on component delete),
  `text_trgm_idx` (GIN `pg_trgm` for future fuzzy partial matching, Q16).

### Rust types

- `InputClass` (`i16`): `Word=1`, `Partial=2`, `Sentence=3`, `KeywordFallback=4` (class 4 is
  created by the retrieval keyword-fallback only — never by the classifier).
- `IntentScope { tenant_id, user_id, agent_id, project_id }` (feature-gated).
- `IntentCandidate { row_id, component_id, component_class_code, input_class, score, class_label }`.
- `IntentResolution` (serde `tag="type"`): `Match { component_id, component_class_code,
  step_link: Option<String>, component_name: String }` | `Disambiguation { candidates:
  Vec<IntentCandidate> }` | `NoMatch`. (The legacy `DbLessFallback` variant was removed in
  `Goals_pre_v3_review.md` Step 9.) `Match.step_link` is the Recipe variant formula (V054) —
  `None` for legacy/non-variant intents; `Match.component_name` is populated for class-16
  Actions via the `resolve_intent` LEFT JOIN on `reborn_actions` (empty string otherwise).
- `IntentSource`: `Seeded`/`LearnedUser`/`LearnedLlm`/`LearnedFallback`; `learned_llm` ⇒
  `needs_review = true`.
- Constants: `DISAMBIGUATION_SPREAD = 2`, `MAX_DISAMBIGUATION_CANDIDATES = 3`, `SCORE_CAP = 100`,
  `SCORE_RATE_LIMIT_PER_HOUR = 50`.

### `class_label` (authoritative class-code → label table — shipped)

`0=tool, 1=skill_rusty, 2=skill_monty, 3=skill_llm, 4–9=extensions
(worker/cron/trigger/webhook/plan/revision), 10=orchestrator, 11=reserved, 12=spec,
13=tool_skill, 14=plan, 15=summary, 16=action, 17=docu, 18=lesson, 19=issue, 20=note,
21=recipe, 22=python_code, 23=extension_catalogue, 50=scaffold`. The 22/23 arms are shipped
(Phase B/C).

## 4. Behavior / flow

1. **Classify** the query with `classify_query(query)` → `InputClass`:
   - Class 3 (sentence): ≥5 whitespace tokens **or** ends with `.`/`!`/`?` (the `?`-rule).
   - Class 2 (partial): 2–4 tokens, no terminal punctuation.
   - Class 1 (word): 0–1 tokens.
   - Class 4 is never produced here.
2. **Match order** (`match_order`): sentence `3→2→1`; partial `2→3→1`; word/fallback `1→2→3`.
3. **Resolve** (`resolve_intent(pool, scope, query)`) — a **single SQL query** (PERF-02) with a
   `CASE WHEN` order + a security-scoped LEFT JOIN on `reborn_actions` (all 4 scope filters on
   the JOIN, FIND-P6-05, so a cross-tenant `component_id` collision cannot leak another
   tenant's Action name):
   `SELECT ii.id, ii.component_id, ii.component_class_code, ii.input_class, ii.score,
   ii.step_link, COALESCE(a.name,'') AS component_name FROM reborn_intent_inputs ii LEFT JOIN
   reborn_actions a ON … WHERE <scope> AND input_text = $q AND input_class = ANY($order)
   ORDER BY CASE input_class … END, score DESC LIMIT 30`.
   - Empty → `NoMatch`.
   - Deduplicate by `component_id` (keep highest-score). Stop when `top_score - score >
     DISAMBIGUATION_SPREAD` (2).
   - One candidate → `Match { component_id, component_class_code, step_link (rows[0].col 5),
     component_name (rows[0].col 6) }` and atomically increment its score. Multiple →
     `Disambiguation(candidates)` (≤3).
4. **Score increment** (PERF-03, atomic): `UPDATE … SET score = LEAST(score + 1, 100)
   RETURNING score` — no SELECT-then-UPDATE race. Rate-limited in-process (SEC-05): ≤50
   increments per scope per hour (token bucket keyed by the 4-part scope).
5. **Host contract** (`handle_resolve_intent` → Monty): `host.resolve_intent(user_input)`
   returns JSON —
   `match`: `{status:"match", component_id, component_class_code, step_link, component_name}`;
   `disambiguation`: `{status:"disambiguation", candidates:[{component_id, component_class_code,
   score, class_label}]}`; `no_match`: `{status:"no_match"}`; `error`: `{status:"error",
   error}`. No pool / non-`skills-db` build → `{status:"no_match"}`. Monty dispatches: `match`
   (with `step_link`) → `host.compose_orchestrator`; `disambiguation` → surface; `no_match` →
   Non-Matching-Mode.
6. **Disambiguation choice** (`record_disambiguation_choice`): Monty surfaces a
   `role:"disambiguation"` message with clickable candidates; the user's selection sends
   `{disambiguation_choice: component_id}`, which records the choice (atomic score increment on
   the chosen row) and returns a `Match` with `step_link: None` (FINDING A — the caller
   re-fetches the recipe row for its `step_link`; the full IBS path runs on the next turn when
   the user's text matches the intent directly) so future identical queries trend toward an
   unambiguous `Match`.
7. **Seeding** (`seed_intent_input`): `INSERT … ON CONFLICT DO UPDATE` (idempotent). Called
   from `retrieval_lookup_impl.rs` (auto-seeding on component validation) +
   `pg_intent_inputs_store.rs` (the WebUI intent-input store). Carries `step_link` for Recipe
   (class 21) variant intents (FIND-NEW-03); `None` for non-Recipe inputs. `purge_component_inputs`
   deletes all inputs for a component (on wipe/Q4).
8. **Retrieval integration** (`PostgresSource::fetch_for_turn`): `resolve_intent` → on `Match`
   fetch the exact component by id + increment score; on `Disambiguation` return candidates to
   the orchestrator; on `NoMatch`/error fall back to `fetch_for_consumer` (keyword UNION ALL).
   `RamSource` has no intent store and uses the default `fetch_for_turn` (→ `fetch_for_consumer`).
   `FetchForTurnResult` (shipped): `Components(Vec<ComponentItem>)` |
   `Disambiguation(Vec<IntentCandidate>)` | `ActionShortCircuit { component_id, name }` |
   `SplitResult { rust_items, orchestrator_items, routing }`.

## 5. Relations

- **Recipe System** (`03-recipe-system.md`): a `Match` whose `component_class_code == 21` is a
  recipe; Monty then calls `host.compose_orchestrator(component_id, step_link, user_input)` and
  runs its steps via `host.run_program` (the IBS two-channel split is `04-ibs.md`).
- **Retrieval System** (`11-retrieval-system.md`): `PostgresSource` is the intent-driven
  backend. The engine loop wires `RamSource` (the `PostgresSource` path is dormant in the
  engine Monty VM); the active production Tier-0/Tier-1 path is the TURNS `PgOrchestratorLookup`
  bridge. The C.5/C.6 driver activates the engine VM host-call path in production.
- **Orchestrator** (`13-orchestrator-default-py.md`): Monty consumes the `host.resolve_intent`
  JSON status and dispatches (the retired `default.py` had no disambiguation handler; Monty
  does).
- **Composition / IBS** (`04-ibs.md`): `Match.step_link` drives variant-aware recipe
  composition.
- **Component Catalog** (`15-component-catalog.md`): `component_id`/`component_class_code`
  reference the unified class tables; `class_label` is the authoritative label table.
- **Sempai-Kohai** (`09-sempai-kohai.md`): the Sempai proposes new `intent_examples` for
  existing components (`SempaiReviewOutcome.proposed_intent_examples`), which enter Q1 and on
  graduation are seeded via `seed_intent_input`.

## 6. Shipped vs. pending

**Shipped:**
- `resolve_intent`, `classify_query`, `match_order`, `record_disambiguation_choice`,
  `seed_intent_input`, `purge_component_inputs`, and the `reborn_intent_inputs` table (V028).
- **`step_link` (V054)** — `reborn_intent_inputs.step_link` + `IntentResolution::Match.step_link`
  + the `resolve_intent` LEFT JOIN (`component_name` for class-16 Actions) + the
  `seed_intent_input` `step_link` param (Phase D).
- **`host.resolve_intent` (C.2)** — the Monty host call + `handle_resolve_intent` returning the
  JSON status contract.
- **`class_label` 22/23 arms** (`python_code`/`extension_catalogue`) — Phase B/C.
- **`FetchForTurnResult`** all four variants: `Components`/`Disambiguation`/`ActionShortCircuit`/`SplitResult`
  (Phase E).
- **Auto-seeding** on component validation (`retrieval_lookup_impl.rs`) + the WebUI intent-input
  store (`pg_intent_inputs_store.rs`).
- `DbLessFallback` removed (no DB-less mode — Postgres is always used).

**Pending:**
- **C.5/C.6 driver** — activates the engine Monty VM host-call path (`host.resolve_intent` →
  `host.compose_orchestrator` → `host.run_program`) in production; today the engine VM path is
  dormant and the active Tier-0/Tier-1 path is the TURNS `PgOrchestratorLookup` bridge.
- **WebUI disambiguation UX** — the clickable-candidates surface for `role:"disambiguation"`.
- **Phase J** — `intent_examples` formalization + `dependency_registry`; the real wiring is
  `auto_passed → seed_intent_input` (the `{input, class}` shape is already in V028).

## 7. LLM-relevant summary

The intent system classifies a user query into 4 classes (word/partial/sentence/keyword) and
resolves it against `reborn_intent_inputs` in a **single ordered SQL query** with a
security-scoped LEFT JOIN: a unique high-score row → `Match(component_id, class_code,
step_link, component_name)` (a recipe when class 21; `step_link` drives variant-aware
composition via `host.compose_orchestrator`); several rows within a 2-point spread →
`Disambiguation(≤3 candidates)` surfaced to the user, whose choice is recorded and scored (and
returns a `Match` with `step_link: None` — the caller re-fetches); nothing → `NoMatch` →
keyword UNION ALL fallback / Non-Matching-Mode. Monty calls `host.resolve_intent(user_input)`
and dispatches on the JSON status. Scores are atomic (`LEAST(score+1,100)`), capped at 100,
rate-limited to 50/scope/hour; `learned_llm` rows are flagged for review. Auto-seeding runs on
component validation; the Sempai may propose new intent examples. The engine VM host-call path
is dormant until C.5/C.6; the active production Tier-0/Tier-1 path is the TURNS
`PgOrchestratorLookup` bridge.
