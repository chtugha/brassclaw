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
repository. The plan adds **five new first-party capabilities** (three already
exist: create, list, remove), **seven facade trait methods**, **seven HTTP
routes**, and **a fully featured SPA page**.

> **Key naming corrections (grounded in live source):**
> - Capability IDs use the `builtin.*` namespace, not `brassclaw.*`.
>   Existing: `builtin.trigger_create`, `builtin.trigger_list`,
>   `builtin.trigger_remove` — defined in
>   `crates/brassclaw_host_runtime/src/first_party_tools/trigger_management.rs`.
> - Create uses `upsert_trigger` (the existing repo method), not a new
>   `create_trigger`. The upsert is idempotent on `trigger_id`.
> - Delete uses `remove_scoped_trigger` (enforces creator-scope check).
> - The `AutomationProductFacade` **trait** lives in
>   `brassclaw_product_workflow/src/reborn_services.rs` (line 408).
>   The **impl** (`RebornWebuiAutomationFacade`) lives in
>   `brassclaw_reborn_composition/src/automation.rs`.
>   `UnsupportedAutomationProductFacade` (same file) must also implement every
>   new method with `automation_unavailable()` fallbacks.

---

## 1. Current State Inventory

### What already exists

| Layer | Already present |
|---|---|
| DB table | `brassclaw_triggers` (V021) — all columns needed |
| Domain types | `TriggerRecord`, `TriggerSchedule::Cron`, `TriggerState`, `TriggerCompletionPolicy`, `TriggerRunStatus` in `brassclaw_triggers/src/lib.rs` |
| Repository trait | `TriggerRepository` — `upsert_trigger`, `get_trigger`, `list_triggers`, `list_scoped_triggers`, `remove_trigger`, `remove_scoped_trigger` in `brassclaw_triggers/src/lib.rs` (lines 608–732). **Missing:** `update_trigger`, `list_trigger_runs`. **No** `create_trigger`, `set_trigger_state`, or `delete_trigger` exist under those names. |
| Poller worker | `brassclaw_triggers/src/worker.rs` — fires due triggers, submits trusted inbound turns |
| Cron parsing | `normalize_cron_expression`, `parse_cron_schedule`, `reject_sub_minute_cadence` in `brassclaw_triggers/src/lib.rs` |
| Existing capabilities | `TRIGGER_CREATE_CAPABILITY_ID` (`"builtin.trigger_create"`), `TRIGGER_LIST_CAPABILITY_ID` (`"builtin.trigger_list"`), `TRIGGER_REMOVE_CAPABILITY_ID` (`"builtin.trigger_remove"`) — all in `trigger_management.rs`. The `create` handler hard-codes `completion_policy: Recurring`; the `remove` handler uses `remove_scoped_trigger` (caller-scope enforced). |
| Composition adapter (existing) | `RebornWebuiAutomationFacade` in `automation.rs` implements the `AutomationProductFacade` trait (defined in `reborn_services.rs` line 408). Only `list_automations` is currently implemented. |
| HTTP route (read) | `GET /api/webchat/v2/automations` → `list_automations` handler |
| Facade method (read) | `RebornServicesApi::list_automations` → `AutomationProductFacade::list_automations` |
| Frontend page (read) | `automations-page.js` + `useAutomations.js` + `automations-list.js` + `automations-summary-strip.js` + `automations-presenters.js` |

### What is missing

| Layer | Gap |
|---|---|
| Host-runtime capabilities | `GET` (new), `UPDATE` (new, name+cron+prompt), `SET_STATE` (new, pause/resume), `FIRE_NOW` (new, manual fire), `RUN_HISTORY` (new). `CREATE`, `LIST`, `REMOVE` already exist — but `CREATE` needs `completion_policy` input support added. |
| `TriggerRepository` methods | `update_trigger` (new), `list_trigger_runs` (new). `upsert_trigger` covers create. `remove_scoped_trigger` covers scoped delete. `get_trigger` already exists. **No need to add** `create_trigger`, `set_trigger_state`, or `delete_trigger` as separate methods. |
| Composition adapter | `create_automation`, `get_automation`, `update_automation`, `delete_automation`, `pause_automation`, `resume_automation`, `fire_automation_now`, `get_automation_run_history` |
| `AutomationProductFacade` trait | 6 new methods to add to the trait in `reborn_services.rs`: `create_automation`, `get_automation`, `update_automation`, `set_automation_state`, `delete_automation`, `fire_automation_now`, `get_automation_run_history`. `UnsupportedAutomationProductFacade` must also implement each with `automation_unavailable()`. |
| `RebornServicesApi` trait | 7 new default-body methods (returns 501), delegating to `automation_facade` |
| HTTP routes | `POST /automations`, `GET /automations/:id`, `PATCH /automations/:id/state`, `PATCH /automations/:id`, `DELETE /automations/:id`, `POST /automations/:id/fire` |
| Frontend | Create/edit modal, detail panel, action buttons (pause/resume/fire/delete), run history tab, i18n strings |
| Routines page | Currently a stub; this plan merges its pattern into the Automations page replacing it |

