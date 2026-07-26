# Subplan: Step 8.1 — Intent Inputs REST API

## Status: ✅ IMPLEMENTED

`PgIntentInputsStore` implemented in `pg_intent_inputs_store.rs`. All 3
REST routes (`GET/PUT/DELETE /api/settings/intent-inputs`) added to
`handlers.rs`, wired in `webui.rs:307`. Trait methods in
`RebornServicesApi` and overrides in `RebornServices` complete.
All steps A–G complete.

## Goal (historical)
Implement `GET/PUT/DELETE /api/settings/intent-inputs` routes so the Settings UI can
list, upsert, and delete intent example rows in `reborn_intent_inputs`.

## Layers (bottom → top)

### Step A — `list_intent_inputs` in `intent_system.rs`
Add `pub async fn list_intent_inputs(pool, scope, component_id: Option<Uuid>)` to
`crates/brassclaw_engine/src/memory/intent_system.rs` (feature `skills-db`).
Returns `Vec<IntentInputRow>` where `IntentInputRow` = `{id, input_text, input_class,
component_id, component_class_code, score, source, needs_review, created_at, updated_at}`.
SELECT FROM `reborn_intent_inputs` WHERE scope + optional component_id filter, ORDER BY
score DESC, created_at ASC, LIMIT 500.

`IntentInputRow` derives `Serialize`/`Deserialize`, defined in `intent_system.rs`.

### Step B — Service trait methods on `RebornServicesApi`
In `crates/brassclaw_product_workflow/src/reborn_services.rs`, add default-501 trait methods:
- `list_intent_inputs(caller, project_id, class_code: Option<u16>, component_id: Option<String>) -> Result<IntentInputListResponse, ...>`
- `upsert_intent_input(caller, project_id, request: UpsertIntentInputRequest) -> Result<IntentInputRow, ...>`
- `delete_intent_inputs_for_component(caller, project_id, class_code: u16, component_id: String) -> Result<u64, ...>`

Types in `crates/brassclaw_product_workflow/src/recipes.rs` (or new `intent_inputs.rs`):
```rust
pub struct IntentInputRow { pub id: String, pub input_text: String, pub input_class: i16,
    pub component_id: String, pub component_class_code: i16, pub score: i32,
    pub source: String, pub needs_review: bool }
pub struct IntentInputListResponse { pub items: Vec<IntentInputRow> }
pub struct UpsertIntentInputRequest { pub component_id: String, pub component_class_code: u16,
    pub input_text: String, pub input_class: i16 }
```

### Step C — `RebornServices` override
In `impl RebornServicesApi for RebornServices`:
- `list_intent_inputs`: requires `pg_pool` + `skills-db` feature; calls `list_intent_inputs`
  from engine via a thin pg_intent_inputs store in composition.
- `upsert_intent_input`: calls `seed_intent_input` with `IntentSource::Seeded`.
- `delete_intent_inputs_for_component`: calls `purge_component_inputs`.

Add `pg_pool: Option<Arc<PgPool>>` field with `with_pg_pool()` setter to `RebornServices`
(if not already present — check first). Otherwise use the existing injection path.

### Step D — Handlers in `handlers.rs`
```
GET  /api/settings/intent-inputs?project_id=&class_code=&component_id=
PUT  /api/settings/intent-inputs         (body: UpsertIntentInputRequest)
DELETE /api/settings/intent-inputs/{component_class_code}/{component_id}?project_id=
```

### Step E — Router + descriptors + lib.rs exports
Add 3 routes + 3 descriptors + 3 route-name constants.

### Step F — Wire `pg_pool` into `RebornServices` (if needed)
In `webui.rs`, pass `pg_pool` into `RebornServices` so the service override can use it.

### Step G — Validation
`cargo clippy -p brassclaw_webui_v2 -p brassclaw_product_workflow -p brassclaw_reborn_composition -- -D warnings`
`cargo test -p brassclaw_webui_v2`
Update checkup.md, commit, push.
