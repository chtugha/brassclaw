# Automations v3 — Full WebUI Automations Page Plan

## Purpose

Deliver a complete, production-grade **Automations page** in the BrassClaw WebUI v2
that lets users create, view, edit, pause, resume, delete, and manually trigger
scheduled cron automations — end to end — wired into the v3 composition/product
stack as described in `saved_plan_to_v3.md`.

The read-only listing flow already exists (`GET /api/webchat/v2/automations`).
This plan adds full CRUD, a create/edit form, manual-fire, a detail panel, run
history, and the backend endpoints + composition adapter that power them. Every
layer is specified — DB, triggers crate, host-runtime capability, composition
adapter, product workflow facade, webui_v2 routes + handlers, and the SPA
frontend.

---

## 0. Architecture Overview

```
Browser (automations-page.js)
  ↓ useAutomations / useAutomationDetail
  ↓ api.js → apiFetch(...)
  ↓ GET|POST|PATCH|DELETE /api/webchat/v2/automations[/...]
  ↓ brassclaw_webui_v2 handler (router.rs + handlers.rs + descriptors.rs)
  ↓ Arc<dyn RebornServicesApi>  (brassclaw_product_workflow)
  ↓ AutomationProductFacade     (brassclaw_reborn_composition/automation.rs)
  ↓ HostRuntime capability calls  (TRIGGER_* capability IDs)
  ↓ brassclaw_host_runtime first-party tools
  ↓ TriggerRepository (brassclaw_triggers/postgres.rs)
  ↓ brassclaw_triggers table (PostgreSQL)
```

All new write endpoints go through the existing `TriggerRepository` — no new DB
tables are required. The host-runtime capability layer already abstracts the
repository. The plan adds **five new capabilities**, **five new facade trait
methods**, **five new HTTP routes**, and **a fully featured SPA page**.

---

## 1. Current State Inventory

### What already exists

| Layer | Already present |
|---|---|
| DB table | `brassclaw_triggers` (V021) — all columns needed |
| Domain types | `TriggerRecord`, `TriggerSchedule::Cron`, `TriggerState`, `TriggerCompletionPolicy`, `TriggerRunStatus` in `brassclaw_triggers/src/lib.rs` |
| Repository trait | `TriggerRepository` — `list_scoped_triggers`, `upsert_trigger`, `delete_trigger`, `get_trigger`, `patch_trigger_state` etc. in `brassclaw_triggers/src/lib.rs` |
| Poller worker | `brassclaw_triggers/src/worker.rs` — fires due triggers, submits trusted inbound turns |
| Cron parsing | `normalize_cron_expression`, `parse_cron_schedule`, `reject_sub_minute_cadence` in `brassclaw_triggers/src/lib.rs` |
| List capability | `TRIGGER_LIST_CAPABILITY_ID` in `brassclaw_host_runtime` → `automation.rs` composition adapter |
| HTTP route (read) | `GET /api/webchat/v2/automations` → `list_automations` handler |
| Facade method (read) | `RebornServicesApi::list_automations` → `AutomationProductFacade::list_automations` |
| Frontend page (read) | `automations-page.js` + `useAutomations.js` + `automations-list.js` + `automations-summary-strip.js` + `automations-presenters.js` |

### What is missing

| Layer | Gap |
|---|---|
| Host-runtime capabilities | `CREATE`, `GET`, `UPDATE_STATE` (pause/resume), `UPDATE_CONFIG` (edit), `MANUAL_FIRE`, `GET_RUN_HISTORY` |
| `TriggerRepository` methods | May be missing `get_trigger`, `create_trigger`, `delete_trigger`, `set_trigger_state` — verify and add if absent |
| Composition adapter | `create_automation`, `get_automation`, `update_automation`, `delete_automation`, `pause_automation`, `resume_automation`, `fire_automation_now`, `get_automation_run_history` |
| Product facade trait | 6 new `RebornServicesApi` methods with request/response DTOs |
| HTTP routes | `POST /automations`, `GET /automations/:id`, `PATCH /automations/:id/state`, `PATCH /automations/:id`, `DELETE /automations/:id`, `POST /automations/:id/fire` |
| Frontend | Create/edit modal, detail panel, action buttons (pause/resume/fire/delete), run history tab, i18n strings |
| Routines page | Currently a stub; this plan merges its pattern into the Automations page replacing it |

---

## 2. DB Layer — Verify Repository Completeness

### Step 2.1 — Audit `TriggerRepository` trait methods

**File:** `crates/brassclaw_triggers/src/lib.rs`

Verify the trait has ALL of the following. If any are absent, add them to the
trait and implement in `crates/brassclaw_triggers/src/postgres.rs`:

```rust
// Must exist:
async fn get_trigger(&self, tenant_id: TenantId, trigger_id: TriggerId)
    -> Result<Option<TriggerRecord>, TriggerError>;

async fn create_trigger(&self, record: TriggerRecord)
    -> Result<TriggerRecord, TriggerError>;

// May already exist as `upsert_trigger` — verify:
async fn upsert_trigger(&self, record: TriggerRecord)
    -> Result<TriggerRecord, TriggerError>;

async fn delete_trigger(&self, tenant_id: TenantId, trigger_id: TriggerId)
    -> Result<bool, TriggerError>;  // returns true if row existed

async fn set_trigger_state(
    &self,
    tenant_id: TenantId,
    trigger_id: TriggerId,
    state: TriggerState,
) -> Result<Option<TriggerRecord>, TriggerError>;  // None if not found

async fn update_trigger(
    &self,
    tenant_id: TenantId,
    trigger_id: TriggerId,
    patch: TriggerUpdatePatch,
) -> Result<Option<TriggerRecord>, TriggerError>;  // None if not found

async fn list_trigger_runs(
    &self,
    tenant_id: TenantId,
    trigger_id: TriggerId,
    limit: usize,
) -> Result<Vec<TriggerRunRecord>, TriggerError>;
```

### Step 2.2 — Add `TriggerUpdatePatch` struct (if absent)

In `crates/brassclaw_triggers/src/lib.rs`, add:

```rust
/// Partial update applied to an existing trigger record.
/// Only `Some` fields are written; `None` fields are left unchanged.
#[derive(Debug, Clone, Default)]
pub struct TriggerUpdatePatch {
    pub name: Option<String>,
    pub schedule: Option<TriggerSchedule>,
    pub prompt: Option<String>,
    pub completion_policy: Option<TriggerCompletionPolicy>,
}

impl TriggerUpdatePatch {
    pub fn validate(&self) -> Result<(), TriggerError> {
        if let Some(schedule) = &self.schedule {
            schedule.validate()?;
        }
        if let Some(name) = &self.name {
            if name.trim().is_empty() {
                return Err(TriggerError::Validation("name must not be empty".into()));
            }
        }
        Ok(())
    }
}
```

### Step 2.3 — Add `TriggerRunRecord` (for run history, if absent)