---

## 2. DB Layer — Verify Repository Completeness

### Step 2.1 — Audit `TriggerRepository` trait methods (grounded in live source)

**File:** `crates/brassclaw_triggers/src/lib.rs` (lines 608–732)

The following methods **already exist** and must be reused, not re-added:

```rust
// Already exists — use for create:
async fn upsert_trigger(&self, record: TriggerRecord) -> Result<(), TriggerError>;

// Already exists — use for get:
async fn get_trigger(&self, tenant_id: TenantId, trigger_id: TriggerId)
    -> Result<Option<TriggerRecord>, TriggerError>;

// Already exists — use for scoped delete (caller-scope enforced):
async fn remove_scoped_trigger(
    &self,
    tenant_id: TenantId,
    creator_user_id: UserId,
    agent_id: Option<AgentId>,
    project_id: Option<ProjectId>,
    trigger_id: TriggerId,
) -> Result<Option<TriggerRecord>, TriggerError>;
```

The following two methods are **genuinely missing** and must be added to the
trait and implemented in `crates/brassclaw_triggers/src/postgres.rs`:

```rust
// NEW — partial field update:
async fn update_trigger(
    &self,
    tenant_id: TenantId,
    trigger_id: TriggerId,
    patch: TriggerUpdatePatch,
) -> Result<Option<TriggerRecord>, TriggerError>;  // None if not found

// NEW — run history from brassclaw_runs table:
async fn list_trigger_runs(
    &self,
    tenant_id: TenantId,
    trigger_id: TriggerId,
    limit: usize,
) -> Result<Vec<TriggerRunRecord>, TriggerError>;
```

**There is no `create_trigger`, `delete_trigger`, or `set_trigger_state` —
these names do not exist in the codebase.** Use `upsert_trigger`,
`remove_scoped_trigger`, and the new `update_trigger` (with only the `state`
field set) for state changes. Alternatively, add a dedicated
`set_trigger_state` only if a state-only SQL UPDATE is preferred for
atomicity; see Step 2.4.

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

### Step 2.3b — Implement new methods on `InMemoryTriggerRepository`

**Critical compile requirement:** `InMemoryTriggerRepository` (lines 764+ of
`brassclaw_triggers/src/lib.rs`) implements the `TriggerRepository` trait in full.
Every new trait method must also be implemented there or the crate will not compile.

```rust
// update_trigger — in-memory: get, apply non-null fields, upsert back
async fn update_trigger(
    &self,
    tenant_id: TenantId,
    trigger_id: TriggerId,
    patch: TriggerUpdatePatch,
) -> Result<Option<TriggerRecord>, TriggerError> {
    let mut state = self.lock_state()?;
    let key = TriggerRepositoryKey::new(&tenant_id, trigger_id);
    let Some(record) = state.get_mut(&key) else { return Ok(None); };
    if let Some(name) = patch.name { record.name = name; }
    if let Some(schedule) = patch.schedule {
        let next = schedule.next_slot_after(Utc::now())
            .map_err(|e| TriggerError::InvalidSchedule { reason: e.to_string() })?
            .ok_or_else(|| TriggerError::InvalidSchedule { reason: "no future slot".into() })?;
        record.schedule = schedule;
        record.next_run_at = next;
    }
    if let Some(prompt) = patch.prompt { record.prompt = prompt; }
    if let Some(policy) = patch.completion_policy { record.completion_policy = policy; }
    Ok(Some(record.clone()))
}

// list_trigger_runs — in-memory: always returns empty (no run table in memory)
async fn list_trigger_runs(
    &self,
    _tenant_id: TenantId,
    _trigger_id: TriggerId,
    _limit: usize,
) -> Result<Vec<TriggerRunRecord>, TriggerError> {
    Ok(Vec::new())
}

// set_trigger_state (if added) — in-memory:
async fn set_trigger_state(
    &self,
    tenant_id: TenantId,
    trigger_id: TriggerId,
    state: TriggerState,
) -> Result<Option<TriggerRecord>, TriggerError> {
    let mut store = self.lock_state()?;
    let key = TriggerRepositoryKey::new(&tenant_id, trigger_id);
    let Some(record) = store.get_mut(&key) else { return Ok(None); };
    record.state = state;
    Ok(Some(record.clone()))
}
```

Also add `TriggerRunRecord` to the `pub use postgres::PostgresTriggerRepository;`
re-export block (or wherever trigger public types are exported from `lib.rs`).

