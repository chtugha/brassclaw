# Plan: WebUI Settings Stubs — Full Implementation

## Context
First-run audit of the BrassClaw WebUI Settings tabs revealed multiple half-implemented
or stub features. This plan implements them in dependency order, one at a time.

## Root causes
1. `list_skills` reads filesystem instead of `reborn_skills` DB table
2. Six `list_settings_*` methods in `RebornServicesApi` are 501 stubs — no composition impl
3. Settings tab icon collision (Actions + Orchestrator both use "bolt")
4. Agent/Networking `fetchSetting`/`updateSetting` in `settings-api.js` are TODO stubs
5. Users tab `fetchUsers()` is a hardcoded empty stub

## Steps

### Step 1 — pg_settings_listing: shared DB helper for settings list endpoints
Create `crates/brassclaw_reborn_composition/src/pg_settings_listing.rs`.
- Single async fn `list_components_for_settings(pool, tenant_id, table, class_code) -> Result<SettingsListResponse, ...>`
- Query: `SELECT id::text, name, class_code, prompt_uid, validation_status, COALESCE(description,'') as description, version FROM {table} WHERE tenant_id = $1 AND validation_status != 'rejected' ORDER BY class_code ASC, prompt_uid ASC LIMIT 500`
- Cover tables: reborn_skills (1-3), reborn_tools (0), reborn_actions (16), reborn_extensions_unified (4-8), reborn_orchestrators (10), reborn_scaffolds (50)
- Export from lib.rs

### Step 2 — Override list_settings_* in RebornServices (composition)
In `crates/brassclaw_reborn_composition/src/webui.rs`:
- Add a `PgSettingsListingService` (holds pool + tenant_id) wired in the postgres block
- Override the 6 methods on the composition-side `RebornServicesApi` impl
- Each calls the pg_settings_listing helper with the right table + class range

### Step 3 — Wire Skills tab to DB: pg-backed SkillsProductFacade
Create `PgSkillsProductFacade` in a new file (or extend lifecycle.rs):
- `list_skills` → queries `reborn_skills WHERE validation_status = 'validated'`
- `install_skill` → delegates to filesystem (keep existing behavior, it installs to file)
- `remove_skill` → delegates to filesystem (keep existing behavior)
- Wire in webui.rs: use `PgSkillsProductFacade` when pool is available, fall back to `RebornLocalSkillsProductFacade`

### Step 4 — Fix settings tab icon collision (JS)
In `settings-schema.js`: change `orchestrator` tab icon from "bolt" to "layers".

### Step 5 — Agent/Networking config persistence (fetchSetting / updateSetting stubs)
Audit what keys the Agent and Networking tabs try to read/save.
Check if a `/api/settings/config` or `/api/webchat/v2/config` endpoint exists.
If not: write `plan_stub_agent_config.md` and implement it.

### Step 6 — Users tab
Audit scope. Reborn currently is single-user (one WebUI token). Users tab may be
legitimately a future feature. If so: document it clearly in the tab as "not yet
available" rather than showing an empty list with no explanation.

## Files to touch
- NEW: `crates/brassclaw_reborn_composition/src/pg_settings_listing.rs`
- MOD: `crates/brassclaw_reborn_composition/src/lib.rs` (pub mod + re-export)
- MOD: `crates/brassclaw_reborn_composition/src/webui.rs` (wire listing service)
- NEW: `crates/brassclaw_reborn_composition/src/pg_skills_facade.rs` (DB-backed skills list)
- MOD: `crates/brassclaw_reborn_composition/src/webui.rs` (wire PgSkillsProductFacade)
- MOD: `crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/settings-schema.js` (icon fix)
- MOD: `crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/settings-api.js` (step 5)
- MOD: `crates/brassclaw_webui_v2_static/static/js/pages/settings/components/users-tab.js` (step 6)