Run history is derived from the existing `brassclaw_runs` table (not a new table).
Add a query in `postgres.rs` that joins `brassclaw_runs` with
`brassclaw_triggers` via `active_run_ref` / the trigger's external event ids.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerRunRecord {
    pub run_id: String,
    pub fire_slot: Timestamp,
    pub started_at: Timestamp,
    pub finished_at: Option<Timestamp>,
    pub status: TriggerRunStatus,
}
```

The `list_trigger_runs` query:
```sql
SELECT r.run_id, r.started_at, r.finished_at,
       CASE WHEN r.error IS NOT NULL THEN 'error' ELSE 'ok' END as status,
       r.metadata->>'fire_slot' as fire_slot
FROM brassclaw_runs r
WHERE r.tenant_id = $1
  AND r.metadata->>'trigger_id' = $2
ORDER BY r.started_at DESC
LIMIT $3
```

If `metadata->>'trigger_id'` is not yet stored on runs, the fallback is to query
by thread context. Verify in `brassclaw_reborn` how the trusted-submit sets the
run metadata before implementing this query.

### Step 2.4 — PostgreSQL implementation

For each new method above, implement in `crates/brassclaw_triggers/src/postgres.rs`
following the existing patterns (deadpool-postgres, param binding, row mapping).
Key SQL shapes:

**create_trigger:**
```sql
INSERT INTO brassclaw_triggers (
    trigger_id, tenant_id, creator_user_id, agent_id, project_id,
    name, source, schedule_expression, completion_policy,
    prompt, state, next_run_at, last_run_at, last_fired_slot,
    last_status, active_fire_slot, active_run_ref, created_at
) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18)
ON CONFLICT (trigger_id) DO NOTHING
RETURNING *
```

**set_trigger_state:**
```sql
UPDATE brassclaw_triggers
SET state = $3
WHERE tenant_id = $1 AND trigger_id = $2
RETURNING *
```

**update_trigger (partial patch, only non-null columns):**
```sql
UPDATE brassclaw_triggers
SET
    name = COALESCE($3, name),
    schedule_expression = COALESCE($4, schedule_expression),
    prompt = COALESCE($5, prompt),
    completion_policy = COALESCE($6, completion_policy),
    next_run_at = COALESCE($7, next_run_at)
WHERE tenant_id = $1 AND trigger_id = $2
RETURNING *
```
When `schedule` is patched, also recompute `next_run_at` in Rust before
persisting (use `TriggerSchedule::next_slot_after(now)`).

**delete_trigger:**
```sql
DELETE FROM brassclaw_triggers
WHERE tenant_id = $1 AND trigger_id = $2
```
Return `rows_affected > 0`.

---

## 3. Host-Runtime Capability Layer

**Location:** `crates/brassclaw_host_runtime/first_party_tools/`

The existing `TRIGGER_LIST_CAPABILITY_ID` capability demonstrates the pattern.
Add six new capability ID constants and their handler implementations.

### Step 3.1 — New capability ID constants

In the trigger capability constants file (or `mod.rs`):

```rust
pub const TRIGGER_CREATE_CAPABILITY_ID: &str  = "brassclaw.triggers.create";
pub const TRIGGER_GET_CAPABILITY_ID: &str     = "brassclaw.triggers.get";
pub const TRIGGER_UPDATE_CAPABILITY_ID: &str  = "brassclaw.triggers.update";
pub const TRIGGER_SET_STATE_CAPABILITY_ID: &str = "brassclaw.triggers.set_state";
pub const TRIGGER_DELETE_CAPABILITY_ID: &str  = "brassclaw.triggers.delete";
pub const TRIGGER_FIRE_NOW_CAPABILITY_ID: &str = "brassclaw.triggers.fire_now";
pub const TRIGGER_RUN_HISTORY_CAPABILITY_ID: &str = "brassclaw.triggers.run_history";
```

### Step 3.2 — Capability handler: `triggers.create`

Input JSON:
```json
{
  "tenant_id": "...",
  "creator_user_id": "...",
  "agent_id": "...",          // optional
  "project_id": "...",        // optional
  "name": "Daily report",
  "cron": "0 8 * * *",
  "prompt": "Generate and send the daily usage report.",
  "completion_policy": "recurring"  // or "complete_after_first_fire"
}
```

Handler steps:
1. Parse and validate `cron` via `TriggerSchedule::cron(cron)?`
2. Generate `trigger_id` as a new ULID
3. Compute `next_run_at` via `schedule.next_slot_after(Utc::now())?`
4. Build `TriggerRecord` with `state: TriggerState::Scheduled`
5. Call `repository.create_trigger(record).await?`
6. Return serialized `TriggerRecord`

Output JSON: serialized `TriggerRecord`.

### Step 3.3 — Capability handler: `triggers.get`

Input: `{ "tenant_id": "...", "trigger_id": "..." }`

Handler: calls `repository.get_trigger(tenant_id, trigger_id).await?`, returns
`Option<TriggerRecord>` serialized as `{ "trigger": {...} | null }`.

### Step 3.4 — Capability handler: `triggers.update`

Input:
```json
{
  "tenant_id": "...",
  "trigger_id": "...",
  "name": "...",              // optional
  "cron": "0 9 * * *",       // optional — recomputes next_run_at
  "prompt": "...",            // optional
  "completion_policy": "..."  // optional
}
```

Handler:
1. Build `TriggerUpdatePatch` from non-null fields
2. Validate patch via `patch.validate()?`
3. If `cron` changed, compute new `next_run_at = schedule.next_slot_after(now)?`
4. Call `repository.update_trigger(tenant_id, trigger_id, patch).await?`
5. Return updated `TriggerRecord` or 404 signal

### Step 3.5 — Capability handler: `triggers.set_state`

Input: `{ "tenant_id": "...", "trigger_id": "...", "state": "paused"|"scheduled" }`

Only allows `paused` and `scheduled` transitions from product code (not
`completed`). `completed` is set only by the poller worker.

Handler: calls `repository.set_trigger_state(...)`.

### Step 3.6 — Capability handler: `triggers.delete`

Input: `{ "tenant_id": "...", "trigger_id": "..." }`

Handler:
1. If trigger has `active_fire_slot` (currently firing), return error
   `{ "error": "trigger_has_active_fire" }` — do not delete while a run is in
   flight. Caller should pause first then retry.
2. Call `repository.delete_trigger(tenant_id, trigger_id).await?`
3. Return `{ "deleted": true|false }`

### Step 3.7 — Capability handler: `triggers.fire_now`

Input: `{ "tenant_id": "...", "trigger_id": "...", "requester_user_id": "..." }`

This performs a **manual one-shot fire** — it synthesizes a fire-slot using the
current wall-clock timestamp (ensuring uniqueness) and submits a trusted inbound
turn request exactly as the poller does.

Handler:
1. Load trigger via `repository.get_trigger(...)`. Return error if not found.
2. Check state is `Scheduled` (not paused / completed).
3. Generate `fire_slot = now` (as RFC-3339 timestamp string, used as
   deduplication key).
4. Call the same `TrustedTriggerFireSubmitter::submit(...)` the poller worker
   uses, passing the fire identity.
5. Return `{ "run_ref": "<turn_run_id>" }` on success.

**Security:** The `TrustedTriggerFireSubmitter` is only available inside the
host-runtime capability layer, which is already trusted. Product code cannot
call it directly.

### Step 3.8 — Capability handler: `triggers.run_history`

Input: `{ "tenant_id": "...", "trigger_id": "...", "limit": 20 }`

Handler: calls `repository.list_trigger_runs(tenant_id, trigger_id, limit)`.
Returns `{ "runs": [...] }`.

### Step 3.9 — Register all new capabilities

In the capability registry (wherever `TRIGGER_LIST_CAPABILITY_ID` is already
registered), register each new constant with its handler. Follow the existing
capability-handler registration pattern.

---

## 4. Composition Adapter — `automation.rs`

**File:** `crates/brassclaw_reborn_composition/src/automation.rs`

Add new public(crate) methods to `RebornWebuiAutomationFacade`.

### Step 4.1 — `create_automation`

```rust
pub(crate) async fn create_automation(
    &self,
    caller: ProductAgentBoundCaller,
    input: CreateAutomationInput,
) -> Result<RebornAutomationInfo, RebornServicesError> {
    let output = self.invoke_trigger(
        caller,
        TRIGGER_CREATE_CAPABILITY_ID,
        json!({
            "name": input.name,
            "cron": input.cron,
            "prompt": input.prompt,
            "completion_policy": input.completion_policy
                .unwrap_or("recurring"),
        }),
    ).await?;
    parse_single_automation_output(output)
}
```

`CreateAutomationInput` (new struct in composition):
```rust
pub(crate) struct CreateAutomationInput {
    pub name: String,
    pub cron: String,
    pub prompt: String,
    pub completion_policy: Option<String>,
}
```

### Step 4.2 — `get_automation`

```rust
pub(crate) async fn get_automation(
    &self,
    caller: ProductAgentBoundCaller,
    trigger_id: String,
) -> Result<Option<RebornAutomationInfo>, RebornServicesError>
```

Invokes `TRIGGER_GET_CAPABILITY_ID`.

### Step 4.3 — `update_automation`

```rust
pub(crate) async fn update_automation(
    &self,
    caller: ProductAgentBoundCaller,
    trigger_id: String,
    patch: UpdateAutomationInput,
) -> Result<Option<RebornAutomationInfo>, RebornServicesError>
```

`UpdateAutomationInput`:
```rust
pub(crate) struct UpdateAutomationInput {
    pub name: Option<String>,
    pub cron: Option<String>,
    pub prompt: Option<String>,
    pub completion_policy: Option<String>,
}
```

### Step 4.4 — `set_automation_state`

```rust
pub(crate) async fn set_automation_state(
    &self,
    caller: ProductAgentBoundCaller,
    trigger_id: String,
    state: RebornAutomationStateAction,  // Pause | Resume
) -> Result<Option<RebornAutomationInfo>, RebornServicesError>
```

`RebornAutomationStateAction` maps `Pause → "paused"`, `Resume → "scheduled"`.

### Step 4.5 — `delete_automation`

```rust
pub(crate) async fn delete_automation(
    &self,
    caller: ProductAgentBoundCaller,
    trigger_id: String,
) -> Result<RebornDeleteAutomationResult, RebornServicesError>
```

`RebornDeleteAutomationResult`:
```rust
pub enum RebornDeleteAutomationResult {
    Deleted,
    NotFound,
    HasActiveFire,  // fires 409 Conflict upstream
}
```

### Step 4.6 — `fire_automation_now`

```rust
pub(crate) async fn fire_automation_now(
    &self,
    caller: ProductAgentBoundCaller,
    trigger_id: String,
) -> Result<RebornFireAutomationResult, RebornServicesError>
```

`RebornFireAutomationResult { run_ref: String }`.

### Step 4.7 — `get_automation_run_history`

```rust
pub(crate) async fn get_automation_run_history(
    &self,
    caller: ProductAgentBoundCaller,
    trigger_id: String,
    limit: usize,
) -> Result<Vec<RebornAutomationRunRecord>, RebornServicesError>
```

`RebornAutomationRunRecord`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebornAutomationRunRecord {
    pub run_id: String,
    pub fire_slot: String,           // RFC-3339
    pub started_at: String,          // RFC-3339
    pub finished_at: Option<String>, // RFC-3339
    pub status: RebornAutomationRunStatus,
}
```