### Step 2.4 — PostgreSQL implementation

For the two **new** methods, implement in `crates/brassclaw_triggers/src/postgres.rs`
following the existing patterns (deadpool-postgres, param binding, row mapping).

**Note on create:** The existing `upsert_trigger` already writes the full record
via an `INSERT … ON CONFLICT DO UPDATE` pattern. The `create_automation`
capability handler (Step 3.2b) will build a `TriggerRecord` and call
`upsert_trigger` directly — no new SQL needed for create.

**Note on set_state:** The preferred approach is to add a dedicated
`set_trigger_state` SQL UPDATE for atomicity (state-only, no race with
schedule recompute). Add it to the trait as:

```rust
// OPTIONAL but recommended — atomic state-only update:
async fn set_trigger_state(
    &self,
    tenant_id: TenantId,
    trigger_id: TriggerId,
    state: TriggerState,
) -> Result<Option<TriggerRecord>, TriggerError>;  // None if not found
```

SQL:
```sql
UPDATE brassclaw_triggers
SET state = $3
WHERE tenant_id = $1 AND trigger_id = $2
RETURNING *
```

If `set_trigger_state` is not added, the `set_state` capability handler can
instead call `get_trigger` + mutate the state field + `upsert_trigger`.

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

**Location:** `crates/brassclaw_host_runtime/src/first_party_tools/trigger_management.rs`

Three capabilities already exist with a `builtin.*` namespace and are fully
implemented. This section covers what to add and what to modify.

### Step 3.1 — Existing capability constants (DO NOT rename or re-add)

```rust
// Already in trigger_management.rs — use as-is:
pub const TRIGGER_CREATE_CAPABILITY_ID: &str = "builtin.trigger_create";  // line 29
pub const TRIGGER_LIST_CAPABILITY_ID: &str   = "builtin.trigger_list";    // line 30
pub const TRIGGER_REMOVE_CAPABILITY_ID: &str = "builtin.trigger_remove";  // line 31
```

### Step 3.1b — New capability ID constants (add to `trigger_management.rs`)

> **Note:** `TRIGGER_FIRE_NOW_CAPABILITY_ID` is **removed** from this list.
> Manual fire cannot be implemented as a first-party capability because
> `TrustedTriggerSubmitRequest::new` is `pub(crate)` and cannot be called
> from `brassclaw_host_runtime`. See §3.7 for the correct composition-layer
> approach.

```rust
pub const TRIGGER_GET_CAPABILITY_ID: &str       = "builtin.trigger_get";
pub const TRIGGER_UPDATE_CAPABILITY_ID: &str    = "builtin.trigger_update";
pub const TRIGGER_SET_STATE_CAPABILITY_ID: &str = "builtin.trigger_set_state";
pub const TRIGGER_RUN_HISTORY_CAPABILITY_ID: &str = "builtin.trigger_run_history";
// TRIGGER_FIRE_NOW_CAPABILITY_ID intentionally omitted — see §3.7
```

Register each in `insert_trigger_handlers` (pattern: `registry.insert_handler(...)`
alongside the existing three registrations, lines 101–109).

Add new manifest entries in `manifests()` (lines 33–56):

```rust
// trigger_get — read, no external write:
first_party_capability_manifest(
    TRIGGER_GET_CAPABILITY_ID,
    "Get a caller-scoped scheduled trigger by ID",
    vec![EffectKind::DispatchCapability],
    PermissionMode::Allow,
    resource_profile(),
)?
// trigger_update — write:
first_party_capability_manifest(
    TRIGGER_UPDATE_CAPABILITY_ID,
    "Update a caller-scoped scheduled trigger",
    vec![EffectKind::DispatchCapability, EffectKind::ExternalWrite],
    PermissionMode::Ask,
    resource_profile(),
)?
// trigger_set_state — write:
first_party_capability_manifest(
    TRIGGER_SET_STATE_CAPABILITY_ID,
    "Pause or resume a caller-scoped scheduled trigger",
    vec![EffectKind::DispatchCapability, EffectKind::ExternalWrite],
    PermissionMode::Ask,
    resource_profile(),
)?
// trigger_fire_now — write + dispatch:
first_party_capability_manifest(
    TRIGGER_FIRE_NOW_CAPABILITY_ID,
    "Manually fire a caller-scoped scheduled trigger now",
    vec![EffectKind::DispatchCapability, EffectKind::ExternalWrite],
    PermissionMode::Ask,
    resource_profile(),
)?
// trigger_run_history — read:
first_party_capability_manifest(
    TRIGGER_RUN_HISTORY_CAPABILITY_ID,
    "Get run history for a caller-scoped scheduled trigger",
    vec![EffectKind::DispatchCapability],
    PermissionMode::Allow,
    resource_profile(),
)?
```

