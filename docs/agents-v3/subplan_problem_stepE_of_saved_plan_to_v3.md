# Subplan — Phase E problem: component-class registry for IBS step fetches

Parent plan: `saved_plan_to_v3.md` → Phase E (`lines 4711–4956`).
Zenflow task: `e81125fc-ce63-449e-922a-dfa80b964019`. Chat: `be1470ab-f612-4526-bc95-e1e37c8f4527`.
Inserted as a **substep** under the Zenflow Phase E step `9fa2c778`.

---

## 1. Why this subplan exists — the FIND-IBS-02 class-code gap

Phase E restructures `PostgresSource::fetch_for_turn` so that a class-21 Recipe
intent match **with a `step_link`** runs the IBS
(`instruction_builder::build_instruction`) and fetches the component items for
each channel (`rust_steps` / `orchestrator_steps`) → `FetchForTurnResult::SplitResult`.

**The gap:** the IBS emits `IbsRecipeStep.include: Vec<uuid::Uuid>` per step with
**no per-UUID `class_code`** (FIND-IBS-02: "UUIDs are opaque to the IBS" —
`instruction_builder.rs:135`, comment at `:593`). But `fetch_component_by_id(pool,
scope, component_id, component_class_code)` needs the `class_code` to pick the
class-specific table via its `match component_class_code` arm. There is **no
per-UUID class source in the data model**: `step_descriptions` carries only
UUIDs; `reborn_intent_inputs` holds intent-matched components (recipes/actions),
not recipe-step includes (ToolSkills/Skills/PythonCode). The plan's literal
`fetch_components_by_ids(ids_by_class: &[(uuid::Uuid, i32)])` signature
presupposes a class per UUID that does not exist.

This is a "plan literal spec inconsistent with the data model" situation
(analogous to the E0-A orphan-rule deviation). The user resolved it (see §2).

---

## 2. User design decisions (this portion — all confirmed via ask_user)

These decisions govern the **whole** of Phase E, not only the registry substep.

1. **Step-component fetch shape (Q1 → C, then Q-F1 → B):** Keep the per-UUID
   `fetch_component_by_id` call shape; resolve each UUID's `class_code` via a
   **registry lookup**. The registry is a **real `reborn_components(id,
   class_code, scope)` table** kept in sync by **triggers** on every class
   table (new migration **V061** — a schema change beyond Phase E's original
   "no migration"; the user explicitly accepted this upgrade). The lookup is one
   indexed `SELECT class_code ... WHERE id=$1 AND scope…` per UUID, then
   `fetch_component_by_id(pool, scope, uuid, class)` per UUID.
2. **`has_validation` for `is_tier0_eligible` (Q2 → A):** `has_validation =
   (validation_status == 'validated')`. Under §0.23 every validated component
   passed Q1+Q2, so validated ⇒ a validation hook ran; the
   `Recipe::is_tier0_eligible()` `has_validation` guard is subsumed by the
   existing `validated` requirement. No extra query, no migration. (The engine
   `Recipe.validation: RecipeValidation` field is in-memory only —
   `reborn_recipes` has no validation-hook column — so the full check is
   uncomputable from the row without this interpretation.)
3. **`llm_call_required` vs `tier0_eligible` (Q3 → A):** `llm_call_required =
   !tier0_eligible` (always complements). `tier0_eligible` =
   `Recipe::is_tier0_eligible()` computed from the row (tier ∈ {mature,
   candidate} && validation_status='validated' && wilson_lower ≥ 0.70 &&
   has_validation). `fetch_for_turn` passes `llm_call_required` into
   `build_instruction` and copies it into `TurnRoutingSignals`.
4. **`TurnRoutingSignals.override_prompt_creation` (Q4 → A):** from the matched
   **Recipe row's own `override_prompt_creation` column** (`reborn_recipes` has
   it; `PgRecipe.override_prompt_creation`).
5. **`ActionShortCircuit` → turns-layer bridge mapping (Q5 → A):** the
   composition `PgRetrievalLookup` maps engine
   `FetchForTurnResult::ActionShortCircuit { component_id, name }` to
   `RetrievalTurnResult { tier0_eligible: true, llm_call_required: false,
   rust_items: json!([]), orchestrator_items:
   json!([{"type":"action_short_circuit","component_id":<uuid>,"name":<name>}]),
   routing_meta: json!({"variant":"action_short_circuit","component_id":<uuid>,
   "name":<name>}) }`. The Phase-H consumer discriminates on
   `routing_meta.variant` (consistent with how `Disambiguation` is already
   encoded). **No turns-layer type change.**