---

## 5. Product Workflow Facade — `brassclaw_product_workflow`

### Step 5.1 — New DTO types

**File:** `crates/brassclaw_product_workflow/src/reborn_services/types.rs`

Add the following types (following the existing `RebornListAutomationsResponse` pattern):

```rust
/// POST /automations request body
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebUiCreateAutomationRequest {
    pub name: String,
    pub cron: String,
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_policy: Option<String>,
}

/// PATCH /automations/:id body
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WebUiUpdateAutomationRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_policy: Option<String>,
}

/// PATCH /automations/:id/state body
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebUiSetAutomationStateRequest {
    pub action: AutomationStateAction,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationStateAction {
    Pause,
    Resume,
}

/// Responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebornGetAutomationResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automation: Option<RebornAutomationInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebornCreateAutomationResponse {
    pub automation: RebornAutomationInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebornUpdateAutomationResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automation: Option<RebornAutomationInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebornDeleteAutomationResponse {
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebornFireAutomationNowResponse {
    pub run_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebornAutomationRunHistoryResponse {
    pub runs: Vec<RebornAutomationRunRecord>,
}

// Also add RebornAutomationRunRecord (mirrors composition type, fully serializable):
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebornAutomationRunRecord {
    pub run_id: String,
    pub fire_slot: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: RebornAutomationRunStatus,
}
```

Also extend `RebornAutomationInfo` with the `prompt` and `completion_policy`
fields (needed by the detail panel and edit form):

```rust
// Add to existing RebornAutomationInfo:
#[serde(default, skip_serializing_if = "Option::is_none")]
pub prompt: Option<String>,
#[serde(default, skip_serializing_if = "Option::is_none")]
pub completion_policy: Option<String>,
```

### Step 5.2 — New `RebornServicesApi` trait methods

**File:** `crates/brassclaw_product_workflow/src/reborn_services.rs`

Add six new methods to the `RebornServicesApi` trait. Each gets a **default
"unavailable" body** that returns a 501 so existing fakes and tests keep
compiling:

```rust
async fn create_automation(
    &self,
    caller: WebUiAuthenticatedCaller,
    request: WebUiCreateAutomationRequest,
) -> Result<RebornCreateAutomationResponse, RebornServicesError> {
    let _ = (caller, request);
    Err(RebornServicesError::from_status(
        RebornServicesErrorCode::InvalidRequest, 501, false,
    ))
}

async fn get_automation(
    &self,
    caller: WebUiAuthenticatedCaller,
    automation_id: String,
) -> Result<RebornGetAutomationResponse, RebornServicesError> {
    let _ = (caller, automation_id);
    Err(RebornServicesError::from_status(
        RebornServicesErrorCode::InvalidRequest, 501, false,
    ))
}

async fn update_automation(
    &self,
    caller: WebUiAuthenticatedCaller,
    automation_id: String,
    request: WebUiUpdateAutomationRequest,
) -> Result<RebornUpdateAutomationResponse, RebornServicesError> {
    let _ = (caller, automation_id, request);
    Err(RebornServicesError::from_status(
        RebornServicesErrorCode::InvalidRequest, 501, false,
    ))
}

async fn set_automation_state(
    &self,
    caller: WebUiAuthenticatedCaller,
    automation_id: String,
    request: WebUiSetAutomationStateRequest,
) -> Result<RebornUpdateAutomationResponse, RebornServicesError> {
    let _ = (caller, automation_id, request);
    Err(RebornServicesError::from_status(
        RebornServicesErrorCode::InvalidRequest, 501, false,
    ))
}

async fn delete_automation(
    &self,
    caller: WebUiAuthenticatedCaller,
    automation_id: String,
) -> Result<RebornDeleteAutomationResponse, RebornServicesError> {
    let _ = (caller, automation_id);
    Err(RebornServicesError::from_status(
        RebornServicesErrorCode::InvalidRequest, 501, false,
    ))
}

async fn fire_automation_now(
    &self,
    caller: WebUiAuthenticatedCaller,
    automation_id: String,
) -> Result<RebornFireAutomationNowResponse, RebornServicesError> {
    let _ = (caller, automation_id);
    Err(RebornServicesError::from_status(
        RebornServicesErrorCode::InvalidRequest, 501, false,
    ))
}

async fn get_automation_run_history(
    &self,
    caller: WebUiAuthenticatedCaller,
    automation_id: String,
    limit: Option<u32>,
) -> Result<RebornAutomationRunHistoryResponse, RebornServicesError> {
    let _ = (caller, automation_id, limit);
    Err(RebornServicesError::from_status(
        RebornServicesErrorCode::InvalidRequest, 501, false,
    ))
}
```

### Step 5.3 — Implement all methods on `RebornServices`

In `crates/brassclaw_product_workflow/src/reborn_services.rs`, add concrete
implementations that:

1. Call `product_agent_bound_caller_from_webui(caller)` — return 400 if `None`
2. Delegate to `self.automation_facade.<method>(...)` using the new composition
   adapter methods from §4
3. Map `RebornDeleteAutomationResult::HasActiveFire` → 409 HTTP status

Validation performed **before** calling the facade (all in the product layer):
- `create_automation`: name non-empty ≤ 200 chars; cron must parse (validate
  client-side too, but server is authoritative); prompt non-empty ≤ 8 000 chars
- `update_automation`: same per-field constraints for any non-null field
- `set_automation_state`: action must be `pause` or `resume`

---

## 6. WebUI v2 HTTP Routes & Handlers

### Step 6.1 — New route constants & patterns

**File:** `crates/brassclaw_webui_v2/src/descriptors.rs`

```rust
// Route IDs
pub const WEBUI_V2_ROUTE_CREATE_AUTOMATION: &str    = "webui.v2.create_automation";
pub const WEBUI_V2_ROUTE_GET_AUTOMATION: &str       = "webui.v2.get_automation";
pub const WEBUI_V2_ROUTE_UPDATE_AUTOMATION: &str    = "webui.v2.update_automation";
pub const WEBUI_V2_ROUTE_SET_AUTOMATION_STATE: &str = "webui.v2.set_automation_state";
pub const WEBUI_V2_ROUTE_DELETE_AUTOMATION: &str    = "webui.v2.delete_automation";
pub const WEBUI_V2_ROUTE_FIRE_AUTOMATION_NOW: &str  = "webui.v2.fire_automation_now";
pub const WEBUI_V2_ROUTE_GET_AUTOMATION_RUNS: &str  = "webui.v2.get_automation_run_history";

// URL patterns
pub const WEBUI_V2_PATTERN_AUTOMATIONS: &str              = "/api/webchat/v2/automations";
pub const WEBUI_V2_PATTERN_AUTOMATION_ID: &str            = "/api/webchat/v2/automations/:automation_id";
pub const WEBUI_V2_PATTERN_AUTOMATION_STATE: &str         = "/api/webchat/v2/automations/:automation_id/state";
pub const WEBUI_V2_PATTERN_AUTOMATION_FIRE: &str          = "/api/webchat/v2/automations/:automation_id/fire";
pub const WEBUI_V2_PATTERN_AUTOMATION_RUNS: &str          = "/api/webchat/v2/automations/:automation_id/runs";
```

Add descriptor functions for each:
```rust
fn create_automation_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_CREATE_AUTOMATION,
        NetworkMethod::Post,
        WEBUI_V2_PATTERN_AUTOMATIONS,
        mutation_policy(
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
            StreamingMode::None,
        ),
    )
}
// ... similarly for each verb/route
```

Manual fire uses a **tighter rate limit** (e.g. 10/minute per caller) to prevent
abuse.

### Step 6.2 — Route mounting

**File:** `crates/brassclaw_webui_v2/src/router.rs`

```rust
// /api/webchat/v2/automations  (list existing, create new)
.route(
    WEBUI_V2_PATTERN_AUTOMATIONS,
    get(handlers::list_automations).post(handlers::create_automation),
)
// /api/webchat/v2/automations/:automation_id  (get, update, delete)
.route(
    WEBUI_V2_PATTERN_AUTOMATION_ID,
    get(handlers::get_automation)
        .patch(handlers::update_automation)
        .delete(handlers::delete_automation),
)
// /api/webchat/v2/automations/:automation_id/state
.route(
    WEBUI_V2_PATTERN_AUTOMATION_STATE,
    post(handlers::set_automation_state),
)
// /api/webchat/v2/automations/:automation_id/fire
.route(
    WEBUI_V2_PATTERN_AUTOMATION_FIRE,
    post(handlers::fire_automation_now),
)
// /api/webchat/v2/automations/:automation_id/runs
.route(
    WEBUI_V2_PATTERN_AUTOMATION_RUNS,
    get(handlers::get_automation_run_history),
)
```

### Step 6.3 — Handler implementations

**File:** `crates/brassclaw_webui_v2/src/handlers.rs`

Each handler is thin — extract params, build request, delegate to
`state.services()`:

```rust
/// POST /api/webchat/v2/automations
pub async fn create_automation(
    State(state): State<WebUiV2State>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    Json(body): Json<WebUiCreateAutomationRequest>,
) -> Result<(StatusCode, Json<RebornCreateAutomationResponse>), WebUiV2HttpError> {
    let response = state.services().create_automation(caller, body).await?;
    Ok((StatusCode::CREATED, Json(response)))
}

/// GET /api/webchat/v2/automations/:automation_id
pub async fn get_automation(
    State(state): State<WebUiV2State>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    Path(automation_id): Path<String>,
) -> Result<Json<RebornGetAutomationResponse>, WebUiV2HttpError> {
    let response = state.services()
        .get_automation(caller, automation_id)
        .await?;
    Ok(Json(response))
}

/// PATCH /api/webchat/v2/automations/:automation_id
pub async fn update_automation(
    State(state): State<WebUiV2State>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    Path(automation_id): Path<String>,
    Json(body): Json<WebUiUpdateAutomationRequest>,
) -> Result<Json<RebornUpdateAutomationResponse>, WebUiV2HttpError> {
    let response = state.services()
        .update_automation(caller, automation_id, body)
        .await?;
    Ok(Json(response))
}

/// POST /api/webchat/v2/automations/:automation_id/state
pub async fn set_automation_state(
    State(state): State<WebUiV2State>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    Path(automation_id): Path<String>,
    Json(body): Json<WebUiSetAutomationStateRequest>,
) -> Result<Json<RebornUpdateAutomationResponse>, WebUiV2HttpError> {
    let response = state.services()
        .set_automation_state(caller, automation_id, body)
        .await?;
    Ok(Json(response))
}

/// DELETE /api/webchat/v2/automations/:automation_id
pub async fn delete_automation(
    State(state): State<WebUiV2State>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    Path(automation_id): Path<String>,
) -> Result<Json<RebornDeleteAutomationResponse>, WebUiV2HttpError> {
    let response = state.services()
        .delete_automation(caller, automation_id)
        .await?;
    Ok(Json(response))
}

/// POST /api/webchat/v2/automations/:automation_id/fire
pub async fn fire_automation_now(
    State(state): State<WebUiV2State>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    Path(automation_id): Path<String>,
) -> Result<Json<RebornFireAutomationNowResponse>, WebUiV2HttpError> {
    let response = state.services()
        .fire_automation_now(caller, automation_id)
        .await?;
    Ok(Json(response))
}

/// GET /api/webchat/v2/automations/:automation_id/runs
pub async fn get_automation_run_history(
    State(state): State<WebUiV2State>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    Path(automation_id): Path<String>,
    Query(query): Query<AutomationRunsQuery>,
) -> Result<Json<RebornAutomationRunHistoryResponse>, WebUiV2HttpError> {
    let response = state.services()
        .get_automation_run_history(caller, automation_id, query.limit)
        .await?;
    Ok(Json(response))
}

#[derive(Debug, Default, Deserialize)]
pub struct AutomationRunsQuery {
    #[serde(default)]
    pub limit: Option<u32>,
}
```

### Step 6.4 — Update descriptor contract test

**File:** `crates/brassclaw_webui_v2/tests/webui_v2_descriptors_contract.rs`

Add all seven new routes to the locked route table. The test will fail to compile
until all new route IDs and patterns are registered.

---

## 7. Frontend — API Client Layer

**File:** `crates/brassclaw_webui_v2_static/static/js/lib/api.js`

Add below the existing `listAutomations`:

```javascript
export function createAutomation({ name, cron, prompt, completionPolicy }) {
  return apiFetch(`${V2_BASE}/automations`, {
    method: "POST",
    body: JSON.stringify({
      name,
      cron,
      prompt,
      completion_policy: completionPolicy,
    }),
  });
}

export function getAutomation(automationId) {
  return apiFetch(`${V2_BASE}/automations/${encodeURIComponent(automationId)}`);
}

export function updateAutomation(automationId, patch) {
  // patch: { name?, cron?, prompt?, completionPolicy? }
  const body = {};
  if (patch.name != null)             body.name = patch.name;
  if (patch.cron != null)             body.cron = patch.cron;
  if (patch.prompt != null)           body.prompt = patch.prompt;
  if (patch.completionPolicy != null) body.completion_policy = patch.completionPolicy;
  return apiFetch(
    `${V2_BASE}/automations/${encodeURIComponent(automationId)}`,
    { method: "PATCH", body: JSON.stringify(body) },
  );
}

export function setAutomationState(automationId, action) {
  // action: "pause" | "resume"
  return apiFetch(
    `${V2_BASE}/automations/${encodeURIComponent(automationId)}/state`,
    { method: "POST", body: JSON.stringify({ action }) },
  );
}

export function deleteAutomation(automationId) {
  return apiFetch(
    `${V2_BASE}/automations/${encodeURIComponent(automationId)}`,
    { method: "DELETE" },
  );
}

export function fireAutomationNow(automationId) {
  return apiFetch(
    `${V2_BASE}/automations/${encodeURIComponent(automationId)}/fire`,
    { method: "POST" },
  );
}

export function getAutomationRunHistory(automationId, { limit } = {}) {
  const params = new URLSearchParams();
  if (limit != null) params.set("limit", String(limit));
  const query = params.toString();
  return apiFetch(
    `${V2_BASE}/automations/${encodeURIComponent(automationId)}/runs${
      query ? `?${query}` : ""
    }`,
  );
}
```

---

## 8. Frontend — Hooks Layer

**Directory:** `crates/brassclaw_webui_v2_static/static/js/pages/automations/hooks/`

### Step 8.1 — Extend `useAutomations.js`

Add mutations to the existing hook:

```javascript
// useAutomations.js (extended)
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  listAutomations, createAutomation, deleteAutomation,
  setAutomationState, fireAutomationNow,
} from "../../../lib/api.js";

// ... existing query unchanged ...

const queryClient = useQueryClient();
const invalidate = () => queryClient.invalidateQueries({ queryKey: ["automations"] });

const create = useMutation({
  mutationFn: createAutomation,
  onSuccess: invalidate,
});
const remove = useMutation({
  mutationFn: (id) => deleteAutomation(id),
  onSuccess: invalidate,
});
const setState = useMutation({
  mutationFn: ({ id, action }) => setAutomationState(id, action),
  onSuccess: invalidate,
});
const fire = useMutation({
  mutationFn: (id) => fireAutomationNow(id),
  onSuccess: invalidate,
});

return {
  // existing fields ...
  create, remove, setState, fire,
};
```

### Step 8.2 — New `useAutomationDetail.js`

**File:** `crates/brassclaw_webui_v2_static/static/js/pages/automations/hooks/useAutomationDetail.js`

```javascript
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  getAutomation, updateAutomation, getAutomationRunHistory,
} from "../../../lib/api.js";

const RUN_HISTORY_LIMIT = 20;

export function useAutomationDetail(automationId) {
  const queryClient = useQueryClient();

  const detailQuery = useQuery({
    queryKey: ["automations", automationId],
    queryFn: () => getAutomation(automationId),
    enabled: !!automationId,
  });

  const runsQuery = useQuery({
    queryKey: ["automations", automationId, "runs"],
    queryFn: () => getAutomationRunHistory(automationId, { limit: RUN_HISTORY_LIMIT }),
    enabled: !!automationId,
  });

  const update = useMutation({
    mutationFn: (patch) => updateAutomation(automationId, patch),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["automations"] });
      queryClient.invalidateQueries({ queryKey: ["automations", automationId] });
    },
  });

  return {
    automation: detailQuery.data?.automation ?? null,
    isLoadingDetail: detailQuery.isLoading,
    detailError: detailQuery.error ?? null,
    runs: runsQuery.data?.runs ?? [],
    isLoadingRuns: runsQuery.isLoading,
    update,
  };
}
```

---

## 9. Frontend — Components Layer

**Directory:** `crates/brassclaw_webui_v2_static/static/js/pages/automations/components/`

### Step 9.1 — Extend `automations-list.js`

Add action buttons per row (pause/resume, fire now, open detail):

```javascript
// Each row gets:
// - [Pause] or [Resume] button depending on is_active
// - [Run now] button (disabled if automation has active_fire)
// - [→] / row click → open detail panel
```

Update `filterAutomations` in presenters to also support a `"completed"` filter
bucket.

### Step 9.2 — New `automation-create-modal.js`

**File:** `crates/brassclaw_webui_v2_static/static/js/pages/automations/components/automation-create-modal.js`

A modal dialog with a controlled form:

```javascript
Fields:
- name:               text input (required, ≤200 chars)
- cron:               text input with inline cron preview ("next fire: …")
- prompt:             textarea (required, ≤8000 chars)
- completionPolicy:   select — "Recurring" | "Run once"
- [Create] button     disabled while mutation.isPending
- [Cancel] button
```

Cron preview: call `nextCronFire(cron)` from `automations-presenters.js` (a new
helper that wraps the existing `scheduleLabel` logic for the preview).

**Validation (client-side mirror of server rules):**
- name non-empty ≤ 200 chars
- cron: try parsing — show error if invalid
- prompt non-empty ≤ 8 000 chars

### Step 9.3 — New `automation-detail-panel.js`

**File:** `crates/brassclaw_webui_v2_static/static/js/pages/automations/components/automation-detail-panel.js`

A side panel (slides in from right, inspired by the routines `routine-detail-panel.js`)
with three tabs:

**Overview tab:**
- Automation name (editable inline on click)
- Status pill + state action button (Pause / Resume)
- Schedule: cron expression + human label
- Next fire / Last run timestamps
- Prompt text (editable, textarea)
- [Save changes] / [Discard] if anything edited
- [Run now] button
- [Delete] button (with confirmation dialog)

**Run history tab:**
- List of recent runs (from `useAutomationDetail.runs`)
- Per run: fire slot date, started_at, finished_at, duration, status pill
- Empty state if no runs yet

**Raw tab:**
- JSON view of the full `RebornAutomationInfo` response for debugging

### Step 9.4 — New `automation-delete-confirm.js`

A small confirmation dialog before deleting:

```javascript
// "Delete automation?" + [Cancel] [Delete]
// Shows a warning if automation has an active fire in flight
// (backend returns 409 HasActiveFire — map to "Automation is currently running.
//  Pause it first, then try again.")
```

### Step 9.5 — Extend `automations-presenters.js`

Add:
```javascript
export function nextCronFire(cronExpression) {
  // Returns a locale-formatted "next fire" string for cron preview.
  // Implementation: parse the 5-field cron manually or use a small
  // client-side cron library (cron-parser via ESM CDN import).
  // Returns null if expression is invalid.
}

export function durationLabel(startedAt, finishedAt) {
  // "2m 14s" style duration, or "—" if finishedAt is null
}

export function runStatusTone(status) {
  if (status === "ok")    return "success";
  if (status === "error") return "danger";
  return "muted";
}
```

---

## 10. Frontend — Page Assembly

### Step 10.1 — Update `automations-page.js`

Restructure to support the side panel pattern (similar to `routines-page.js`):

```javascript
export function AutomationsPage() {
  const { automationId } = useParams();     // /automations/:automationId
  const navigate = useNavigate();
  const t = useT();
  const automationsState = useAutomations();
  const [showCreate, setShowCreate] = React.useState(false);

  return html`
    <div className="flex h-full overflow-hidden">
      <!-- Main list column -->
      <div className="flex flex-col flex-1 overflow-y-auto min-w-0">
        <div className="v2-page-entrance p-4 sm:p-6">
          <!-- Header row: title + [+ New automation] button -->
          <div className="flex items-center justify-between mb-4">
            <h1 className="text-lg font-semibold">${t("automations.title")}</h1>
            <button onClick=${() => setShowCreate(true)} className="btn-primary">
              ${t("automations.new")}
            </button>
          </div>

          ${automationsState.error && html`<${ErrorBanner} />`}
          ${automationsState.isLoading
            ? html`<${SkeletonLoader} />`
            : html`
              <${AutomationsSummaryStrip} summary=${automationsState.summary} />
              <${AutomationsList}
                automations=${automationsState.automations}
                selectedId=${automationId}
                onSelect=${(id) => navigate(\`/automations/\${id}\`)}
                onPause=${(id) => automationsState.setState.mutate({ id, action: "pause" })}
                onResume=${(id) => automationsState.setState.mutate({ id, action: "resume" })}
                onFire=${(id) => automationsState.fire.mutate(id)}
                onDelete=${(id) => navigate(\`/automations/\${id}?delete=1\`)}
              />
            `}
        </div>
      </div>

      <!-- Detail panel (when an automation is selected) -->
      ${automationId && html`
        <${AutomationDetailPanel}
          automationId=${automationId}
          onClose=${() => navigate("/automations")}
          onDeleted=${() => {
            automationsState.refetch();
            navigate("/automations");
          }}
          onUpdated=${automationsState.refetch}
        />
      `}

      <!-- Create modal -->
      ${showCreate && html`
        <${AutomationCreateModal}
          onClose=${() => setShowCreate(false)}
          onCreated=${(automation) => {
            setShowCreate(false);
            automationsState.refetch();
            navigate(\`/automations/\${automation.automation_id}\`);
          }}
          create=${automationsState.create}
        />
      `}
    </div>
  `;
}
```

### Step 10.2 — Update React Router in `app.js`

The automations route already exists as `/automations`. Add the ID sub-route:

```javascript
// app.js — inside the authenticated layout routes
{ path: "/automations", element: html`<${AutomationsPage} />` },
{ path: "/automations/:automationId", element: html`<${AutomationsPage} />` },
```

### Step 10.3 — Update navigation sidebar

**File:** `crates/brassclaw_webui_v2_static/static/js/layout/gateway-layout.js`

Ensure "Automations" appears in the sidebar nav (it should already be there; if
not, add it with the calendar/schedule icon next to "Jobs").

---

## 11. Frontend — i18n Strings

**File:** `crates/brassclaw_webui_v2_static/static/js/i18n/en.js`

Add all new strings under an `automations` namespace:

```javascript
automations: {
  title:             "Automations",
  new:               "New automation",
  empty_title:       "No automations yet",
  empty_body:        "Create a scheduled automation to run tasks automatically.",
  filter_all:        "All",
  filter_active:     "Active",
  filter_paused:     "Paused",
  filter_completed:  "Completed",

  // Summary strip
  summary_scheduled: "Scheduled",
  summary_active:    "Active",
  summary_paused:    "Paused",
  summary_next_run:  "Next run",

  // Detail panel tabs
  tab_overview:      "Overview",
  tab_runs:          "Run history",
  tab_raw:           "Raw",

  // Overview fields
  field_name:        "Name",
  field_schedule:    "Schedule",
  field_next_fire:   "Next fire",
  field_last_run:    "Last run",
  field_prompt:      "Prompt",
  field_policy:      "Completion policy",
  field_created:     "Created",
  field_id:          "Automation ID",

  // Completion policy labels
  policy_recurring:              "Recurring",
  policy_complete_after_first:   "Run once",

  // Actions
  action_pause:      "Pause",
  action_resume:     "Resume",
  action_fire:       "Run now",
  action_edit:       "Edit",
  action_delete:     "Delete",
  action_save:       "Save changes",
  action_discard:    "Discard",

  // Create modal
  create_title:          "New automation",
  create_name_label:     "Name",
  create_name_hint:      "A short label for this automation",
  create_cron_label:     "Schedule (cron)",
  create_cron_hint:      "e.g. 0 8 * * * (daily at 8 AM)",
  create_cron_preview:   "Next fire:",
  create_prompt_label:   "Prompt",
  create_prompt_hint:    "What should the agent do when this fires?",
  create_policy_label:   "Completion policy",
  create_button:         "Create",

  // Delete confirm
  delete_title:      "Delete automation?",
  delete_body:       "This will permanently delete "{name}" and all its history.",
  delete_confirm:    "Delete",
  delete_cancel:     "Cancel",
  delete_active_warning: "This automation is currently running. Pause it first.",

  // Run history
  runs_empty:        "No runs yet.",
  run_started:       "Started",
  run_finished:      "Finished",
  run_duration:      "Duration",
  run_status:        "Status",

  // Status labels (aligned with existing STATE_PRESENTATION)
  state_active:      "Active",
  state_scheduled:   "Scheduled",
  state_paused:      "Paused",
  state_completed:   "Completed",
  state_unknown:     "Unknown",
  status_ok:         "Done",
  status_error:      "Error",

  // Errors
  error_invalid_cron:    "Invalid cron expression",
  error_name_required:   "Name is required",
  error_prompt_required: "Prompt is required",
  error_has_active_fire: "Automation is running — pause it first, then try again.",
  error_not_found:       "Automation not found",
},
```