### Step 3.2 — Capability handler: `triggers.create` (MODIFY existing handler)

The `create_trigger` function in `trigger_management.rs` already exists (lines
210–261) and handles name, prompt, cron, ULID generation, `next_run_at`
computation, `upsert_trigger`, and rollback on hook failure. It hard-codes:

```rust
completion_policy: TriggerCompletionPolicy::Recurring,  // line 229
```

**Only change needed:** Parse an optional `completion_policy` field from the
input and apply it. Add to `TriggerCreateInput`:

```rust
#[derive(Deserialize)]
struct TriggerCreateInput {
    name: String,
    prompt: String,
    cron: String,
    #[serde(default)]
    completion_policy: Option<String>,  // NEW
}
```

In `create_trigger`, replace the hard-coded `Recurring` with:

```rust
completion_policy: match input.completion_policy.as_deref() {
    Some("complete_after_first_fire") => TriggerCompletionPolicy::CompleteAfterFirstFire,
    _ => TriggerCompletionPolicy::Recurring,  // default
},
```

### Step 3.2b — Extend `trigger_output()` to emit `prompt` (only)

**Ground truth:** `trigger_output()` at line 312 of `trigger_management.rs`
**already emits `completion_policy`** (line 320). Only `prompt` is missing.

Add `"prompt": record.prompt,` to `trigger_output()` (after `"name"`, before
`"source"`):

```rust
fn trigger_output(record: &TriggerRecord) -> Value {
    json!({
        "trigger_id": record.trigger_id.to_string(),
        "agent_id": record.agent_id.as_ref().map(|id| id.as_str()),
        "project_id": record.project_id.as_ref().map(|id| id.as_str()),
        "name": record.name,
        "prompt": record.prompt,           // ADD — only this is missing
        "source": record.source,
        "schedule": record.schedule,
        "completion_policy": record.completion_policy,  // already present
        "state": record.state,
        "next_run_at": record.next_run_at,
        "last_run_at": record.last_run_at,
        "last_status": record.last_status,
        "is_active": record.has_active_fire(),
        "created_at": record.created_at,
    })
}
```

This also fixes `list_automations` to include `prompt` for the detail panel.
Update `RawAutomationRecord` in `automation.rs` to accept these new fields:

```rust
#[derive(Debug, Deserialize)]
struct RawAutomationRecord {
    // ... existing fields ...
    #[serde(default)]
    prompt: Option<String>,          // ADD
    #[serde(default)]
    completion_policy: Option<String>, // ADD — raw string from JSON
}
```

And propagate them through `automation_info()`:

```rust
fn automation_info(record: RawAutomationRecord) -> Option<RebornAutomationInfo> {
    Some(RebornAutomationInfo {
        // ... existing fields ...
        prompt: record.prompt,
        completion_policy: record.completion_policy,
    })
}
```

### Step 3.3 — Capability handler: `triggers.get`

Input: `{ "trigger_id": "..." }` — `tenant_id` comes from `request.scope`, never from JSON.

Handler: calls `repository.get_trigger(scope.tenant_id, trigger_id).await?`.

Returns `{ "trigger": <trigger_output> }` when found, `{ "trigger": null }` when not found.
Reuse `trigger_output()` for the full record shape.

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

Input: `{ "trigger_id": "...", "state": "paused"|"scheduled" }`

`tenant_id`, `creator_user_id`, `agent_id`, `project_id` come from
`request.scope` (the `ResourceScope`), never from the JSON input.

Only allows `paused` and `scheduled` transitions from product code (not
`completed`). `completed` is set only by the poller worker. Reject any other
value with `input_error()`.

Handler:
1. Parse `trigger_id` and `state` from input.
2. Validate `state` ∈ `{"paused", "scheduled"}`.
3. Call `repository.set_trigger_state(scope.tenant_id, trigger_id, state).await?`
   (or `get_trigger` + mutate + `upsert_trigger` if `set_trigger_state` was not
   added as a dedicated method — see Step 2.4).
4. If `None` returned (not found), emit `{ "found": false }`.
5. Return `{ "found": true, "trigger": trigger_output(&record) }`.

### Step 3.6 — Capability handler: `triggers.delete` (uses `remove_scoped_trigger`)

**Important:** The existing `remove_trigger` function (lines 289–310) already
uses `remove_scoped_trigger` to enforce caller-scope. The new `delete`
capability must do the same — use `remove_scoped_trigger`, not a bare
`remove_trigger`.

Input: `{ "trigger_id": "..." }` (scope fields come from `request.scope`)