6. **`{{vars.name}}` capture+substitution (Q6 → A):** add **pure helpers** to
   `instruction_builder.rs` — `capture_variables(user_text,
   &variable_patterns) -> HashMap<String,String>` (compile each
   `VariablePattern.pattern` regex, match against `user_text`, extract the named
   group whose name is `VariablePattern.name`) and `substitute_vars(text,
   &captures) -> String` (replace `{{vars.<name>}}` with the captured value).
   Called from `fetch_for_turn`. Applied to **both** `rust_items` and
   `orchestrator_items` `effective_content` **and** `tool_bindings.params`
   (`ToolBinding.params` uses `{{vars.name}}` — `ibs.rs:75`).

**Other grounded facts (no decision needed):**
- `IntentResolution::Match` already carries `step_link: Option<String>` +
  `component_name: String` (Phase D built both + the `reborn_actions` LEFT JOIN
  in `resolve_intent`). Phase E consumes `component_name` for
  `ActionShortCircuit` and `step_link` for the SplitResult dispatch.
- `RecipeVariant.variant_key` (human-readable, `recipe.rs:153`) is the
  `TurnRoutingSignals.variant_label` source (no `label` field exists; no schema
  change). The matched variant is the one whose `step_link` equals the
  `IntentResolution::Match.step_link`.
- The E0-A re-target introduced the turns-layer `PgRetrievalLookup` bridge
  (`retrieval_lookup_impl.rs`). Growing the engine `FetchForTurnResult` enum in
  Phase E makes that bridge's `match` non-exhaustive → **Phase E MUST add
  `SplitResult` + `ActionShortCircuit` arms to the bridge** (forced consequence
  of E0-A, not a new decision). The bridge replaces the conservative
  `tier0_eligible=false`/`llm_call_required=true`/unsplit-items E.0 helpers with
  real routing booleans + split channels for `SplitResult`.

---

## 3. The V061 registry migration + lookup helper (the big change)

### `crates/brassclaw_pg/migrations/V061__reborn_components_registry.sql`

