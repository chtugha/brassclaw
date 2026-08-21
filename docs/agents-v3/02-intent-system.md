# 02 — Intent System

> **Subsystem:** Intent matching — classifying a user message and resolving it to a component
> (typically a recipe) or falling back to keyword retrieval.
> **Grounded in:** `crates/brassclaw_engine/src/memory/intent_system.rs`,
> `crates/brassclaw_engine/src/memory/retrieval_source.rs`, `crates/brassclaw_pg/migrations/V028__reborn_intent_inputs.sql`,
> `saved_plan_to_v3.md` (Phases D, E, E.0, J), `MESSAGE_FLOW_AND_PLAN_AUDIT.md`.

## 1. Purpose

The intent system replaces the legacy intent-detection helpers (`signals_tool_intent`,
`signals_execution_intent`, `score_skill`, `extract_explicit_skills`, …) with a single
DB-backed routing lookup. Given a user message, it classifies the query, looks it up in
`reborn_intent_inputs`, and returns one of: an unambiguous **match** (a component id + class
code — typically a recipe), a **disambiguation** request (multiple near-equal candidates), or
**no match** (fall back to keyword retrieval / full LLM). It is the entry point of the v3
"match → recipe" branch of the message flow.

## 2. Location

- **Rust:** `crates/brassclaw_engine/src/memory/intent_system.rs` (pure-Rust helpers always
  compiled; DB functions behind `#[cfg(feature = "skills-db")]`).
- **Retrieval integration:** `crates/brassclaw_engine/src/memory/retrieval_source.rs`
  (`RetrievalSource::fetch_for_turn`, `PostgresSource::fetch_for_turn`, `FetchForTurnResult`).
- **Migration:** `crates/brassclaw_pg/migrations/V028__reborn_intent_inputs.sql` (table +
  indexes; requires `pg_trgm`).
- **Spec:** §3.12 (design), §4 (table), §6.1 (SEC-05 / PERF-01..04), §7 Q10/Q11/Q12/Q16/Q18.

## 3. Data model

### `reborn_intent_inputs` (V028)

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

- **Unique:** `(tenant_id, user_id, agent_id, project_id, input_text, input_class, component_id)`.
- **Indexes:** `scope_text_class_idx` (B-tree, PERF-01 exact match), `scope_text_idx`
  (disambiguation + learning), `scope_component_idx` (purge on component delete),
  `text_trgm_idx` (GIN `pg_trgm` for future fuzzy partial matching, Q16).

### Rust types

- `InputClass` (`i16`): `Word=1`, `Partial=2`, `Sentence=3`, `KeywordFallback=4` (class 4 is
  created by `RetrievalEngine` only — never by the classifier).
- `IntentScope { tenant_id, user_id, agent_id, project_id }` (feature-gated).
- `IntentCandidate { row_id, component_id, component_class_code, input_class, score, class_label }`.
- `IntentResolution` (serde `tag="type"`): `Match { component_id, component_class_code }` |
  `Disambiguation { candidates: Vec<IntentCandidate> }` | `NoMatch`. (The legacy
  `DbLessFallback` variant was removed in `Goals_pre_v3_review.md` Step 9.)
- `IntentSource`: `Seeded`/`LearnedUser`/`LearnedLlm`/`LearnedFallback`; `learned_llm` ⇒
  `needs_review = true`.
- Constants: `DISAMBIGUATION_SPREAD = 2`, `MAX_DISAMBIGUATION_CANDIDATES = 3`, `SCORE_CAP = 100`,
  `SCORE_RATE_LIMIT_PER_HOUR = 50`.

### `class_label` (authoritative class-code → label table)

`0=tool, 1=skill_rusty, 2=skill_monty, 3=skill_llm, 4–9=extensions
(worker/cron/trigger/webhook/plan/revision), 10=orchestrator, 11=reserved, 12=spec,
13=tool_skill, 14=plan, 15=summary, 16=action, 17=docu, 18=lesson, 19=issue, 20=note,
21=recipe, 50=scaffold`. (v3 Phase B/C add `22=python_code`, `23=extension_catalogue` — not yet
present.)

## 4. Behavior / flow

1. **Classify** the query with `classify_query(query)` → `InputClass`:
   - Class 3 (sentence): ≥5 whitespace tokens **or** ends with `.`/`!`/`?` (the `?`-rule).
   - Class 2 (partial): 2–4 tokens, no terminal punctuation.
   - Class 1 (word): 0–1 tokens.
   - Class 4 is never produced here.
2. **Match order** (`match_order`): sentence `3→2→1`; partial `2→3→1`; word/fallback `1→2→3`.
3. **Resolve** (`resolve_intent(pool, scope, query)`) — a **single SQL query** (PERF-02):
   `SELECT id, component_id, component_class_code, input_class, score FROM reborn_intent_inputs
   WHERE <scope> AND input_text = $q AND input_class = ANY($order) ORDER BY CASE input_class
   WHEN $o0 THEN 0 … END, score DESC LIMIT 30`.
   - Empty → `NoMatch`.
   - Deduplicate by `component_id` (keep highest-score). Stop when `top_score - score >
     DISAMBIGUATION_SPREAD` (2).
   - One candidate → `Match { component_id, component_class_code }`. Multiple →
     `Disambiguation(candidates)` (≤3).
