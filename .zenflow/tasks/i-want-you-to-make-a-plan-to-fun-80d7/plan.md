# BrassClaw Design Transition — Implementation Plan

Reference: `spec.md` (v5) in this directory. Read it first; this plan assumes
its glossary (Monty, Rusty, DocPlans, PlanA-Memory, Chunk system, Actions,
Intent system, Intent input class) and validation criteria (Section 2.6).

## Guiding rules

- Each phase is independently shippable behind a feature flag; the old path
  stays until the phase's verification passes.
- Every DB write filters by the full `(tenant_id, user_id, agent_id, project_id)`
  scope tuple. Uniqueness on `(scope, name)`.
- Validation is fail-closed and reuses `RecipeValidator` / `SimilarityChecker` /
  `brassclaw_skills::validation` — do not reimplement the agentskills.io rules.
- No `.unwrap()`/`.expect()` in production; `thiserror` for errors; clippy zero
  warnings; `cargo clippy --all --benches --tests --examples --all-features -- -D warnings`.
- Narrowest tests: unit for local logic, integration for DB/routing, E2E/trace
  for UI + gateway. Test through the caller, not just the helper.

## Phase 0 — Spec & plan (this document)

- [ ] `spec.md` written (v5.5: class codes 00-21+50, trust removal, PlanA-memory
  connector, prompt ordering, validation split, **component tag system §3.9**,
  **Monty VM settings §3.10 + kernel-owned restart §3.10**, **4-queue
  validation lifecycle §3.5.1 with validator-tag greyed-out mechanism**,
  **Actions class §3.11 (default.py executor, 13 step types, no size limits,
  token-budget exemption, 8-step dispatch flow, step vocabulary is Python
  tunable logic)**, **unified intent system §3.12 replacing 8
  intent-detection functions**, **intent-driven retrieval §3.13**,
  **Rust-owned formatting §3.13/§3.14 ("content is king" + Solution Override)**,
  **token-budget prior-knowledge limit**, **"try it with AI" fallback with
  class-4 keyword intent matching**, **9 new class codes 12-20 for former
  doctypes**, **orchestrator formatting ban §3.14**, **DB-less fallback file
  for intent system §3.4**, **"AI before User" flip switch §3.12 rule f-ai**,
  **Phase 1.5 blocking rationale (self-modification bootstrapping paradox —
  validator-validates-its-own-patches loop)**, **v5.5: validator independence
  §3.5**, **v5.5: LLM code-audit for Orchestrator (10) + Scaffold (50) §3.5**,
  **v5.5: self-improvement mission writes rerouted through validation gate §3.6**,
  **v5.5: Q19 formatting resolved — "content is king" + Solution Override**,
  **v5.5: interceptor architecture §3.15 (Sempai–Kohai forensic audit +
  rerouting, 3-part Sempai prompt, component_refs instead of full prompt,
  proposed_recipe_updates + proposed_intent_examples → Q1, Actions bypass
  (structural — Python-only, pipeline never entered), DB-less disables
  interceptor, Part A via direct SQL — Q20)**,
  **v5.5: `brassclaw_forensic_packets` table §4 (V51 migration)**,
  **v5.5: ALL 31 open questions RESOLVED (Q1-Q31) — §7 fully resolved**).
- [ ] `plan.md` written (v5.5: Phase 1 Step 1.6 rewritten for default.py Action
  execution + 13 step types + token-budget exemption; Phase 5 Step 5.1c Monty
  VM lifecycle manager; Phase 5 Step 5.2 Actions exempt from token-budget
  truncation; Phase 5 Step 5.2a "content is king" + Solution Override (no
  per-class Rust formatters); Phase 5.5 interceptor activation (6 steps: V51
  migration, InterceptorResult trait change, PgInterceptorStore wiring,
  set_active live-swap, Sempai gateway + 3-part prompt + pre-warm, interceptor
  config service via direct SQL); Phase 6 Step 6.1
  Monty restart + status endpoints; Phase 6 Step 6.2 10-tab editor +
  Interceptor Config tab + Actions tab + Monty VM restart button; verification
  matrix + risks updated for v5.5 + interceptor risks).
- **Verify:** glossary terms match user clarifications; validation table matches
  `recipe_validator.rs` + `brassclaw_skills/validation.rs`; trust-removal targets
  (`SkillTrust`, `V2SkillMetadata.trust`, `default_trust`, `registry.rs`
  directory-trust, skill-trust attenuation) confirmed present in the tree;
  **existing WebUI validation-queue routes** (`list_validation_queue`,
  `validate_recipe`/`validate_tool_skill`, `reject_*`, `request_*_review`) +
  `is_valid_transition` + `is_queue_status` + `RecipeReviewService` confirmed
  present (note: `RecipeReviewService` is referenced in `recipe_store.rs`
  comments but may not exist as standalone Rust code — the auto-review
  functionality is implemented inline in the store; Phase 3 formalizes it as a
  named queue);
  present; **Monty `ResourceLimits` compiled-in constants** (`orchestrator.rs`
  lines 99-128) + `BRASSCLAW_ORCHESTRATOR_MAX_DURATION_SECS` env override
  confirmed present; **self-improvement mission** (`mission_self_improvement.md`
  Level 1/1.5) patches `prompt:codeact_preamble` + `orchestrator:main`
  `MemoryDoc`s confirmed; **8 intent-detection functions** confirmed present
  (`signals_tool_intent` `default.py:101`, `signals_execution_intent`
  `default.py:147`, `llm_signals_tool_intent` `reasoning.rs:48`,
  `user_signals_execution_intent` `reasoning.rs:121`, `score_skill`
  `default.py:678`, `extract_keywords` `retrieval.rs:80`, `extract_explicit_skills`
  `default.py:754`, `RecipeTrigger` `recipe.rs:65`); **formatting functions**
  confirmed present (`format_docs` `default.py:234`, `format_skills`
  `default.py:932`, `append_system_append` `default.py:270`, `format_docs_as_context`
  `context.rs:78`); **`__retrieve_docs__(goal, 5)`** call site confirmed at
  `default.py:1068` with hardcoded 5-doc limit; **`RetrievalEngine` "load all
  docs"** path confirmed at `retrieval.rs:41`; **`doc_type_weight`** priority
  table confirmed at `retrieval.rs:122`; **v5: `default.py` confirmed as the
  Python orchestrator** (Action executor target — no separate Rust
  `ActionExecutor` will be created); **v5: `EffectExecutor` confirmed as the
  Rust tool bridge** (Action `tool_call` steps dispatch through it); **v5:
  no existing `action_executor.rs`** in the tree (confirming the "no separate
  Rust executor" decision); **v5: Monty VM lifecycle is kernel-owned**
  (confirm the kernel can stop/restart the Python VM instance — default.py
  runs inside Monty and cannot restart itself; check `orchestrator.rs` for
  the VM spawn/stop entry points the lifecycle manager will call); **v5:
  Phase 1.5 blocking rationale documented** (self-modification bootstrapping
  paradox — validator-validates-its-own-patches loop; the three candidate
  approaches: (a) validator is Rust/Stable, (b) validator is Python/Tunable
  with Rust ground-truth, (c) two-tier Rust constitutional + Python policy).
  **v5.5: interceptor infrastructure confirmed present** (`ForensicPacket` +
  `CapturedPrompt` + `SempaiReviewOutcome` in `packet.rs`,
  `SharedInterceptorMode` in `mode.rs`, `PgInterceptorStore` in `pg_store.rs`,
  `LoopInterceptorPort` trait in `host.rs:2103`, `InterceptorStage` in
  `interceptor.rs`, `RebornLoopDriverHost` saves packets in
  `loop_driver_host.rs:1800`, `interceptor_store` wired in `runtime.rs:557`,
  `ProviderRole::Sempai` + `set_active` in `llm_config_service.rs:898`,
  `sempai_swappable` scaffolded in `runtime.rs:2494`, `LlmProviderModelGateway`
  in `model_gateway.rs:350`, `InstructionBundleBuilder.build()` in
  `instruction_bundle.rs:202`); **v5.5: five wiring gaps confirmed** (Gap 0:
  `brassclaw_forensic_packets` table missing — V34 in interceptor2.md → V51 in
  our plan; Gap 1: `interceptor_store: None` in composition `runtime.rs:1974`;
  Gap 2: `sempai_swappable` always `None`; Gap 3: `SharedInterceptorMode`
  never created; Gap 4: `on_prompt_assembled` returns `Option<String>` not
  `Option<InterceptorResult>`).
- **Stop-after:** await user sign-off on spec Section 7 open questions —
  **ALL 31 RESOLVED (Q1-Q31).** Q1-Q19 resolved: Q7 Q4 retention window
  configurable via `q4_retention_days`, Q8 single row per scope tuple, Q9
  WebUI validation tab full route extension §3.5.2, Q10 query classifier 4
  classes + ? rule, Q11 disambiguation special chat message type, Q12 "try it
  with AI" fallback entirely Rust-side, Q13 Action step types expanded to 13,
  Q14 per-class validation config, Q15 Recipes get own class code 21, Q16
  `pg_trgm` installed at installation time, Q17 DB-less fallback file created
  at installation time, Q18 "AI before User" switch default OFF per-user.
  Q20-Q31 resolved: Q20 Sempai model free choice (warning if same as Kohai),
  Q21 `forensic_packet_retention_days` (default 90), Q22 settings_adjustments
  stored in ForensicPacket.sempai_review + Apply button in WebUI, Q23
  `components_since_rebuild` badge (passive nudge, manual rebuild only), Q24
  timeout 120s / 0 retries / fallback to original, Q25 `schema_version: 1` +
  Actions bypass is structural (Python-only, pipeline never entered), Q26 mode
  visible in Settings only (not chat), Q27 persona is config text
  (immediate-write, no validation gate), Q28 per-caller rate limit 1/min, Q29
  direct SQL (NOT `reborn_component_catalog`) + interceptor after
  `__assemble_prior_knowledge__` but before final prompt composition, Q30
  `proposed_intent_examples` → Q1 validation queue, Q31 feature flag
  `interceptor` (default off). **ALL RESOLVED — Phase 0 ready for sign-off.**)

## Phase 1 — DB-stored Skills (3-class, explicit columns, no trust) + Intent system + Actions class

**Goal:** Move Skills from `SKILL.md` files into `reborn_skills` with the
agentskills.io schema, explicit columns, the 3-class system in `compatibility`,
`class_code` + `prompt_uid`, and **no `trust` column**. **Add the unified
intent system** (§3.12) replacing all 8 intent-detection functions. **Add the
Actions class** (§3.11) for LLM-free deterministic execution.

### Step 1.1 — Schema & migration
- Add migration `V34__reborn_skills.sql`: `reborn_skills` table per spec §4
  (scope tuple, unique `(scope, name)`). **Explicit columns** for content
  (`name`, `description`, `body`, `compatibility`, `license`, `allowed_tools`,
  `version`, `class_code`, `prompt_uid`), activation (`keywords[]`,
  `exclude_keywords[]`, `patterns[]`, `tags[]` (legacy activation tags),
  `max_context_tokens`, `setup_marker`, `required_binaries[]`,
  `required_env[]`, `required_config[]`), **`intent_examples jsonb`** (spec
  §3.1/§3.12 — array of `{input, class}` entries fed to the intent system on
  validation; replaces `keywords[]`/`patterns[]`/`tags[]` as the primary
  activation mechanism), **consumer gating
  (`consumer_tags[]` text array — spec §3.9; CHECK
  `^[0-9]{2}(:[a-z0-9-]+)?$` per entry; `05:validator` greys out the rest)**,
  reward (immediate-write: `tier`, `usage_count`, `success_count`,
  `failure_count`, `wilson_lower`, `confidence`), provenance (`source`),
  decision (gated: `validation_status`, `validation_errors[]`,
  `review_feedback`, `review_attempts`, `rejected_at`, `queue_code`), lineage
  (`similarity_parent_id`, `replaces_id`, `parent_version`, `content_hash`,
  `last_audit_at`, `audit_failure_count`, `parent_mission_id`).
- **No `trust` column.** `class_code` for Skills: `01` (Rusty), `02` (Monty),
  `03` (LLM). `prompt_uid` assigned by a sequence at insert. `consumer_tags[]`
  seeded at import: Rusty skills `{00:rusty,01:monty}`, Monty skills
  `{01:monty,02:orchestrator}`, LLM skills `{02:orchestrator,03:llm}` — adjust
  per skill during import; new rows also get `05:validator` until validated.
- Indexes: `(tenant_id, user_id, agent_id, project_id, name)` unique;
  `(scope, validation_status)`; `(scope, class_code, prompt_uid)` for ordered
  prompt assembly; **GIN on `consumer_tags[]`** for tag-gated retrieval.
- **Verify:** `cargo test -p brassclaw_pg` migration applies; scope isolation
  contract test (wrong-scope read returns empty).

### Step 1.2 — Skill store & validator wiring
- New module `crates/brassclaw_skills/src/db_store.rs`: CRUD over `reborn_skills`
  implementing the existing `LoadedSkill` read shape used by the selector.
- Reuse `RecipeValidator::validate_tool_skill` for body/name/description/
  token-budget checks. Reuse `brassclaw_skills::validation`
  (`escape_skill_content`, `escape_xml_attr`, `validate_skill_version`) for
  content safety. **Reconcile** the name pattern to the stricter
  `^[a-z0-9]([a-z0-9-]*[a-z0-9])?$`; normalize legacy names via
  `normalize_skill_identifier` (fail closed if unnormalizable).
- Parse the class out of `compatibility` (`brassclaw-class:llm|monty|rusty`);
  reject unknown classes; set `class_code` accordingly. **Seed
  `consumer_tags[]`** from class defaults (spec §3.9): Rusty →
  `{00:rusty,01:monty}`, Monty → `{01:monty,02:orchestrator}`, LLM →
  `{02:orchestrator,03:llm}`. **Add `05:validator` to every new/updated row**
  (the validator tag greys out the others until Step-2 validation removes it —
  spec §3.5.1).