Handler:
1. Parse `trigger_id`.
2. Load trigger via `repository.get_trigger(scope.tenant_id, trigger_id)`.
3. If `None`, return `{ "removed": false }`.
4. If trigger has `active_fire_slot` (`has_active_fire()` returns true), return
   error `{ "error": "trigger_has_active_fire" }` — do not delete while a run
   is in flight. Caller should pause first then retry.
5. Call `repository.remove_scoped_trigger(scope.tenant_id, scope.user_id,
   scope.agent_id, scope.project_id, trigger_id).await?`
6. Return `{ "removed": removed.is_some() }`

### Step 3.7 — Capability handler: `triggers.fire_now` — **ARCHITECTURAL CONSTRAINT**

> **Critical:** `TrustedTriggerSubmitRequest::new` is `pub(crate)` inside
> `brassclaw_triggers` (line 21 of `worker/ports.rs`). Nothing outside that
> crate can construct one. Therefore, `TriggerManagementToolHandler` in the
> host-runtime capability layer **cannot** call `TrustedTriggerFireSubmitter`
> directly — it cannot assemble the required `TrustedTriggerSubmitRequest`.

**Correct architecture for manual fire:**

The `fire_now` capability must be handled at the **composition layer**
(`brassclaw_reborn_composition/src/automation.rs`), not in `trigger_management.rs`.
There are two valid approaches:

**Approach A (Recommended) — Composition-layer method, no new capability constant:**
Skip the `builtin.trigger_fire_now` capability entirely. Instead, add a
`fire_automation_now` method directly to `RebornWebuiAutomationFacade` in
`automation.rs` that goes **around** the capability call and directly invokes
the trusted submit path already wired in composition (the poller's
`TrustedTriggerFireSubmitter` instance).

This requires:
1. `RebornWebuiAutomationFacade` gains a second field:
   ```rust
   pub struct RebornWebuiAutomationFacade {
       host_runtime: Arc<dyn HostRuntime>,
       trigger_repository: Arc<dyn TriggerRepository>,  // ADD
       trusted_submitter: Arc<dyn TrustedTriggerFireSubmitter>,  // ADD
       backend_timeout: Duration,
   }
   ```
2. `RebornWebuiAutomationFacade::new` gains two new parameters.
3. The wiring in `crates/brassclaw_reborn_composition/src/webui.rs` (lines 61–64)
   must pass `trigger_repository` and `trusted_submitter` when constructing
   the facade. Both are available on the composition services struct at that
   point.
4. `fire_automation_now` implementation:
   - Load trigger via `trigger_repository.get_trigger(caller.tenant_id, trigger_id)`
   - Verify state is `Scheduled` and `!record.has_active_fire()`
   - Build `TriggerFireIdentity::new(tenant_id, trigger_id, now)` (public constructor)
   - Build `TriggerFire { identity, creator_user_id, agent_id, project_id, prompt }`
   - Call `TriggerPromptMaterializer::materialize_prompt(fire.clone())` — the
     materializer is already wired in composition
   - Construct `TrustedTriggerSubmitRequest::new(fire, materialized_prompt, now)`
     — **only works because this code is inside the `brassclaw_triggers` crate
     boundary via composition**, which has the trusted submit path
   - Actually: `TrustedTriggerSubmitRequest::new` is `pub(crate)` in the
     **worker** module of `brassclaw_triggers`. Composition is a separate crate.
     This means even approach A cannot call `TrustedTriggerSubmitRequest::new`
     directly.

**Approach B (Correct) — Expose a public constructor or a fire helper on `TriggerFire`:**
Add a `pub` constructor or a small public helper to `brassclaw_triggers` that
creates the submit request:

```rust
// In brassclaw_triggers/src/worker/ports.rs — make new() pub:
impl TrustedTriggerSubmitRequest {
    pub fn new(  // change pub(crate) → pub
        fire: TriggerFire,
        materialized_prompt: TriggerMaterializedPrompt,
        received_at: Timestamp,
    ) -> Self { ... }
}
```

This is the minimal, safe change. The struct's fields are private; callers still
cannot forge fields. The caller just needs to hold valid `TriggerFire` and
`TriggerMaterializedPrompt` values (which are themselves well-typed).

**Approach C — Dedicated trait in `brassclaw_triggers` for manual fire:**
Add a `ManualFirePort` trait to `brassclaw_triggers/src/worker/ports.rs`:
```rust
#[async_trait]
pub trait ManualTriggerFireSubmitter: Send + Sync {
    async fn submit_manual_fire(
        &self,
        tenant_id: TenantId,
        trigger_id: TriggerId,
        fire_slot: Timestamp,
    ) -> Result<TurnRunId, TriggerError>;
}
```
Implement this in composition (which can call the existing trusted submit path
internally), and expose it to the capability handler via the handler struct.