4. **Score increment** (PERF-03, atomic): `UPDATE … SET score = LEAST(score + 1, 100)
   RETURNING score` — no SELECT-then-UPDATE race. Rate-limited in-process (SEC-05): ≤50
   increments per scope per hour (token bucket keyed by the 4-part scope).
5. **Disambiguation choice** (`record_disambiguation_choice`): the orchestrator surfaces a
   `role:"disambiguation"` message with clickable candidates; the user's selection sends
   `{disambiguation_choice: component_id}`, which records the choice and increments that
   component's score (so future identical queries trend toward an unambiguous `Match`).
6. **Retrieval integration** (`PostgresSource::fetch_for_turn`): `resolve_intent` → on `Match`
   fetch the exact component by id + increment score; on `Disambiguation` return candidates to
   the orchestrator; on `NoMatch`/error fall back to `fetch_for_consumer` (keyword UNION ALL).
   `RamSource` has no intent store and uses the default `fetch_for_turn` (→ `fetch_for_consumer`).

`FetchForTurnResult` (today): `Components(Vec<ComponentItem>)` | `Disambiguation(Vec<IntentCandidate>)`.

## 5. Relations

- **Recipe System** (`03-recipe-system.md`): a `Match` whose `component_class_code == 21` is a
  recipe; the orchestrator then runs its steps via the IBS (`04-ibs.md`).
- **Retrieval System** (`11-retrieval-system.md`): `PostgresSource` is the intent-driven backend;
  it is implemented but **not wired in production today** (`manager.rs:383` wires `RamSource`).
  Wiring is the Phase E.0 prerequisite.
- **Orchestrator** (`13-orchestrator-default-py.md`): consumes `FetchForTurnResult`; today
  `default.py` has no disambiguation handler (Phase G adds it — see Gap 5 in
  `MESSAGE_FLOW_AND_PLAN_AUDIT.md`).
- **Component Catalog** (`15-component-catalog.md`): `component_id`/`component_class_code`
  reference the unified class tables.

## 6. Status — today vs. v3

**Today:**
- `resolve_intent`, `classify_query`, `match_order`, `record_disambiguation_choice`,
  `PostgresSource::fetch_for_turn`, and the `reborn_intent_inputs` table (V028) **all exist**.
- **Not reached in production**: `RamSource` is the wired backend, so the intent path is dormant.
- `IntentResolution::Match` carries only `{ component_id, component_class_code }` — **no
  `step_link`**.
- `FetchForTurnResult` has only `Components`/`Disambiguation` — no `SplitResult`/`ActionShortCircuit`.
- `class_label` has no 22/23 arms.
- No automatic seeding of intent inputs from auto-passed components.

**v3 plan adds:**
- **Phase D (V054):** add `step_link: Option<String>` and `component_name: String` to
  `IntentResolution::Match` via a LEFT JOIN; new SELECT columns appended at positions 5/6
  (`step_link=5`, `component_name=6`), keeping `id=0, component_id=1, class=2, input_class=3,
  score=4` (FIND-P10-01/05). `step_link` drives variant-aware recipe dispatch.
- **Phase E:** add `FetchForTurnResult::SplitResult { rust_items, orchestrator_items, routing }`
  and `FetchForTurnResult::ActionShortCircuit` (the recipe-routing variants).
- **Phase E.0:** wire `PostgresSource::new(pg_pool)` into composition (`with_retrieval_source`),
  so `fetch_for_turn` is **live** in production (no-pool → hard error). This is the prerequisite
  for Phases E/F/G/H to be exercised live (C2/C3/C4 in `saved_plan_to_v3_review.md`).
- **Phase B/C:** add `class_label` arms `22 => "python_code"`, `23 => "extension_catalogue"`.
- **Phase J (V055):** `intent_examples` + `dependency_registry`; the real wiring is
  `auto_passed → seed_intent_input` (H5: the `SkillManifest` `Vec<String>` shape change is
  dropped; `{input, class}` is already in V027).

## 7. LLM-relevant summary

The intent system classifies a user query into 4 classes (word/partial/sentence/keyword) and
resolves it against `reborn_intent_inputs` in a **single ordered SQL query**: a unique
high-score row → `Match(component_id, class_code)` (a recipe when class 21); several rows within
a 2-point spread → `Disambiguation(≤3 candidates)` surfaced to the user, whose choice is recorded
and scored; nothing → `NoMatch` → keyword UNION ALL fallback. Scores are atomic
(`LEAST(score+1,100)`), capped at 100, rate-limited to 50/scope/hour; `learned_llm` rows are
flagged for review. `PostgresSource::fetch_for_turn` drives this path but is **not wired in
production today** (RamSource is); v3 Phase E.0 wires it, Phase D adds `step_link` to the match,
Phase E adds `SplitResult`/`ActionShortCircuit` variants that route to the recipe/IBS two-channel
delivery.