- **Validation split (spec §3.6):** content/activation/**tag membership**
  columns are gated (save → Step-1 validation → commit on pass); the
  `05:validator` tag is added/removed by the queue lifecycle, not by direct
  edit. Reward/provenance columns are immediate-write.
- **Verify:** unit tests for store CRUD + validation accept/reject; caller-level
  test that an invalid skill save is rejected at the store boundary; immediate-
  write reward update succeeds without re-validation; **consumer-tag CHECK
  constraint rejects malformed tag codes**; **a row with `05:validator` is not
  returned by `fetch_for_consumer`**.

### Step 1.3 — Split existing SKILL.md files into ≤1-tool units + extract intent examples
- One-shot importer `crates/brassclaw_reborn_composition/src/skill_import.rs`:
  walk `skills/*/SKILL.md`, split large skills (e.g. `code-review/SKILL.md`
  11 KiB, `github/SKILL.md` 6 KiB) into multiple `reborn_skills` rows, one per
  tool usage pattern. Apply `normalize_skill_identifier` to names.
- Set `compatibility` + `class_code` based on whether the body documents a
  Monty callable (`monty`/`02`), a Rusty tool (`rusty`/`01`), or pure prompt
  guidance (`llm`/`03`).
- **Extract `intent_examples`** from each skill's `keywords[]`, `patterns[]`,
  and description sentences: classify each as class 1 (word), 2 (partial), 3
  (sentence) per §3.12. These feed the intent system on validation.
- Idempotent via `content_hash`; re-runs skip unchanged rows.
- **Verify:** importer test on the real `skills/` tree asserts every emitted
  row passes `RecipeValidator`; token budget ≤5000 per row; intent_examples
  non-empty for every row.

### Step 1.4 — Prompt assembler reads from `reborn_skills` + ordered injection
- Update `brassclaw_engine::executor::context` / `prompt` and
  `brassclaw_skills::selector` to load candidates from the DB store instead of
  filesystem discovery (feature-gated: `skills-db`).
- Keep the deterministic selection pipeline (gating → scoring → budget) —
  **attenuation removed in Phase 3**, not here. For Phase 1, keep the trust-
  based attenuation reading from a **stub that returns `Trusted` for all
  skills** (PH-02 fix — defined semantics: the stub is a no-op pass-through
  that preserves the existing code path without reading a `trust` column from
  `reborn_skills`; the attenuation phase becomes a no-op since all skills are
  `Trusted`; Phase 3 deletes the stub + the attenuation phase entirely).
- **Ordered injection (spec §3.7):** selected skills are appended to the system
  prompt bottom ordered by `(class_code asc, prompt_uid asc)`. Apply
  `escape_skill_content` at injection time.
- **Verify:** trace coverage test `reborn_trace_first_party_tool_coverage.rs`
  still passes; new E2E that a Monty-class skill's function is callable and an
  LLM-class skill's prompt guidance is injected; ordering test asserts byte-
  identical system prefix for the same selection set across two turns.

### Step 1.5 — Intent system: `reborn_intent_inputs` table + `__resolve_intent__` host function
- Add migration `V35__reborn_intent_inputs.sql`: `reborn_intent_inputs` table
  per spec §3.12/§4 — **normalized schema** (PERF-04, §6.1 — one row per
  `(scope, input_text, input_class, component_id)`, NOT a `uuid[]` array):
  `id` uuid PK, scope tuple, `input_text text`, `input_class smallint`
  CHECK 1-4, `component_id` uuid, `component_class_code` int, `score int`
  default 1 (**hard cap 100 — SEC-05/PERF-03 §6.1**), `source` text
  (`seeded`/`learned_user`/`learned_llm`/`learned_fallback`), `needs_review
  bool` default false (true for `learned_llm` — SEC-05), timestamps.
  Composite unique: `(scope, input_text, input_class, component_id)`.
  Indexes: **B-tree on `(scope, input_text, input_class)`** (exact-match
  path — PERF-01), `(scope, input_text)`, `(scope, component_id)`, **GIN
  trigram on `input_text`** (fuzzy partial — future). **`pg_trgm`
  extension installed at brassclaw installation time** (spec §7 Q16 resolved
  — standalone/embedded Postgres setup script runs `CREATE EXTENSION IF NOT
  EXISTS pg_trgm` alongside existing `pgvector`; for external Postgres, the
  installation script checks for `pg_trgm` and fails with a clear error if
  not installed, same pattern as `pgvector`).
- New `crates/brassclaw_engine/src/memory/intent_system.rs`: the
  `__resolve_intent__(query, sender_class_code)` host function (spec §3.12):
  - **Query classification heuristic** (spec §7 Q10 resolved — 4 classes + ?
    rule): class 3 (full sentence): ≥5 words OR ends with `.`/`!`/`?` (the `?`
    rule: a 3-word question like "why this fails?" is class 3); class 2
    (partial): 2–4 words, no terminal punctuation; class 1 (single word): 1
    word, no terminal punctuation; class 4 (keyword fallback): only created by
    RetrievalEngine fallback, never by user query classification. NLP sentence
    boundary detection deferred — the classifier only affects match order, not
    correctness, and the learning mechanism compensates over time.
  - **Match order** per spec §3.12 rules a-c (sentence: 3→2→1; partial: 2→3→1;
    word: 1→2→3; keyword fallback: 1→2→3). **PERF-02, §6.1:** the match-order
    logic is a **single query with `CASE WHEN` ordering** (not 3 sequential
    queries) — `WHERE input_text = ? AND input_class IN (ordered_set) ORDER BY
    CASE input_class WHEN [first] THEN 0 ... END, score DESC LIMIT 10`. One
    query, one round-trip. Class-4 keyword fallback runs one query per keyword.
  - **Scoring (PERF-03, §6.1 — atomic increment):** score increments use
    `UPDATE ... SET score = score + 1 WHERE ... RETURNING score` (atomic
    read-modify-write, no SELECT-then-UPDATE). **SEC-05, §6.1:** score hard
    cap = 100, rate limit = 50 increments per scope per hour (token bucket),
    `learned_llm` inputs flagged `needs_review: true` and purged on source
    component wipe. Seeded inputs (from validated `intent_examples`) are the
    trust anchor.
  - **Scoring rules** per spec §3.12 rules d-f (Q11 resolved): single match classes
    3/2 → score+1, return id; multiple matches within 2-point spread →
    **special `disambiguation` chat message type** with clickable buttons (up
    to 3 component descriptions, payload
    `{type: "disambiguation", candidates: [{component_id, component_class_code,
    description, class_label}]}`); user clicks → structured payload
    `{disambiguation_choice: component_id}` sent directly back to
    `__resolve_intent__` → score+1; after 3 choices the winner is auto-
    selected. A regular text message is explicitly rejected (friction +
    requires another intent-detection round).
  - **No-match handling** per spec §3.12 rules e-f: no kohai → "reformulate"
    message + learn-on-next-match; kohai connected → 5 LLM rephrase sentences
    run through intent system, up to 2 rounds (10 total), user confirms
    meaning equivalence → new input learned.
  - **"Try it with AI" fallback** per spec §3.12 rule f-fallback (Q12
    resolved — entirely Rust-side): after 10 failed LLM sentences →
    "reformulate" + "or try it with AI" button → orchestrator passes
    `fallback: true` flag to `__resolve_intent__` → Rust runs
    `extract_keywords` → keywords sent as class-4 inputs → each keyword
    matched (1→2→3 order) → matching keywords return component IDs →
    RetrievalEngine fetches + assembles prior knowledge **all in Rust** →
    orchestrator receives the final result (no Python→Rust→Python round-
    trip). Per-query only; does not persist as a mode.
  - **"AI before User" flip switch** per spec §3.12 rule f-ai (Q18
    resolved): the WebUI chat window has a flip-switch labeled "AI before
    User". When ON, after 10 failed LLM-sentence matches the intent system
    **suppresses** the "reformulate" message + "or try it with AI" button
    and **silently** passes `fallback: true` flag to `__resolve_intent__`
    (same Rust-side class-4 path as "try it with AI"). The user sees the
    turn proceed without interruption. When OFF (default), the behavior is
    as in rule f-fallback. **No new `reborn_intent_inputs` rows are
    created** from the "AI before User" path (the user did not confirm the
    match — learning only happens on explicit user confirmation or the
    "try it with AI" button click). **Switch persistence (Q18 resolved):**
    default OFF, per-user (not per-scope), stored in
    `reborn_user_preferences` table (new — `(user_id, preference_key,
    preference_value)`; key = `ai_before_user`, value = `true`/`false`),
    visible in chat window only (NOT in Settings UI), hidden/disabled in
    DB-less mode.
- **On validation of any component** (skill/tool/extension/etc.): the
  validator extracts `intent_examples` from the component row and inserts them
  into `reborn_intent_inputs` (class 1/2/3 per the example's `class` field),
  linked to the component's `id` with initial score 1. This is the learning
  seed.
- **Verify:** unit tests for query classification; match-order a-c; scoring d-f;
  no-match e-f; "try it with AI" fallback; **"AI before User" flip switch
  (ON: silent keyword fallback, no reformulate message, no new
  `reborn_intent_inputs` rows; OFF: default behavior)**; disambiguation UX;
  learning-on-match; GIN trigram index works (or exact-match fallback if
  `pg_trgm` unavailable — spec §7 Q16); integration test that validating a
  skill populates `reborn_intent_inputs`.

### Step 1.6 — Actions class: `reborn_actions` table + default.py execution
- Add migration `V36__reborn_actions.sql`: `reborn_actions` table per spec
  §3.11/§4 (scope tuple, unique `(scope, name)`, `description`,
  `preconditions jsonb`, `steps jsonb` (ordered array of step descriptors:
  `tool_call`/`conditional`/`set_var`/`loop`/`return`/`evaluate`/`call_skill`/
  `try_catch`/`parallel`/`call_action`/`spawn_subprocess`/`wait`/`emit_event`
  — **13 step types** per spec §7 Q13 resolved), `error_handling jsonb`,
  `timeout_secs`, `allowed_tools[]`, `class_code` = `16`, `prompt_uid`,
  `consumer_tags[]` (Actions carry `{01:monty,02:orchestrator}` + `05:validator`
  until validated), `intent_examples jsonb`, `param_schema jsonb`,
  `param_template jsonb`, **`prior_knowledge_content TEXT NULL`** (§3.13/§3.14),
  **`override_prompt_creation BOOLEAN NOT NULL DEFAULT true`** (§3.13/§3.14 —
  Actions default to Solution Override), validation/lineage columns as per
  skills). **Actions do not conform to skill size limits** — no token budget
  enforcement on Action content. **Hard limits (PERF-18, §6.1 — compiled-in,
  not configurable):** max content size = 256KB, max step count = 500, max
  `allowed_tools[]` = 50. An Action exceeding any limit is rejected at
  validation. **Recursion bounding (SEC-09, §6.1):** `call_action` max depth =
  5, cycle detection, total step budget = 1000 across nesting levels.
- **No separate Rust executor.** There is no `action_executor.rs` file and no
  `__execute_action_procedure__` host function. default.py (the Python
  orchestrator) is the executor. Tool calls within an Action go through the
  same Rust `EffectExecutor` bridge as any other tool call.
- **default.py Action execution logic (spec §3.11):** default.py gains an
  Action-execution mode. When it recognizes class_code 16 in the retrieved
  components, it **stops further prompt creation** (does not call
  `__llm_complete__`) and performs the Action directly by following its step
  descriptors:
  - `tool_call`: dispatch to the Rust `EffectExecutor` bridge. default.py
    checks the tool against `allowed_tools[]` before dispatch — a tool not in
    the list is rejected. **The Rust `EffectExecutor` bridge also checks
    `allowed_tools[]` before execution (SEC-07, §6.1 — defense in depth).**
    The Rust bridge receives the Action's `allowed_tools[]` as part of the
    turn context, not from the orchestrator's self-reported list.
  - `conditional`: evaluate a condition and branch (`then_step`/`else_step`).
  - `set_var`: bind a variable in the Action's variable scope.
  - `loop`: iterate a step list until an exit condition is met (bounded by
    `timeout_secs`).
  - `return`: terminate the Action with a result (becomes the turn result).
  - `evaluate`: run a Python expression in default.py's scope to evaluate a
    tool result and decide next steps.
  - `call_skill`: invoke a Rusty/Monty skill (not just a raw tool) as part of
    the Action.
  - `try_catch`: wrap a sub-sequence of steps with per-block error recovery.
  - `parallel`: run multiple tool calls concurrently and wait for all (or
    first-success).
  - `call_action`: invoke another Action (Action-to-Action chaining).
  - `spawn_subprocess` (spec §7 Q13, **SEC-08 §6.1**): spawn a subprocess via
    the **host runtime's script lane** (`services/script_runtime`) — NOT a raw
    `subprocess.Popen`. The script lane enforces capability lease + approval
    gate + sandbox boundary. `allowed_tools[]` must include `spawn_subprocess`
    explicitly. `command`/`args`/`cwd` validated against the script lane's
    allowlist. Output is captured; `timeout_secs` bounds the run.
  - `wait` (spec §7 Q13): pause execution for a fixed duration
    (`duration_secs`) or until a polling condition is met (`file_exists`/
    `env_set`/`var_eq`) with a timeout. Useful for waiting on async side
    effects (deployment completion, file appearance).
  - `emit_event` (spec §7 Q13): emit a structured event to the event bus
    (`brassclaw_events` table, migration V013) for webhook triggers, extension
    notifications, or operator dashboards. Enables event-driven Actions: one
    Action's `emit_event` can trigger another Action's execution via a trigger
    binding.
- **Orchestrator dispatch flow (spec §3.11):**
  1. Step 0: orchestrator sends the goal query to
     `__resolve_intent__(goal, "02")`.
  2. Intent system responds with the unique id of an Action element
     (class_code 16).
  3. RetrievalEngine pulls that Action element by id (same as any other
     component retrieval).
  4. Prior knowledge is created via `__assemble_prior_knowledge__` (Step 5.2a)
     — the Action's `prior_knowledge_content` (or `content` if PKC is NULL) is
     returned via the **Solution Override path** (Actions default to
     `override_prompt_creation: true`). No per-class formatter — "content is
     king" (§3.13/§3.14). **The Action is exempt from
     `prior_knowledge_token_budget` truncation** — full content included.
  5. Prior knowledge is given to default.py.
  6. default.py recognizes class_code 16 and stops further prompt creation.
  7. default.py performs the Action directly.
  8. The Action's `return` value becomes the turn result.
- **Step vocabulary is Python tunable logic:** new step types can be added by
  patching default.py (through the validation gate), not by changing Rust
  code.
- **Verify:** unit tests for each of the **13 step types** (`tool_call`/
  `conditional`/`set_var`/`loop`/`return`/`evaluate`/`call_skill`/`try_catch`/
  `parallel`/`call_action`/`spawn_subprocess`/`wait`/`emit_event` — spec §7
  Q13 resolved); integration test that an Action `tool_call` step goes through
  the Rust `EffectExecutor` bridge; `allowed_tools[]` enforcement test
  (default.py rejects a tool not in the list); timeout enforcement test;
  orchestrator dispatch test (intent match → Action retrieved → prior
  knowledge created → default.py performs Action, no LLM call); `call_action`
  chaining test; `try_catch` error recovery test; `parallel` concurrency test;
  **`spawn_subprocess` test** (subprocess runs via host runtime script lane,
  output captured, timeout enforced, **`allowed_tools[]` must include
  `spawn_subprocess`**, **`command`/`args`/`cwd` validated against allowlist
  — SEC-08**); **`wait` test** (pause for duration +
  polling condition with timeout); **`emit_event` test** (event dispatched to
  event bus via `brassclaw_events`); **token-budget exemption test** (Action
  content not truncated regardless of budget); **hard limits test** (Action
  >256KB or >500 steps or >50 tools rejected at validation — PERF-18);
  **recursion bounding test** (`call_action` depth >5 rejected, cycle detected,
  total step budget >1000 rejected — SEC-09); **defense-in-depth test**
  (`allowed_tools[]` enforced at BOTH default.py AND Rust `EffectExecutor`
  — SEC-07); intent examples extraction on
  validation.

## Phase 1.5 — Prompt-path dedup + self-modification boundary (UNBLOCKED)

**Goal:** Resolve the v1→v2 duplications (`context.rs` dead path,
`score_skill`/`signals_tool_intent` Rust+Python twins, `format_docs`/
`format_skills`/`append_system_append` Python formatters) and define the
self-modification boundary now that all code changes must pass validation.

**STATUS: UNBLOCKED** (spec §7 Q1 resolved). The bootstrapping paradox is
resolved by **validator independence** (spec §3.5): the Step-1 validator is
Rust-side infrastructure, NOT part of default.py — the self-improvement mission
cannot patch it. All self-improvement mission writes are validation-gated
(spec §3.6): `memory_write` for code/component changes creates update-candidates
that enter Q1. Orchestrator (10) + Scaffold (50) components require an LLM
code-audit before Q2 manual validation (spec §3.5).

**Design decisions confirmed:**
- `build_step_context` is **resurrected** (Option A: User-message injection at
  N-1, the KV-cache-friendly path). Spec §3.7 assumes Option A; this phase
  implements it.
- Python `score_skill` / `signals_tool_intent` / all 8 intent-detection
  functions (§2.3a) are **deleted entirely** — the intent system (§3.12)
  replaces them. They are not kept as self-modifiable logic.
- **Formatting dedup (spec §3.13/§3.14 — Q19 RESOLVED: "content is king" +
  Solution Override):**
  `format_docs` (`default.py:234`) and `format_skills` (`default.py:932`) are
  **deleted** — the orchestrator no longer formats. `append_system_append`
  (`default.py:270`) is **deleted** — replaced by User-message-at-N-1
  (Rust-owned). `format_output` (`default.py:194`), `_reduce_prompt`
  (`default.py:542`), and `compact_if_needed` (`default.py:562`) are **kept** in
  Python (tool output formatting, prompt reduction policy, compaction policy —
  not prior-knowledge formatting). The Rust formatting approach is **resolved**:
  one `__assemble_prior_knowledge__` function + a static class-code→label
  lookup table. No per-class Rust formatters. Components store their content as
  the exact prior-knowledge text; Rust concatenates in `(class_code asc,
  prompt_uid asc)` order with per-item headers. Solution-class components
  (Extensions/Plans/Recipes/Actions) have `prior_knowledge_content TEXT NULL` +
  `override_prompt_creation BOOLEAN NOT NULL DEFAULT false` (Actions default
  `true`) for the Solution Override path.
- **Orchestrator formatting ban (spec §3.14):** the orchestrator cannot format
  retrieved content. The orchestrator CAN: decide WHEN to retrieve, decide the
  retrieval query (goal), decide WHEN to compact, decide WHEN to reduce prompt.
  The orchestrator CANNOT: format retrieved content, mutate the System message
  prefix, choose injection point.
- **Self-improvement mission writes rerouted (spec §3.6):** the mission's
  `memory_write` host function is intercepted by the Rust bridge and routed to
  `__validate_component__` instead of writing directly. Level 1 (prompt
  overlays) and Level 1.5 (orchestrator code) both create update-candidates
  that enter Q1. The 3-failure auto-rollback (`load_orchestrator`) is retained
  as a safety net behind the validation gate.
- **`__resolve_intent__` DB-less fallback (spec §3.4):** in DB-less mode the
  intent system is **overridden** — `__resolve_intent__` returns
  `{db_less_fallback: true}`. The orchestrator falls back to the keyword-
  retrieval path against the DB-less fallback-content file loaded by
  `RamSource` (Step 5.1b). The "try it with AI" fallback and "AI before User"
  flip switch are **not available** in DB-less mode. Confirm: (a) the
  `__resolve_intent__` return shape accommodates `{db_less_fallback: true}`;
  (b) the keyword-retrieval path is preserved (relocated to
  `retrieval_dbless.rs`, not deleted in Phase 5.5); (c) the fallback file
  format (spec §7 Q17) and the `RamSource` in-memory index shape.

**Remaining work for this phase:**
- Implement the User-message-at-N-1 injection path (resurrect
  `build_step_context` as the Rust-side User-message insertion).
- Delete the 8 intent-detection functions from default.py (they're obsolete
  once the intent system is live from Phase 1). **PH-03 fix:** `extract_keywords`
  (`retrieval.rs:80`) is a **Rust** function, NOT a Python function — it is NOT
  deleted in Phase 1.5. It remains in `retrieval.rs` throughout Phase 1/1.5
  and is relocated to `retrieval_dbless.rs` in Phase 5. The "try it with AI"
  fallback (Phase 1 Step 1.5) uses `extract_keywords` in Rust — it works
  throughout.
- Delete `format_docs`/`format_skills`/`append_system_append` from default.py.
- Reroute `memory_write` for code/component changes through
  `__validate_component__`.
- Implement the LLM code-audit gate for Orchestrator (10) + Scaffold (50)
  components at Q1→Q2 (kohai-provider audit, Q2 "Validate" button disabled
  until audit clean).

**Verify:** `build_step_context` User-at-N-1 injection test; grep confirms no
remaining `signals_tool_intent`/`signals_execution_intent`/`score_skill`/
`extract_explicit_skills`/`format_docs`/`format_skills`/`append_system_append`
in default.py; self-improvement `memory_write` reroute test (write creates
update-candidate with `05:validator` tag, enters Q1); LLM code-audit gate test
(Orchestrator/Scaffold component passes Q1 → LLM audit runs → Q2 button
disabled until clean → audit flags issue → routed to Q3 with findings); 3-failure
auto-rollback still works behind the validation gate.

**Follow-up task (cross-phase, for Phase 5.5 interceptor):** after Phase 1.5
ships (User-at-N-1 injection), verify that the interceptor's Part C stripping
logic (§3.15) correctly identifies and strips the new stable-tier injections
and preserves only the per-turn volatile tail. The stripping boundary
(priorities 1–5 = stable, 6–7 = volatile) remains the same conceptually, but
the specific messages in priority 6 change shape when Phase 1.5 lands. Add
this as a verification item in Phase 5.5.

**Stop-after:** Phase 1.5 design is fully unblocked (Q19 resolved, validator
independence confirmed). No stop-after — proceed to Phase 2.

## Phase 2 — DB-stored Tools (Rusty-only)

**Goal:** Move Rusty tool definitions into `reborn_tools`; tools carry Rusty
instructions only; Monty/LLM are instructed via Skills.

### Step 2.1 — Schema & migration
- `V37__reborn_tools.sql`: `reborn_tools` table per spec §4 (scope tuple,
  unique `(scope, name)`, `param_schema jsonb`, `param_template jsonb`,
  `effect_type`, `preconditions`, `error_handling`, `class_code` = `00`,
  `prompt_uid`, **`consumer_tags[]`** (spec §3.9; tools always carry
  `{00:rusty}` + `05:validator` until validated), `validation_status`,
  `validation_errors[]`, `review_feedback`, `review_attempts`, `rejected_at`,
  `queue_code`, `content_hash`, `source`, lineage, timestamps). GIN on
  `consumer_tags[]`.
- **Verify:** migration + scope isolation contract.

### Step 2.2 — Tool store & capability surface
- `crates/brassclaw_capabilities` (or a new `db_tool_source`): read tool
  definitions from `reborn_tools` into the `ToolRegistry` capability surface.
  This surface is what `RecipeValidator::validate_tool_skill` checks
  `tool_name` against.
- Strip any Monty/LLM prompt text from tool rows — tools are Rusty-only now.
- **Verify:** caller-level test that a ToolSkill whose `tool_name` is absent
  from the DB-backed surface is rejected; integration test that a Rusty tool
  executes via `EffectExecutor`.

### Step 2.3 — Monty/LLM instruction via Skills only
- Remove tool-definition prompt text from the Monty/LLM prompt paths; ensure
  Monty callables and LLM guidance come from Monty/LLM-class Skills
  (Phase 1 rows). Document this in `codeact_preamble.md` if needed.
- **Verify:** trace test that a tool is callable from Monty via a
  `rusty`-class Skill description, not via inline tool prompt text.

## Phase 3 — Remove source-driven security + 4-queue validation lifecycle (validation = sole trust gate)

**Goal:** Remove `SkillTrust` + source-driven trust + skill-trust attenuation;
the two-step validation system becomes the **only** gate to usability.
`Validated == trusted` (+ no `05:validator` consumer tag). Expand the validator
to validate code. Formalize the **4-queue validation lifecycle** (Q1 auto / Q2
manual-WebUI / Q3 revision / Q4 rejection+wipe) with the **validator-tag
greyed-out mechanism** (spec §3.5.1).

### Step 3.1 — Delete the trust layer
- Delete `SkillTrust` enum (`brassclaw_skills::types`); delete
  `V2SkillMetadata.trust` + `default_trust()` (`v2.rs`); delete trust-by-
  source-directory logic in `registry.rs` (the Workspace/User/Bundled = Trusted,
  Installed = Installed mapping).
- Delete the **skill-trust attenuation** phase of the selection pipeline (tool
  ceiling = `min(trust)` across active skills) in `brassclaw_skills::selector`
  and in Python `default.py` if mirrored.
- Delete the `Installed`/`Trusted` tool-access distinction in the capability
  surface.
- **Verify:** `cargo test -p brassclaw_skills` passes with all trust tests
  removed or converted to validation-status tests; grep confirms no remaining
  `SkillTrust` / `default_trust` / `attenuate.*trust` references; clippy clean.

### Step 3.2 — `source` becomes pure provenance; confidence factor universal
- `source` (`extracted`/`authored`/...) is retained for audit/display only —
  **no behavioral effect**. Remove the `if source == "extracted"` gate in
  `default.py::score_skill` (line ~743) and in the Rust `selector.rs` mirror.
- The confidence factor (`0.5 + 0.5*confidence`) is **source-independent** and
  **kept as a fallback routing signal only** (spec §7 Q2 resolved): in normal
  mode the intent system's score is the primary routing signal; the confidence
  factor is used only when the fallback mechanism is triggered (user "AI before
  User" switch ON, or intent system finds no match). Skills with no usage data
  default to confidence 1.0. Telemetry columns displayed in WebUI Reliability
  tab regardless of mode.
- **Verify:** unit test that an `authored` skill with usage metrics gets the
  same confidence factor as an `extracted` skill with the same metrics; E2E
  that scoring is deterministic across source values; **fallback routing test**
  (confidence factor influences retrieval only when fallback is triggered, not
  in normal intent-driven mode).

### Step 3.3 — `Validated == trusted` (+ no validator tag) invariant
- Audit every call site that filters skills/tools/recipes for the loop:
  `recipe_library.rs` already filters to `ValidationStatus::Validated` —
  confirm this is the **sole** gate. Any trust-based filter becomes a
  validation-status filter **plus** a `consumer_tags[] NOT CONTAINS
  '05:validator'` filter (spec §3.5.1 — the validator tag greys out delivery).
- Add a regression test: an `AutoPassed` (not yet `Validated`) component is
  **not** reachable by the loop; only `Validated` is. A `Validated` component
  that somehow still carries `05:validator` (should not happen — Step 3.5 pops
  it on validation) is also not reachable.
- **Verify:** `recipe_library` contract test; `is_valid_transition` guard test
  (`Pending → Validated` still blocked); **`fetch_for_consumer` excludes
  `05:validator`-tagged rows**.

### Step 3.4 — Expand validator to validate code + validator independence + LLM code-audit
- Extend `RecipeValidator` (or a new `CodeValidator`) to validate the
  orchestrator Python + Monty-class extension payloads, not just skill text.
  All code changes — including self-improvement mission patches — must pass
  validation before applying. Versioned rollback retained.
- **Validator independence (spec §3.5):** the Step-1 validator is Rust-side
  infrastructure — it is NOT part of default.py and cannot be patched by the
  self-improvement mission. The orchestrator calls the validator via a host
  function (`__validate_component__`); it cannot alter the validator's logic.
  This breaks the bootstrapping paradox: even if the self-improvement mission
  patches default.py (Level 1.5), it cannot modify the validator.
- **LLM code-audit for Orchestrator (10) + Scaffold (50) components (spec §3.5):**
  for Orchestrator-class and Scaffold-class components specifically, the Q1→Q2
  transition includes an extra step: a **kohai-provider LLM code-audit** that
  checks for security issues from the self-modification looping problem
  (validator bypass, infinite recursion, privilege escalation through
  self-patching, sandbox escape, secret exfiltration). The audit prompt is a
  minimal, Rust-side-constructed prompt (no orchestrator involvement) sent to
  the kohai provider. The Q2 manual "Validate" button is **disabled** in the
  WebUI until the LLM audit returns clean. If the audit flags issues, the
  component stays in Q1 and is routed to Q3 (revision) with the audit findings
  attached as `review_feedback`.
- **Self-improvement mission writes rerouted (spec §3.6):** the mission's
  `memory_write` host function is intercepted by the Rust bridge and routed to
  `__validate_component__` instead of writing directly. Level 1 (prompt
  overlays) and Level 1.5 (orchestrator code) both create update-candidates
  that enter Q1. The 3-failure auto-rollback (`load_orchestrator`) is retained
  as a safety net behind the validation gate.
- **Keep (not source-driven security):** subagent capability attenuation
  (`brassclaw_loop_support::attenuate_child_capability_port`); install-bundle
  ingestion (imported skills go through validation); credential specs.
- **Verify:** code-validation gate test: an invalid orchestrator patch is
  rejected; a valid patch commits only on pass; **validator independence test**
  (the validator runs outside default.py — confirm the validator code is not
  in the orchestrator's patchable surface); **LLM code-audit gate test**
  (Orchestrator/Scaffold component passes Q1 → LLM audit runs → Q2 button
  disabled until clean → audit flags issue → routed to Q3 with findings);
  **self-improvement `memory_write` reroute test** (write creates
  update-candidate with `05:validator` tag, enters Q1, does not write directly).

### Step 3.5 — Validator-tag greyed-out mechanism + 4 queues
- **Validator-tag lifecycle (spec §3.5.1):**
  - On create/import/update → add `05:validator` to `consumer_tags[]`. All
    other tags become greyed-out (inactive for delivery; still toggleable in
    the WebUI so the operator can pre-set the audience).
  - On Step-2 manual validation (`AutoPassed → Validated` via the WebUI
    "Validate" click) → **automatically remove `05:validator`** from
    `consumer_tags[]`. The formerly greyed-out tags become active.
  - **Update-candidate inheritance:** when a new version row is created, seed
    its `consumer_tags[]` as `active_version.consumer_tags[] ∪ {05:validator}`
    (minus any `05:validator` the active version no longer carries). The
    inherited audience is preserved; the validator tag greys it out until the
    candidate passes Step 2.
  - **Derived greyed-out state:** a tag is greyed iff the row also carries
    `05:validator`. No separate "pending tags" column — the same
    `consumer_tags[]` serves both states.
- **4-queue lifecycle (spec §3.5.1):**
  - **Q1 (auto-validation):** formalize the existing `RecipeReviewService`
    auto-path as a named queue. `Pending`/`AutoFailed` rows live here. On pass
    → `AutoPassed` → move to Q2. On fail → `AutoFailed` → move to Q3 (if
    repairable) or Q4 (if unrepairable / structural).
  - **Q2 (manual WebUI validation — spec §7 Q9 resolved, §3.5.2):** **reviewed
    the existing `list_validation_queue` route + `validate_recipe`/
    `validate_tool_skill`/`reject_*`/`request_*_review` endpoints** in
    `crates/brassclaw_webui_v2/src/{router,descriptors,handlers}.rs`. They
    project cleanly onto Q2 (they filter `AutoPassed`/`ReviewRequested`/
    `UpgradeQueued` via `is_queue_status` — exactly Q2's population) but are
    **recipe/tool_skill-specific** — generalize to all ~20 class codes per
    §3.5.2: `PUT /components/{class_code}/{id}/validate` replaces
    `validate_recipe`/`validate_tool_skill`; `PUT /components/{class_code}/{id}/
    reject` replaces `reject_*`; `PUT /components/{class_code}/{id}/send-to-
    revision` replaces `request_*_review`. Extend the `PUT .../validate`
    handler to **pop `05:validator`** on the `AutoPassed → Validated`
    transition. Add a `queue_code` column (derived, stored for query
    convenience) so the WebUI can group items by queue. **LLM code-audit guard
    for class 10/50:** the validate handler checks the LLM-audit-clean flag
    before allowing validation for Orchestrator/Scaffold components — returns
    `403 Forbidden` with `{error: "llm_audit_pending", findings: [...]}` if the
    audit hasn't run or flagged issues. Add `GET /components/{class_code}/{id}/
    audit-status` route for the WebUI to poll audit status. **Q1 visibility:**
    add `?q=auto` filter to `list_validation_queue` for `AutoFailed` items.
    **Q3 tab:** add `?q=revision` filter for `Rejected` with
    `review_attempts < 3`. **Q4 tab:** add `?q=rejection` filter + `DELETE
    /components/{class_code}/{id}` wipe route + `PUT /components/{class_code}/
    {id}/re-review` route. **`is_queue_status` extended** for all 4 queues
    (§3.5.2). **`is_valid_transition` extended** with `AutoFailed → Pending`,
    `Rejected → Pending`, `Rejected → Garbage` (§3.5.2). **`ValidationQueueItem`
    response extended** with `class_code`/`class_label`/`queue_code`/
    `validator_tag_present`/`consumer_tags[]`/`llm_audit_status`/
    `llm_audit_findings` (§3.5.2). **Frontend gets 4 queue tabs** with badge
    counts + tag chip greyed-out rendering (§3.5.2). Old recipe/tool_skill
    routes kept as aliases during migration, removed in Phase 7.
  - **Q3 (revision — spec §7 Q6 resolved):** **new work.** Q3 is automated via
    a **scheduled revision Extension** (class 09, tagged `01:monty`) connected
    to kohai/sempai. The revision mission runs on schedule when the LLM is not
    busy, reads rejected components from Q3, uses the kohai/sempai LLM to
    propose repairs based on `review_feedback`, and re-submits repaired
    candidates to Q1. The revision mission is itself a validated Extension
    (goes through the same two-step validation gate). The
    `is_valid_transition` guard gains `Rejected → Pending` (revision re-submit
    to Q1) when `review_attempts < 3`. After 3 failed review cycles → Q4.
    Operator can also manually send-to-revision via the generalized WebUI
    route (§3.5.2).
  - **Q4 (rejection — spec §7 Q7 resolved):** **new work.** `Rejected` with
    `review_attempts >= 3` + `rejected_at` age < `q4_retention_days` (configurable
    in `reborn_monty_vm_settings`, default 30 — not per-class, one knob is
    sufficient). After the window: either operator re-review (→ Q3, via
    generalized `PUT /components/{class_code}/{id}/re-review` route §3.5.2) or
    **delete + wipe creation-process data** (provenance `source`,
    `similarity_parent_id`, `source_thread_id`, `review_feedback`). **Never
    wipe thread messages/steps/events** (spec §6). Add a background sweeper (or
    a manual WebUI `DELETE /components/{class_code}/{id}` action §3.5.2) that
    performs the wipe; the `is_valid_transition` guard gains `Garbage →
    <deleted>` (a terminal wipe, not a status transition). The wipe guard reads
    `q4_retention_days` from `reborn_monty_vm_settings` instead of a hardcoded
    constant.
- **Queue-state invariant (spec §3.5.1):** `validation_status` +
  `review_attempts` + `rejected_at` age + `05:validator` presence together
  encode the queue. `queue_code` is derived and stored for WebUI grouping.
- **Verify:**
  - Unit test: create a row → `05:validator` present → `fetch_for_consumer`
    returns empty; toggle a non-validator tag → still returns empty (greyed);
    Step-2 validate → `05:validator` popped → `fetch_for_consumer` returns the
    row for the tagged consumers.
  - Update-candidate test: new version row inherits active version's tags +
    `05:validator`; after Step-2 the inherited audience is active.
  - Q1→Q2 transition test (`Pending → AutoPassed`); Q2→Q3 (`AutoPassed →
    Rejected` via `reject_*`); Q3→Q1 (`Rejected → Pending` revision re-submit);
    Q3→Q4 (`review_attempts` reaches 3); Q4 wipe test (row + provenance gone,
    thread data intact).
  - `is_valid_transition` guard tests for all new transitions; grep confirms
    no path where `05:validator` is added/removed outside the queue lifecycle.

### Step 3.6 — Per-class validation configuration (spec §3.5.2, §7 Q14 resolved)
- **Only Skills (classes 01-03) require the full agentskills.io validation**
  (name pattern, description length + actionable, token budget 5000,
  activation criteria, `allowed_tools` presence, `param_schema`/
  `param_template` validation). All other component classes use **lighter
  validation** — name format + description length + content non-empty + soft
  token budget (warnings, not hard errors). Each class's validation thresholds
  are **configurable in the WebUI Settings → Validation tab**.
- **`reborn_validation_config` table** (new migration
  `V38__reborn_validation_config.sql`): `(scope, class_code, name_min_len,
  name_max_len, name_pattern, description_min_len, description_max_len,
  token_budget, token_budget_hard_error, require_tool_name,
  require_param_schema, require_activation_criteria, updated_at)`. One row per
  `(scope, class_code)`. Immediate-write (knobs, not content/code) — changes
  do not retroactively re-validate existing components; they apply to the next
  validation cycle.
- **Defaults per class (seeded in the migration):**
  - **Skills (01-03):** full agentskills.io — `name_pattern = ^[a-z0-9-]+$`,
    `description_min_len = 10`, `token_budget = 5000`,
    `token_budget_hard_error = true`, `require_activation_criteria = true`.
  - **Tools (00):** `token_budget = 5000`, `token_budget_hard_error = true`,
    `require_tool_name = true`, `require_param_schema = true`.
  - **Extensions (04-09):** `token_budget = 10000`,
    `token_budget_hard_error = false`.
  - **Actions (16):** **no token budget** (Actions are exempt from size limits
    — §3.11), `require_activation_criteria = false`.
  - **Former doctypes (12-15, 17-20):** `token_budget = 10000` (Notes: 2000),
    `token_budget_hard_error = false` (soft warning).
  - **Recipes (21):** `token_budget = 10000`, `token_budget_hard_error = false`,
    `require_activation_criteria = true` (trigger validation).
  - **Orchestrator (10) + Scaffold (50):** code validation (LLM audit) —
    `token_budget = 50000`, `token_budget_hard_error = false`.
- **Validator dispatch:** rename `RecipeValidator` to `ComponentValidator`
  (in `crates/brassclaw_engine/src/memory/recipe_validator.rs` — or a new
  `component_validator.rs`). The validator reads the `reborn_validation_config`
  row for the component's `class_code` and applies the corresponding checks.
  Dispatch: `validate_by_class(class_code, component, config_row,
  available_tools, existing_skill_names)`. The existing
  `validate_tool_skill`/`validate_recipe` methods become the Skills/Recipes
  branches of the dispatch; other classes get a lightweight
  `validate_generic` branch that checks name format + description length +
  content non-empty + soft token budget.
- **WebUI Validation tab (Phase 6 Step 6.1):** a sub-panel "Validation Config"
  shows each class code with its current thresholds as editable fields.
  Changes are immediate-write but do not retroactively re-validate existing
  components.
- **Verify:** migration + scope isolation; `ComponentValidator::validate_by_class`
  dispatch test for each class code (Skills get full validation, Tools get
  tool_name + param_schema, Extensions get soft, Actions get no token budget,
  former doctypes get soft, Recipes get trigger validation, Orchestrator/
  Scaffold get LLM audit); config override test (changing `token_budget` in
  `reborn_validation_config` changes the validation outcome for the next
  validation cycle); WebUI Validation Config sub-panel test (edit + save
  thresholds per class).

## Phase 4 — Unified Extensions (Extensions + DocPlans + Recipes)

**Goal:** Merge today's Extensions, DocPlans, and Recipes into
`reborn_extensions_unified` with class enum
(`mcp_server`, `mcp_client`, `rusty`, `monty`, `llm`, `misc`). DocPlans are
**dissected**, not migrated whole. **Recipes get their own class code 21**
(spec §7 Q15 resolved) with a dedicated `reborn_recipes` table — they are
solution-class with a distinct schema (trigger + ordered steps + skill
references) and have `override_prompt_creation` + `prior_knowledge_content`
columns. The `reborn_extensions_unified` class 09 (Misc) retains non-Recipe
Misc extensions only.

### Step 4.1 — Schema & migration
- `V39__reborn_extensions_unified.sql`: table per spec §4. `class` enum check
  constraint. `payload jsonb` carries the manifest / recipe step list / plan
  document body depending on class. `class_code` per spec §3.7
  (04 Rusty, 05 Monty, 06 MCP-Server, 07 MCP-Client, 08 LLM, 09 Misc — **non-
  Recipe Misc only; Recipes go to `reborn_recipes` class 21, spec §7 Q15
  resolved**). `prompt_uid` from a sequence. **`consumer_tags[]`** (spec §3.9)
  with GIN index; seed per class: Rusty → `{00:rusty}`, Monty → `{01:monty,02:orchestrator}`,
  MCP-Server/Client → `{01:monty,02:orchestrator}`, LLM → `{03:llm}`, Misc →
  `{02:orchestrator}`; add `05:validator` until validated. **`prior_knowledge_content
  TEXT NULL`** (§3.13/§3.14 — solution-class override for prior-knowledge
  assembly; SCH-02 fix), **`override_prompt_creation BOOLEAN NOT NULL DEFAULT
  false`** (§3.13/§3.14 — if true, Solution Override path; SCH-02 fix),
  `review_feedback`, `review_attempts`, `rejected_at`, `queue_code` columns.
- **`V40__reborn_recipes.sql`** (new — spec §7 Q15 resolved): `reborn_recipes`
  table per spec §4 (class 21 — solution-class). Columns: `name`, `description`,
  `class_code` (21), `prompt_uid`, **`intent_examples jsonb`** (§3.12),
  **`consumer_tags[]`** (§3.9; default `{02:orchestrator,03:llm}`),
  `trigger jsonb` (trigger condition: type + payload), `steps jsonb` (ordered
  array of `{skill, params}` references to validated ToolSkills/Skills),
  `status`, **`prior_knowledge_content TEXT NULL`** (§3.13/§3.14 — solution-
  class override for prior-knowledge assembly), **`override_prompt_creation
  BOOLEAN NOT NULL DEFAULT false`** (§3.13/§3.14 — if true, Solution Override
  path), `validation_status`, `validation_errors[]`, `review_feedback`,
  `review_attempts`, `rejected_at`, `queue_code`, reward columns (`tier`,
  `usage_count`, `success_count`, `failure_count`, `wilson_lower`), `source`,
  lineage, `content_hash`, timestamps. The `RecipeLookup` trait boundary is
  preserved — `brassclaw_agent_loop` stays free of `brassclaw_engine`.
- **Verify:** migration + scope isolation for both tables; `reborn_recipes`
  has `override_prompt_creation` + `prior_knowledge_content` columns;
  `reborn_extensions_unified` class 09 does NOT include Recipes.

### Step 4.2 — Unified store & class adapters
- New `crates/brassclaw_extensions/src/unified_store.rs`: CRUD over
  `reborn_extensions_unified`.
- Adapters that project a row into the existing shapes consumed by callers:
  - `class=mcp_server|mcp_client` → `ExtensionManifestV2` projection (reuse
    `brassclaw_extensions::v2` validation — fail-closed).
  - `class=rusty` → tool capability projection (feeds Phase 2 surface).
  - `class=monty` → recipe/plan projection (feeds `RecipeStage` /
    `plan_library`).
  - `class=llm` → prompt-template projection.
- **Verify:** projection contract tests per class; manifest v2 contract test
  (`manifest_v2_contract.rs`) passes against the projected row.

### Step 4.3 — Dissect DocPlans + migrate Recipes into `reborn_recipes` (class 21)
- **DocPlans are dissected:** each plan document (`plan_library.rs` `.plan-
  library/{type}/{slug}.md` and `plan-mode` skill's plan `MemoryDoc`s) is
  decomposed into its constituent tools/skills/recipe-steps, which become
  first-class rows in `reborn_skills` / `reborn_tools` /
  `reborn_extensions_unified` / `reborn_recipes`. The plan survives only as a
  thin `monty`-class extension row (the orchestration recipe).
- **Migrate `DocType::Recipe` MemoryDocs into `reborn_recipes` (class 21)**
  (spec §7 Q15 resolved — NOT into `reborn_extensions_unified`): Recipe step
  lists become the `steps jsonb` column; the trigger condition becomes
  `trigger jsonb`; `prior_knowledge_content` + `override_prompt_creation`
  columns are seeded from the Recipe's content. **Migrate `DocType::ToolSkill`
  MemoryDocs into `reborn_tool_skills` (class 13)** — they have their own
  dedicated table (spec §4, Phase 5 migration `V44`). ToolSkills do NOT go into
  `reborn_extensions_unified`. The Phase 5 `component_import.rs` handles this
  migration; Phase 4 does not touch ToolSkills.
- Retire `recipe_store.rs` REST store and `recipe_library.rs` loop adapter in
  favor of the unified store + `reborn_recipes` store; keep the `RecipeLookup`
  trait boundary so `brassclaw_agent_loop` stays free of `brassclaw_engine`
  deps. The `RecipeLookup` trait now reads from `reborn_recipes` (class 21),
  not from `reborn_extensions_unified`.
- `PlanLibraryProcessor` writes to the unified store + `reborn_recipes`.
- **Verify:** `recipe_library.rs` contract tests pass against `reborn_recipes`
  store; `RecipeLookup` trait reads from `reborn_recipes` (class 21); plan-mode
  E2E (`tests/e2e/scenarios/test_plan_mode.py`) passes; Recipes in
  `reborn_recipes` have `override_prompt_creation` + `prior_knowledge_content`
  columns; `reborn_extensions_unified` has NO Recipe rows.

### Step 4.4 — Migrate existing Extensions
- Import installed extensions (`brassclaw_extensions::pg_store`) into
  `reborn_extensions_unified` with `class` derived from `runtime`
  (`mcp` → `mcp_server`/`mcp_client`, `wasm`/`script` → `misc` or `rusty`).
- **Verify:** `extension_contract.rs` + `installations_contract.rs` pass.

## Phase 5 — PlanA-Memory universal connector + de-chunk + DB-less fallback + intent-driven retrieval + Rust formatting + former-doctype tables

**Goal:** Make PlanA-Memory the universal retrieval connector for the turn
(one source of truth: the DB). Remove chunking/embedding/hybrid-search. Add
`RamSource` + baked-in fallback so DB-less mode works with identical
prompt-composition code. **Replace "load all docs" retrieval with intent-driven
retrieval** (§3.13). **Replace Python formatting with Rust-owned class-specific
formatters** (§3.13/§3.14). **Replace the 5-doc limit with a token-budget
limit**. **Migrate former doctypes into first-class component tables** (classes
12-20). **Split large documents into component rows**.

### Step 5.1 — `RetrievalSource` trait + two backends + tag-gated `fetch_for_consumer` + `reborn_component_catalog`
- Promote `RetrievalEngine` to the **universal turn-retrieval interface**: the
  turn asks it for durable memories, selected skills, active tools, active
  extensions, the active orchestrator, scaffold sections, **and Monty VM
  settings**. Same calls regardless of backend.
- **`reborn_component_catalog` read model (PERF-05, §6.1):** a read-only
  materialized view (or `UNION ALL` view) across all component tables. The
  RetrievalEngine queries the catalog by `component_id` +
  `component_class_code` in a **single query** instead of fan-out to 15+
  class-specific tables. Columns: `id`, `scope`, `class_code`, `name`,
  `content`/`prior_knowledge_content`, `override_prompt_creation`,
  `validation_status`, `consumer_tags[]`, `prompt_uid`. Filtered by
  `validation_status = 'Validated' AND '05:validator' != ANY(consumer_tags)`
  at query time (SEC-01). Class-specific tables remain the write path; the
  catalog is read-only (refreshed via trigger or `REFRESH MATERIALIZED VIEW`).
  In DB-less mode, `RamSource` builds an in-memory catalog from the fallback
  file.
- New `RetrievalSource` trait with two impls:
  - `PostgresSource` — production; reads `reborn_skills`/`reborn_tools`/
    `reborn_extensions_unified`/`reborn_recipes` (class 21, spec §7 Q15)/
    `reborn_orchestrators`/`reborn_scaffolds`/
    `reborn_monty_vm_settings`/`reborn_validation_config` (spec §7 Q14)/
    `reborn_user_preferences` (spec §7 Q18 — `ai_before_user` key)/
    `reborn_memory_*`/`reborn_actions`/
    `reborn_intent_inputs`/`reborn_specs`/`reborn_tool_skills`/`reborn_plans`/
    `reborn_summaries`/`reborn_docus`/`reborn_lessons`/`reborn_issues`/
    `reborn_notes`/`reborn_component_catalog` (read model).
  - `RamSource` — DB-less; serves **compiled-in default** skills/tools/
    extensions/orchestrator/scaffold/**Monty VM settings** (baked into the
    binary) + stores thread memories in RAM
    (reuses `InMemoryMemoryDocumentRepository`). **Additionally loads the
    DB-less fallback-content file** (spec §3.4, Step 5.1b) into an in-memory
    index at startup so the RetrievalEngine can perform keyword-based retrieval
    (the pre-v4 path) in DB-less mode. The intent system is overridden in
    DB-less mode — `__resolve_intent__` returns `{db_less_fallback: true}`
    and `fetch_for_turn` falls back to the keyword-retrieval path (load all
    fallback-file entries, score by keyword + type-weight, return top results
    within token budget). The "try it with AI" fallback and "AI before User"
    flip switch are not available in DB-less mode.
- **`fetch_for_consumer(consumer_tag)` (spec §3.9):** the trait exposes a
  consumer-tag-gated fetch — returns only rows that (a) carry the requested
  consumer tag, (b) do **not** carry `05:validator`, (c) are
  `validation_status == Validated`. This is the token-saving mechanism: a
  component tagged `monty`+`orchestrator` but not `llm` never enters the LLM
  prompt. Both backends implement the same filter.
- **Baked-in fallback system prompt + prior-knowledge** ship inside PlanA-
  memory and are served **only** by `RamSource` when no DB is present.
- **Verify:** trait contract test: both backends return the same shapes; a
  DB-less turn composes a prompt from `RamSource` defaults identical to a DB
  turn with the same default rows; **`fetch_for_consumer('03:llm')` excludes
  rows tagged only `01:monty`**; **`fetch_for_consumer` excludes
  `05:validator`-tagged rows**; **DB-less `RamSource` loads the fallback-content
  file and `fetch_for_turn` uses the keyword-retrieval path (not intent-driven)
  when `__resolve_intent__` returns `{db_less_fallback: true}`**.

### Step 5.1b — DB-less fallback-content file (spec §3.4, §7 Q17 resolved)
- **Fallback-content file:** a static file **created at installation time**
  when the user selects not to install a DB (spec §7 Q17 resolved — **not**
  exported from the DB, which is impossible in a DB-less installation).
  Contains **selected compiled-in entries** — most of the skills, tools,
  Plans, Specs, Lessons, etc. — up to a **filesize approximately matching 5
  of the original DocPlans combined** (~256KB, ~50,000 tokens). The file is
  **static**: it does not learn or update during DB-less operation. Learned
  inputs from the intent system are only persisted when a DB is present.
- **Format:** the file is a serialized index of component rows (class code,
  prompt_uid, title, content, consumer_tags, intent_examples) — the same
  shape the `RetrievalSource` trait serves, minus validation/lineage columns
  (the fallback file contains only `Validated`-equivalent content). The
  `RamSource` deserializes it into an in-memory index at startup.
- **RetrievalEngine DB-less path:** when `__resolve_intent__` returns
  `{db_less_fallback: true}`, `fetch_for_turn` falls back to the pre-v4
  keyword-retrieval path: load all fallback-file entries, score by
  `extract_keywords` + `keyword_match_score` + `doc_type_weight` (the original
  `retrieval.rs` logic, preserved for this path), return top results within
  the `prior_knowledge_token_budget`. This means the RetrievalEngine works
  **as it did before the architecture change** in DB-less mode.
- **Compiled-in inclusion priority (spec §7 Q17 resolved):** Tools (class 00)
  → Scaffold (50) → Orchestrator (10) → Skills (01-03) → Extensions (04-09)
  Monty-class first → Recipes (21) → Specs/Lessons (12, 18) →
  Issues/Notes/Summaries (19, 20, 15) **excluded** (volatile/low-value for
  fallback).
- **Verify:** the fallback file loads into `RamSource` at startup; a DB-less
  turn retrieves components via the keyword path (not intent-driven);
  `fetch_for_turn` returns results within `prior_knowledge_token_budget`; the
  fallback file does not grow during DB-less operation (no learning); the
  "try it with AI" fallback + "AI before User" flip switch are unavailable
  (graceful no-op or hidden in the WebUI when DB-less).

### Step 5.1a — `reborn_monty_vm_settings` migration + wiring
- `V41__reborn_monty_vm_settings.sql`: `reborn_monty_vm_settings` table per
  spec §3.10/§4 (scope tuple, single row per scope via upsert — spec §7 Q8
  resolved, columns
  `max_duration_secs` default 300, `max_allocations` default 5_000_000,
  `max_memory_bytes` default 134_217_728, `failure_rollback_threshold` default
  3, `active_orchestrator_id` uuid FK → `reborn_orchestrators.id` nullable,
  **`prior_knowledge_token_budget` default 2000** (spec §3.10/§3.13 — replaces
  the hardcoded 5-doc limit; editable in the WebUI Monty VM tab),
  **`q4_retention_days` default 30** (spec §7 Q7 resolved — Q4 rejection queue
  retention window before terminal wipe; editable in the WebUI Monty VM tab;
  the wipe guard reads this value instead of a hardcoded constant),
  **`forensic_packet_retention_days` default 90** (spec §7 Q21 resolved —
  `brassclaw_forensic_packets` retention window; scheduled daily cleanup task
  deletes packets older than this; set to 0 to disable pruning; editable in
  the WebUI Monty VM tab),
  `updated_at`). CHECK: `max_duration_secs` between 30 and 3600;
  `prior_knowledge_token_budget` > 0; `q4_retention_days` > 0;
  `forensic_packet_retention_days` >= 0.
- `crates/brassclaw_engine/src/executor/orchestrator.rs`: replace the
  compiled-in `orchestrator_limits()` constants with a `RetrievalSource` lookup
  (falls back to compiled-in defaults via `RamSource`). Keep the
  `BRASSCLAW_ORCHESTRATOR_MAX_DURATION_SECS` env override as the DB-less
  fallback when no `RamSource` row is present.
- The `active_orchestrator_id` switch is **gated**: the pointed-to
  `reborn_orchestrators` row must be `Validated` and must not carry
  `05:validator`. A switch to an unvalidated orchestrator is rejected.
- **Verify:** migration + scope isolation; `orchestrator_limits()` reads from
  the DB in production and from compiled-in defaults in DB-less mode; the env
  override still works in DB-less mode; an `active_orchestrator_id` pointing at
  a non-`Validated` row is rejected; `prior_knowledge_token_budget` is read
  from DB in production and defaults to 2000 in DB-less mode.

### Step 5.1a2 — `reborn_user_preferences` migration (spec §3.12, §7 Q18 resolved)
- `V42__reborn_user_preferences.sql`: `reborn_user_preferences` table per
  spec §4 — `(user_id, preference_key, preference_value, updated_at)`. Simple
  key-value store for per-user UX preferences. Composite unique:
  `(user_id, preference_key)`. Current keys: `ai_before_user` (boolean, default
  `false`). **Not exposed in the Settings UI** — these are runtime preferences,
  not operator-managed configurations. Used by the chat-surface "AI before
  User" flip switch (§3.12 rule f-ai). The switch is **hidden/disabled in
  DB-less mode** (no intent system to fall back from).
- The chat surface reads `ai_before_user` on turn start to determine the
  flip-switch state; toggling the switch writes to this table via a
  `PUT /api/chat/preferences/{key}` route (Phase 6 Step 6.1).
- **Verify:** migration + composite unique constraint; `ai_before_user`
  defaults to `false`; the chat surface reads the preference on turn start;
  toggling the switch persists the value; the preference is NOT exposed in the
  Settings UI; DB-less mode hides/disables the switch.

### Step 5.1c — Monty VM lifecycle manager (restart capability)
- **Kernel-owned Monty VM lifecycle manager** (spec §3.10): a new
  `crates/brassclaw_engine/src/executor/monty_lifecycle.rs` module that can
  stop the current Python VM instance, apply new settings from
  `reborn_monty_vm_settings`, and start a fresh instance. This is a
  **kernel-owned runtime operation** — it goes through the kernel, not through
  default.py (which runs inside Monty and cannot restart itself).
- **Restart flow (PERF-16, §6.1 — drain + admission control):**
  1. The WebUI sends `POST /api/settings/monty-vm/restart` (Phase 6 Step 6.1).
  2. The kernel lifecycle manager receives the request.
  3. **Admission control:** the kernel sets an `admission_paused` flag — new
     turns are **queued** (not rejected) with `MontyRestartPending` status.
  4. **Drain:** in-flight turns are allowed to complete or timeout (max
     `max_duration_secs`). If `force=true` is passed (with a WebUI confirmation
     dialog), in-flight turns are aborted immediately.
  5. Once all in-flight turns complete/abort, the lifecycle manager stops the
     current Python VM instance.
  6. It reads the latest `reborn_monty_vm_settings` row (including
     `active_orchestrator_id` + `prior_knowledge_token_budget`).
  7. It starts a fresh Python VM instance with the new settings.
  8. **Resume:** queued turns are admitted in order. The `admission_paused`
     flag is cleared.
  9. The lifecycle manager reports the new status
     (`running`/`draining`/`restarting`/`stopped`/`error`) back to the WebUI.
- **Status polling:** the lifecycle manager exposes a
  `GET /api/settings/monty-vm/status` endpoint returning the current Monty VM
  state (`running`/`restarting`/`stopped`/`error`), the current orchestrator
  version, and the current settings hash (so the WebUI can detect drift
  between DB settings and the running instance).
- **Verify:** restart test (change settings → restart → new settings applied);
  in-flight turn handling test (restart waits for turn completion or aborts
  with `force=true`); status polling test; the lifecycle manager is
  kernel-owned (default.py cannot trigger a restart directly).

### Step 5.2 — Intent-driven retrieval + token-budget limit + `__assemble_prior_knowledge__`
- **Replace "load all docs" with intent-driven retrieval (spec §3.13):** the
  `RetrievalEngine::retrieve_context` path (`retrieval.rs:41` —
  `list_memory_docs_with_shared` then score all) is replaced by
  `fetch_for_turn(query, sender_class_code, token_budget)`:
  1. Call `__resolve_intent__(query, sender_class_code)` (Phase 1.5) →
     resolved component IDs.
  2. Fetch those components by ID from the `RetrievalSource` via the
     **`reborn_component_catalog` read model** (PERF-05, §6.1 — single query
     by `component_id` + `component_class_code`, no fan-out to 15+ tables).
     **SEC-01, §6.1:** the by-ID fetch path filters
     `validation_status = 'Validated' AND '05:validator' != ANY(consumer_tags)`
     — an intent-resolved ID pointing to an in-queue or rejected component is
     silently dropped (orchestrator receives empty result → no-match path).
  3. If the intent system returns no match → the "try it with AI" fallback
     (spec §3.12 rule f-fallback) activates: `extract_keywords` → class-4
     intent matching → fetch matched component IDs. **If the "AI before User"
     flip switch (spec §3.12 rule f-ai) is ON, this fallback activates
     silently** (no reformulate message, no user confirmation, no new
     `reborn_intent_inputs` rows). **If `__resolve_intent__` returns
     `{db_less_fallback: true}` (DB-less mode), skip the intent system
     entirely** and use the keyword-retrieval path against the fallback-content
     file (Step 5.1b) — the "try it with AI" fallback and "AI before User" flip
     switch are not available in DB-less mode.
  4. Assemble prior knowledge from the fetched components, respecting the
     `prior_knowledge_token_budget` from `reborn_monty_vm_settings` (replaces
     the hardcoded 5-doc limit at `default.py:1068`). **Exception: Actions
     (class_code 16) are exempt from token-budget truncation** (spec §3.11) —
     when the RetrievalEngine fetches an Action, its full content is included
     in prior knowledge regardless of the budget. The budget applies to other
     components, not to the Action being executed.
- **`__assemble_prior_knowledge__(goal, token_budget, sender_class_code)` host
  function (spec §3.13 — Q19 RESOLVED: "content is king" + Solution Override):**
  Rust-owned. Calls `__resolve_intent__(goal, sender_class_code)` internally →
  gets matched component IDs → fetches those components → assembles them into
  the prior-knowledge section, truncating to `token_budget`. Returns
  `PriorKnowledgeResult { content, override_prompt_creation, matched_component_ids }`.
  The orchestrator calls this instead of `format_docs` + `append_system_append`.
  **No per-class Rust formatters** — one function + a static class-code→label
  lookup table. Components store their content as the exact prior-knowledge text;
  Rust concatenates in `(class_code asc, prompt_uid asc)` order with per-item
  headers (`### [{class_code}:{CLASS-LABEL}] {name}`). See Step 5.2a for the
  full Solution Override + Normal Assembly design.