**Table** — a flat UUID → class_code + scope registry, one row per component
across all 14 class tables. UUIDs are globally unique (v4), so `PRIMARY KEY(id)`
is safe; the scope columns are denormalized so the lookup enforces SEC-01
tenant isolation (never returns a class for another tenant's UUID):

```sql
CREATE TABLE IF NOT EXISTS reborn_components (
    id          UUID        NOT NULL,
    tenant_id   TEXT        NOT NULL,
    user_id     TEXT        NOT NULL,
    agent_id    TEXT        NOT NULL,
    project_id  TEXT        NOT NULL,
    class_code  INT         NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (id)
);
CREATE INDEX IF NOT EXISTS reborn_components_scope_class_idx
    ON reborn_components (tenant_id, user_id, agent_id, project_id, class_code);
```

**Generic trigger function** — reads `NEW.class_code` directly (every class
table exposes `class_code` — verified by `fetch_component_by_id`'s
parameterised `SELECT class_code::int FROM {table}`, which is existing
production code), so no per-table class argument is needed and tables that host
multiple classes (`reborn_skills` → 1/2/3/10/50, `reborn_extensions_unified` →
4–9) are handled correctly:

```sql
CREATE OR REPLACE FUNCTION maintain_components_registry() RETURNS trigger AS $$
BEGIN
    IF (TG_OP = 'INSERT') THEN
        INSERT INTO reborn_components
            (id, tenant_id, user_id, agent_id, project_id, class_code, created_at, updated_at)
        VALUES
            (NEW.id, NEW.tenant_id, NEW.user_id, NEW.agent_id, NEW.project_id, NEW.class_code, now(), now())
        ON CONFLICT (id) DO UPDATE SET
            tenant_id  = EXCLUDED.tenant_id,
            user_id    = EXCLUDED.user_id,
            agent_id   = EXCLUDED.agent_id,
            project_id = EXCLUDED.project_id,
            class_code = EXCLUDED.class_code,
            updated_at = now();
        RETURN NEW;
    ELSIF (TG_OP = 'UPDATE') THEN
        UPDATE reborn_components SET
            tenant_id  = NEW.tenant_id,
            user_id    = NEW.user_id,
            agent_id   = NEW.agent_id,
            project_id = NEW.project_id,
            class_code = NEW.class_code,
            updated_at = now()
        WHERE id = NEW.id;
        RETURN NEW;
    ELSIF (TG_OP = 'DELETE') THEN
        DELETE FROM reborn_components WHERE id = OLD.id;
        RETURN OLD;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;
```

**Trigger attachments** — one multi-event trigger per class table (14 tables):
`reborn_skills`, `reborn_extensions_unified`, `reborn_actions`, `reborn_specs`,
`reborn_tool_skills`, `reborn_plans`, `reborn_summaries`, `reborn_docus`,
`reborn_lessons`, `reborn_issues`, `reborn_notes`, `reborn_recipes`,
`reborn_python_code`, `reborn_extension_catalogues`. Pattern (×14):

```sql
CREATE TRIGGER maintain_components_registry
    AFTER INSERT OR UPDATE OR DELETE ON reborn_skills
    FOR EACH ROW EXECUTE FUNCTION maintain_components_registry();
```

(Use `DROP TRIGGER IF EXISTS` + `CREATE TRIGGER` so the migration is
re-runnable; trigger names are unique per table so 14 identical-named triggers
coexist — Postgres trigger names are scoped to their table.)

**Backfill** — seed the registry from existing rows (one `INSERT ... SELECT ...
UNION ALL ... ON CONFLICT (id) DO NOTHING` over all 14 tables; per-arm verify
each table has `created_at`/`updated_at` at implementation time, else omit the
timestamp columns from that arm and rely on the table DEFAULT).

### `crates/brassclaw_engine/src/memory/retrieval_source.rs` — lookup helper

```rust
/// Resolve a component UUID to its class_code via the `reborn_components`
/// registry (V061). Scoped so a foreign-tenant UUID never resolves (SEC-01).
/// Returns Ok(None) when the UUID is absent (caller skips that step's item).
#[cfg(feature = "skills-db")]
async fn lookup_component_class(
    pool: &brassclaw_pg::PgPool,
    scope: &ComponentScope,
    component_id: uuid::Uuid,
) -> Result<Option<i32>, RetrievalSourceError> {
    let client = pool.get().await.map_err(|e| RetrievalSourceError::Db(e.to_string()))?;
    let row = client
        .query_opt(
            "SELECT class_code::int FROM reborn_components
             WHERE id = $1 AND tenant_id = $2 AND user_id = $3 AND agent_id = $4 AND project_id = $5",
            &[&component_id, &scope.tenant_id, &scope.user_id, &scope.agent_id, &scope.project_id],
        )
        .await
        .map_err(|e| RetrievalSourceError::Db(e.to_string()))?;
    Ok(row.map(|r| r.get::<_, i32>(0)))
}
```

---

## 4. Phase E implementation substeps (run one-by-one, commit+push each)

- **E.1 — V061 registry (this subplan's core):** create the migration (table +
  generic trigger fn + 14 trigger attachments + backfill) + the
  `lookup_component_class` helper. Verify the migration applies (refinery/embedded
  PG) + a unit test asserting a seeded row resolves. Commit+push.
- **E.2 — engine `retrieval_source.rs` types:** add `FetchForTurnResult::ActionShortCircuit
  { component_id: Uuid, name: String }` + `FetchForTurnResult::SplitResult {
  rust_items, orchestrator_items, routing: TurnRoutingSignals }`; add the
  `TurnRoutingSignals` struct (§0.8 shape); extract `class_code_to_table(code:
  i32) -> Option<(&'static str, &'static str)>` (with the mandatory `10 | 50`
  arm + 22/23 + the `⚠️ WHEN ADDING A NEW CLASS CODE` comment) shared by
  `fetch_component_by_id` and available to the new path. Commit+push.
- **E.3 — engine `instruction_builder.rs` helpers:** add pure
  `capture_variables` + `substitute_vars` (Q6→A) with unit tests. Commit+push.
- **E.4 — restructure `PostgresSource::fetch_for_turn`:** class-16 match →
  `ActionShortCircuit { component_id, component_name }` BEFORE any
  `fetch_component_by_id`; class-21 match with `step_link.is_some()` → query the
  recipe row (`step_descriptions`, `variants`, `wilson_lower`, `tier`,
  `validation_status`, `override_prompt_creation`), deserialize `variants` →
  `Vec<RecipeVariant>`, find the variant whose `step_link` == match step_link,
  extract `variable_patterns`, call `build_instruction(step_link,
  &step_descriptions, &variable_patterns, llm_call_required)` where
  `llm_call_required = !tier0_eligible` (computed from the row per Q2→A/Q3→A),
  apply `capture_variables`+`substitute_vars` to the user text, then per UUID in
  `rust_steps`/`orchestrator_steps`: `lookup_component_class` →
  `fetch_component_by_id` → items (with substitution applied to
  `effective_content` + `tool_bindings.params`); build `TurnRoutingSignals`
  (override_prompt_creation ← recipe row per Q4→A, matched_component_ids ←
  orchestrator UUIDs as Vec<String>, variant_label ← matched
  `RecipeVariant.variant_key`, step_link, llm_call_required, wilson_lower,
  tier0_eligible); return `SplitResult`. class-21 with `step_link: None` →
  existing `fetch_component_by_id` → `Components` (unchanged). Commit+push.
- **E.5 — composition `retrieval_lookup_impl.rs` bridge:** add `SplitResult`
  arm (real booleans from `routing` + split `rust_items`/`orchestrator_items` +
  `routing_meta` carrying variant/wilson/tier0/llm/step_link/variant_label) and
  `ActionShortCircuit` arm (Q5→A). Keep `Components`/`Disambiguation` arms.
  Commit+push.
- **E.6 — tests + verify:** the 7 plan unit tests (SplitResult channel split;
  `knowledge: both` in both channels; ActionShortCircuit; `step_link: None`
  unchanged; `{{vars.dir}}` substitution in orchestrator_items; `routing.wilson_lower`
  populated; registry lookup resolves a seeded UUID) + 1 integration test (full
  intent match → correct channel split by class_code). `cargo fmt` +
  `cargo clippy -p brassclaw_engine -p brassclaw_reborn_composition --all-targets -- -D warnings`
  (default + `--features brassclaw_reborn_composition/skills-db`) + `cargo test`.
  Commit+push. Mark Phase E + substep Completed.

---

## 5. Acceptance

- `PostgresSource::fetch_for_turn` returns `SplitResult` for a class-21 +
  `step_link` intent match, with `rust_items`/`orchestrator_items` split by IBS
  channel and `TurnRoutingSignals` carrying real booleans
  (`tier0_eligible` from the row, `llm_call_required = !tier0_eligible`).
- class-16 match returns `ActionShortCircuit` without a second DB fetch.
- The `reborn_components` registry resolves step UUIDs → class_code (scoped),
  and triggers keep it in sync on insert/update/delete across all 14 class
  tables.
- The composition `PgRetrievalLookup` bridge surfaces `SplitResult` /
  `ActionShortCircuit` to the turns layer with real routing booleans (replacing
  E.0's conservative defaults) — Phase H's `LoopOrchestratorPort` consumer can
  then dispatch Tier-0/Tier-1.

---

## 6. Upgrade — Phase M.4/M.5 template-extraction/substitution front-loaded into E.3

**Why this is here (task rule: "do not blindly remove upgrades; document,
repair, complete or leave them").** E.4 (`PostgresSource::fetch_for_turn`) must
substitute `{{vars.name}}` into `orchestrator_content` + ToolBinding `params`
at assembly time (§0.20.3, §0.4.1). The plan originally scheduled the slot
extraction as `extract_template_slots` in **Phase M.4** and the
`variable_patterns` refinement as **Phase M.5**, with no substitution-helper
signature specified at all (§0.20.3 only describes the behaviour: "the IBS
applies `{{vars.name}}` → literal value substitution"). Phase E runs before
Phase M, so Phase E cannot call a Phase-M function that does not yet exist.
The clean resolution (accepted as an upgrade, not a deletion of Phase M): E.3
implements the pure helpers NOW in the IBS home
(`crates/brassclaw_engine/src/memory/instruction_builder.rs`), and Phase M
later re-references them rather than re-creating them.

**What E.3 adds (all pure, synchronous, no DB, no feature-gate — they are IBS
builder helpers, compiled in both default and `skills-db` configs):**

- `pub fn extract_template_slots(template: &str, user_text: &str) -> Vec<(String, String)>`
  — Phase M.4 canonical signature + algorithm verbatim. Splits `template` on
  `%` into literal segments; prefix anchors the start (found left), suffix
  anchors the end (found right), middle separators are searched left-to-right
  within `[cursor, suffix_start]`; each gap is a positional slot
  (`slot0`, `slot1`, …). Empty vec when no `%`; partial vec when a middle
  separator is missing (remaining slots dropped).
- `pub fn capture_variables(template, user_text, variable_patterns: &[VariablePattern]) -> Vec<(String, String)>`
  — Phase M.5 refinement on top of `extract_template_slots`. Each
  `variable_patterns` entry is paired with its slot **by order** (the
  `VariablePattern` struct has no slot-index field — order is the only
  field-less mapping, confirmed against M.5). The entry's regex is applied to
  the auto-extracted value (NOT the full `user_text`); on a match the slot is
  renamed to the entry's semantic `name` and, if the regex captures a named
  group, that group's value replaces the raw value (transformation). On a
  regex mismatch OR compile failure the slot is **demoted** — raw value +
  positional name kept (§0.17.3: "extraction just gets the raw value"; a bad
  regex is a Q1 authoring error caught at Phase I, not a runtime turn
  failure). Entries beyond the slot count are ignored (dangling — Q1 warns at
  Phase I).
- `pub fn substitute_vars(content: &str, vars: &[(String, String)]) -> String`
  — §0.20.3 substitution. Replaces each literal `{{vars.name}}` token with its
  value. Distinct names never overlap. Unresolved placeholders are left intact
  (Q1 "missing template" authoring error — Phase I — not a runtime
  fabrication).
- `pub fn substitute_vars_in_value(value: &serde_json::Value, vars) -> serde_json::Value`
  — §0.4.1 ToolBinding `params` substitution. Recursively walks
  Object/Array/String; substitutes string leaves only (keys + non-string
  leaves pass through unchanged).

**Naming reconciliation.** The E-subplan's E.3 step called these
`capture_variables` + `substitute_vars` (entry-point names); the plan's Phase
M.4 called the pure extractor `extract_template_slots`. Both names now coexist:
`extract_template_slots` (Phase M.4 pure extractor, with the M.4-mandated unit
tests) is called by `capture_variables` (the E.3/E.4 entry point that adds the
M.5 refinement). Phase M later references `extract_template_slots` from
`fetch_for_turn` per its M.4 "Files to modify" — it will find it already
implemented here (no re-creation). The Phase M.4 `parse_template` helper
(template → `(prefix, suffix)` for DB indexing) is a DIFFERENT operation and
stays in `intent_system.rs` as Phase M's own concern (E.3 does not implement
it).

**Tests added (22, all in `instruction_builder::tests`, both configs):** the 7
Phase M.4 canonical `extract_template_slots` tests (single-slot, two-slot
middle-separator, no-slots, trailing-`%`, leading-`%`, adjacent-`%%`
degenerate, missing-middle-separator partial); 8 `capture_variables` tests
(empty patterns, named-group transform+rename, validation-only rename,
validation-failure demote, no-pattern rename, bad-regex demote,
more-patterns-than-slots, two-slots-two-patterns paired by order); 4
`substitute_vars` tests (replace-all, distinct-names-no-overlap,
unresolved-left-intact, no-placeholders); 3 `substitute_vars_in_value` tests
(string-leaves-only, nested array+object, preserves object keys).

**Verification:** `cargo clippy -p brassclaw_engine --all-targets -- -D
warnings` clean in BOTH default and `--features skills-db`; `cargo fmt` clean;
`cargo test -p brassclaw_engine --lib` → 666 (default) / 677 (skills-db), both
+22 vs the E.2 baseline (644 / 655), no regression.

**Open item deferred to E.4 (NOT a Phase-M front-load):** the template
expression source at `fetch_for_turn` time. `IntentResolution::Match` today
carries `{ component_id, class_code, step_link, component_name }` — no
template/prefix/suffix — so `capture_variables`'s `template` argument has no
source yet. E.4 must decide how `fetch_for_turn` obtains the matched template
(extend `Match` with `template_prefix`/`template_suffix`, re-query
`reborn_intent_inputs`, or re-match `RecipeVariant.intent_examples`). That is
an E.4 design decision to raise with the user before E.4 implementation.

---

## 7. E.4 design decisions (user-resolved 2026-08-22)

Five design questions were raised with the user before E.4 implementation
(task rule: "ask all design questions before implementing; do not decide
design decisions unilaterally"). The resolutions below are canonical for
E.4 and supersede any conflicting wording in §4's E.4 substep bullet.

### 7.1 Template source for `capture_variables` — pass `query`

Phase E uses **exact-match** intent (`ii.input_text = $5`), so the matched
intent `input_text == query`. `capture_variables(template, user_text,
variable_patterns)` is therefore called with `template = query` AND
`user_text = query`. Because a Phase-E intent `input_text` is a literal
(no `%`), `extract_template_slots(query, query)` returns `[]` → `vars = []`
→ `substitute_vars` / `substitute_vars_in_value` are no-ops. The wiring is
REAL (the helpers are invoked), just inert until Phase M switches
`resolve_intent` to 3-path template matching, at which point Phase M
changes THIS call site to pass the matched `%`-containing template instead
of `query`. No `IntentResolution::Match` enum change, no extra DB query
(this E.4). Chosen over "extend `Match` with `matched_input_text`"
(forward-compatible but more churn now) and "re-query
`reborn_intent_inputs`" (extra round-trip).

### 7.2 Recipe-row query — scope filter only, NO SEC-01 hard gate

The class-21+`step_link` recipe-row query filters on the scope 4-tuple
ONLY (`tenant_id`/`user_id`/`agent_id`/`project_id`). It does NOT
hard-gate on `validation_status = 'validated'` / `'05:validator' !=
ALL(consumer_tags)`. Instead it SELECTs `validation_status`, `tier`, and
`wilson_lower` to compute `tier0_eligible` (§7.4). A recipe demoted
between the intent match and this fetch therefore STILL runs — via Tier-1
(`llm_call_required = true`, `tier0_eligible = false`). The per-UUID
sub-component fetches (`fetch_components_by_ids`) still enforce the full
SEC-01 gate on every sub-component, so a demoted sub-component is dropped
but the turn degrades, not fails. If the recipe row is entirely absent
(deleted in the TOCTOU window; unreachable in practice) → fall back to
the NoMatch broad-scan (`fetch_for_consumer` → `Components`).

### 7.3 `variant_label` fallback — recipe row's `name`

If `step_link.is_some()` but no `RecipeVariant` in `variants` matches it
(or `variants` is empty/NULL), `variable_patterns = vec![]` (pinned) and
`TurnRoutingSignals.variant_label = recipe.name` (the row's `name`
column). When a variant DOES match, `variant_label = variant.variant_key`
(as documented on `TurnRoutingSignals`).

### 7.4 `build_instruction` error — soft-fail to empty `SplitResult`

If `build_instruction` errors (bad `step_link` parse, step-order
violation, or S7 guard), `fetch_for_turn` returns a `SplitResult` with
**empty channels** (`rust_items = []`, `orchestrator_items = []`),
`routing.llm_call_required = true`, `routing.tier0_eligible = false`,
`routing.variant_label = recipe.name` (§7.3 fallback),
`routing.matched_component_ids = []`, `routing.override_prompt_creation`
+ `routing.wilson_lower` + `routing.step_link` from the row/match, and
`instruction = None` (§7.5). The turn degrades to Tier-1 rather than
hard-failing. Chosen over "hard fail `RetrievalSourceError`" (a bad
recipe should not break the whole turn) and "degrade to legacy
`Components` path" (loses the intent-match routing signal).

### 7.5 Upgrade — `SplitResult` + `RetrievalTurnResult` carry the compiled `BuildInstruction`

**Task rule: "do not blindly remove upgrades; document, repair, complete
or leave them".** Plan §0.8 defines `FetchForTurnResult::SplitResult {
rust_items, orchestrator_items, routing }` (no `BuildInstruction`), and
FIND-P9-03 defines `RetrievalTurnResult { tier0_eligible,
llm_call_required, rust_items, orchestrator_items, routing_meta }` (no
instruction field). The §0.8 step `iii` says "Apply `{{vars.name}}`
substitution" and §0.20.3 says substitution is into `orchestrator_content`
(the Skill/PythonCode BODIES = the fetched `ComponentItem`.
effective_content`). But `ToolBinding.params` (carrying `{{vars.name}}`
placeholders per §0.4.1) live on the compiled `BuildInstruction`'s
rust-channel `IbsRecipeStep`s — which the §0.8 `SplitResult` DROPS. So a
Phase-E-only `SplitResult` would compute the substituted `tool_bindings`
and throw them away (wasted; Phase H would have to re-compile +
re-substitute).

**User decision (Q-E4-5 → option B):** extend BOTH types to carry the
compiled `BuildInstruction` (with substituted `tool_bindings` + per-step
structure) so Phase H's `RecipeStage` / `TierZeroExecutionStage` consumer
receives everything without re-compiling. This is an upgrade to document:

- **Engine `FetchForTurnResult::SplitResult`** gains
  `instruction: Option<BuildInstruction>` — `Some` on a successful
  compile (with `substitute_vars_in_value` already applied to every
  rust-channel `tool_bindings[].params`), `None` on the §7.4 soft-fail.
  Typed (engine-internal). `Option` honestly represents "no instruction
  compiled" on soft-fail.
- **Turns `RetrievalTurnResult`** gains
  `instruction: serde_json::Value` — the composition bridge serializes
  the engine `Option<BuildInstruction>` into it (`serde_json::to_value`
  → `null` or the object). This PRESERVES the turns↔engine decoupling
  (turns sees opaque JSON, never the `BuildInstruction` type; the
  composition crate — the sole one depending on both — does the typed→
  JSON serialization at the boundary, exactly as it already does for
  `rust_items`/`orchestrator_items` via `serde_json::to_value(&items)`).
  Non-split arms (`Components`/`Disambiguation`/`ActionShortCircuit`)
  set `instruction: serde_json::json!(null)`.

**Substitution application scope in E.4** (the complete, non-stub
implementation): `vars = capture_variables(query, query,
&variable_patterns)` (§7.1) is applied to (a) every fetched
`ComponentItem.effective_content` in BOTH channels via
`substitute_vars` (the bodies — §0.20.3), and (b) every
`BuildInstruction.rust_steps[].tool_bindings[].params` via
`substitute_vars_in_value` (§0.4.1), mutating the `BuildInstruction` in
place before it is placed in `SplitResult.instruction`. At Phase E
`vars = []` so both are no-ops; the wiring is real and Phase M activates
it. This gives E.3's `substitute_vars_in_value` its E.4 caller (closing
the E.3 "front-loaded but unused" gap).

**`FetchForTurnResult::SplitResult` updated shape (deviates from §0.8
verbatim block, lines 1085–1091):**
```rust
SplitResult {
    rust_items:         Vec<ComponentItem>,
    orchestrator_items: Vec<ComponentItem>,
    routing:            TurnRoutingSignals,
    instruction:        Option<BuildInstruction>,   // §7.5 upgrade
}
```
A cross-reference to this section is added to `saved_plan_to_v3.md` §0.8.

**`RetrievalTurnResult` updated shape (deviates from FIND-P9-03 /
§0.8 lines 5324+):** adds `pub instruction: serde_json::Value`. All 8
construction sites updated (2 in `brassclaw_turns` test + stub, 4 in
composition bridge arms, 2 in `brassclaw_agent_loop` tests).

### 7.6 Channel fetch — two batched fetches (one per channel), faithful to §0.8 PERF-02

The §0.8 step `iv` + PERF-02 note (saved_plan lines 1182–1189) specify:
"two batched fetches (one per channel) replace N per-UUID queries". The
E.4 implementation honours this literally, NOT as one combined fetch +
Rust-side partition:

1. Gather `rust_steps[].include` UUIDs into a deduped `HashSet<Uuid>`,
   same for `orchestrator_steps[].include` (dedup within a channel).
2. Resolve each UUID's class via `lookup_component_class` (one indexed
   SELECT per UUID → `rust_pairs` / `orch_pairs` `Vec<(Uuid, i32)>`).