**Recommended path:** Use **Approach B** (make `TrustedTriggerSubmitRequest::new`
`pub`). It is the minimal change, preserves the existing type system, and lets
`fire_automation_now` live in composition as planned. Update §3.1b to **not**
include `builtin.trigger_fire_now` as a first-party capability — instead,
`fire_automation_now` is a direct composition method (no capability dispatch).
Remove `TRIGGER_FIRE_NOW_CAPABILITY_ID` from the plan.

**fire_now flow (Approach B, via composition):**

In `RebornWebuiAutomationFacade::fire_automation_now`:
1. Load via `trigger_repository.get_trigger(caller.tenant_id, trigger_id)`. 404 if None.
2. Reject if state ≠ `Scheduled` — return `state_conflict`.
3. Reject if `record.has_active_fire()` — return `trigger_has_active_fire`.
4. `fire_slot = Utc::now()`
5. Build `TriggerFireIdentity::new(tenant_id, trigger_id, fire_slot)` (public)
6. Build `TriggerFire { identity, creator_user_id, agent_id, project_id, prompt }`
7. Materialize: `materializer.materialize_prompt(fire.clone()).await?`
8. Submit: `TrustedTriggerSubmitRequest::new(fire, materialized, now)` (now public)
9. `trusted_submitter.submit_trusted_trigger_fire(request).await?`
10. Return `{ "run_ref": run_id.to_string() }` on `Accepted`; on `Replayed` return
    the existing run_id.

Required dependency additions to `RebornWebuiAutomationFacade`:
- `trigger_repository: Arc<dyn TriggerRepository>`
- `trusted_submitter: Arc<dyn TrustedTriggerFireSubmitter>`
- `materializer: Arc<dyn TriggerPromptMaterializer>`

All three are already wired in the composition layer — consult
`crates/brassclaw_reborn_composition/src/trigger_poller.rs` and
`trigger_poller_trusted_submit.rs` for how to access them.

### Step 3.8 — Capability handler: `triggers.run_history`

Input: `{ "trigger_id": "...", "limit": 20 }` — `tenant_id` from `request.scope`.

Handler: calls `repository.list_trigger_runs(scope.tenant_id, trigger_id, limit)`.
Cap `limit` at a safe maximum (e.g. 50).
Returns `{ "runs": [...] }`.

### Step 3.9 — Register new capabilities and update public exports

**In `trigger_management.rs`:** Add the four new constants (`get`, `update`,
`set_state`, `run_history`) to `insert_trigger_handlers` and `manifests()`.
**Do not add `fire_now`** — that is handled at the composition layer (see §3.7).

**In `crates/brassclaw_host_runtime/src/lib.rs` (lines 75–86):**
The `pub use first_party_tools::{ ... }` block exports `TRIGGER_CREATE_CAPABILITY_ID`,
`TRIGGER_LIST_CAPABILITY_ID`, `TRIGGER_REMOVE_CAPABILITY_ID`. Add the four new
constants to the same block:

```rust
// lib.rs lines 82-83 — extend:
TRIGGER_CREATE_CAPABILITY_ID, TRIGGER_GET_CAPABILITY_ID,
TRIGGER_LIST_CAPABILITY_ID, TRIGGER_REMOVE_CAPABILITY_ID,
TRIGGER_RUN_HISTORY_CAPABILITY_ID, TRIGGER_SET_STATE_CAPABILITY_ID,
TRIGGER_UPDATE_CAPABILITY_ID,
```

Also update the two public factory functions that wire trigger handlers
(`builtin_first_party_handlers_with_trigger_create_hook` and
`builtin_first_party_handlers_from_tools_with_trigger` in `mod.rs`, lines
210–241) — no signature change needed; they already call
`insert_handlers_with_create_hook` which will pick up the new handlers
automatically once they are registered inside `insert_trigger_handlers`.

---

## 4. Composition Adapter + `AutomationProductFacade` Trait

**Trait location:** `crates/brassclaw_product_workflow/src/reborn_services.rs`
(lines 408–434 — `AutomationProductFacade` trait + `UnsupportedAutomationProductFacade`).

**Impl location:** `crates/brassclaw_reborn_composition/src/automation.rs`
(`RebornWebuiAutomationFacade` implements the trait).

**All six new methods must be added to the `AutomationProductFacade` trait first.**
Then implemented on `RebornWebuiAutomationFacade`. Then stubbed on
`UnsupportedAutomationProductFacade` with `automation_unavailable()` (or a
similar error sentinel already used in that file).

The `invoke_trigger` helper (lines 55–116 in `automation.rs`) is the shared
entry point for all capability-dispatched methods (create, get, update,
set_state, delete, run_history). **`fire_automation_now` is the exception** —
it does not call `invoke_trigger` but instead calls the trusted submit path
directly. See §3.7 for the full rationale and flow.

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