- **Token-budget limit (spec §3.13):** the `max_docs` parameter
  (`__retrieve_docs__(goal, 5)`) is replaced by the `prior_knowledge_token_budget`
  from `reborn_monty_vm_settings`. The assembler stops adding components when
  the budget is exhausted, not when a doc count is reached. Components are
  added in `(class_code asc, prompt_uid asc)` order so the most
  foundation-critical content is included first.
- **Verify:** integration test that `fetch_for_turn` returns only
  intent-matched components (not all docs); **SEC-01 test: by-ID fetch
  drops in-queue/rejected components** (intent-resolved ID → validation
  gate filter → empty result if not Validated); **PERF-05 test:
  `reborn_component_catalog` single-query fetch** (no fan-out to 15+
  class-specific tables); token-budget truncation test
  (components beyond budget are excluded); "try it with AI" fallback test
  (no intent match → keyword fallback → components fetched); **"AI before
  User" flip switch test** (ON: silent keyword fallback, no reformulate
  message, no new `reborn_intent_inputs` rows; OFF: default behavior);
  **DB-less fallback test** (`__resolve_intent__` returns
  `{db_less_fallback: true}` → keyword-retrieval path against fallback file,
  no intent system, no "try it with AI" / "AI before User"); the
  `__assemble_prior_knowledge__` host function returns correctly formatted
  prior knowledge within the token budget.