3. Call `fetch_components_by_ids` **once per channel**
   (`fetch_components_by_ids(&pool, scope, &rust_pairs)` then
   `&orch_pairs`) → `rust_items` / `orchestrator_items` directly. No
   `HashMap` partition on the result; the channel split is NATURAL
   because each channel fetches only its own UUIDs.
4. A UUID included by BOTH channels is fetched per-channel and so
   appears in BOTH result lists — this matches §0.8 ("fetches
   per-channel"). `fetch_components_by_ids` is O(tables) per call, so
   two calls = O(2·tables) ≈ O(tables) total (the table set per channel
   is usually disjoint: rust channel ≈ tool_skills, orch channel ≈
   skills + python_code).

An earlier draft used ONE combined `fetch_components_by_ids` call
(rust+orch UUIDs together) + a Rust-side `HashMap<Uuid, ComponentItem>`
lookup to partition — that was an unasked design deviation (it would
dedup cross-channel and lose the "appears in both lists" semantics).
Refactored to the two-per-channel form above during E.4 self-review.
Both forms verified clippy-clean; the two-per-channel form is canonical.

### 7.7 Transient test observation (not a defect)

During E.4 re-verification, `executor::orchestrator::tests::
load_reduction_rules_db_error_returns_empty_and_caches` (orchestrator.rs,
a file E.4 does NOT touch) failed once in a full
`cargo test --lib -p brassclaw_engine --features skills-db` run, then
passed in the immediately-following 4-crate combined run AND in
isolation. Reproduction attempts (isolation x3 + full engine lib suite
x2) all passed (677/677 every time). Root cause: a transient — the test
is correctly isolated (fresh `ProjectId::new()` per run giving a unique
`REDUCTION_RULE_CACHE` key + `invalidate_reduction_rules_cache()` at
test start), so cross-test cache pollution is impossible. No code change
made (modifying `orchestrator.rs` for a non-reproducible transient would
be unsolicited churn outside E.4 scope). Recorded here for traceability.

### 7.8 E.6 tests + verification (user-resolved 2026-08-22)

E.6 splits the plan's test list into a **pure-mechanism half** (true unit
tests, no DB, in `brassclaw_engine::memory::instruction_builder::tests`) and a
**DB-integration half** (testcontainer Postgres-16, in
`crates/brassclaw_reborn_composition/tests/fetch_for_turn.rs`). Three design
questions were raised with the user before E.6 implementation:

- **Q-E6-1 (where DB-dependent tests live):** DB-backed integration tests in
  the composition `tests/` tier (testcontainers + skip-if-no-docker, mirroring
  `components_registry.rs`); only #2 `knowledge:both` (pure `build_instruction`)
  + #5 substitution (pure `substitute_vars`) stay in-crate as true unit tests.
- **Q-E6-2 (test #5 at Phase E):** a pure unit test of `substitute_vars` +
  `substitute_vars_in_value` + `capture_variables` with non-empty vars (proves
  the E.4-wired mechanism) PLUS a DB test asserting the no-op
  (`{{vars.dir}}` placeholder preserved unchanged) at Phase E, with a
  doc-comment that Phase M activates real substitution.
- **Q-E6-3 (test #7 vs the existing E.1 `components_registry.rs`):** add a NEW
  distinct E.6 test (seed recipe + step-include UUIDs → `fetch_for_turn`
  resolves them end-to-end in the SplitResult flow, registry lookup exercised
  in context) IN ADDITION to the existing `components_registry.rs`.

**Pure-unit half (in `instruction_builder.rs::tests`, both configs):**
- `build_instruction_both_step_include_uuid_appears_in_both_channels` (plan
  #2) — a `knowledge: both` Component step's `include` UUID appears in BOTH
  `rust_steps` and `orchestrator_steps` of the compiled `BuildInstruction`.
- `capture_variables_then_substitute_replaces_vars_dir_in_body_and_params`
  (plan #5, pure-mechanism half) — the full `capture_variables` →
  `substitute_vars` (body) + `substitute_vars_in_value` (nested JSON params)
  chain with a non-empty `vars = [("dir","/tmp")]`.

**DB-integration half (`tests/fetch_for_turn.rs`, `#![cfg(feature="skills-db")]`,
testcontainer Postgres-16 + V000–V061 migrations + skip-if-no-docker):**
- `split_result_channels_split_by_class` (#1) — rust channel = tool_skill
  (class 13), orchestrator channel = skill (3) + python_code (22); asserts
  `instruction.rust_steps.len()==1` / `orchestrator_steps.len()==2`.
- `action_match_returns_action_short_circuit` (#3) — class-16 intent match →
  `ActionShortCircuit { component_id, name }` with `name` from the
  `resolve_intent` `reborn_actions` LEFT JOIN (no second fetch).
- `recipe_match_step_link_none_returns_components` (#4) — class-21 match with
  `step_link: None` → legacy `fetch_component_by_id` → `Components([item])`
  (class 21, recipe validated for the SEC-01 gate).
- `substitution_noop_at_phase_e_preserves_placeholder` (#5b) — exact-match
  intent ⇒ no `%` template ⇒ `vars=[]` ⇒ `{{vars.dir}}` in the skill body
  preserved unchanged (Phase M activates real substitution via the same wiring).
- `routing_wilson_lower_populated_from_recipe_row` (#6) — `tier='mature'` +
  `validated` + `wilson_lower=0.82` ⇒ `tier0_eligible=true`,
  `llm_call_required=false`, `routing.wilson_lower==0.82`.
- `registry_lookup_resolves_seeded_step_include_uuids` (#7) — seed tool_skill
  + skill, `fetch_for_turn` resolves both via the V061 trigger-maintained
  `reborn_components` registry in context (distinct from `components_registry`).
- `full_intent_match_correct_channel_split_by_class_code` (Integration#1) —
  Rust + Both + Orchestrator steps ⇒ rust_items 2×class 13 (A+B),
  orchestrator_items {13,3} (B+C), the both-step UUID B in BOTH channels,
  `wilson_lower==0.75`, `instruction.rust_steps.len()==2` /
  `orchestrator_steps.len()==2`.

Seeding: raw SQL inserts supplying only NOT-NULL-no-default columns; sub-
components seeded `validation_status='validated'` so the SEC-01 gate in
`fetch_components_by_ids` returns them; the V061 AFTER-INSERT trigger
auto-populates `reborn_components` on every insert (no manual registry rows).
`step_descriptions`/`variants` built as `serde_json::Value` then stringified
and bound `$n::jsonb` (the engine idiom — exercises the same deserialization
path production uses; UUIDs emitted as strings, which uuid's serde reads).
Intent inputs use `input_class=2` (Partial — 2-word queries) with `score=10`
for an unambiguous single-row `Match`; `step_link="0:0-0:E"` consistently.

**`.expect("instruction compiled")` safety (verified, not assumed):**
`build_instruction("0:0-0:E", &step_descs, &variable_patterns, …)` succeeds
for every test shape because: `parse_step_link("0:0-0:E")` →
`[StepRange{desc_idx:0, start:0 (first), end:End}]` selecting ALL steps in
desc_idx 0; stepnumbers are monotonic (1,2,3) ⇒ no `StepOrderViolation`; all
steps are `RecipeStepType::Component` (no `UnpromotedSnippet`); every rust
step has EMPTY `tool_bindings` ⇒ the S7 guard (`rust_has_bindings` ⇒ orch step
with non-empty include) is not triggered. So `instruction` is always `Some`
for #1 / Integration#1.

**Verification state:**
- `cargo fmt --all -- --check` clean.
- `cargo clippy -p brassclaw_engine --all-targets -- -D warnings` clean
  (default) AND `--features skills-db` clean.
- `cargo clippy -p brassclaw_reborn_composition --features
  brassclaw_reborn_composition/skills-db --all-targets -- -D warnings` clean
  (compiles `tests/fetch_for_turn.rs`).
- `cargo test -p brassclaw_engine --lib` → 668 passed (default); `--features
  skills-db` → 679 passed. Both +2 vs the E.4 baseline (666 / 677) — the two
  new pure-unit tests, no regression.
- The DB-integration half (`fetch_for_turn.rs`) is docker-gated via
  `pg_rig_or_skip` (skip-if-no-docker, the repo's canonical harness shared
  with `components_registry.rs` / `intent_step_link.rs`). **No container
  runtime (docker/colima/orbstack) and no local Postgres exist on this host**,
  so the DB half cannot be executed here — it skips to a pass, exactly as the
  pre-existing E.1 `components_registry.rs` and Phase-D `intent_step_link.rs`
  tests do on this host. It compiles + lints + formats clean in both configs
  and is correct-by-grounding against the live migration files (V027/V029/
  V033/V037/V050/V052/V028/V054/V061), the `resolve_intent` SQL, the
  `build_instruction` body, and the V061 registry triggers. It will execute
  in any environment with docker.