### Step 4.6 — `fire_automation_now` (composition-layer direct path)

This method does **not** use `invoke_trigger`. It directly calls the trusted
submit path. Requires the following new fields on `RebornWebuiAutomationFacade`
(see §3.7 for full explanation):

```rust
pub struct RebornWebuiAutomationFacade {
    host_runtime: Arc<dyn HostRuntime>,         // existing
    trigger_repository: Arc<dyn TriggerRepository>,      // ADD
    trusted_submitter: Arc<dyn TrustedTriggerFireSubmitter>, // ADD
    materializer: Arc<dyn TriggerPromptMaterializer>,    // ADD
    backend_timeout: Duration,                  // existing
}
```

Update `RebornWebuiAutomationFacade::new` and `with_backend_timeout` accordingly.

Update `webui.rs` (lines 61–64) to pass the three new dependencies when
constructing the facade.

**Prerequisite in `brassclaw_triggers`:**
Make `TrustedTriggerSubmitRequest::new` public (change `pub(crate)` → `pub` in
`crates/brassclaw_triggers/src/worker/ports.rs` line 21).

```rust
pub(crate) async fn fire_automation_now(
    &self,
    caller: ProductAgentBoundCaller,
    trigger_id_str: String,
) -> Result<RebornFireAutomationResult, RebornServicesError> {
    let trigger_id = TriggerId::parse(&trigger_id_str)
        .map_err(|_| not_found_error())?;
    let record = self.trigger_repository
        .get_trigger(caller.tenant_id.clone(), trigger_id)
        .await
        .map_err(map_trigger_error)?
        .ok_or_else(not_found_error)?;

    if record.state != TriggerState::Scheduled {
        return Err(state_conflict_error("trigger is not in scheduled state"));
    }
    if record.has_active_fire() {
        return Err(active_fire_error());
    }

    let fire_slot = Utc::now();
    let identity = TriggerFireIdentity::new(
        caller.tenant_id.clone(), trigger_id, fire_slot,
    );
    let fire = TriggerFire {
        identity,
        creator_user_id: caller.user_id.clone(),
        agent_id: caller.agent_id.clone(),
        project_id: caller.project_id.clone(),
        prompt: record.prompt.clone(),
    };
    let materialized = self.materializer
        .materialize_prompt(fire.clone())
        .await
        .map_err(map_trigger_error)?;

    let request = TrustedTriggerSubmitRequest::new(fire, materialized, fire_slot);
    let outcome = self.trusted_submitter
        .submit_trusted_trigger_fire(request)
        .await
        .map_err(map_trigger_error)?;

    let run_ref = match outcome {
        TrustedTriggerFireSubmitOutcome::Accepted { run_id, .. } => run_id.to_string(),
        TrustedTriggerFireSubmitOutcome::Replayed { original_run_id, .. } => {
            original_run_id.to_string()
        }
    };
    Ok(RebornFireAutomationResult { run_ref })
}
```

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
// - [Pause] or [Resume] button depending on automation.is_active
//   (field name: `is_active` — set to true when active_fire_slot is non-null)
// - [Run now] button — disabled when automation.is_active is true (run in flight)
//   or when state === "paused" (409 state_conflict from backend)
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
   must never mint a `TrustedInboundTurnRequest` directly. Manual fire is
   assembled in `RebornWebuiAutomationFacade::fire_automation_now` inside
   `brassclaw_reborn_composition` — NOT in the host-runtime capability layer,
   because `TrustedTriggerSubmitRequest::new` is crate-internal to
   `brassclaw_triggers`. The composition layer is trusted by design.

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

### Step 15.1b — `brassclaw_triggers` — `TrustedTriggerSubmitRequest::new` is now public

