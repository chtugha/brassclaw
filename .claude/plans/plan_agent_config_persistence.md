# Plan: Agent/Networking Config Persistence (Settings Stubs)

## Context
The Agent and Networking settings tabs in the WebUI use `fetchSettingsExport()` and
`updateSetting(key, value)` to read/write config keys. Both are hardcoded stubs in
`settings-api.js` returning empty/unsuccessful responses. The underlying storage layer
(`brassclaw_config` table, `db_config.rs`) already exists.

## Root cause
`fetchSettingsExport()` → `Promise.resolve({ settings: {}, todo: true })` — no backend call.
`updateSetting(key, value)` → `Promise.resolve({ success: false, message: "TODO" })` — no backend call.

## Scope
- Read: `GET /api/settings/config` → returns agent.* heartbeat.* sandbox.* routines.* 
  safety.* skills.* search.* channels.* tunnel.* keys as `{ settings: { key: value } }`
- Write: `PUT /api/settings/config/{key}` → saves one key-value pair to `brassclaw_config`

## Security notes
- Only keys under whitelisted prefixes are exposed.
- No key starting with `brassclaw.internal.*`, `interceptor.*`, or `llm.*` is exposed.
- Writes are `ConfigWriteContext::Operator` (existing convention).

## Steps

### Step A — Add ConfigStore trait + DTOs in settings.rs
Add `SettingsConfigResponse`, `UpdateSettingRequest`, `ConfigStoreError`, `ConfigStore` trait.

### Step B — Implement in brassclaw_reborn_composition (pg_config_store.rs)
Reads via `list_config_keys`, writes via `save_config_key` with a key allowlist.

### Step C — Wire in webui.rs (postgres block)

### Step D — Add descriptors + handlers + router mount in brassclaw_webui_v2

### Step E — Wire frontend: replace `fetchSettingsExport` + `updateSetting` stubs

## Key file locations
- Trait: `crates/brassclaw_product_workflow/src/settings.rs`
- Impl: `crates/brassclaw_reborn_composition/src/pg_config_store.rs`
- Frontend: `crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/settings-api.js`

## Status: QUEUED — will be executed after parent plan completes Steps 4-6