---

## 12. Routines Page — Replace Stub with Real Implementation

The `routines-page.js` and its `routines-api.js` stub are dead code since the
Automations page now covers the same domain. Two options:

**Option A (Recommended):** Redirect `/routines/*` → `/automations/*` by
updating `app.js`:

```javascript
// Replace routines route with a redirect:
{ path: "/routines", element: html`<${Navigate} to="/automations" replace />` },
{ path: "/routines/:routineId", element: html`<${Navigate} to="/automations" replace />` },
```

Remove the Routines link from the sidebar nav. Keep the files to avoid breaking
any external links cached by browsers, but mark them as deprecated in a
comment.

**Option B:** Implement routines as a **scoped view** — same API endpoints,
same backend, but filtered to only `completion_policy: "complete_after_first_fire"`
automations. Only choose this if the product distinction between "routine" and
"automation" is meaningful to users.

The recommended path is **Option A** — one page, all triggers.

---

## 13. Validation / Error Mapping

### HTTP error codes

| Condition | HTTP status | `error_code` field |
|---|---|---|
| Caller not bound to agent | 400 | `invalid_request` |
| Trigger not found | 404 | `not_found` |
| Trigger has active fire (on delete) | 409 | `trigger_has_active_fire` |
| Cron expression invalid | 422 | `validation_error` |
| Name/prompt fails length check | 422 | `validation_error` |
| Pausing completed trigger | 409 | `state_conflict` |
| Fire now on paused trigger | 409 | `state_conflict` |
| Internal / unexpected | 500 | `internal_error` |

### Frontend error handling

`apiFetch` throws `ApiError` on non-2xx. Components should:
- Show inline form validation errors for 422 (`error.payload.details`)
- Show inline action error banners for 409 (`error_code === "trigger_has_active_fire"`)
- Show toast notifications for 500s
- Silently redirect to list on 404 in detail panel

---

## 14. Security Invariants

1. **Caller scoping is non-negotiable.** Every capability call must pass the
   `tenant_id` resolved from `ProductAgentBoundCaller`, never from the request
   body. The JSON inputs to capabilities are assembled by composition code, not
   by the browser.

2. **Manual fire goes through `TrustedTriggerFireSubmitter`.** Product code
   must never mint a `TrustedInboundTurnRequest` directly. The capability
   handler (in host_runtime) is the only place this is assembled.

3. **State machine protection.** Only `Scheduled ↔ Paused` transitions are
   allowed from product code. Setting `Completed` is a poller-internal
   operation. Enforce this in the `set_trigger_state` capability handler.

4. **Deletion guard.** Triggers with `active_fire_slot IS NOT NULL` must not
   be deleted mid-flight. The capability checks `has_active_fire()` and returns
   an error; the handler maps it to 409.

5. **Rate limiting.** `fire_automation_now` should have a tighter rate limit
   (10/minute per caller) than list/read endpoints to prevent manual firing
   from being used as a DoS vector against the turn runner.

6. **Creator-scoped listing.** `list_scoped_triggers` filters by
   `(tenant_id, creator_user_id, agent_id)` — a user cannot see or operate on
   another user's triggers. Verify this is enforced in `list_automations`.

---

## 15. Tests

### Step 15.1 — `brassclaw_triggers` unit tests

- `TriggerUpdatePatch::validate()` — empty name, long name, invalid cron
- `set_trigger_state` SQL correctness (mocked pool)
- `list_trigger_runs` query shape

### Step 15.2 — `brassclaw_reborn_composition` unit tests

- `RebornWebuiAutomationFacade::create_automation` — happy path + cron parse
  error propagation
- `RebornWebuiAutomationFacade::delete_automation` — `HasActiveFire` variant
  propagation
- `parse_single_automation_output` — unknown fields are tolerated, unknown
  schedule kind filtered

### Step 15.3 — `brassclaw_product_workflow` unit tests

- Validation rejection for empty name, invalid cron, prompt > 8 000 chars
- `delete_automation` maps `HasActiveFire` → 409
- `product_agent_bound_caller_from_webui` returns None → 400 propagation

### Step 15.4 — `brassclaw_webui_v2` handler tests

- All seven new routes present in `webui_v2_descriptors_contract.rs`
- `create_automation` returns 201 on success
- `delete_automation` returns 409 when facade returns `HasActiveFire`
- `set_automation_state` with invalid action body returns 422

### Step 15.5 — Frontend unit tests

Extend `automations-presenters.test.mjs`:
- `nextCronFire` returns null for invalid cron
- `durationLabel` formats sub-minute, minute, and multi-hour durations
- `runStatusTone` maps all three values correctly
- `normalizeAutomations` correctly carries through `prompt` and
  `completion_policy` fields added to the response

---

## 16. Build Verification Sequence

Run in this order (fastest feedback loop first):

```bash
# 1. Triggers crate — new repo methods + patch struct
cargo clippy -p brassclaw_triggers --all-targets -- -D warnings
cargo test -p brassclaw_triggers

# 2. Composition adapter
cargo clippy -p brassclaw_reborn_composition --all-features --all-targets -- -D warnings
cargo test -p brassclaw_reborn_composition

# 3. Product workflow — new DTO types + trait methods + impls
cargo build -p brassclaw_product_workflow --all-features
cargo clippy -p brassclaw_product_workflow --all-targets -- -D warnings
cargo test -p brassclaw_product_workflow

# 4. WebUI v2 — new routes + handlers
cargo build -p brassclaw_webui_v2 --features webui-v2-beta
cargo clippy -p brassclaw_webui_v2 --all-targets -- -D warnings
cargo test -p brassclaw_webui_v2

# 5. CLI (full serve graph)
cargo build --release --bin brassclaw

# 6. Frontend syntax check (no build step)
node --check crates/brassclaw_webui_v2_static/static/js/lib/api.js
node --check crates/brassclaw_webui_v2_static/static/js/pages/automations/automations-page.js
node --check crates/brassclaw_webui_v2_static/static/js/pages/automations/hooks/useAutomations.js
node --check crates/brassclaw_webui_v2_static/static/js/pages/automations/hooks/useAutomationDetail.js
node --check crates/brassclaw_webui_v2_static/static/js/pages/automations/components/automation-create-modal.js
node --check crates/brassclaw_webui_v2_static/static/js/pages/automations/components/automation-detail-panel.js
node --check crates/brassclaw_webui_v2_static/static/js/pages/automations/components/automation-delete-confirm.js

# 7. Frontend presenter tests
node --test crates/brassclaw_webui_v2_static/static/js/pages/automations/lib/automations-presenters.test.mjs
```