### Step 5.2a — Prior knowledge formatting (spec §3.13/§3.14 — Q19 RESOLVED: "content is king" + Solution Override)
- **Delete Python formatters** (spec §2.3b): `format_docs` (`default.py:234`),
  `format_skills` (`default.py:932`), `append_system_append`
  (`default.py:270`). These are no longer needed — the orchestrator does not
  format.
- **Q19 Resolution — "content is king" + Solution Override (spec §3.13/§3.14):**
  components store their content as the exact prior-knowledge text. Rust does
  NOT "format" — it concatenates `content`/`prior_knowledge_content` fields in
  `(class_code asc, prompt_uid asc)` order with a per-item header. No per-class
  Rust formatters (the original v5 design's 13 formatters are eliminated). One
  `__assemble_prior_knowledge__` function + a static class-code→label lookup
  table.
- **Two assembly paths:**
  - **Solution Override path:** if the intent system matches to a single
    solution-class component (Extension/Plan/Recipe/Action) with
    `override_prompt_creation: true`, `__assemble_prior_knowledge__` returns
    `prior_knowledge_content` (or `content` if PKC is NULL) as the COMPLETE
    prompt text — no headers, no section wrapper. Sets
    `override_prompt_creation: true` in the return shape. The orchestrator
    skips the LLM call (Actions) or uses the content as the full prompt
    (Plans/Extensions). **Actions default to `override_prompt_creation: true`**
    since they skip the LLM call.
  - **Normal Assembly path:** if multiple components are matched or no
    override flag is set, assembles components in `(class_code asc, prompt_uid
    asc)` order. Each component contributes its `prior_knowledge_content` (or
    `content` if PKC is NULL) under a per-item header:
    ```
    ## Prior Knowledge

    ### [00:TOOL] {name}
    {prior_knowledge_content or content}

    ### [01:SKILL-RUSTY] {name}
    {prior_knowledge_content or content}
    ```
    The `## Prior Knowledge` section header appears once. The `###` per-item
    header uses `{class_code}:{CLASS-LABEL}` from a static Rust lookup table
    (`00`→`TOOL`, `01`→`SKILL-RUSTY`, `02`→`SKILL-MONTY`, `03`→`SKILL-LLM`,
    `04`→`EXT-RUSTY`, `05`→`EXT-MONTY`, `06`→`EXT-MCP-SERVER`,
    `07`→`EXT-MCP-CLIENT`, `08`→`EXT-LLM`, `09`→`EXT-MISC`,
    `10`→`ORCHESTRATOR`, `11`→`RESERVED`, `12`→`SPEC`, `13`→`TOOLSKILL`,
    `14`→`PLAN`, `15`→`SUMMARY`, `16`→`ACTION`, `17`→`DOCU`, `18`→`LESSON`,
    `19`→`ISSUE`, `20`→`NOTE`, `21`→`RECIPE`, `50`→`SCAFFOLD`). Sets
    `override_prompt_creation: false`.
- **`__assemble_prior_knowledge__` return shape:**
  `PriorKnowledgeResult { content: String, override_prompt_creation: bool,
  matched_component_ids: Vec<Uuid> }`.
- **DB columns added to solution-class tables** (Extensions, Plans, Recipes,
  Actions): `prior_knowledge_content TEXT NULL` (overrides `content` for
  prior-knowledge assembly) + `override_prompt_creation BOOLEAN NOT NULL
  DEFAULT false` (Actions default `true`). Non-solution classes do NOT have
  these columns — they always use `content` and are always assembled normally.
- **Orchestrator step-0 block (spec §3.13):**
  ```python
  if step == 0:
      token_budget = config.get("prior_knowledge_token_budget", 2000)
      result = __assemble_prior_knowledge__(goal, token_budget, "02")
      if result.override_prompt_creation:
          # Solution Override — use result.content as the complete prompt
          working_messages = [{"role": "User", "content": result.content}]
      elif result.content:
          # Normal Assembly — inject as User message at N-1
          insert_as_user_message_at_n_minus_1(working_messages, result.content)
  ```
- **What is certain (fully defined):**
  - The orchestrator formatting ban holds — `default.py` does not format.
  - `__assemble_prior_knowledge__` is a Rust host function (one function, no
    per-class formatters).
  - KV-cache discipline requires deterministic assembly order.
  - Actions (class 16) are exempt from token-budget truncation.
  - The self-improvement mission CAN tune content by patching
    `content`/`prior_knowledge_content` fields through the validation gate.
    It CANNOT patch the assembly mechanism (Rust).
- **Verify:** unit tests for `__assemble_prior_knowledge__` covering both
  paths (Solution Override + Normal Assembly); integration test that
  `__assemble_prior_knowledge__` produces correctly assembled output for a
  mix of component classes; the orchestrator (`default.py`) has no
  `format_docs`/`format_skills`/`append_system_append` calls (grep confirms
  deletion); the System message is byte-identical across turns with the same
  static components; volatile prior-knowledge is in a User message at N-1;
  **Solution Override test** (single matched Action with
  `override_prompt_creation: true` → return shape has
  `override_prompt_creation: true`, content is PKC/content with no headers,
  orchestrator skips LLM call); **Normal Assembly test** (multiple matched
  components → return shape has `override_prompt_creation: false`, content has
  `## Prior Knowledge` + per-item `###` headers in `(class_code asc,
  prompt_uid asc)` order); **PKC NULL fallback test** (solution component with
  `prior_knowledge_content: NULL` → uses `content` for assembly); **Actions
  exempt from token-budget truncation test** (Action content included in full
  regardless of budget); **non-solution classes do NOT have PKC/override
  columns** (migration check).

### Step 5.2b — Prompt composition: volatile memories as User message at N-1
- Wire the resurrected path (`deduplication-plan.md` Finding 1 Option A, spec
  §3.7): prior-knowledge (volatile memories) is injected as a **User message
  at N-1**, not appended to the System message. Static DB objects stay in the
  System prefix, ordered by `(class_code asc, prompt_uid asc)`.
- This is the implementation of the Phase 1.5 design decision — blocked until
  Phase 1.5 design is approved, **unless** Phase 5 ships with the current
  System-append path behind a flag and flips to User-at-N-1 when 1.5 lands.
- **Verify:** integration test that `ExecutionLoop::run` with a populated
  `Store` puts prior-knowledge docs in a **User** message at position N-1 and
  the System message is byte-identical across two turns with different
  retrieval results.

### Step 5.3 — Former-doctype component tables (classes 12-20) + document splitting
- Add migrations for the 9 former-doctype tables per spec §4 (scope tuple,
  unique `(scope, name)`, `class_code`, `prompt_uid`, `title`, `content`,
  `intent_examples jsonb`, `consumer_tags[]`, validation/lineage columns as
  per skills — spec §7 Q14 on lighter validation for non-skill doctypes):
  - `V43__reborn_specs.sql` (class 12, `consumer_tags[]` = `{02:orchestrator,03:llm}`)
  - `V44__reborn_tool_skills.sql` (class 13, `consumer_tags[]` = `{00:rusty,01:monty,02:orchestrator}`)
  - `V45__reborn_plans.sql` (class 14, `consumer_tags[]` = `{01:monty,02:orchestrator}`)
  - `V46__reborn_summaries.sql` (class 15, `consumer_tags[]` = `{02:orchestrator,03:llm}`)
  - `V47__reborn_docus.sql` (class 17, `consumer_tags[]` = `{02:orchestrator,03:llm}`)
  - `V48__reborn_lessons.sql` (class 18, `consumer_tags[]` = `{02:orchestrator,03:llm}`)
  - `V49__reborn_issues.sql` (class 19, `consumer_tags[]` = `{02:orchestrator,03:llm}`)
  - `V50__reborn_notes.sql` (class 20, `consumer_tags[]` = `{02:orchestrator}`)
  - (Actions class 16 — `reborn_actions` already created in Phase 1.6.)
- **Document splitting + content conversion (spec §3.13):** one-shot importer
  `crates/brassclaw_reborn_composition/src/component_import.rs`:
  - Walk all `MemoryDoc` rows with `DocType::Spec`/`ToolSkill`/`Plan`/
    `Summary`/`Lesson`/`Issue`/`Note` and migrate each into the corresponding
    new table as a first-class component row.
  - **Split large documents** into smaller, more specialized component rows so
    that the prior-knowledge assembly can select only the relevant pieces
    (token savings). Each split row gets its own `prompt_uid` + `intent_examples`.
  - Extract `intent_examples` from each document's title, tags, and content
    sentences (class 1/2/3 per §3.12).
  - Idempotent via `content_hash`.
- **Verify:** migration + scope isolation for each table; importer test that
  every `MemoryDoc` of a retired `DocType` is migrated; split-row test that
  large documents are broken into ≤5000-token rows; intent_examples non-empty
  for every migrated row.

### Step 5.4 — Remove chunk/embedding machinery
- Delete `crates/brassclaw_memory/src/chunking.rs`,
  `indexer.rs` chunk paths, `search.rs` hybrid fusion, `embedding.rs`
  memory-retrieval wiring. Update `lib.rs` exports.
- **Fully remove `brassclaw_embeddings`** (spec §7 Q3 resolved): delete the
  crate, its dependencies, and all embedding-based search paths. The intent
  system (§3.12) replaces all runtime similarity/search needs. Install-time
  dedup uses content-hash + exact-name uniqueness constraint (no embedding
  similarity needed).
- Stop writing chat chunks (`pg_chat_memory_record_store.rs`,
  `chat_memory.rs`, `MemoryChunkWrite.chat_record_id`); chat records become
  flat `Note` component rows in `reborn_notes` (class 20, spec §7 Q4 resolved)
  with no embedding index, retrieved by the intent system or project-scope
  lookup like any other component.
- **Verify:** `cargo test -p brassclaw_memory` passes with chunk tests removed
  or converted to document-level equivalents; clippy clean.

### Step 5.5 — Retire ALL DocTypes (full DocType retirement)
- **Remove ALL `DocType` variants** from the `DocType` enum
  (`crates/brassclaw_engine/src/types/memory.rs`): `Skill`, `Recipe`,
  `ToolSkill`, `Plan`, `Summary`, `Lesson`, `Issue`, `Spec`, `Note`. They now
  live in `reborn_skills` / `reborn_extensions_unified` / `reborn_actions` /
  `reborn_specs` / `reborn_tool_skills` / `reborn_plans` / `reborn_summaries` /
  `reborn_docus` / `reborn_lessons` / `reborn_issues` / `reborn_notes`.
- The `DocType` enum itself is **deleted**. The `MemoryDoc` struct is reduced
  to volatile-only fields (thread-scoped memories not yet promoted to
  component rows). The `reborn_memory_*` tables are reduced to volatile
  thread memory (content that changes per-turn) — all durable content is in
  the class-specific tables.
- Update `brassclaw_engine::memory` modules (`recipe_matcher`,
  `recipe_validator`, `similarity_checker`, `skill_tracker`) to read from the
  new tables instead of `MemoryDoc` metadata.
- Update `RetrievalEngine` (`retrieval.rs`): delete `doc_type_weight`
  (priority is now encoded in `class_code` ordering); delete
  `keyword_match_score` (replaced by intent system matching); delete
  `extract_keywords` (moved to the intent system's class-4 fallback path).
  **Exception:** the DB-less fallback path (Step 5.1b) still uses
  `extract_keywords` + `keyword_match_score` + `doc_type_weight` against the
  fallback-content file. These functions are **relocated** to a DB-less-only
  module (e.g. `retrieval_dbless.rs`), not deleted entirely — they are the
  RetrievalEngine's fallback when `__resolve_intent__` returns
  `{db_less_fallback: true}`. The DB-mode `RetrievalEngine` does not call
  them.
- Update `context.rs`: `build_step_context` now calls
  `fetch_for_turn` + `__assemble_prior_knowledge__` instead of
  `retrieve_context` + `format_docs_as_context`.
- **Verify:** engine v2 skill codeact test (`engine_v2_skill_codeact.rs`)
  passes with the new data sources; `cargo test -p brassclaw_engine` passes;
  grep confirms no remaining `DocType::` references in production code;
  grep confirms `doc_type_weight`/`keyword_match_score`/`extract_keywords`
  are gone from the DB-mode `retrieval.rs` path (relocated to
  `retrieval_dbless.rs` for the DB-less fallback).

## Phase 5.5 — Interceptor activation (Sempai–Kohai forensic audit + rerouting)

**Goal:** Wire the interceptor so it actually works. The infrastructure
(`ForensicPacket`, `InterceptorStage`, `LoopInterceptorPort` trait,
`ProviderRole::Sempai`, `set_active`, `sempai_swappable` scaffold,
`SharedInterceptorMode`) already exists. Five wiring gaps prevent it from
functioning. This phase closes all five and adds the Sempai gateway + 3-part
prompt + KV-cache pre-warm + interceptor config WebUI tab.

**Depends on Phase 5:** the interceptor uses
`PriorKnowledgeResult.matched_component_ids` for the Part C component manifest.
Phase 5 creates this. The interceptor's `reassemble_base_prompt()` uses direct
SQL to individual component tables (Q20 — NOT `reborn_component_catalog`); this
keeps the interceptor independent of the catalog's refresh timing and schema.

**Adapted from `interceptor2.md`:** the original plan used direct SQL for
`reassemble_base_prompt()` — our plan retains this approach (Q20). The
unification with the PlanA-System is at the **storage** layer
(ForensicPacket stores `component_refs` — IDs only, not full prompt content —
preventing double-saving), NOT at the **retrieval** layer. The original plan
stored full `prompt JSONB` in ForensicPacket; our plan stores
`component_refs JSONB` (array of `{class_code, prompt_uid, component_id}`) +
`volatile_tail TEXT` only — prevents double-saving (the prompt content is
already in the DB as component rows).

### Step 5.5.0 — `brassclaw_forensic_packets` migration (V51)
- `V51__brassclaw_forensic_packets.sql`: create table per spec §4/§3.15.
  Schema adapted from interceptor2.md: replace `prompt JSONB` with
  `component_refs JSONB` (NOT NULL — array of
  `{class_code, prompt_uid, component_id}` from
  `PriorKnowledgeResult.matched_component_ids`) + `volatile_tail TEXT` (thread
  history only). Keep `kohai_response`/token columns, `sempai_review JSONB`,
  `chat_record_id`, `status` (`awaiting_kohai`/`complete`/`sempai_reviewed`),
  `captured_at`/`completed_at`/`updated_at`. PK `id`, unique
  `(tenant_id, run_id, iteration)`. Indexes:
  `(tenant_id, captured_at DESC)`, `(tenant_id, run_id, iteration)`.
- Fix stale reference in `pg_store.rs` module doc: `V026` → `V051`.
- **Verify:** migration applies cleanly after V50; `cargo test -p brassclaw_interceptor`; integration test (embedded Postgres applies V34–V51 in order).

### Step 5.5.1 — `InterceptorResult` trait change (breaking, additive elsewhere)
- **`crates/brassclaw_turns/src/run_profile/host.rs`:** add `InterceptorResult`
  struct: `{ packet_id: String, adjusted_messages: Option<Vec<(String, String)>> }`.
  Change `on_prompt_assembled` return from `Option<String>` to
  `Option<InterceptorResult>`. `NoInterceptor` default returns `None`.
  `adjusted_messages` is `Vec<(role, content_text)>` — resolved text, not refs.
  The host resolves refs from the `InstructionMaterializationStore` before
  calling Sempai; adjusted text comes back as plain strings (no ref-resolution
  needed again).
- **`crates/brassclaw_agent_loop/src/executor/interceptor.rs`:** add
  `adjusted_messages: Option<Vec<(String, String)>>` field to
  `InterceptorPromptOutput`. When `on_prompt_assembled` returns
  `Some(result)`: extract `packet_id` + `adjusted_messages`.
- **`crates/brassclaw_agent_loop/src/executor/canonical.rs`:** when
  `interceptor_out.adjusted_messages` is `Some(pairs)`, convert
  `Vec<(String, String)>` → `Vec<HostManagedModelMessage>` and pass to
  `ModelStage` directly, bypassing `resolve_model_messages`. When `None`,
  forward `interceptor_out.messages` (refs) unchanged — existing Kohai path.
  Add `resolved_messages: Option<Vec<HostManagedModelMessage>>` to `ModelInput`;
  in **`crates/brassclaw_loop_support/src/lib.rs`**
  (`ThreadBackedLoopModelPort::stream_model`), if `resolved_messages` is
  `Some`, skip `resolve_model_messages`.
- **`crates/brassclaw_interceptor/src/packet.rs`:** update `SempaiReviewOutcome`:
  replace `adjusted_messages` with `adjusted_volatile_messages` +
  `bridge_messages` + `composition_summary` + `proposed_recipe_updates` +
  `proposed_intent_examples` (Q30) + `settings_adjustments` (spec §3.15).
- **Update 6 test stub files** (mechanical return-type update):
  `brassclaw_agent_loop/src/executor/tests/support.rs`,
  `brassclaw_agent_loop/src/test_support/mod.rs`,
  `brassclaw_turns/tests/agent_loop_host_contract.rs`,
  `brassclaw_reborn/src/planned_driver.rs`,
  `brassclaw_reborn/tests/planned_driver_e2e.rs`,
  `brassclaw_reborn/src/turn_runner/tests/mod.rs`.
- **Verify:** `cargo clippy --all -- -D warnings` clean; existing tests pass.

### Step 5.5.2 — Wire `PgInterceptorStore` + allocate `sempai_swappable` + create `SharedInterceptorMode`
- **`crates/brassclaw_reborn_composition/src/runtime.rs` (line ~1974):**
  replace `interceptor_store: None` with:
  ```rust
  #[cfg(feature = "postgres")]
  interceptor_store: services.pg_pool.as_ref().map(|pool| {
      Arc::new(brassclaw_interceptor::PgInterceptorStore::new(
          Arc::clone(pool),
          validated_identity.tenant_id.as_str(),
      )) as Arc<dyn brassclaw_interceptor::InterceptorStore>
  }),
  ```
- **Allocate `sempai_swappable`** in `wrap_swappable_gateway`:
  ```rust
  let sempai_inner: Arc<dyn LlmProvider> = Arc::new(PlaceholderLlmProvider);
  let sempai_swappable = Arc::new(SwappableLlmProvider::new(sempai_inner));
  ```
  Replace `sempai_swappable: None` in `RebornLlmReloadParts` with
  `sempai_swappable: Some(Arc::clone(&sempai_swappable))`. Remove
  `#[allow(dead_code)]`.
- **Create `SharedInterceptorMode`**:
  ```rust
  #[cfg(feature = "root-llm-provider")]
  let interceptor_mode = brassclaw_interceptor::SharedInterceptorMode::new();
  ```
  Add `interceptor_mode: Option<SharedInterceptorMode>` to
  `DefaultPlannedRuntimeParts`. Wire through `build_default_planned_runtime`:
  `host_factory.with_interceptor_mode(mode)`. Add field + builder to
  `RebornLoopDriverHostFactory`. Thread into `RebornLoopDriverHost`. Carry on
  `RebornRuntime` (cfg-gated), exposed via accessor for Step 5.5.3.
- **Feature gate:** store is `#[cfg(feature = "postgres")]`; mode flag +
  live-swap are `#[cfg(all(feature = "postgres", feature = "root-llm-provider"))]`.
  The `postgres`-only path (store wired, mode always `Routing`) is valid
  forensic-logging-only mode. `root-llm-provider` without `postgres` must not
  compile the rerouting branch.
- **DB-less mode:** `interceptor_store: None`, `sempai_swappable: None`,
  `interceptor_mode: None`. The `on_prompt_assembled` hook is a no-op.
- **Verify:** `cargo clippy -p brassclaw_reborn -p brassclaw_reborn_composition -- -D warnings` clean.

### Step 5.5.3 — `set_active(Sempai)` live-swap + mode flip
- **`crates/brassclaw_reborn_composition/src/llm_config_service.rs`:** add
  `sempai_swappable: Option<Arc<SwappableLlmProvider>>` +
  `interceptor_mode: Option<SharedInterceptorMode>` fields (cfg-gated) +
  `with_sempai_swappable`/`with_interceptor_mode` builders. Wire from WebUI
  facade composition using accessors on `RebornRuntime`.
- **Extend `ProviderRole::Sempai` arm:** after existing DB write, add live-swap:
  ```rust
  #[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
  if let Some(swappable) = &self.sempai_swappable {
      let new_provider = if id.is_empty() {
          Arc::new(PlaceholderLlmProvider)
      } else {
          match self.build_sempai_provider(&id, request.model.as_deref()).await {
              Ok(p) => p,
              Err(e) => { tracing::debug!(%e, "sempai build failed; mode stays Routing"); return self.build_snapshot().await; }
          }
      };
      swappable.swap(new_provider);
      if let Some(mode) = &self.interceptor_mode {
          if id.is_empty() { mode.set_routing(); } else { mode.set_rerouting(); }
      }
  }
  ```
- **`build_sempai_provider`:** reuses `brassclaw_llm::build_static_provider_chain`
  exactly as Kohai. Reads stored API key from `self.keys.read(&provider_id)`.
  Returns `Result<Arc<dyn LlmProvider>, String>`. No streaming session manager —
  Sempai audit is a plain `CompletionRequest`.
- **Verify:** `cargo clippy -p brassclaw_reborn_composition -- -D warnings`; `cargo test -p brassclaw_reborn_composition`.

### Step 5.5.4 — Sempai gateway + rerouting branch + 3-part prompt + KV-cache pre-warm
- **4a. Sempai gateway:** wrap `sempai_swappable` in its own
  `LlmProviderModelGateway`. Pass through
  `DefaultPlannedRuntimeParts.sempai_gateway` → `host_factory.with_sempai_gateway(...)`. Add field + builder to `RebornLoopDriverHostFactory`. Thread into `RebornLoopDriverHost`.
- **4b. Rerouting branch in `on_prompt_assembled`**
  (`crates/brassclaw_reborn/src/loop_driver_host.rs` ~line 1801):
  - **Routing path** (mode == Routing OR sempai_gateway is None): save
    ForensicPacket (existing code, adapted to store `component_refs` from
    `matched_component_ids` + `volatile_tail` instead of full `prompt JSONB`),
    return `Some(InterceptorResult { packet_id, adjusted_messages: None })`.
  - **Rerouting path** (mode == Rerouting AND sempai_gateway is Some):
    1. Resolve snapshot's message refs to text using
       `InstructionMaterializationStore` + thread context. Produces
       `Vec<(role, content_text)>`.
    2. Build the Sempai audit prompt (3-part — §4c below).
    3. Call `sempai_gateway.stream_model(audit_request).await`. On error:
       `tracing::debug!`, fall back → `adjusted_messages: None`.
    4. Parse response as `SempaiReviewOutcome` JSON. On parse failure: fall
       back → `adjusted_messages: None`.
    5. Update ForensicPacket: `sempai_review = Some(outcome)`,
       `status = SempaiReviewed`. Save.
    6. **Route `proposed_recipe_updates` + `proposed_intent_examples` to Q1
       validation queue** (Phase 3's `ComponentValidator`) — the Sempai cannot
       directly create components or intent inputs. `proposed_intent_examples`
       (Q30) are new `intent_examples` entries the Sempai suggests for existing
       components; once validated, they are added to the component's
       `intent_examples` and seeded into `reborn_intent_inputs` by the
       validator.
    7. Return `Some(InterceptorResult { packet_id, adjusted_messages: Some(recomposed) })`.
  - **Actions bypass:** Actions are Python-only — when the intent system
    matches an Action with `override_prompt_creation: true`, the prompt
    creation process is **disrupted**: the orchestrator does not proceed to
    `__assemble_prior_knowledge__` → interceptor → `__llm_complete__`.
    Instead, it dispatches the Action's steps directly (§3.11). The
    interceptor's hook point (`on_prompt_assembled`) is never reached for
    Action-only turns — the interceptor **cannot intercept** because there
    is no prompt to assemble and no LLM call to adjust. No ForensicPacket is
    created. This is a structural consequence, not a design choice.
- **4c. 3-part Sempai prompt (spec §3.15):**
  - **Part A (static base, KV-cache prefix):** from `brassclaw_config` key
    `interceptor.sempai_base_prompt`. Assembled by
    `reassemble_base_prompt()` (Step 5.5.5) using **direct SQL to individual
    component tables** (Q20 — NOT `reborn_component_catalog`) — queries all
    `Validated` components in `(class_code asc, prompt_uid asc)` order, filter
    `validation_status = 'Validated' AND '05:validator' != ANY(consumer_tags)`.
    Includes Orchestrator (class 10) — Sempai needs it for audit (unlike
    Kohai prompt). Manual rebuild, not per-turn.
  - **Part B (persona):** from `brassclaw_config` key
    `interceptor.sempai_persona`. Default from
    `crates/brassclaw_engine/prompts/sempai_audit.md` via `include_str!()`.
    Editable in WebUI interceptor tab.
  - **Part C (per-turn volatile tail + component manifest):** derived from
    `PriorKnowledgeResult.matched_component_ids` — the component manifest is
    `{class_code}:{prompt_uid}  {type}  "{name}"` per matched component.
    Static components are stripped (already in Part A). Volatile tail (thread
    history + inline nudges) remains. Stored in ForensicPacket
    `component_refs` + `volatile_tail`.
  - **Recomposition:** stable-base messages (Part A content) +
    `outcome.bridge_messages` + `outcome.adjusted_volatile_messages` →
    complete adjusted Kohai prompt. `ModelStage` forwards to Kohai (KV-cache
    hit on stable prefix + Sempai-adjusted volatile tail).
- **4d. KV-cache pre-warm (manual button):**
  - `POST /api/interceptor/prewarm` endpoint. Handler reads
    `interceptor.sempai_base_prompt` from config, sends as single system
    message to Sempai, discards response. Writes
    `interceptor.sempai_prewarm_last_at = now()`.
  - Rate-limit: 1 request/minute/caller. `429 Too Many Requests` with
    `retry_after_seconds: 60` on exceed. WebUI spinner handles `429`
    explicitly.
- **4e. Interceptor config service** (Step 5.5.5 below).
- **4f. `on_kohai_response`:** no change — existing implementation reads
  packet by id and calls `packet.with_kohai_response(...)` regardless of
  mode.
- **Verify:** `cargo clippy --all -- -D warnings`; `cargo test -p brassclaw_agent_loop -p brassclaw_reborn -p brassclaw_interceptor -p brassclaw_reborn_composition`; integration test: configure Sempai mock → mode flips → full turn → Kohai receives Sempai-adjusted messages → packet `status = sempai_reviewed` → `component_refs` present in ForensicPacket; integration test: Sempai error → Kohai receives original messages → packet `status = complete`; integration test: `set_active(Sempai, "")` clears → mode flips to Routing; integration test: Action-only turn → no ForensicPacket (interceptor bypassed); integration test: `proposed_recipe_updates` → Q1 validation queue (component enters Q1, not production tables).

### Step 5.5.5 — Interceptor config service + `reassemble_base_prompt()` via direct SQL
- **`crates/brassclaw_product_workflow/src/reborn_services/interceptor_config.rs`** (new):
  `InterceptorConfigService` trait with `snapshot()`, `update()`,
  `reassemble_base_prompt()`, `prewarm()`. Add
  `Option<Arc<dyn InterceptorConfigService>>` to `RebornServicesApi`.
  `InterceptorConfigSnapshot { sempai_connected, mode, base_prompt_assembled_at, base_prompt_size_chars, persona, prewarm_last_at }`.
- **`crates/brassclaw_interceptor/src/config_store.rs`** (new):
  `InterceptorConfigStore` trait with `load()`/`save()`. Backed by existing
  `brassclaw_config` table (no new migration). Keys:
  `interceptor.sempai_base_prompt`,
  `interceptor.sempai_base_prompt_assembled_at`,
  `interceptor.sempai_persona`, `interceptor.sempai_prewarm_last_at`.
- **`crates/brassclaw_reborn_composition/src/interceptor_config_service.rs`** (new):
  `RebornInterceptorConfigService` holds `Arc<PgPool>`,
  `Arc<SharedInterceptorMode>`, `Arc<dyn HostManagedModelGateway>` (sempai_gateway).
- **`reassemble_base_prompt()` — uses direct SQL to individual component
  tables (Q20 — NOT `reborn_component_catalog`):**
  1. Query each component table (reborn_tools, reborn_skills,
     reborn_extensions_unified, reborn_recipes, reborn_orchestrators,
     reborn_scaffolds, reborn_actions, reborn_specs, reborn_tool_skills,
     reborn_plans, reborn_summaries, reborn_docus, reborn_lessons,
     reborn_issues, reborn_notes) for all rows where
     `validation_status = 'Validated' AND '05:validator' != ANY(consumer_tags)`.
  2. Merge results from all tables, sort by `(class_code asc, prompt_uid asc)`.
  3. Serialize each row into Part A: one block per row with
     `{class_code}:{prompt_uid}` header + full content text.
  4. Append `SempaiReviewOutcome` JSON schema as literal `include_str!()` block.
  5. Write assembled string to `brassclaw_config` key
     `interceptor.sempai_base_prompt` + timestamp to
     `interceptor.sempai_base_prompt_assembled_at`.
  6. Return updated snapshot.
  - **Direct SQL is acceptable here** because this is a manual rebuild (not
    per-turn). The fan-out cost (15+ queries) is paid once per Reassemble
    button click. Using direct SQL keeps the interceptor independent of the
    `reborn_component_catalog` read model (which is the RetrievalEngine's
    interface, PERF-05). The unification with the PlanA-System is at the
    **storage** layer (ForensicPacket `component_refs`), not the retrieval
    layer.
  - **Missing-table guard:** before querying each component table, check
    `information_schema.tables` for existence. Tables from earlier phases
    (reborn_tools V37, reborn_skills V34) will exist; tables from later
    phases may not yet exist if the interceptor ships incrementally. Skip
    non-existent tables (empty result) rather than erroring. This makes
    `reassemble_base_prompt()` resilient to partial phase rollouts.
- **`prewarm()`:** delegates to the `POST /api/interceptor/prewarm` handler
  logic (Step 5.5.4d).
- **HTTP endpoints** (`crates/brassclaw_webui_v2/src/descriptors.rs` +
  `crates/brassclaw_webui_v2/src/router.rs`):
  `GET /api/interceptor/config`, `POST /api/interceptor/config`,
  `POST /api/interceptor/reassemble`, `POST /api/interceptor/prewarm`.
  All require `WebUiAuthenticatedCaller` bearer token. `reassemble` and
  `prewarm` are synchronous with 120s/60s timeout, rate-limited 1/min/caller.
- **Wire `RebornInterceptorConfigService` into WebUI facade**
  (`crates/brassclaw_reborn_composition/src/webui.rs`): add
  `Option<Arc<dyn InterceptorConfigService>>` to `RebornServicesApi` and
  thread from the composition into the WebUI ingress handlers.
- **`crates/brassclaw_webui_v2_static/pages/settings/interceptor/`** (new):
  Settings tab — Sempai status display, Reassemble button (calls
  `POST /api/interceptor/reassemble`, shows spinner), Pre-warm button (calls
  `POST /api/interceptor/prewarm`, handles `429` with "wait 60s" message),
  persona editor textarea (save calls `POST /api/interceptor/config`).
  `node --check` for syntax validation (no build step).
- **`crates/brassclaw_engine/prompts/sempai_audit.md`** (new): default persona
  text (Part B), loaded via `include_str!()`.
- **ForensicPacket cleanup task (Q21):** a scheduled daily task in
  `brassclaw_reborn_composition` that reads `forensic_packet_retention_days`
  from `reborn_monty_vm_settings` and deletes `brassclaw_forensic_packets` rows
  older than the retention window (`WHERE captured_at < now() - interval '1
  day' * forensic_packet_retention_days`). When
  `forensic_packet_retention_days = 0`, the task is a no-op (pruning disabled).
  The task runs via the existing scheduler infrastructure (same mechanism as
  the Q3 revision Extension's scheduled runs). Uses the
  `(tenant_id, captured_at DESC)` index for efficient deletion batches.
- **Verify:** `cargo clippy -p brassclaw_product_workflow -p brassclaw_webui_v2 -p brassclaw_reborn_composition -p brassclaw_interceptor -- -D warnings`; `cargo test -p brassclaw_product_workflow -p brassclaw_reborn_composition`; integration test: `POST /api/interceptor/reassemble` → Part A written to `brassclaw_config` from direct SQL to individual component tables; integration test: `POST /api/interceptor/prewarm` with empty base prompt → `400`; with assembled prompt → `200` + timestamp updated; **integration test: ForensicPacket cleanup task deletes packets older than `forensic_packet_retention_days`** (insert old packet → run task → packet deleted; set `forensic_packet_retention_days = 0` → run task → no deletion); `tests/webui_v2_descriptors_contract.rs` updated with 4 new descriptors.

### Phase 5.5 — File Change Summary (consolidated from interceptor2.md)

| File | Change |
|---|---|
| `migrations/V51__brassclaw_forensic_packets.sql` *(new)* | Create `brassclaw_forensic_packets` table (adapted: `component_refs JSONB` + `volatile_tail TEXT` instead of `prompt JSONB`) |
| `crates/brassclaw_interceptor/src/pg_store.rs` | Fix module doc: `V026` → `V051` |
| `crates/brassclaw_interceptor/src/packet.rs` | Replace `adjusted_messages` with `adjusted_volatile_messages` + `bridge_messages` + `composition_summary` + `proposed_recipe_updates` + `proposed_intent_examples` (Q30) + `settings_adjustments` in `SempaiReviewOutcome` |
| `crates/brassclaw_interceptor/src/config_store.rs` *(new)* | `InterceptorConfigStore` trait + Pg impl; 4 config keys in `brassclaw_config` |
| `crates/brassclaw_turns/src/run_profile/host.rs` | Add `InterceptorResult`; change `on_prompt_assembled` return type from `Option<String>` to `Option<InterceptorResult>` |
| `crates/brassclaw_agent_loop/src/executor/interceptor.rs` | Add `adjusted_messages` field to `InterceptorPromptOutput`; extract from `InterceptorResult` |
| `crates/brassclaw_agent_loop/src/executor/canonical.rs` | Thread `adjusted_messages` → `ModelInput.resolved_messages`; bypass `resolve_model_messages` when pre-resolved |
| `crates/brassclaw_loop_support/src/lib.rs` | Add `resolved_messages` fast path in `ThreadBackedLoopModelPort::stream_model`; skip `resolve_model_messages` when pre-resolved |
| 6 test stub files | Mechanical return-type update (`Option<String>` → `Option<InterceptorResult>`) |
| `crates/brassclaw_reborn/src/runtime.rs` | Add `interceptor_mode` + `sempai_gateway` to `DefaultPlannedRuntimeParts`; wire both |
| `crates/brassclaw_reborn/src/loop_driver_host.rs` | Add `interceptor_mode` + `sempai_gateway` fields + builders; rerouting branch in `on_prompt_assembled`; strip stable-base messages + build component manifest from `matched_component_ids` |
| `crates/brassclaw_reborn_composition/src/runtime.rs` | Wire `PgInterceptorStore`; allocate `sempai_swappable`; create `SharedInterceptorMode`; build Sempai gateway |
| `crates/brassclaw_reborn_composition/src/llm_config_service.rs` | `set_active(Sempai)` live-swap + mode flip; `build_sempai_provider` |
| `crates/brassclaw_reborn_composition/src/interceptor_config_service.rs` *(new)* | `RebornInterceptorConfigService` impl; `reassemble_base_prompt()` via direct SQL with `information_schema.tables` guard + `prewarm()` |
| `crates/brassclaw_reborn_composition/src/webui.rs` | Wire `RebornInterceptorConfigService` into WebUI facade (`RebornServicesApi`) |
| `crates/brassclaw_product_workflow/src/reborn_services/interceptor_config.rs` *(new)* | Port trait + DTOs; 4 methods |
| `crates/brassclaw_webui_v2/src/descriptors.rs` | Add 4 interceptor descriptors |
| `crates/brassclaw_webui_v2/src/router.rs` | Mount 4 interceptor routes |
| `crates/brassclaw_webui_v2/tests/webui_v2_descriptors_contract.rs` | Add 4 new descriptors |
| `crates/brassclaw_webui_v2_static/pages/settings/interceptor/` *(new)* | Settings tab (Reassemble + Pre-warm buttons + persona editor + Recent Sempai Suggestions list Q22 + `components_since_rebuild` badge Q23) |
| `crates/brassclaw_engine/prompts/sempai_audit.md` *(new)* | Default persona text (Part B) |

### Phase 5.5 — Execution Order

```
Step 5.5.0 (V51 migration — prerequisite, no Rust changes)
  │
  ▼
Step 5.5.1 (InterceptorResult trait change — only breaking change)
  │            Must be fully resolved (clippy clean) before 5.5.2 or 5.5.3.
  │
  ├──► Step 5.5.2 (PgStore + swappable + mode flag — pure composition, 3 independent edits)
  │            Requires: Step 5.5.1 complete.
  │
  └──► Step 5.5.3 (set_active live-swap — extends config service)
               Requires: Step 5.5.1 + Step 5.5.2 (sempai_swappable allocated).
                    │
                    ▼
              Step 5.5.4a–c (Sempai gateway + rerouting branch + 3-part prompt)
              Requires: Step 5.5.2 (mode flag in host) + Step 5.5.3 (swappable allocated)
                    │
              Step 5.5.5 can run in parallel with 5.5.4a–c
              (interceptor config service + WebUI tab; only blocks 5.5.4d pre-warm)
                    │
                    ▼
              Step 5.5.4d (pre-warm endpoint)
              Requires: 5.5.4a (gateway wired) + 5.5.5 (config store with base prompt key)
```

## Phase 6 — Settings UI: 10-tab browser + editor (Skills/Tools/Extensions/Actions/Orchestrator/Scaffold/Monty VM/Validation Queue/Reliability/Interceptor Config)

**Goal:** New Settings section to browse, edit, and class-tag Skills/Tools/
Extensions/Actions/Orchestrators/Scaffolds stored in the DB. **10-tab** editor +
validation queue (review + integrate the **existing** validation-queue WebUI
tab) + Monty VM settings + reward immediate-write + **consumer-tag chip editor
with greyed-out state** + **intent examples editor** + **prior-knowledge token
budget field** + **disambiguation UX for intent system** + **Interceptor Config
tab** (Sempai status, Reassemble button, Pre-warm button, persona editor).

### Step 6.1 — WebUI v2 backend routes
- `crates/brassclaw_reborn_webui_ingress` + `brassclaw_webui_v2`: REST routes
  `GET/PUT/POST/DELETE` for `/api/settings/skills`, `/api/settings/tools`,
  `/api/settings/extensions`, **`/api/settings/actions`**, `/api/settings/orchestrators`,
  `/api/settings/scaffolds`, **`/api/settings/monty-vm`** (GET/PUT; the knobs
  are immediate-write, the `active_orchestrator_id` switch is gated),
  **`POST /api/settings/monty-vm/restart`** (triggers the kernel-owned Monty
  VM lifecycle manager — Step 5.1c; optional `force=true` to abort in-flight
  turns), **`GET /api/settings/monty-vm/status`** (returns current Monty VM
  state: `running`/`restarting`/`stopped`/`error`, orchestrator version,
  settings hash). Auth via existing WebUI token; body limits and rate limits
  unchanged.
- Every content/activation/**tag membership** save runs `RecipeValidator` +
  content-safety + code validation; invalid saves return 400 with
  `validation_errors`. Reward/provenance writes are immediate (spec §3.6).
- **Validation-queue routes — review + integrate existing:** the existing
  `GET /api/webchat/v2/validation-queue` + `PUT .../validate` / `.../reject` /
  `.../review-request` endpoints (in `descriptors.rs`/`router.rs`/`handlers.rs`)
  already serve Q2 (manual queue). **Extend** rather than replace:
  - Add `queue_code` query param (`q1_auto`/`q2_manual`/`q3_revision`/
    `q4_rejection`) so the WebUI can group items by queue.
  - Extend `PUT .../validate` to **pop `05:validator`** from `consumer_tags[]`
    on the `AutoPassed → Validated` transition (spec §3.5.1).
  - Add `PUT .../re-submit` for Q3 → Q1 (revision re-submit, `Rejected →
    Pending` when `review_attempts < 3`).
  - Add `DELETE .../wipe` for Q4 post-window wipe (deletes the row + creation-
    process provenance; never thread data).
- **Intent system routes:** `GET /api/settings/intent-inputs` (browse learned
  inputs + their matched component IDs + scores), `PUT .../score` (manually
  adjust a score), `DELETE .../input` (remove a learned input).
- **Verify:** route contract tests; auth/rate-limit/body-limit regression tests
  pass; boundary check (`scripts/check-boundaries.sh`); **the extended
  `validate` endpoint pops `05:validator`**; `re-submit` and `wipe` guard
  tests; intent-input routes scope-isolated.

### Step 6.2 — React SPA Settings section (10 tabs + tag chips + intent examples + interceptor config)
- `crates/brassclaw_webui_v2_static`: new "Skills", "Tools", "Extensions",
  **"Actions"**, "Orchestrator", "Scaffold", **"Monty VM"**, "Validation
  Queue", "Reliability", **"Interceptor Config"** tabs in Settings.
  - List view (name, class, `class_code`, `prompt_uid`, validation status,
    tier, **`consumer_tags[]`**).
  - Editor view (frontmatter + body + class selector writing `compatibility`;
    reward columns read-only, updated via immediate-write).
  - **Intent examples editor (spec §3.12):** each component (skill/tool/
    extension/action/etc.) has an `intent_examples` editor — a list of
    `{input, class}` entries. The operator can add/edit/remove example
    sentences/partial-sentences/words and classify them (1=word, 2=partial,
    3=sentence). On save, these are fed to the intent system.
  - **Consumer-tag chip editor (spec §3.9):** tag chips for each consumer
    (`00:rusty`/`01:monty`/`02:orchestrator`/`03:llm`/`04:scaffold`). While the
    row carries `05:validator` (in queue), the non-validator chips render
    **greyed but toggleable** — the operator can pre-set the audience; the
    chips have no behavioral effect until Step-2 validation pops the validator
    tag. The `05:validator` chip itself is shown read-only (added/removed only
    by the queue lifecycle).
  - **Actions tab (spec §3.11):** list + editor for Action components. The
    editor shows the step list (ordered, draggable) with each step's type
    (`tool_call`/`conditional`/`set_var`/`loop`/`return`/`evaluate`/`call_skill`/
    `try_catch`/`parallel`/`call_action`) + parameters.
    `allowed_tools[]` is a multi-select from the validated tool surface.
    `intent_examples` editor for the Action's trigger phrases.
    `param_schema`/`param_template` editor for caller/intent-extracted params.
    Preconditions editor + error handling editor + `timeout_secs` field.
    A **test runner** ("dry-run this Action against a sample query") that
    dispatches the Action through default.py's Action-execution mode without
    an LLM call. **No size limit enforcement** — the editor does not enforce a
    token budget on Action content (spec §3.11).
  - **Validation Queue tab (review + integrate existing):** the existing
    post-extraction review tab already lists `AutoPassed` items. Extend it to
    show all 4 queues (Q1/Q2/Q3/Q4) via the `queue_code` param; one-click
    "Validate" → `Validated` (pops `05:validator`); "Reject" → Q3/Q4; "Re-submit"
    → Q1; "Wipe" → Q4 post-window delete. Show `review_attempts` +
    `rejected_at` age so the operator can see Q3→Q4 progression.
  - **Monty VM tab (spec §3.10):** editable resource limits
    (`max_duration_secs`, `max_allocations`, `max_memory_bytes`,
    `failure_rollback_threshold`, **`prior_knowledge_token_budget`**), active
    orchestrator pointer (dropdown of `Validated` orchestrators only),
    read-only display of the kernel-owned host-function extensions
    (`__llm_complete__` etc.). **"Restart Monty" button** (spec §3.10) —
    stops the current Python VM instance, applies the new settings from
    `reborn_monty_vm_settings`, and starts a fresh instance via the kernel-
    owned lifecycle manager (Step 5.1c). **Confirmation dialog** warns that
    restart interrupts any in-flight turns. **Status indicator** shows
    whether Monty is `running`/`restarting`/`stopped`/`error`, the current
    orchestrator version, and the settings hash (so the operator can detect
    drift between DB settings and the running instance). Polls
    `GET /api/settings/monty-vm/status` for live status. The restart
    endpoint is `POST /api/settings/monty-vm/restart` (with optional
    `force=true` to abort in-flight turns).
  - **Intent system disambiguation UX (spec §3.12 rule f, §7 Q11 resolved):**
    when the intent system returns multiple matches, a **special
    `disambiguation` chat message type** with clickable buttons is emitted
    (payload: `{type: "disambiguation", candidates: [{component_id,
    component_class_code, description, class_label}]}`). The WebUI renders
    each candidate as a button showing `description` + `class_label`. The user
    clicks one → a structured payload `{disambiguation_choice: component_id}`
    is sent directly back to `__resolve_intent__` via a host function call —
    no intent detection for the reply, no ambiguity. `__resolve_intent__`
    increments the clicked match's score by 1. A regular text message is
    explicitly rejected (friction + requires another intent-detection round).
    This is a chat-surface feature, not a Settings feature, but the Settings
    intent-inputs tab lets the operator see/manage learned inputs + scores.
  - **"AI before User" flip switch (spec §3.12 rule f-ai):** the WebUI chat
    window has a small flip-switch at the top labeled **"AI before User"**.
    When ON, after 10 unsuccessful LLM-sentence matches the intent system
    suppresses the "reformulate" message + "or try it with AI" button and
    silently activates the RetrievalEngine keyword fallback — the user sees
    the turn proceed without interruption. When OFF (default), the user is
    asked to reformulate and offered the "or try it with AI" button. The
    switch state is persisted per-user (spec §7 Q18 — confirm persistence +
    visibility before shipping). **Hidden/disabled in DB-less mode** (the
    intent system is overridden; the flip switch has no effect).
  - Reliability: `reborn_action_reliability` EMA table, operator-resettable.
  - Orchestrator: view current orchestrator Python code, version history,
    activate/rollback; every save → code validation gate (Phase 3.4).
  - Scaffold: view/edit preamble, postamble, platform section, learned-rules
    overlay; version history; every save → validation gate.
  - **Interceptor Config tab (spec §3.15, Phase 5.5):** displays on load
    (`GET /api/interceptor/config`):
    - **Sempai status:** connected/disconnected, mode (routing/rerouting).
    - **Base prompt:** last assembled timestamp + char count. Button:
      **"Reassemble Basic Prompt"** — calls `POST /api/interceptor/reassemble`,
      shows spinner, refreshes snapshot on completion. Calls
      `reassemble_base_prompt()` which queries individual component tables via
      direct SQL (all `Validated` components, no `05:validator`) and writes
      Part A to `brassclaw_config`.
    - **KV-cache:** last pre-warm timestamp. Button: **"Pre-warm Sempai
      KV-cache"** — calls `POST /api/interceptor/prewarm`, shows spinner,
      refreshes on completion. Handles `429` explicitly ("Please wait 60
      seconds before re-warming").
    - **Persona:** editable textarea bound to `snapshot.persona`. Save button
      calls `POST /api/interceptor/config` with `{ persona: "..." }`.
    - **Hidden/disabled in DB-less mode** (interceptor requires Postgres).
- **Verify:** Playwright E2E for browse → edit → save → validation error path;
  Validation Queue → click → status flips to `Validated` + `05:validator`
  popped; tag-chip greyed-state E2E (toggle a chip on an in-queue row → chip
  stays grey, delivery still excluded); Monty VM settings save E2E (incl.
  `prior_knowledge_token_budget`); **Monty VM restart E2E** (change settings →
  click "Restart Monty" → confirmation dialog → status indicator shows
  `restarting` → `running` → new settings applied; `force=true` aborts
  in-flight turns); Orchestrator version rollback E2E; **Actions step-list
  editor E2E** (add/reorder steps with all 13 step types, save, validate,
  dry-run test runner); intent examples editor E2E (add/edit/remove examples,
  classify, save); disambiguation UX E2E (intent match → chat message with
  buttons → click → component dispatched); **"AI before User" flip switch E2E**
  (ON: 10 failed LLM sentences → no reformulate message, turn proceeds
  silently via keyword fallback; OFF: reformulate message + "or try it with
  AI" button appears; DB-less mode: switch hidden/disabled); **Interceptor
  Config tab E2E** (Sempai status display, Reassemble button → Part A written
  from direct SQL to component tables, Pre-warm button → 200 or 429, persona
  editor save, hidden in DB-less mode); i18n parity
  (`scripts/check-i18n-parity.sh`).

## Phase 7 — Cleanup

**Goal:** Delete retired paths once Phases 1–6 are verified.

- Delete `skills/*/SKILL.md` on-disk discovery (`brassclaw_skills::registry`
  filesystem discovery, feature-gated behind the inverse of `skills-db`).
- Delete `migrated_skills.rs`, `bundled_skills.rs` v1→v2 blob migration
  (replaced by Phase 1 importer).
- Drop chunk-related columns/tables (after a retention window).
- Remove v1 skill shim (`src/skills/` if any remains) and
  `skill_migration.rs` bridge.
- Delete the stale `executor/compaction.rs` reference in
  `crates/brassclaw_engine/CLAUDE.md` (phantom file — `deduplication-plan.md`
  Finding 2).
- **Delete the 8 intent-detection functions** (spec §2.3a): `signals_tool_intent`
  (`default.py:101`), `signals_execution_intent` (`default.py:147`),
  `llm_signals_tool_intent` (`reasoning.rs:48`),
  `user_signals_execution_intent` (`reasoning.rs:121`), `score_skill`
  (`default.py:678`), `extract_explicit_skills` (`default.py:754`), and the
  `RecipeTrigger` matching logic (`recipe.rs:65`). All replaced by the unified
  intent system (§3.12). `extract_keywords` (`retrieval.rs:80`) is moved to
  the intent system's class-4 fallback path **and** preserved in
  `retrieval_dbless.rs` for the DB-less fallback-content file path (not
  deleted, relocated to two consumers).
- **Delete the Python formatting functions** (spec §2.3b): `format_docs`
  (`default.py:234`), `format_skills` (`default.py:932`), `append_system_append`
  (`default.py:270`). All replaced by Rust-owned class-specific formatters
  (§3.13). `format_output` (`default.py:194`), `_reduce_prompt`
  (`default.py:542`), `compact_if_needed` (`default.py:562`) are **kept**.
- Update `AGENTS.md` / `CLAUDE.md` / `CHANGELOG.md` for the new architecture
  (Products/Loops/Kernel layering unchanged; Skills/Tools/Extensions/Actions
  now DB + class-tagged + validation-gated; trust layer removed; memory is
  document-level + PlanA-memory connector; **consumer-tag gating §3.9**;
  **4-queue validation lifecycle §3.5.1**; **Monty VM settings DB-stored
  §3.10**; **unified intent system §3.12 replacing 8 intent-detection
  functions**; **Actions class §3.11 for LLM-free deterministic execution**;
  **Rust-owned class-specific formatting §3.13/§3.14**; **intent-driven
  retrieval §3.13**; **token-budget prior-knowledge limit**; **"try it with
  AI" fallback**; **9 new class codes 12-20 for former doctypes**;
  **orchestrator formatting ban §3.14**; **DB-less fallback-content file
  §3.4**; **"AI before User" flip switch §3.12 rule f-ai**; the
  `BRASSCLAW_ORCHESTRATOR_MAX_DURATION_SECS` env var is demoted to DB-less
  fallback only — production reads from `reborn_monty_vm_settings`).
- **Verify:** full `cargo clippy --all ... -- -D warnings`; `cargo test`;
  `scripts/check_gateway_boundaries.py`; `scripts/check_no_panics.py`;
  E2E suite (`scripts/reborn-e2e-rust.sh`); grep confirms no remaining
  `signals_tool_intent`/`signals_execution_intent`/`llm_signals_tool_intent`/
  `user_signals_execution_intent`/`score_skill`/`extract_explicit_skills`/
  `format_docs`/`format_skills`/`append_system_append` references in production
  code; grep confirms no remaining `DocType::` references.

## Verification matrix (per phase)

| Phase | Targeted tests |
|-------|----------------|
| 1 | `cargo test -p brassclaw_skills -p brassclaw_pg`; `reborn_trace_first_party_tool_coverage.rs`; prompt-ordering byte-identity test; consumer-tag CHECK constraint; `05:validator` exclusion from `fetch_for_consumer`; **intent system: query classification (4 classes + `?` rule), match-order a-c (**PERF-02: single query with CASE ordering, not 3 sequential**), scoring d-f (special `disambiguation` chat message type, **PERF-03: atomic UPDATE...RETURNING score increment, SEC-05: score hard cap 100, rate limit 50/scope/hour, `learned_llm` flagged `needs_review`**), no-match e-f, "try it with AI" fallback (class-4, **entirely Rust-side**), "AI before User" flip switch (ON: silent; OFF: default), learning-on-match, **B-tree exact-match index (PERF-01) + GIN trigram (future)**, **normalized schema (PERF-04: one row per `(scope, input_text, input_class, component_id)`, NOT `uuid[]`)**; **Actions: all 13 step types, **`allowed_tools[]` defense in depth (SEC-07: enforced at BOTH default.py AND Rust `EffectExecutor`)**, timeout, orchestrator dispatch (intent match → no LLM call), `call_action` chaining (**SEC-09: max depth 5, cycle detection, step budget 1000**), `try_catch` error recovery, `parallel` concurrency, `spawn_subprocess` (**SEC-08: host runtime script lane, NOT raw Popen, `allowed_tools[]` must include `spawn_subprocess`, command/args/cwd validated against allowlist**), `wait` (duration + polling), `emit_event` (event bus via `brassclaw_events`), token-budget exemption, **hard limits (PERF-18: 256KB content, 500 steps, 50 tools — rejected at validation if exceeded)**; **intent examples extraction on validation** |
| 1.5 | `build_step_context` User-at-N-1 injection test; grep confirms no `signals_tool_intent`/`signals_execution_intent`/`score_skill`/`extract_explicit_skills`/`format_docs`/`format_skills`/`append_system_append` in default.py; self-improvement `memory_write` reroute test (write creates update-candidate with `05:validator` tag, enters Q1, does not write directly); LLM code-audit gate test (Orchestrator/Scaffold component passes Q1 → LLM audit runs → Q2 button disabled until clean → audit flags issue → routed to Q3 with findings); 3-failure auto-rollback still works behind the validation gate; **validator independence test** (the validator runs outside default.py — confirm the validator code is not in the orchestrator's patchable surface) |
| 2 | `cargo test -p brassclaw_capabilities`; `reborn_trace_core_builtin_tools_parity.rs`; tool `consumer_tags[]` = `{00:rusty}` |
| 3 | `cargo test -p brassclaw_skills`; `recipe_library` contract; `is_valid_transition` guard (incl. `Rejected → Pending` revision + `Garbage → wipe`); **`is_queue_status` extended for all 4 queues** (Q1: Pending/AutoFailed; Q2: AutoPassed/ReviewRequested/UpgradeQueued; Q3: Rejected+review_attempts<3; Q4: Rejected+review_attempts>=3+rejected_at age < `q4_retention_days`); confidence-factor source-independence test; **fallback routing test** (confidence factor influences retrieval only when fallback is triggered, not in normal intent-driven mode); validator-tag pop on `AutoPassed → Validated`; update-candidate tag inheritance; Q1→Q2→Q3→Q4 lifecycle test; **Q4 wipe test** (preserves thread data, reads `q4_retention_days` from `reborn_monty_vm_settings` instead of hardcoded constant); **generalized route test** (`PUT /components/{class_code}/{id}/validate|reject|send-to-revision|re-review` works for all class codes; `DELETE` wipe route guarded; `GET /components/{class_code}/{id}/audit-status` returns LLM audit findings for class 10/50; old recipe/tool_skill routes kept as aliases); **code-validation gate test** (invalid orchestrator patch rejected; valid patch commits only on pass); **validator independence test** (validator code not in orchestrator's patchable surface); **LLM code-audit gate test** (Orchestrator/Scaffold Q1→Q2 transition: audit runs → button disabled → flagged → routed to Q3); **self-improvement `memory_write` reroute test** (write creates update-candidate with `05:validator` tag, enters Q1, does not write directly); **per-class validation config test (Q14)** (`ComponentValidator::validate_by_class` dispatch: Skills get full agentskills.io, Tools get tool_name+param_schema, Extensions get soft, Actions get no token budget, former doctypes get soft, Recipes get trigger validation, Orchestrator/Scaffold get LLM audit; `reborn_validation_config` override changes validation outcome for next cycle) |
| 4 | `extension_contract.rs`, `installations_contract.rs`, `manifest_v2_contract.rs`; `test_plan_mode.py`; extension `consumer_tags[]` per class; **Recipe class 21 test (Q15)** (`reborn_recipes` has `override_prompt_creation` + `prior_knowledge_content` columns; `RecipeLookup` reads from `reborn_recipes` class 21; `reborn_extensions_unified` class 09 has NO Recipe rows; Recipe migration from `DocType::Recipe` populates `steps`/`trigger`/`prior_knowledge_content`/`override_prompt_creation`) |
| 5 | `cargo test -p brassclaw_memory -p brassclaw_engine`; `engine_v2_skill_codeact.rs`; DB-less `RamSource` prompt-parity test; User-at-N-1 injection test; `fetch_for_consumer('03:llm')` excludes `01:monty`-only rows; `reborn_monty_vm_settings` read from DB + DB-less fallback; `active_orchestrator_id` gated to `Validated`; **`fetch_for_turn` returns only intent-matched components**; **SEC-01: by-ID fetch drops in-queue/rejected components** (intent-resolved ID → validation gate filter → empty if not Validated); **PERF-05: `reborn_component_catalog` single-query fetch** (no fan-out to 15+ tables); **token-budget truncation test (Actions exempt)**; **"try it with AI" fallback test**; **"AI before User" flip switch test** (ON: silent, no new rows; OFF: default); **DB-less fallback test** (`{db_less_fallback: true}` → keyword path, no intent system); **DB-less fallback-content file loads into `RamSource`**; **SEC-10: `RamSource` refuses non-local runtime profiles**; **`__assemble_prior_knowledge__` both paths** (Solution Override + Normal Assembly); **PKC NULL fallback test**; **non-solution classes do NOT have PKC/override columns**; **orchestrator has no `format_docs`/`format_skills`/`append_system_append` calls**; **former-doctype tables migration + scope isolation**; **document splitting into ≤5000-token rows**; **`prior_knowledge_token_budget` read from DB + DB-less default 2000**; **all `DocType` variants removed**; **`doc_type_weight`/`keyword_match_score`/`extract_keywords` relocated to `retrieval_dbless.rs`**; **Monty VM restart test (PERF-16: drain + admission control — `admission_paused` flag, in-flight turns complete/timeout, queued turns admitted in order, `force=true` aborts, status `draining`/`restarting`/`running`)**; **Monty VM lifecycle manager is kernel-owned**; **`reborn_user_preferences` migration test (Q18)**; **SEC-04: rollback CAS protection test** (`WHERE id = ? AND failure_count = ?` prevents concurrent race); **SEC-11: Q4 wipe is single transaction test** (component + intent_inputs + reliability + provenance deleted atomically) |
| 5.5 | `cargo test -p brassclaw_interceptor -p brassclaw_agent_loop -p brassclaw_reborn -p brassclaw_reborn_composition`; **V51 migration applies after V50**; `InterceptorResult` trait change (`on_prompt_assembled` returns `Option<InterceptorResult>` with `adjusted_messages`); **6 test stub files updated** (mechanical return-type); **`PgInterceptorStore` wired** (composition passes store, not `None`); **`sempai_swappable` allocated** (not `None`, `#[allow(dead_code)]` removed); **`SharedInterceptorMode` created + threaded** (composition → factory → host); **`set_active(Sempai)` live-swap** (DB write + provider swap + mode flip; clearing → Routing); **Sempai gateway + rerouting branch** (Routing: save packet + `adjusted_messages: None`; Rerouting: resolve refs → 3-part prompt → Sempai call → parse → recompose → `adjusted_messages: Some`); **3-part prompt** (Part A from **direct SQL to individual component tables** via `reassemble_base_prompt()` (Q20 — NOT `reborn_component_catalog`), Part B persona from config, Part C volatile tail + component manifest from `matched_component_ids`); **ForensicPacket stores `component_refs` + `volatile_tail`** (NOT full `prompt JSONB` — prevents double-saving); **Actions bypass interception** (Action-only turn → no `__llm_complete__` → no ForensicPacket); **`proposed_recipe_updates` + `proposed_intent_examples` → Q1 validation queue** (Sempai cannot directly create components or intent inputs); **KV-cache pre-warm** (`POST /api/interceptor/prewarm` → 200 or 429); **`reassemble_base_prompt()` uses direct SQL** (queries individual component tables, NOT `reborn_component_catalog` — Q20); **DB-less mode disables interceptor** (`interceptor_store: None`, `sempai_swappable: None`, `interceptor_mode: None`, `on_prompt_assembled` is no-op); **interceptor config service** (`InterceptorConfigService` trait, `InterceptorConfigStore` via `brassclaw_config`, 4 HTTP endpoints); integration test: Sempai mock → mode flips → Kohai receives adjusted messages → packet `sempai_reviewed` → `component_refs` present; integration test: Sempai error → Kohai receives originals → packet `complete`; integration test: `set_active(Sempai, "")` → Routing; integration test: `POST /api/interceptor/reassemble` → Part A from direct SQL; integration test: `POST /api/interceptor/prewarm` empty → 400, assembled → 200; **ForensicPacket cleanup task test (Q21)** (old packet deleted, `forensic_packet_retention_days = 0` → no-op); **Part C stripping verification (cross-phase from Phase 1.5)** (after Phase 1.5's User-at-N-1 injection ships, verify Part C stripping correctly identifies stable-tier injections vs volatile tail — the stripping boundary priorities 1–5 = stable, 6–7 = volatile remains, but priority 6 messages change shape) |
| 6 | route contract tests; Playwright Settings E2E (**10 tabs**); **Validation Queue E2E (4 queue tabs: Q1 Auto/Q2 Manual/Q3 Revision/Q4 Rejection, badge counts, generalized `PUT /components/{class_code}/{id}/validate|reject|send-to-revision|re-review` routes, `DELETE` wipe route, `05:validator` pop, tag-chip greyed-state, LLM-audit guard for class 10/50 — "Validate" button disabled until audit clean, audit findings shown inline)**; **Validation Config sub-panel E2E (Q14)** (per-class thresholds editable in WebUI Validation tab — name/description/token_budget/require_tool_name/require_param_schema/require_activation_criteria per class code; save → next validation cycle uses new thresholds); Monty VM settings save E2E (incl. `prior_knowledge_token_budget` + **`q4_retention_days`** + **`forensic_packet_retention_days`**); **Monty VM restart E2E** (change settings → "Restart Monty" → confirmation → status `restarting` → `running` → new settings applied; `force=true` aborts in-flight turns); Orchestrator version rollback E2E; **Actions step-list editor E2E** (all 13 step types incl. spawn_subprocess/wait/emit_event, add/reorder, save, validate, dry-run test runner); **Recipe editor E2E (Q15)** (class 21 `reborn_recipes` — trigger/steps/prior_knowledge_content/override_prompt_creation editable; `RecipeLookup` reads from `reborn_recipes`); **intent examples editor E2E**; **disambiguation UX E2E** (special `disambiguation` chat message type with clickable buttons, structured `{disambiguation_choice}` payload back to `__resolve_intent__`); **"AI before User" flip switch E2E** (ON: silent fallback; OFF: reformulate + button; DB-less: hidden; toggle persists to `reborn_user_preferences`); **Interceptor Config tab E2E** (Sempai status display, Reassemble button → Part A from direct SQL to component tables, Pre-warm button → 200 or 429 with "wait 60s" message, persona editor save, hidden in DB-less mode); `check-i18n-parity.sh` |
| 7 | full clippy + `cargo test`; `check_gateway_boundaries.py`; `reborn-e2e-rust.sh`; grep confirms deletion of 8 intent-detection functions + 3 Python formatters + all `DocType::` references |

## Risks & mitigations

- **Skill splitting changes prompt behavior** — mitigated by feature flag and
  trace-fixture replay (`scripts/replay-snap.sh`) to diff prompts before/after.
- **Removing chunk search reduces recall** — mitigated by **intent-driven
  retrieval** (§3.13) which is O(matched_components) not O(all_docs), and by
  Skills being smaller + more targeted after the split. The "try it with AI"
  fallback provides a keyword-based safety net.
- **Extension merge breaks install flows** — mitigated by keeping
  `ExtensionManifestV2` validation as the projection contract (fail-closed).
- **Scope isolation regressions** — mitigated by a scope-isolation contract
  test on every new table (including the 9 former-doctype tables + intent
  inputs + actions), mirroring the existing parity tests.
- **Trust removal creates an unvalidated-component path** — mitigated by the
  `Validated == trusted` + no `05:validator` invariant (Phase 3.3) and
  `recipe_library`-style filtering + consumer-tag gating; regression test that
  `AutoPassed` ≠ usable and `05:validator`-tagged ≠ deliverable.
- **Validator-tag greyed-out mechanism confuses operators** — mitigated by the
  WebUI rendering greyed chips as visually distinct + a tooltip explaining the
  queue lifecycle; the `05:validator` chip is read-only.
- **Q3 revision mechanism** (spec §7 Q6 resolved) — Q3 is automated via a
  **scheduled revision Extension** (class 09, tagged `01:monty`) connected to
  kohai/sempai. The revision mission runs on schedule when the LLM is not busy,
  reads rejected components from Q3, uses the kohai/sempai LLM to propose
  repairs based on `review_feedback`, and re-submits repaired candidates to Q1.
  The revision mission is itself a validated Extension (goes through the same
  two-step validation gate). After 3 failed review cycles → Q4. Mitigated by:
  the revision Extension is validation-gated like any other component; it only
  reads Q3 and writes to Q1 (no direct production-table access); LLM cost is
  bounded by the schedule (runs only when LLM is idle); the 3-cycle cap prevents
  infinite repair loops.
- **Q4 wipe deletes the wrong data** — mitigated by the wipe being scoped to
  component-row + creation-process provenance only; thread data is never wiped
  (spec §6); the wipe is a terminal action with its own guard test.
- **Monty VM settings drift between DB and compiled-in defaults** — mitigated
  by `RamSource` serving the compiled-in defaults in DB-less mode and the env
  override remaining as the last-resort fallback; the WebUI shows the effective
  value (DB → env → compiled-in).
- **Self-modification bootstrapping paradox** (Phase 1.5) — **RESOLVED** (spec
  §7 Q1 unblocked) via validator independence (spec §3.5): the Step-1
  validator is Rust-side infrastructure NOT part of default.py, so the
  self-improvement mission cannot patch it. All self-improvement mission writes
  are validation-gated (spec §3.6) — `memory_write` for code/component changes
  creates update-candidates that enter Q1. Orchestrator (10) + Scaffold (50)
  components require an LLM code-audit before Q2 manual validation. The
  3-failure auto-rollback (`load_orchestrator`) is retained as a safety net
  behind the validation gate.
- **Intent system misclassification** (spec §7 Q10) — mitigated by the
  query-classifier heuristic being conservative (defaults to higher classes)
  and the learning mechanism (user confirms meaning equivalence → new inputs
  learned); the "try it with AI" fallback provides a keyword-based escape
  hatch when the intent system fails entirely.
- **Actions execute without LLM oversight** — mitigated by `allowed_tools[]`
  **defense in depth (SEC-07, §6.1)**: enforced at BOTH default.py AND the
  Rust `EffectExecutor` bridge (the Rust bridge receives the Action's
  `allowed_tools[]` as part of the turn context, not from the orchestrator's
  self-reported list). `timeout_secs` bounding. Two-step validation gate
  (Actions must be `Validated` before they can execute). **Hard limits
  (PERF-18, §6.1):** max content 256KB, max steps 500, max `allowed_tools[]`
  50 — rejected at validation if exceeded. **Recursion bounding (SEC-09,
  §6.1):** `call_action` max depth 5, cycle detection, total step budget
  1000. **`spawn_subprocess` sandbox (SEC-08, §6.1):** dispatches through the
  host runtime's script lane (NOT raw `subprocess.Popen`) with capability
  lease + approval gate + sandbox boundary; `allowed_tools[]` must include
  `spawn_subprocess` explicitly; `command`/`args`/`cwd` validated against
  allowlist. default.py is the executor; the step vocabulary is Python tunable
  logic (new step types land via the validation gate). `try_catch` provides
  per-block error recovery; uncaught errors fall through to the global
  `error_handling` (`abort_and_report`).
- **Formatting ban + Q19 resolved (spec §3.13/§3.14)** — the
  orchestrator cannot format retrieved content. Q19 is resolved: "content is
  king" + Solution Override. One `__assemble_prior_knowledge__` Rust host
  function + a static class-code→label lookup table (no per-class formatters).
  Components store content as the exact prior-knowledge text; Rust concatenates
  in `(class_code asc, prompt_uid asc)` order with per-item headers. Solution-
  class components (Extensions/Plans/Recipes/Actions) have `prior_knowledge_content
  TEXT NULL` + `override_prompt_creation BOOLEAN NOT NULL DEFAULT false` (Actions
  default `true`) for the Solution Override path. The self-improvement mission
  CAN tune content by patching `content`/`prior_knowledge_content` fields through
  the validation gate; it CANNOT patch the assembly mechanism (Rust). Mitigation:
  `format_docs`/`format_skills`/`append_system_append` are deleted from Python
  only after the Rust `__assemble_prior_knowledge__` host function is verified
  (Phase 5 Step 5.2a verify section covers both paths).
- **Intent system trigram index unavailable** (spec §7 Q16) — mitigated by a
  fallback to exact-match-only if `pg_trgm` is not available; the intent system
  degrades gracefully (no fuzzy partial matching, but exact matches still
  work).
- **Token-budget limit too aggressive** — mitigated by the
  `prior_knowledge_token_budget` being editable in the WebUI Monty VM tab
  (default 2000 tokens); the operator can increase it if prior knowledge is
  being truncated too aggressively.
- **Former-doctype validation too heavy or too light** (spec §7 Q14
  **resolved**) — mitigated by **per-class validation configuration** (§3.5.2):
  only Skills (classes 01-03) require the full agentskills.io validation; all
  other classes use lighter validation (name + description + content + soft
  token budget). Each class's thresholds are configurable in the WebUI Settings
  → Validation tab via a `reborn_validation_config` table (one row per
  `(scope, class_code)`). The validator (`ComponentValidator`, renamed from
  `RecipeValidator`) dispatches by class code: `validate_by_class(class_code,
  component, config_row, ...)`. Changes are immediate-write but do not
  retroactively re-validate existing components — they apply to the next
  validation cycle.
- **DB-less fallback file stale or incomplete** (spec §3.4, §7 Q17
  **resolved**) — mitigated by the fallback file being **created at
  installation time** when the user selects not to install a DB (not exported
  from the DB — impossible in a DB-less installation). The file contains
  selected compiled-in entries (Tools → Scaffold → Orchestrator → Skills →
  Extensions → Recipes → Specs/Lessons; Issues/Notes/Summaries excluded) up to
  ~256KB (~50,000 tokens, ~5 original DocPlans). The file is static during
  DB-less operation (no learning) so it cannot drift within a session. The
  keyword-retrieval path (pre-v4) is preserved in `retrieval_dbless.rs` so
  DB-less mode works as before the architecture change.
- **"AI before User" flip switch proceeds on wrong intent** (spec §3.12 rule
  f-ai) — mitigated by the switch defaulting to OFF (the user is asked to
  reformulate by default); the ON mode is opt-in for operators who trust the
  keyword fallback and want minimal interruptions. The tradeoff is explicit:
  flow continuity over user confirmation. **No learning happens** from the
  "AI before User" path (no new `reborn_intent_inputs` rows) — the system
  does not reinforce unconfirmed fallback matches. If the keyword fallback
  assembles poor prior knowledge, the turn proceeds but the user can
  reformulate on the next turn (the switch does not suppress normal chat).
  The switch is hidden/disabled in DB-less mode (no intent system to fall
  back from).
- **Monty VM restart interrupts in-flight turns** (spec §3.10, **PERF-16 §6.1**)
  — mitigated by **drain + admission control**: the kernel-owned lifecycle
  manager sets an `admission_paused` flag (new turns queued, not rejected),
  waits for in-flight turns to complete or timeout (or aborts with
  `force=true` + confirmation), then stops/restarts the VM, then admits queued
  turns in order. The restart is kernel-owned (default.py cannot trigger it);
  the status indicator (`draining`/`restarting`/`running`) + settings hash
  drift detection let the operator see state. Restart is an explicit operator
  action.
- **`reborn_validation_config` weakens the trust gate** (SEC-02, §6.1) —
  mitigated by **compiled-in safety floors**: the `ComponentValidator` enforces
  a minimum for each class regardless of the config row (`token_budget` cannot
  exceed the hard cap, `require_tool_name`/`require_param_schema` cannot be
  `false` for Tools, `require_activation_criteria` cannot be `false` for
  Skills). The WebUI shows the floor as a disabled minimum and prevents saving
  below it. A config row violating the floor is rejected at save time.
- **Intent system poisoning / score manipulation** (SEC-05, §6.1) — mitigated
  by **bounded + rate-limited scores**: hard cap 100 per row, rate limit 50
  increments per scope per hour (token bucket), `learned_llm` inputs flagged
  `needs_review: true` and purged on source component wipe. Seeded inputs
  (from validated `intent_examples`) are the trust anchor; learned inputs are
  routing hints with bounded influence. An operator can purge all
  `learned_llm` inputs from the WebUI if a kohai provider is compromised.
- **By-ID retrieval bypasses validation gate** (SEC-01, §6.1) — mitigated by
  the by-ID fetch path filtering `validation_status = 'Validated' AND
  '05:validator' != ANY(consumer_tags)`. An intent-resolved ID pointing to an
  in-queue or rejected component is silently dropped → orchestrator falls back
  to the no-match path.
- **Cross-table fan-out for matched components** (PERF-05, §6.1) — mitigated
  by the `reborn_component_catalog` read model (materialized view / UNION ALL
  view): the RetrievalEngine queries the catalog by `component_id` +
  `component_class_code` in a single query instead of fan-out to 15+ tables.
- **v5.5 comprehensive review findings** (§6.1) — the v5.5 review (4 parallel
  subagents: codebase verification, security, correctness, performance)
  identified 12 security issues, 13 correctness issues, and 19 performance
  issues. All BLOCKING issues are fixed (Scaffold 11→50, Actions schema,
  migration ordering, stale `format_action`, ToolSkill placement). All
  CRITICAL/HIGH issues are mitigated in §6.1 (SEC-08 spawn_subprocess sandbox,
  SEC-01 by-ID gate, SEC-02 config floors, SEC-05 score bounding, SEC-07
  defense in depth, PERF-05 catalog, PERF-16 drain, PERF-18 hard limits,
  PERF-03 atomic increment, PERF-02 single query, PERF-01 B-tree). Remaining
  MAJOR/MEDIUM issues are addressed in the same review pass.
- **Interceptor Sempai compromise** (§3.15) — if the Sempai provider is
  compromised, it could inject malicious `adjusted_messages`,
  `proposed_recipe_updates`, or `proposed_intent_examples`. Mitigated by:
  (a) `proposed_recipe_updates` + `proposed_intent_examples` are routed
  through the Q1 validation queue (Phase 3's `ComponentValidator`) — the
  Sempai cannot directly create components or intent inputs; (b)
  `adjusted_messages` only affect the volatile tail (thread history), not the
  stable base (Part A is read-only from `brassclaw_config`); (c) the Sempai is
  an OpenAI-compatible HTTP provider configured by the operator — if
  compromised, the operator clears it (`set_active(Sempai, "")`) → mode flips
  to Routing (forensic logging only, no rerouting); (d)
  `settings_adjustments` are stored but never auto-applied — they require
  manual operator action.
- **Interceptor Part A stale** (§3.15) — Part A is a manual rebuild. If the
  operator adds/removes components and forgets to click "Reassemble Basic
  Prompt", the Sempai's Part A is stale (missing new components or including
  deleted ones). Mitigated by: (a) the WebUI Interceptor Config tab shows the
  last assembled timestamp + char count — the operator can see when it was
  last rebuilt; (b) after Phase 5 migrations, the operator should click
  Reassemble to include new component tables; (c) stale Part A does not break
  correctness — the Sempai audits with incomplete information, but the Kohai
  prompt is assembled from the live DB (not from Part A). The worst case is
  the Sempai misses a new component in its audit — a degradation, not a
  failure.
- **Interceptor ForensicPacket storage grows unbounded** (§3.15, Q21) — one
  ForensicPacket per turn. Mitigated by: (a) the `component_refs` +
  `volatile_tail` schema is compact (references, not full prompt content —
  prevents double-saving); (b) `forensic_packet_retention_days` column in
  `reborn_monty_vm_settings` (default 90) + scheduled daily cleanup task
  deletes packets older than the retention window (operator can set to 0 to
  disable pruning — mirrors `q4_retention_days` pattern Q7); (c) the
  `(tenant_id, captured_at DESC)` index supports efficient pagination for
  cleanup queries.
- **Interceptor breaks DB-less mode** (§3.15) — the interceptor requires
  Postgres (`PgInterceptorStore`). In DB-less mode, the interceptor is
  disabled (no-op). This is acceptable because DB-less mode is not for
  production (§3.4, SEC-10). The `on_prompt_assembled` hook is a no-op in
  DB-less mode.
