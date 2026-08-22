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
