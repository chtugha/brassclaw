# Subplan: Phase P Step 10 — WebUI Docs Section

**Status:** ✅ FULLY COMPLETE AND VALIDATED
**Parent plan:** subplan_phase_p_steps_2_to_11.md Step 10  
**Created:** Phase P implementation session

---

## Goal

Add a "Docs" tab to the WebUI Settings that:
1. **Lists** all `reborn_docus` rows (source + converted, with `validation_status` badge)
2. **Allows manual editing** of any doc's `content`
3. **Saving** sends the edited doc to the validation queue (`validation_status='pending'`) — never writes `'validated'` directly

---

## Architecture

The Docs tab follows the exact same pattern as the existing Skills, Recipes, and Validation Queue tabs:

| Layer | Pattern to follow |
|-------|------------------|
| DB store | `PgMemoryDocStore` / `PgPythonCodeStore` pattern |
| Rust handler | `crates/brassclaw_webui_v2/src/handlers.rs` (`list_skills`, `get_skill`, `update_skill_body`) |
| Route patterns | `crates/brassclaw_webui_v2/src/router.rs` |
| Service trait | `crates/brassclaw_product_workflow/src/reborn_services.rs` (`RebornServicesApi`) |
| Frontend JS | `crates/brassclaw_webui_v2_static/static/js/pages/settings/` (validation-queue-tab.js pattern) |
| i18n | All 10 language packs + en.js base |

---

## Implementation Steps

### Step A — PgDocusStore (Rust backend)
**Files:** `crates/brassclaw_reborn_composition/src/pg_docus_store.rs` (new)

Minimal read+write store:
```rust
pub struct PgDocusStore { pool: Arc<PgPool> }

impl PgDocusStore {
    pub async fn list_docus(&self, scope: &ComponentScope)
        -> Result<Vec<DocusRow>, PgDocusStoreError>
    pub async fn get_docus(&self, scope: &ComponentScope, id: Uuid)
        -> Result<Option<DocusRow>, PgDocusStoreError>
    pub async fn update_docus_content(&self, scope: &ComponentScope, id: Uuid, content: &str)
        -> Result<(), PgDocusStoreError>
        // Also submits to validation queue (validation_status='pending').
        // Never writes 'validated' directly.
}

pub struct DocusRow {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub content: Option<String>,
    pub content_hash: String,
    pub source: String,
    pub validation_status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
```

Reads from `reborn_docus` (V040). `update_docus_content` sets `content` + recomputes `content_hash` + sets `validation_status='pending'`.

### Step B — RebornServicesApi methods
**Files:** `crates/brassclaw_product_workflow/src/reborn_services.rs`

Add 3 methods to the `RebornServicesApi` trait:
```rust
async fn list_docus(&self, actor: &TurnActor) -> Result<Vec<DocusItem>, RebornServicesError>;
async fn get_docus(&self, actor: &TurnActor, id: Uuid) -> Result<Option<DocusItem>, RebornServicesError>;
async fn update_docus(&self, actor: &TurnActor, id: Uuid, content: String) -> Result<(), RebornServicesError>;
```

Wire in `ConnectableChannelsProductFacade`.

### Step C — WebUI v2 handler
**Files:** `crates/brassclaw_webui_v2/src/handlers.rs`

Add handler functions:
- `list_docus(State<Arc<dyn RebornServicesApi>>)` → `GET /v2/docus`
- `get_docus(Path<Uuid>, ...)` → `GET /v2/docus/:id`
- `update_docus(Path<Uuid>, Json<UpdateDocusBody>, ...)` → `PUT /v2/docus/:id`

### Step D — Route patterns
**Files:** `crates/brassclaw_webui_v2/src/router.rs`, `crates/brassclaw_webui_v2/src/descriptors.rs`

Add:
```rust
pub const WEBUI_V2_PATTERN_LIST_DOCUS: &str = "/v2/docus";
pub const WEBUI_V2_PATTERN_GET_DOCUS:  &str = "/v2/docus/:id";
```

Wire routes.

### Step E — Composition wiring
**Files:** `crates/brassclaw_reborn_composition/src/webui.rs`, `crates/brassclaw_reborn_composition/src/lib.rs`

Add `PgDocusStore` to the api builder when `pg_pool` is available. 

### Step F — Frontend JS
**Files:** `crates/brassclaw_webui_v2_static/static/js/pages/settings/components/docs-tab.js` (new)

Minimal docs tab following `validation-queue-tab.js` pattern:
- Fetch `GET /api/v1/docus` → render list with name, source, validation_status badge
- Click row → expand editor (textarea)
- Save button → `PUT /api/v1/docus/:id` with `{content: ...}` → re-fetch

### Step G — Settings nav wiring
**Files:** `crates/brassclaw_webui_v2_static/static/js/pages/settings/settings-page.js` (or equivalent)

Add "Docs" entry to the settings navigation.

### Step H — i18n keys
**Files:** All 10 language packs in `crates/brassclaw_webui_v2_static/static/js/i18n/`

Add keys:
- `settings.docs.title` = "Agent Documentation"
- `settings.docs.status.*` = validation status labels
- `settings.docs.save` = "Save"
- `settings.docs.empty` = "No docs"
- `settings.docs.source_label` = "source"
- `settings.docs.converted_label` = "converted"

---

## Commit messages

- **Step A-E:** `feat(composition,webui): Phase P Step 10a — PgDocusStore + docus API endpoints`
- **Step F-H:** `feat(webui): Phase P Step 10b — Docs settings tab + i18n keys`

---

## Out-of-scope fixes applied during implementation

### Pre-existing stubs fixed (dead-field references in tests)

The `skill_context_source` field was removed from `DefaultPlannedRuntimeParts` as part
of an earlier `plan_skill_context_removal` plan, but 3 test files still referenced it,
causing build failures. All 3 were repaired:

| File | Fix |
|------|-----|
| `crates/brassclaw_product_workflow/tests/support/planned_agent_loop.rs` | Removed `skill_context_source: None` |
| `crates/brassclaw_product_workflow/tests/inbound_turn_contract.rs` | Removed 3× `skill_context_source: None` |
| `tests/support/reborn/harness.rs` | Removed `skill_context_source: None` |

### Pre-existing struct gap fixed (new field not yet in test harness)

The `monty_driver` field (C.6 slice 4d) was added to `DefaultPlannedRuntimeParts`
but the root-workspace E2E test harness (`tests/support/reborn/harness.rs`) was never
updated to include it, causing a compile error (`missing field monty_driver`).
Fixed by adding `monty_driver: None` to the struct literal in that file.

### Validation

After all fixes, the following passed with zero warnings:
- `cargo clippy -p brassclaw_reborn -p brassclaw_reborn_composition -p brassclaw_product_workflow -p brassclaw_webui_v2 --all-targets -- -D warnings` ✅
- `cargo check --tests` ✅
- `cargo clippy --tests -- -D warnings` ✅