Add a test confirming that `TrustedTriggerSubmitRequest::new(fire, materialized, now)` can
be called from outside the worker module (verify visibility change didn't break anything).

### Step 15.2 — `brassclaw_reborn_composition` unit tests

- `RebornWebuiAutomationFacade::create_automation` — happy path + cron parse
  error propagation
- `RebornWebuiAutomationFacade::delete_automation` — `HasActiveFire` variant
  propagation
- `parse_single_automation_output` — unknown fields are tolerated, unknown
  schedule kind filtered
- `RebornWebuiAutomationFacade::fire_automation_now` — happy path returns `run_ref`;
  `state != Scheduled` returns `state_conflict`; `has_active_fire()` returns
  `trigger_has_active_fire`; materializer error propagates

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
# 0. Triggers crate — pub visibility + new trait methods + InMemory impls
cargo clippy -p brassclaw_triggers --all-targets -- -D warnings
cargo test -p brassclaw_triggers

# 1. Host runtime — 4 new capabilities + lib.rs exports
cargo clippy -p brassclaw_host_runtime --all-targets -- -D warnings
cargo test -p brassclaw_host_runtime

# 2. Composition adapter — new fields + fire_now direct path + webui.rs wiring
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
| **P0** | Make `TrustedTriggerSubmitRequest::new` public (`pub(crate)` → `pub`) in `crates/brassclaw_triggers/src/worker/ports.rs` line 21. Required before P3. | `brassclaw_triggers` |
| **P1** | Add `update_trigger` + `list_trigger_runs` (+ optional `set_trigger_state`) to `TriggerRepository` trait + postgres impl; add `TriggerUpdatePatch` + `TriggerRunRecord` structs; implement all new methods on `InMemoryTriggerRepository`. `upsert_trigger`, `get_trigger`, `remove_scoped_trigger` already exist — do not duplicate. | `brassclaw_triggers` |
| **P2** | (a) Patch `TriggerCreateInput` + `create_trigger` to accept `completion_policy`. (b) Extend `trigger_output()` to emit `"prompt"` only (`completion_policy` already present at line 320). (c) Add 4 new capability constants (`builtin.trigger_get/update/set_state/run_history` — fire_now excluded per §3.7) + handlers; register in `insert_trigger_handlers`; add manifests; update `lib.rs` pub-use exports (lines 82–83). | `brassclaw_host_runtime` |
| **P3** | (a) Add 3 new fields to `RebornWebuiAutomationFacade` (`trigger_repository`, `trusted_submitter`, `materializer`); update `new()` + `with_backend_timeout`; update `webui.rs` construction (lines 61–64). (b) Add 6 new methods to `AutomationProductFacade` trait; stub on `UnsupportedAutomationProductFacade`; implement all 6 on `RebornWebuiAutomationFacade`. (c) Extend `RawAutomationRecord` + `automation_info()` for `prompt`/`completion_policy`. | `brassclaw_product_workflow` + `brassclaw_reborn_composition` |
| **P4** | Add 8 DTO types; add 7 default-501 trait methods to `RebornServicesApi`; implement concretely on `RebornServices`; extend `RebornAutomationInfo` with `prompt` + `completion_policy` fields. | `brassclaw_product_workflow` |
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

1. **Repository method names — RESOLVED.** `upsert_trigger`, `get_trigger`, and
   `remove_scoped_trigger` all exist in `TriggerRepository` (lines 609, 611, 640
   of `brassclaw_triggers/src/lib.rs`). Only `update_trigger` and
   `list_trigger_runs` are genuinely new. `set_trigger_state` is optional but
   recommended for atomicity.

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
| `crates/brassclaw_triggers/src/worker/ports.rs` | Make `TrustedTriggerSubmitRequest::new` `pub` (was `pub(crate)`, line 21) |
| `crates/brassclaw_triggers/src/lib.rs` | Add `TriggerUpdatePatch`, `TriggerRunRecord`; add `update_trigger` + `list_trigger_runs` (+ optional `set_trigger_state`) to `TriggerRepository` trait; implement all on `InMemoryTriggerRepository` |
| `crates/brassclaw_triggers/src/postgres.rs` | Implement `update_trigger`, `list_trigger_runs`, optionally `set_trigger_state` |
| `crates/brassclaw_host_runtime/src/lib.rs` | Add 4 new capability constants to `pub use first_party_tools::{ ... }` block (lines 82–83) |
| `crates/brassclaw_host_runtime/src/first_party_tools/trigger_management.rs` | Extend `TriggerCreateInput` + `create_trigger` for `completion_policy`; extend `trigger_output()` to add `"prompt"` only (`completion_policy` already present); add 4 new capability constants (`get/update/set_state/run_history` only) + handler branches + manifests |
| `crates/brassclaw_reborn_composition/src/automation.rs` | Add 3 new fields to `RebornWebuiAutomationFacade`; update `new()` + `with_backend_timeout`; implement 6 new methods; extend `RawAutomationRecord` + `automation_info()` for `prompt`/`completion_policy`; add `CreateAutomationInput` / `UpdateAutomationInput` structs |
| `crates/brassclaw_reborn_composition/src/webui.rs` | Update facade construction (lines 61–64) to pass `trigger_repository`, `trusted_submitter`, `materializer` |
| `crates/brassclaw_product_workflow/src/reborn_services.rs` | Add 6 new methods to `AutomationProductFacade` trait; stub on `UnsupportedAutomationProductFacade`; add 7 new `RebornServicesApi` trait methods (default 501 + concrete impl on `RebornServices`) |
| `crates/brassclaw_product_workflow/src/reborn_services/types.rs` | Add 8 request/response DTO types; extend `RebornAutomationInfo` with `prompt` + `completion_policy` fields |
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