---

## 17. Implementation Phase Sequence

Execute in strict dependency order:

| Phase | Work | Crate(s) |
|---|---|---|
| **P1** | Audit + fill `TriggerRepository` (get, create, update, delete, set_state, list_runs); add `TriggerUpdatePatch`; add `TriggerRunRecord` | `brassclaw_triggers` |
| **P2** | Add 7 new capability ID constants + handlers in host_runtime | `brassclaw_host_runtime` |
| **P3** | Extend `RebornWebuiAutomationFacade` with 7 new methods | `brassclaw_reborn_composition` |
| **P4** | Add new DTO types; add 7 new trait methods (default unavailable) to `RebornServicesApi`; implement concretely on `RebornServices` | `brassclaw_product_workflow` |
| **P5** | Add route constants + descriptor functions; add routes to router; add handler functions; update descriptor contract test | `brassclaw_webui_v2` |
| **P6** | Add 8 new API client functions to `api.js` | `brassclaw_webui_v2_static` |
| **P7** | Extend `useAutomations.js` with mutations; add `useAutomationDetail.js` | `brassclaw_webui_v2_static` |
| **P8** | Extend `automations-presenters.js`; extend `automations-list.js` with action buttons | `brassclaw_webui_v2_static` |
| **P9** | Build `automation-create-modal.js` | `brassclaw_webui_v2_static` |
| **P10** | Build `automation-detail-panel.js` (overview + runs + raw tabs) | `brassclaw_webui_v2_static` |
| **P11** | Build `automation-delete-confirm.js` | `brassclaw_webui_v2_static` |
| **P12** | Update `automations-page.js` to full master-detail layout | `brassclaw_webui_v2_static` |
| **P13** | Add i18n strings to `en.js` | `brassclaw_webui_v2_static` |
| **P14** | Update `app.js` for `/automations/:automationId` route + routines redirect | `brassclaw_webui_v2_static` |
| **P15** | Run full build verification sequence (§16) | all |
| **P16** | Dogfood: create, pause, resume, edit, fire manually, delete a trigger via the UI | manual |

---

## 18. Open Questions to Resolve Before P1

1. **`get_trigger` / `create_trigger` / `delete_trigger`** — do they already
   exist in `TriggerRepository`? Run:
   ```bash
   grep -n "async fn " crates/brassclaw_triggers/src/lib.rs | head -60
   ```
   If any are present under different names (e.g. `upsert_trigger`), reuse them.

2. **`active_run_ref` as run-history link** — verify that the trusted-submit
   path stores the `trigger_id` somewhere on the run record (in run `metadata`
   or as a tag on `brassclaw_runs`) so `list_trigger_runs` can query by it. If
   not, the simplest fallback is to return the `last_fired_slot` + `last_status`
   directly from the trigger row as a one-item "run history" until proper
   run-metadata tagging is added.

3. **`TriggerFireAccessChecker` for manual fire** — does the
   `TriggerPollerAuthorizerConfig::CreatorAccessRequired` authorizer already
   accept non-poller manual-fire requests? If the `fire_now` capability reuses
   the same `TrustedTriggerFireSubmitter` as the poller worker, it must pass
   the same access-check. Review the authorizer logic to confirm.

4. **Cron preview client-side library** — the existing `automations-presenters.js`
   parses cron strings purely in JS without a library. For the "next fire"
   preview in the create/edit form, either extend the existing parser or add a
   small ESM CDN dependency (`cron-parser` from jspm.io). Decide before P9.

5. **Routines redirect vs. keep** — confirm with the team whether `/routines`
   should redirect to `/automations` (Option A) or be kept as a filtered view
   (Option B) before P14.

---

## 19. File Index — All Files Touched

| File | Change type |
|---|---|
| `crates/brassclaw_triggers/src/lib.rs` | Add `TriggerUpdatePatch`, `TriggerRunRecord`; extend `TriggerRepository` trait |
| `crates/brassclaw_triggers/src/postgres.rs` | Implement new repository methods |
| `crates/brassclaw_host_runtime/first_party_tools/<triggers>.rs` | Add 7 capability constants + handlers |
| `crates/brassclaw_reborn_composition/src/automation.rs` | Add 7 composition methods + `CreateAutomationInput` / `UpdateAutomationInput` structs |
| `crates/brassclaw_product_workflow/src/reborn_services/types.rs` | Add 8 request/response DTO types; extend `RebornAutomationInfo` |
| `crates/brassclaw_product_workflow/src/reborn_services.rs` | Add 7 trait methods (default + concrete impl) |
| `crates/brassclaw_webui_v2/src/descriptors.rs` | Add 7 route constants + descriptor functions |
| `crates/brassclaw_webui_v2/src/router.rs` | Mount 5 new route patterns |
| `crates/brassclaw_webui_v2/src/handlers.rs` | Add 7 handler functions + `AutomationRunsQuery` struct |
| `crates/brassclaw_webui_v2/tests/webui_v2_descriptors_contract.rs` | Add 7 new routes to locked table |
| `crates/brassclaw_webui_v2_static/static/js/lib/api.js` | Add 8 API client functions |
| `crates/brassclaw_webui_v2_static/static/js/pages/automations/hooks/useAutomations.js` | Add mutation returns |
| `crates/brassclaw_webui_v2_static/static/js/pages/automations/hooks/useAutomationDetail.js` | **New file** |
| `crates/brassclaw_webui_v2_static/static/js/pages/automations/components/automations-list.js` | Add action buttons + selected state |
| `crates/brassclaw_webui_v2_static/static/js/pages/automations/components/automation-create-modal.js` | **New file** |
| `crates/brassclaw_webui_v2_static/static/js/pages/automations/components/automation-detail-panel.js` | **New file** |
| `crates/brassclaw_webui_v2_static/static/js/pages/automations/components/automation-delete-confirm.js` | **New file** |
| `crates/brassclaw_webui_v2_static/static/js/pages/automations/lib/automations-presenters.js` | Add `nextCronFire`, `durationLabel`, `runStatusTone` |
| `crates/brassclaw_webui_v2_static/static/js/pages/automations/lib/automations-presenters.test.mjs` | Extend tests |
| `crates/brassclaw_webui_v2_static/static/js/pages/automations/automations-page.js` | Full master-detail rewrite |
| `crates/brassclaw_webui_v2_static/static/js/app/app.js` | Add `/automations/:automationId` sub-route + routines redirect |
| `crates/brassclaw_webui_v2_static/static/js/i18n/en.js` | Add `automations.*` namespace |
| `crates/brassclaw_webui_v2_static/static/js/layout/gateway-layout.js` | Ensure Automations nav item present; remove Routines |
| `crates/brassclaw_webui_v2_static/static/js/pages/routines/lib/routines-api.js` | Mark deprecated (or delete if Option A chosen) |
