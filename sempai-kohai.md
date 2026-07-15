# Sempai–Kohai: Dual-Role Provider Architecture & Prompt Interception System

> **Status**: Planning document — not yet implemented.  
> **Scope**: This document is the canonical implementation plan for the Sempai–Kohai split. Every step is a self-contained, shippable unit. Each step validates against the repository's zero-warning Clippy policy and full test suite before being considered done.

---

## Background and Motivation

BrassClaw currently treats all LLM providers as interchangeable inference backends selected by a single "Use" button. The Sempai–Kohai model introduces a hard role distinction and an independent interception service that sits permanently between prompt assembly and the tokenizer.

- **Kohai** (後輩 — "junior") is the primary inference provider. It receives the final assembled and tokenized prompt, executes tool calls, writes code, and produces visible output. The existing `llm.active_provider` maps directly onto the Kohai role.
- **Sempai** (先輩 — "senior") is the teaching and auditing provider. It is never visible in normal output. It operates through a **standalone interceptor service** that receives every fully-assembled prompt before it reaches the tokenizer. The interceptor stores the prompt and all segment-level construction data in its own database, then waits for the Sempai to connect and audit. After the Sempai completes its audit, the (possibly modified) prompt is forwarded to the tokenizer and then to the Kohai model.

The interceptor is not a function call inside the loop engine. It is a service boundary. The prompt does not move forward until the interceptor has persisted it and the Sempai has reviewed it.

---

## Component Type System

All recipes, skills, and tools carry one or more **type tags** that declare which role they are intended for. A component can carry any combination of all four types simultaneously.

| Type tag | Meaning |
|----------|---------|
| `LLM` | Used during general LLM interaction, available to both Kohai and Sempai contexts |
| `Kohai` | Available exclusively when building prompts for the Kohai inference model |
| `Sempai` | Available exclusively when building prompts for the Sempai auditor |
| `Agent` | Available to the agentic execution loop (tool calls, code, planning) |

These tags replace the old `role_only` binary. A component tagged `[Kohai, Agent]` appears in Kohai prompts and in the agent tool inventory, but not in Sempai audit prompts. A component tagged `[Sempai]` only appears in Sempai interception prompts. Components tagged with all four types are universal.

This type system is the sorting mechanism for prompt block assembly — each block in the final prompt only includes components whose type set intersects the target role.

---

## IBM Bob Integration: Two Connection Types

IBM Bob (HiBob) is an HR/People platform exposing a comprehensive REST API at `https://api.hibob.com/v1`. The API is authenticated via a Service User ID + Token pair sent as a Basic Auth header (`Authorization: Basic base64(serviceUserId:token)`).

BrassClaw integrates IBM Bob in **two separate, non-interchangeable ways**:

---

### Connection Type 1 — General (Direct REST, per API group)

The General connection is not a provider in the LLM sense. It is a collection of **recipes, skills, and tools** that call the HiBob REST API directly over HTTPS. There is no wrapper protocol and no Rust HTTP client abstraction beyond the standard `reqwest`-based HTTP capability already in the codebase.

Each IBM Bob API group becomes its own dedicated skill bundle containing:
- One or more **recipes** describing the full call flow for that group (auth header construction, URL assembly, expected response shape)
- **Tools** wrapping individual endpoints as structured capability calls
- A **skill** (`SKILL.md`) documenting the group, its data model, and when to use it

The recipe for each group encodes everything needed for the agent to make a correct call without any ambient knowledge: the base URL, how to build the `Authorization: Basic base64(id:token)` header from stored credentials, the exact URL template for each endpoint, required query/body parameters, and the expected response format and field types.

**IBM Bob API groups → BrassClaw skill bundles:**

| HiBob API Group | Skill bundle | Component types | Tools |
|-----------------|-------------|-----------------|-------|
| **People / Employee Data** | `skills/ibm_bob/people/` | `[Agent, Kohai]` | `bob_search_employees`, `bob_get_employee`, `bob_create_employee`, `bob_update_employee`, `bob_terminate_employee`, `bob_invite_employee`, `bob_get_avatar` |
| **Employee Tables** | `skills/ibm_bob/employee_tables/` | `[Agent, Kohai]` | `bob_get_work_history`, `bob_get_lifecycle`, `bob_get_employment_history`, `bob_get_salary_history`, `bob_get_equity_grants`, `bob_get_variable_pay`, `bob_get_training`, `bob_get_bank_accounts` |
| **Metadata (Fields + Lists)** | `skills/ibm_bob/metadata/` | `[Agent, LLM]` | `bob_get_fields`, `bob_get_lists`, `bob_get_list_by_name`, `bob_create_field`, `bob_update_field`, `bob_add_list_item` |
| **Custom Tables** | `skills/ibm_bob/custom_tables/` | `[Agent, Kohai]` | `bob_list_custom_tables`, `bob_get_custom_table_entries`, `bob_create_custom_entry`, `bob_update_custom_entry`, `bob_delete_custom_entry` |
| **Time Off** | `skills/ibm_bob/timeoff/` | `[Agent, Kohai]` | `bob_submit_timeoff`, `bob_get_timeoff_request`, `bob_cancel_timeoff`, `bob_get_timeoff_changes`, `bob_whos_out`, `bob_whos_out_today`, `bob_get_balance`, `bob_create_balance_adjustment`, `bob_get_policy_types`, `bob_get_policies`, `bob_search_calendar_events` |
| **Attendance** | `skills/ibm_bob/attendance/` | `[Agent, Kohai]` | `bob_import_punches`, `bob_fetch_summaries`, `bob_fetch_daily_breakdown`, `bob_search_entries`, `bob_create_entries`, `bob_update_entries`, `bob_clock_in`, `bob_clock_out`, `bob_delete_entry` |
| **Attendance Projects** | `skills/ibm_bob/attendance_projects/` | `[Agent, Kohai]` | `bob_search_projects`, `bob_create_projects`, `bob_update_project`, `bob_archive_project`, `bob_restore_project`, `bob_search_project_tasks`, `bob_create_project_tasks`, `bob_search_project_clients` |
| **Tasks** | `skills/ibm_bob/tasks/` | `[Agent, Kohai]` | `bob_get_open_tasks`, `bob_get_employee_tasks`, `bob_complete_task` |
| **Reports** | `skills/ibm_bob/reports/` | `[Agent, LLM]` | `bob_list_reports`, `bob_download_report`, `bob_get_report_download_url`, `bob_download_report_by_name` |
| **Documents** | `skills/ibm_bob/documents/` | `[Agent, Kohai]` | `bob_list_folders`, `bob_list_employee_docs`, `bob_upload_to_shared_folder`, `bob_upload_to_confidential_folder`, `bob_upload_to_custom_folder`, `bob_delete_document` |
| **Goals** | `skills/ibm_bob/goals/` | `[Agent, Kohai]` | `bob_search_goals`, `bob_create_goal`, `bob_update_goal`, `bob_delete_goal`, `bob_search_key_results`, `bob_create_key_result`, `bob_update_key_result_progress`, `bob_delete_key_result`, `bob_search_goal_cycles` |
| **Workforce Planning** | `skills/ibm_bob/workforce/` | `[Agent, LLM]` | `bob_search_positions`, `bob_create_position`, `bob_update_position`, `bob_search_position_openings`, `bob_create_position_opening`, `bob_search_position_budgets`, `bob_create_position_budget`, `bob_cancel_position` |
| **Job Catalog** | `skills/ibm_bob/job_catalog/` | `[Agent, LLM]` | `bob_search_job_profiles`, `bob_get_job_roles`, `bob_get_job_families`, `bob_get_job_family_groups` |
| **Hiring** | `skills/ibm_bob/hiring/` | `[Agent, Kohai]` | `bob_search_job_openings`, `bob_search_candidates`, `bob_search_applications`, `bob_get_job_ads`, `bob_get_job_ad`, `bob_search_interviews`, `bob_search_evaluations`, `bob_search_offers` |
| **Learning** | `skills/ibm_bob/learning/` | `[Agent, Kohai]` | `bob_create_learning_integration`, `bob_delete_learning_integration`, `bob_create_training_content`, `bob_update_training_content`, `bob_archive_training_content`, `bob_submit_xapi_statement` |
| **Onboarding** | `skills/ibm_bob/onboarding/` | `[Agent, Kohai]` | `bob_get_onboarding_wizards` |
| **Webhooks** | `skills/ibm_bob/webhooks/` | `[Agent, LLM]` | `bob_register_webhook` (recipe only — documents the webhook payload shapes for all event types) |

Each recipe carries:
```yaml
auth:
  type: basic
  credential_key: hibob_service_user_id     # stored in BrassClaw secrets
  credential_secret: hibob_service_user_token
base_url: https://api.hibob.com/v1
headers:
  Authorization: "Basic base64({credential_key}:{credential_secret})"
  Content-Type: application/json
  Accept: application/json
response_format: json
error_handling:
  401: credential_invalid
  403: permission_missing
  429: rate_limited        # Bob enforces per-endpoint rate limits
  404: not_found
```

---

### Connection Type 2 — Interference (openai_compatible provider for Sempai)

The Interference connection is a **standard LLM provider** registered in `providers.json` using the `openai_compatible` protocol. It is the connection used when IBM Bob is assigned to the **Sempai role**. It connects to the HiBob API endpoint that exposes an OpenAI-compatible chat completions interface.

No wrapper or custom Rust client is needed — the existing `openai_compatible` protocol handler in `crates/brassclaw_llm` covers this exactly.

```json
{
  "id": "ibm_bob_inference",
  "aliases": ["hibob_inference", "bob_inference"],
  "protocol": "openai_compatible",
  "display_name": "IBM Bob (Inference)",
  "setup": {
    "kind": "api_key",
    "display_name": "IBM Bob Inference",
    "key_url": "https://app.hibob.com/settings/service-users",
    "can_list_models": false
  },
  "env": {
    "api_key":  "HIBOB_INFERENCE_TOKEN",
    "base_url": "HIBOB_INFERENCE_BASE_URL",
    "model":    "HIBOB_INFERENCE_MODEL"
  },
  "defaults": {
    "base_url": "https://api.hibob.com/v1/ai",
    "model":    "bob-latest"
  },
  "context_window_tokens": 32768,
  "unsupported_params": []
}
```

Authentication follows the same Service User ID + Token pattern. The `api_key` field holds the `base64(serviceUserId:token)` string, passed as `Authorization: Bearer <api_key>` per the OpenAI-compatible spec.

---

## Implementation Plan

The plan consists of **12 atomic steps**. Each step is a single, mergeable PR. Steps are ordered so that every preceding step is a prerequisite for the one that follows. No step leaves the codebase in a broken or partially-valid state.

---

### Step 1 — Data model: `ProviderRole`, component `types` field, DB migration

**Goal**: Introduce the `ProviderRole` enum and the four-value component type system everywhere that stores provider identity and component metadata — without changing any runtime behaviour. After this step the system still behaves identically to today but has the schema to support Sempai and the type-tagged component system.

#### 1.1 `ProviderRole` enum

File: `crates/brassclaw_llm/src/role.rs` (new)

```rust
/// The functional role a configured LLM provider is assigned to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRole {
    /// Primary inference model — executes tool calls, writes code, produces output.
    Kohai,
    /// Teaching/auditing model — intercepts assembled prompts before Kohai receives them.
    Sempai,
}
```

Re-export from `crates/brassclaw_llm/src/lib.rs`.

#### 1.2 `ComponentType` enum

File: `crates/brassclaw_skills/src/component_type.rs` (new)

```rust
/// Declares which execution context a recipe, skill, or tool is intended for.
/// A component may carry any combination of these types simultaneously.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentType {
    /// Available during general LLM interaction (both Kohai and Sempai contexts).
    Llm,
    /// Available only when assembling prompts for the Kohai inference model.
    Kohai,
    /// Available only when assembling prompts for the Sempai auditor.
    Sempai,
    /// Available to the agentic execution loop (tool calls, code, planning).
    Agent,
}
```

A `ComponentTypeSet` is `Vec<ComponentType>` with a helper `fn contains(role: ProviderRole) -> bool`.

#### 1.3 Skill frontmatter gains `types` field

Extend the YAML skill frontmatter schema in `crates/brassclaw_skills/src/schema.rs`:

```yaml
types: [llm, kohai, agent]   # replaces the old role_only field (removed)
```

Default when absent: `[llm, kohai, agent]` — i.e. universally available except to the Sempai auditor, which is the conservative default.

`SkillRegistry::select_for_thread()` is updated to filter by `ComponentTypeSet` intersection with the caller's role context.

#### 1.4 Settings key rename

| Old key | New key | Notes |
|---------|---------|-------|
| `llm.active_provider` | `llm.kohai_provider` | renamed; old key still accepted as read alias |
| _(new)_ | `llm.sempai_provider` | optional; absent = interceptor waits but has no Sempai to connect |

Migration: on first read of `llm.kohai_provider`, if absent but `llm.active_provider` exists, copy the value and write the new key. The old key is never deleted (backward compat).

#### 1.5 DB migration

File: `migrations/NNNN_provider_role.sql`

```sql
-- Record that this instance has been migrated to the sempai-kohai schema.
-- Settings table is key-value; no column changes needed.
INSERT INTO settings (key, value)
  VALUES ('schema.sempai_kohai_version', '"1"')
  ON CONFLICT DO NOTHING;
```

Both Postgres and libSQL run this via `Database::run_migrations()`.

#### 1.6 `LlmConfigResponse` update

```rust
pub struct LlmConfigResponse {
    /// The Kohai (primary inference) provider id.
    pub kohai_provider_id: Option<String>,
    /// The Sempai (auditor) provider id. Absent = interceptor disabled.
    pub sempai_provider_id: Option<String>,
    /// Back-compat alias for kohai_provider_id. Clients that only know
    /// the old field continue to work.
    pub active_provider_id: Option<String>,
    pub providers: Vec<LlmProviderInfo>,
}

pub struct LlmProviderInfo {
    pub id: String,
    pub display_name: String,
    pub model: String,
    pub configured: bool,
    pub is_kohai: bool,
    pub is_sempai: bool,
}
```

#### 1.7 Tests

- Unit test: `ProviderRole` round-trips through `serde_json`.
- Unit test: `ComponentType` set intersection correctly includes/excludes skills by role context.
- Unit test: skill frontmatter with `types: [sempai]` is excluded from Kohai prompt selection.
- Unit test: skill frontmatter without `types` field defaults to `[llm, kohai, agent]`.
- Unit test: settings migration shim promotes `llm.active_provider` → `llm.kohai_provider` exactly once.
- Existing `set_active_llm` test passes (now targets Kohai role implicitly).

---

### Step 2 — IBM Bob: General connection (skills/recipes/tools per API group)

**Goal**: Create the full IBM Bob General connection — one skill bundle per HiBob API group, each containing a recipe with auth/URL/response instructions, and tools for individual endpoints. No new Rust protocol code needed. Uses the existing HTTP capability.

#### 2.1 Credential store

Two secrets are stored via the existing `SecretsStore`:
- `hibob_service_user_id` — the service user's login ID
- `hibob_service_user_token` — the service user's token

The `Authorization` header is assembled by every Bob recipe as:
```
Authorization: Basic base64({hibob_service_user_id}:{hibob_service_user_token})
```

This is documented in each recipe. No credential processing happens in Rust code — the recipe carries the construction rule and the agent assembles it at call time using the credential values from the secrets store.

#### 2.2 Skill bundle structure

Each API group lives at `skills/ibm_bob/<group>/SKILL.md` with the common frontmatter pattern:

```yaml
---
name: ibm_bob_<group>
version: "1.0.0"
types: [agent, kohai]         # or [agent, llm] for metadata/reporting groups
description: >
  IBM Bob <Group> API. Manages <domain description>.
activation:
  keywords: [...]
  tags: ["ibm_bob", "hr", "<group>"]
credentials:
  - name: hibob_service_user_id
    location: { type: header, header: Authorization, template: "Basic base64({id}:{secret})" }
    secret_name: hibob_service_user_token
    hosts: ["api.hibob.com"]
---
```

#### 2.3 Skill bundles created (one per API group)

All 17 bundles listed in the IBM Bob Integration section above are created in this step. Each bundle's Markdown body contains:

1. **Auth section**: step-by-step instruction for constructing the `Authorization: Basic base64(id:token)` header, noting that the id and token come from the `hibob_service_user_id` / `hibob_service_user_token` secrets.
2. **Base URL**: `https://api.hibob.com/v1`
3. **Rate limit guidance**: Bob enforces per-endpoint rate limits; recipes must check for `HTTP 429` and back off.
4. **Endpoint table**: each endpoint in the group with method, URL template, required parameters, and response fields.
5. **Response format**: JSON with `humanReadable` + `value` field duality for list-linked fields.
6. **Permission notes**: which permission category the service user needs for each operation.
7. **Error codes**: `401` (bad credentials), `403` (permission missing), `404` (not found), `422` (validation error), `429` (rate limited).

#### 2.4 Capability tools per group

Each tool is registered in `crates/brassclaw_extensions/capabilities/ibm_bob/<group>/mod.rs` as an `ActionDef`:

```rust
ActionDef {
    name: "bob_search_employees",
    description: "Search for employees in IBM Bob using filters. POST /v1/people/search.",
    effect: Effect::ReadExternal,
    requires_approval: RequiresApproval::Never,
    parameters: json_schema!({
        "filters": { "type": "array", "description": "Bob search filter objects" },
        "fields":  { "type": "array", "items": { "type": "string" },
                     "description": "Fields to return (dot-path format)" },
        "humanReadable": { "type": "boolean", "default": true }
    }),
}
```

Write tools (`bob_create_employee`, `bob_terminate_employee`, `bob_upload_*`, `bob_create_*`, `bob_delete_*`) use `requires_approval: RequiresApproval::Required` and `effect: Effect::WriteExternal`.

All tools tagged with `component_types: [Agent, Kohai]` (or `[Agent, LLM]` for metadata/reporting tools) matching the skill bundle.

#### 2.5 Tests

- Unit test: each skill bundle parses through `SkillRegistry::load_skill()`.
- Unit test: all 17 skill bundles resolve correct `ComponentTypeSet`.
- Unit test: `bob_search_employees` tool definition round-trips through capability registry.
- Unit test: write tools have `RequiresApproval::Required`.
- Unit test: metadata group tools have `ComponentType::Llm` in their type set.

---

### Step 3 — IBM Bob: Interference connection (openai_compatible Sempai provider)

**Goal**: Register the IBM Bob Inference provider in `providers.json` so it appears in the WebUI provider list and can be assigned to the Sempai role. This is a pure data change — no new Rust code required because `openai_compatible` already handles this protocol.

#### 3.1 `providers.json` entry

```json
{
  "id": "ibm_bob_inference",
  "aliases": ["hibob_inference", "bob_inference", "bob_sempai"],
  "protocol": "openai_compatible",
  "display_name": "IBM Bob (Inference / Sempai)",
  "setup": {
    "kind": "api_key",
    "display_name": "IBM Bob Inference",
    "key_url": "https://app.hibob.com/settings/service-users",
    "can_list_models": false,
    "notes": "Set api_key to base64(serviceUserId:token). Assign this provider to the Sempai role for prompt interception."
  },
  "env": {
    "api_key":  "HIBOB_INFERENCE_TOKEN",
    "base_url": "HIBOB_INFERENCE_BASE_URL",
    "model":    "HIBOB_INFERENCE_MODEL"
  },
  "defaults": {
    "base_url": "https://api.hibob.com/v1/ai",
    "model":    "bob-latest"
  },
  "context_window_tokens": 32768,
  "unsupported_params": []
}
```

#### 3.2 Tests

- Unit test: `ibm_bob_inference` resolves through `ProviderRegistry::find("ibm_bob_inference")`.
- Unit test: alias `"bob_inference"` resolves to the same definition.
- Unit test: `protocol` field is `openai_compatible` — no custom factory dispatch needed.

---

### Step 4 — WebUI v2: "Use as Kohai" + "Use as Sempai" buttons

**Goal**: Replace the single "Use" button with two role-specific buttons. Update the API to accept a `role` field. Update the provider card component.

#### 4.1 API change: `POST /api/webchat/v2/llm/active`

Extended request body:

```typescript
interface SetActiveLlmRequest {
  provider_id: string;
  role: "kohai" | "sempai";  // required; defaults to "kohai" if absent (back-compat)
}
```

Server side:

```rust
pub async fn set_active_llm(
    caller: WebUiAuthenticatedCaller,
    request: SetActiveLlmRequest,
    services: &dyn RebornServicesApi,
) -> Result<(), RebornServicesError> {
    // Cannot assign same provider to both roles.
    let other_key = match request.role.unwrap_or(ProviderRole::Kohai) {
        ProviderRole::Kohai  => "llm.sempai_provider",
        ProviderRole::Sempai => "llm.kohai_provider",
    };
    let other = services.db.get_setting(other_key).await?;
    if other.as_deref() == Some(&request.provider_id) {
        return Err(RebornServicesError::Conflict {
            reason: "provider_already_assigned_to_other_role".into(),
        });
    }
    let key = match request.role.unwrap_or(ProviderRole::Kohai) {
        ProviderRole::Kohai  => "llm.kohai_provider",
        ProviderRole::Sempai => "llm.sempai_provider",
    };
    services.db.set_setting(key, &request.provider_id).await?;
    services.llm_reload_handle.reload().await?;
    Ok(())
}
```

#### 4.2 Frontend provider card update

File: `crates/brassclaw_webui_v2/src/components/ProviderCard.tsx` (or equivalent)

- Remove the single `<Button label="Use" />`.
- Add `<Button label="Use as Kohai" variant="primary" />` — calls `POST /llm/active` with `{ provider_id, role: "kohai" }`.
- Add `<Button label="Use as Sempai" variant="secondary" />` — calls `POST /llm/active` with `{ provider_id, role: "sempai" }`.
- Active state badges: `Kohai` (blue pill) and `Sempai` (violet pill) displayed on the currently-assigned cards.
- If no Sempai is assigned, show a muted banner at the top of the provider panel: `"No Sempai assigned — prompt interception is disabled"`.

#### 4.3 Tests

- Unit test: `role: "kohai"` writes `llm.kohai_provider`.
- Unit test: `role: "sempai"` writes `llm.sempai_provider`.
- Unit test: conflict check returns `HTTP 409` when same provider assigned to both roles.
- Unit test: absent `role` field defaults to Kohai.
- Snapshot test: provider card renders "Use as Kohai" and "Use as Sempai" buttons; correct active badges shown.

---

### Step 5 — Sempai provider slot in `SwappableLlmProvider`

**Goal**: Boot and maintain a live `Arc<dyn LlmProvider>` for the Sempai role inside `SwappableLlmProvider`, parallel to the Kohai slot. The slot is optional — absent = interceptor service starts but has no Sempai to connect to.

#### 5.1 `SempaiSlot` type

File: `crates/brassclaw_llm/src/sempai_slot.rs` (new)

```rust
/// Represents the availability state of the configured Sempai provider.
pub enum SempaiSlot {
    /// No Sempai provider configured. The interceptor service starts but
    /// queues prompts indefinitely until a Sempai is configured.
    Unconfigured,
    /// A Sempai provider is configured and ready to receive interception calls.
    Active(Arc<dyn LlmProvider>),
}
```

#### 5.2 `SwappableLlmProvider` gains Sempai field

```rust
pub struct SwappableLlmProvider {
    kohai: Arc<RwLock<Arc<dyn LlmProvider>>>,
    sempai: Arc<RwLock<SempaiSlot>>,
}
```

`LlmReloadHandle::reload()` loads both `llm.kohai_provider` and `llm.sempai_provider` from settings, creates both providers (or `Unconfigured`), and atomically swaps both `RwLock`s.

#### 5.3 Forensic builder overhead guard

The `ForensicBuilder` (Step 6) accumulates segment data only when `SempaiSlot::Active`. When `Unconfigured`, all `record_*` calls are `#[inline(always)]` no-ops. This ensures zero runtime overhead when no Sempai is configured.

#### 5.4 Tests

- Unit test: reload with no `llm.sempai_provider` setting → `SempaiSlot::Unconfigured`.
- Unit test: reload with a valid provider id → `SempaiSlot::Active(...)`.
- Unit test: reload is atomic — concurrent reads never see partial state.

---

### Step 6 — Interceptor service: standalone architecture + DB schema

**Goal**: Implement the interceptor as a standalone async service with its own database. The interceptor is the permanent boundary between prompt assembly and the tokenizer. Every prompt passes through it and is stored before being forwarded.

#### 6.1 Architecture

The interceptor is not a function call inside the orchestrator loop. It is a separate `tokio` task (or microservice process) that communicates with the orchestrator via a bounded channel. The orchestrator submits an assembled prompt to the interceptor channel and suspends. The interceptor stores the prompt, waits for the Sempai to audit it, then sends back the (possibly modified) prompt. The orchestrator resumes and sends the result to the tokenizer and then to Kohai.

```
Orchestrator assembles prompt
          │
          │ PromptInterceptRequest { prompt_id, final_messages, forensic_packet }
          ▼
  ┌───────────────────────────────────┐
  │  InterceptorService               │
  │  ┌─────────────────────────────┐  │
  │  │ interceptor_db              │  │
  │  │  · prompts                  │  │
  │  │  · prompt_segments          │  │
  │  │  · prompt_tokenized         │  │
  │  │  · sempai_audits            │  │
  │  │  · proposed_updates         │  │
  │  └─────────────────────────────┘  │
  │                                   │
  │  Waits for Sempai connection ──────┼──→ Sempai (LLM provider)
  │  or human connection ─────────────┼──→ Human reviewer (iPhone push)
  └───────────────────────────────────┘
          │
          │ PromptInterceptResponse { prompt_id, revised_messages, audit_summary, proposed_updates }
          ▼
  Orchestrator forwards to tokenizer → Kohai
```

The interceptor service is created at runtime boot and wired into `RebornRuntime`. It exposes a single async method:

```rust
pub trait PromptInterceptorService: Send + Sync {
    /// Submit an assembled prompt for interception. Suspends until the
    /// Sempai (or human reviewer) has completed the audit.
    async fn intercept(
        &self,
        request: PromptInterceptRequest,
    ) -> Result<PromptInterceptResponse, InterceptorError>;
}
```

#### 6.2 `PromptInterceptRequest`

```rust
pub struct PromptInterceptRequest {
    /// Unique identifier for this prompt (UUID, assigned by the orchestrator).
    pub prompt_id: PromptId,
    /// The assembled messages ready to send to Kohai (before tokenization).
    pub final_messages: Vec<AssembledMessage>,
    /// The complete forensic packet describing how this prompt was built.
    pub forensic_packet: PromptForensicPacket,
    /// All model/tokenizer parameters that apply to this prompt.
    pub model_params: ModelParams,
}

pub struct ModelParams {
    pub model_id: String,
    pub model_max_len: u32,
    pub context_window_tokens: u32,
    pub max_completion_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub stop_sequences: Vec<String>,
    pub seed: Option<u64>,
    /// Any additional provider-specific parameters passed verbatim.
    pub extra_params: serde_json::Value,
}
```

#### 6.3 `PromptInterceptResponse`

```rust
pub struct PromptInterceptResponse {
    pub prompt_id: PromptId,
    /// The messages to forward to the tokenizer and then to Kohai.
    /// May be identical to the input if the Sempai made no changes.
    pub revised_messages: Vec<AssembledMessage>,
    /// Human-readable audit summary written by the Sempai or human reviewer.
    pub audit_summary: String,
    /// Whether the messages were changed from the original.
    pub was_modified: bool,
    /// Proposed updates to skills/recipes/tools. Stored in the DB; not applied.
    pub proposed_updates: Vec<ProposedArtifactUpdate>,
}
```

#### 6.4 Interceptor database schema

Migration file: `migrations/NNNN_interceptor_db.sql`

This migration creates all interceptor tables. No `tenant_id` columns — single shared store.

```sql
-- One row per assembled prompt entering the interceptor.
CREATE TABLE interceptor_prompts (
    id              TEXT    PRIMARY KEY,   -- PromptId (UUID)
    run_id          TEXT    NOT NULL,
    thread_id       TEXT    NOT NULL,
    status          TEXT    NOT NULL DEFAULT 'pending',
    -- status values: pending | sempai_connected | human_connected
    --                audited | forwarded | error
    assembled_at    TEXT    NOT NULL,
    forwarded_at    TEXT,                  -- set when Kohai receives the prompt
    audit_completed_at TEXT,
    model_params_json  TEXT NOT NULL,      -- ModelParams serialised (zstd compressed)
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE INDEX idx_ip_run_id    ON interceptor_prompts(run_id);
CREATE INDEX idx_ip_thread_id ON interceptor_prompts(thread_id);
CREATE INDEX idx_ip_status    ON interceptor_prompts(status);

-- The final assembled messages for each prompt (before Sempai review).
CREATE TABLE interceptor_prompt_final (
    prompt_id       TEXT    PRIMARY KEY REFERENCES interceptor_prompts(id),
    messages_json   TEXT    NOT NULL       -- Vec<AssembledMessage> zstd compressed
);

-- The tokenized version of each prompt (stored after tokenization, post-audit).
CREATE TABLE interceptor_prompt_tokenized (
    prompt_id       TEXT    PRIMARY KEY REFERENCES interceptor_prompts(id),
    token_ids_json  TEXT    NOT NULL,      -- Vec<u32> JSON
    token_count     INTEGER NOT NULL,
    tokenized_at    TEXT    NOT NULL
);

-- Per-segment decision data. Each segment of the prompt creation process
-- sends its data here as it finishes, tagged with the prompt_id.
CREATE TABLE interceptor_prompt_segments (
    id              TEXT    PRIMARY KEY,
    prompt_id       TEXT    NOT NULL REFERENCES interceptor_prompts(id),
    segment_name    TEXT    NOT NULL,
    source          TEXT    NOT NULL,
    token_count     INTEGER NOT NULL,
    -- The decision path: every choice made and every alternative not taken.
    decision_path_json   TEXT NOT NULL,
    -- What this segment chose to include and why.
    chosen_content  TEXT    NOT NULL,
    -- Content before reduction rules were applied.
    verbatim_content TEXT   NOT NULL,
    -- Escape paths: what else could have been chosen here, and the conditions
    -- under which those alternatives would have been selected instead.
    escape_paths_json TEXT  NOT NULL,
    segment_order   INTEGER NOT NULL,      -- position in the final prompt
    received_at     TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE INDEX idx_ips_prompt_id ON interceptor_prompt_segments(prompt_id);

-- Sempai audit results (one per prompt).
CREATE TABLE interceptor_audits (
    id                  TEXT PRIMARY KEY,
    prompt_id           TEXT NOT NULL REFERENCES interceptor_prompts(id),
    audited_by          TEXT NOT NULL,     -- "sempai" | "human:<user_id>"
    audit_summary       TEXT NOT NULL,
    was_modified        INTEGER NOT NULL DEFAULT 0,
    revised_messages_json TEXT,            -- NULL if unchanged
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE INDEX idx_ia_prompt_id ON interceptor_audits(prompt_id);

-- Proposed artifact updates from the Sempai (queued, never auto-applied).
CREATE TABLE interceptor_proposed_updates (
    id              TEXT    PRIMARY KEY,
    audit_id        TEXT    NOT NULL REFERENCES interceptor_audits(id),
    artifact_kind   TEXT    NOT NULL,   -- "skill" | "recipe" | "tool"
    artifact_id     TEXT    NOT NULL,
    change_type     TEXT    NOT NULL,   -- "update" | "create" | "delete"
    patch_json      TEXT    NOT NULL,
    rationale       TEXT    NOT NULL,
    -- Two-step validation state:
    auto_validated  INTEGER NOT NULL DEFAULT 0,   -- 0=pending, 1=pass, -1=fail
    auto_validated_at TEXT,
    auto_validation_notes TEXT,
    manual_validated  INTEGER NOT NULL DEFAULT 0, -- 0=pending, 1=approved, -1=rejected
    manual_validated_by TEXT,
    manual_validated_at TEXT,
    manual_validation_notes TEXT,
    -- Effective status derived from both validation steps:
    status          TEXT    NOT NULL DEFAULT 'pending',
    -- status: pending | auto_failed | awaiting_manual | approved | rejected
    applied_at      TEXT,
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE INDEX idx_ipu_audit_id ON interceptor_proposed_updates(audit_id);
CREATE INDEX idx_ipu_status   ON interceptor_proposed_updates(status);
```

PostgreSQL variant uses `UUID`, `TIMESTAMPTZ`, `JSONB` instead of `TEXT` where appropriate.

#### 6.5 `InterceptorStore` trait

File: `src/db/interceptor_store.rs` (new)

```rust
#[async_trait]
pub trait InterceptorStore: Send + Sync {
    async fn save_prompt(&self, req: &PromptInterceptRequest) -> Result<(), DbError>;
    async fn save_segment(&self, prompt_id: &PromptId, seg: &SegmentRecord) -> Result<(), DbError>;
    async fn save_tokenized(&self, prompt_id: &PromptId, token_ids: &[u32]) -> Result<(), DbError>;
    async fn save_audit(&self, audit: &AuditRecord) -> Result<(), DbError>;
    async fn queue_proposed_update(&self, audit_id: &str, update: &ProposedArtifactUpdate) -> Result<(), DbError>;
    async fn set_prompt_status(&self, prompt_id: &PromptId, status: PromptStatus) -> Result<(), DbError>;
    async fn get_prompt(&self, prompt_id: &PromptId) -> Result<Option<StoredPrompt>, DbError>;
    async fn list_pending_prompts(&self) -> Result<Vec<StoredPrompt>, DbError>;
    async fn list_pending_updates(&self) -> Result<Vec<PendingArtifactUpdate>, DbError>;
    async fn set_auto_validation(&self, update_id: &str, result: ValidationResult, notes: &str) -> Result<(), DbError>;
    async fn set_manual_validation(&self, update_id: &str, result: ValidationResult, by: &str, notes: &str) -> Result<(), DbError>;
}
```

Added as a supertrait of `Database`. Implemented for both `PostgresDb` and `LibSqlDb`.

#### 6.6 Size and compression

- `messages_json`, `model_params_json`, `token_ids_json`, `decision_path_json`, `escape_paths_json`: all stored compressed (zstd level 3), decompressed on read.
- Max uncompressed `messages_json` size: 4 MiB. If exceeded, `verbatim_content` fields in segments are stripped and flagged.
- `interceptor_prompts` rows older than 90 days are archived to a cold table (`interceptor_prompts_archive`) on a nightly background job, not deleted.

#### 6.7 Tests

- Unit test: `save_prompt` + `save_segment` + `save_tokenized` + `save_audit` lifecycle for both backends.
- Unit test: zstd compress/decompress round-trip for all JSON columns.
- Unit test: packet exceeding 4 MiB strips verbatim content and sets the flag.
- Unit test: `list_pending_prompts` returns only prompts with `status = 'pending'`.

---

### Step 7 — `PromptForensicPacket`: type definition + segment push model

**Goal**: Define the forensic data type and instrument every prompt-assembly subsystem to push its segment data to the interceptor **as it finishes**, tagged with the unique `PromptId`. Segments do not wait for all other segments — each pushes immediately when complete.

#### 7.1 `PromptForensicPacket` type

File: `crates/brassclaw_engine/src/types/forensic.rs` (new)

```rust
/// Full forensic description of a single assembled prompt.
/// This is the payload sent to the interceptor and from there to the Sempai.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PromptForensicPacket {
    pub prompt_id: PromptId,
    pub run_id: RunId,
    pub original_query: String,
    pub segments: Vec<PromptSegment>,
    pub final_messages: Vec<AssembledMessage>,
    pub token_accounting: TokenAccounting,
    pub reduction_decisions: Vec<ReductionDecision>,
    pub skill_selection: SkillSelectionOutcome,
    pub capability_inventory: Vec<CapabilitySnapshot>,
    pub kv_cache_analysis: KvCacheAnalysis,
    pub orchestrator_config: OrchestratorSnapshot,
    pub monty_design: MontyDesign,
    pub agent_design: AgentDesign,
    pub v2_design: V2DesignSnapshot,
    pub recipe_skill_tool_registry: RegistrySnapshot,
    pub model_params: ModelParams,
    pub assembled_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PromptSegment {
    /// Human-readable name (e.g. "system_preamble", "skill:github", "memory_doc:3").
    pub name: String,
    /// Which subsystem produced this segment.
    pub source: SegmentSource,
    /// Token count contributed by this segment.
    pub token_count: u32,
    /// Every decision that caused this content to be chosen over alternatives.
    pub decision_path: Vec<SegmentDecision>,
    /// Content as produced before any reduction rules ran.
    pub verbatim_content: String,
    /// Content after reduction rules ran (may equal verbatim).
    pub chosen_content: String,
    /// Alternative content choices that were not selected, with the conditions
    /// under which they would have been chosen instead.
    pub escape_paths: Vec<EscapePath>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EscapePath {
    /// What would have been chosen instead.
    pub alternative_content_summary: String,
    /// The condition that would have triggered this alternative.
    pub trigger_condition: String,
    /// Why the current content was preferred over this.
    pub reason_not_taken: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TokenAccounting {
    pub model_id: String,
    pub model_max_len: u32,
    pub limit_set_by: String,           // "config" | "provider_default" | "user_override"
    pub pre_reduction_estimated: u32,
    pub post_reduction_estimated: u32,
    pub compaction_threshold: f32,
    pub compaction_triggered: bool,
    pub reserved_for_completion: u32,
    pub effective_input_budget: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KvCacheAnalysis {
    /// True when the prompt structure maximises KV-cache hit rate:
    /// stable prefix first, volatile content last, no instruction shuffling.
    pub cache_friendly: bool,
    /// Segment names that introduce instability in the cached prefix.
    pub cache_busting_segments: Vec<String>,
    /// Estimated cache hit rate 0.0–1.0 vs. a naive uncached prompt.
    pub estimated_hit_rate: f32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReductionDecision {
    pub rule_type: String,  // "truncate" | "drop" | "history_compact" | "summarize" | "priority"
    pub applied_to: String,
    pub reason: String,
    pub tokens_saved: u32,
    /// The alternative that would have been chosen if this rule hadn't fired.
    pub escape_if_not_reduced: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillSelectionOutcome {
    pub budget_tokens: u32,
    pub candidates_evaluated: u32,
    pub skills_selected: Vec<String>,
    pub skills_dropped_overflow: Vec<String>,
    pub skills_dropped_gating: Vec<String>,
    pub skills_dropped_type_mismatch: Vec<String>,   // filtered out by ComponentType
}
```

(Sub-structs `CapabilitySnapshot`, `OrchestratorSnapshot`, `MontyDesign`, `AgentDesign`, `V2DesignSnapshot`, `RegistrySnapshot`, `SegmentSource`, `SegmentDecision` are plain POD types with `Serialize`/`Deserialize` — defined in the same file.)

#### 7.2 `ForensicBuilder` — segment push model

File: `crates/brassclaw_engine/src/executor/forensic_builder.rs` (new)

Each prompt-assembly subsystem receives a `ForensicBuilder` reference and calls `push_segment()` **immediately when its segment is complete** — it does not wait for all other segments. The builder accumulates state in a `Vec` protected by a `Mutex`. At the end of assembly, `build()` seals the packet.

```rust
pub struct ForensicBuilder {
    prompt_id: PromptId,
    run_id: RunId,
    original_query: String,
    segments: Mutex<Vec<PromptSegment>>,
    token_accounting: Option<TokenAccounting>,
    reduction_decisions: Vec<ReductionDecision>,
    skill_selection: Option<SkillSelectionOutcome>,
    // ... other fields
}

impl ForensicBuilder {
    pub fn new(prompt_id: PromptId, run_id: RunId, original_query: String) -> Self { ... }

    /// Called by each prompt segment source immediately when it finishes.
    /// Thread-safe — multiple segments may push concurrently.
    pub fn push_segment(&self, seg: PromptSegment) { ... }

    pub fn record_token_accounting(&mut self, ta: TokenAccounting) { ... }
    pub fn record_reduction(&mut self, rd: ReductionDecision) { ... }
    pub fn record_skill_selection(&mut self, ss: SkillSelectionOutcome) { ... }
    pub fn record_kv_analysis(&mut self, kv: KvCacheAnalysis) { ... }

    /// Seal the packet. Consumes the builder.
    pub fn build(self, final_messages: Vec<AssembledMessage>, model_params: ModelParams) -> PromptForensicPacket { ... }
}
```

When `SempaiSlot::Unconfigured`, `push_segment` is an `#[inline(always)]` no-op and `build` returns a minimal stub. The interceptor service checks the slot state before persisting anything.

#### 7.3 Integration points — where each subsystem pushes

Each segment source calls `builder.push_segment(...)` as soon as its segment is ready:

| Subsystem | Segment name pushed | When pushed |
|-----------|--------------------|----|
| `build_system_prompt()` | `"system_preamble"` | after preamble is assembled |
| `SkillRegistry::select_for_thread()` | `"skill:<name>"` per selected skill | after each skill is included |
| `_reduce_prompt()` (Monty bridge) | calls `record_reduction()` | after each rule fires |
| `CapabilityLeaseManager` | `"capability_inventory"` | after lease snapshot taken |
| Context compaction | `"history_compact"` | after compaction logic runs |
| Memory doc retrieval | `"memory_doc:<id>"` | after each doc is included |
| Recipe injection | `"recipe:<id>"` | after recipe is included |

#### 7.4 Tests

- Unit test: `ForensicBuilder` accumulates segments from multiple concurrent callers.
- Unit test: `EscapePath` records what was not chosen and why.
- Unit test: `TokenAccounting.model_max_len` correctly captured from `ModelParams`.
- Unit test: `KvCacheAnalysis::cache_friendly = false` when a timestamp is injected mid-system-prompt.
- Unit test: `build()` with `SempaiSlot::Unconfigured` returns stub packet (no panic).
- Property test: `PromptForensicPacket` round-trips through `serde_json`.

---

### Step 8 — Interceptor service: connection model, Sempai wait, human/iPhone review

**Goal**: Implement the full interceptor service runtime — how it waits for the Sempai, what happens when a human connects instead, and how the iPhone push-notification skill enables remote human review.

#### 8.1 Interceptor service runtime

File: `crates/brassclaw_engine/src/sempai/interceptor_service.rs` (new)

```rust
pub struct InterceptorService {
    /// The Sempai provider slot (shared with SwappableLlmProvider).
    sempai_slot: Arc<RwLock<SempaiSlot>>,
    /// The interceptor's own database handle.
    db: Arc<dyn InterceptorStore>,
    /// Channel for receiving interception requests from the orchestrator.
    request_rx: tokio::sync::mpsc::Receiver<InterceptorRequest>,
    /// Notification channel for signalling when a pending prompt is ready.
    notify: Arc<tokio::sync::Notify>,
}
```

The service loop:

```
loop {
    wait for next PromptInterceptRequest from orchestrator channel

    1. Assign PromptId (if not already set)
    2. Persist prompt: save_prompt(), save all segments that have pushed so far
    3. Set status = 'pending'
    4. Notify any connected Sempai or human reviewer
    5. Wait for audit response (indefinitely — no timeout, no fallback)
    6. Persist audit: save_audit(), queue_proposed_updates()
    7. Set status = 'audited'
    8. Persist tokenized version: save_tokenized()
    9. Set status = 'forwarded'
    10. Return PromptInterceptResponse to orchestrator
}
```

**The interceptor never times out and never falls back.** If no Sempai or human is connected, prompts queue in the database with `status = 'pending'`. The orchestrator waits. The user's turn is suspended — not failed — until a reviewer connects and processes the queue.

This is intentional. The interceptor is a quality gate, not a passthrough. Unchecked prompts do not reach Kohai.

#### 8.2 Sempai audit flow

When a prompt has `status = 'pending'` and `SempaiSlot::Active(provider)`:

1. Build the Sempai interception prompt from the forensic packet and all stored segment data, following KV-cache token-saving rules (stable reference content first, variable prompt-specific content last).
2. Call `provider.complete(sempai_messages, ...)` — single call, no retry decorator.
3. Parse the structured JSON response:

```json
{
  "audit_summary": "...",
  "revised_messages": [...],    // null if unchanged
  "was_modified": true,
  "proposed_updates": [
    {
      "artifact_kind": "skill",
      "artifact_id": "ibm_bob_people",
      "change_type": "update",
      "patch": { ... },
      "rationale": "..."
    }
  ]
}
```

4. If the JSON response is malformed, write the raw response to `interceptor_audits.audit_summary` with a parse-error prefix and treat `was_modified = false` (original messages forwarded unchanged). Malformed responses are flagged in the WebUI.

The Sempai interception prompt is built from `crates/brassclaw_engine/prompts/sempai_intercept.md` (`include_str!()`). It includes:
- The full `PromptForensicPacket` as formatted JSON.
- Per-segment decision data and escape paths.
- All model parameters (`model_max_len`, `temperature`, etc.).
- The complete recipe/skill/tool registry snapshot.
- Orchestrator, Monty, agent, and v2 design snapshots.
- KV-cache analysis with actionable guidance.
- The original user query.

#### 8.3 Human review flow

When `SempaiSlot::Unconfigured` and a human connects to the interceptor:

The interceptor exposes a review API (internal, not public WebUI routes):

```
GET  /internal/interceptor/queue              — list pending prompts
GET  /internal/interceptor/prompt/{id}        — full prompt + forensic data
POST /internal/interceptor/prompt/{id}/audit  — submit human audit result
```

A human reviewer (authenticated via the same bearer token as the WebUI) can:
- Read the pending prompt and its full forensic packet.
- Submit an audit: `{ audit_summary, revised_messages?, proposed_updates? }`.
- The interceptor service wakes up, persists the audit, and forwards the prompt.

#### 8.4 iPhone auto-connect skill

File: `skills/sempai/iphone_connect/SKILL.md` (new)

Component types: `[Sempai, Agent]`

This skill equips the Sempai and agent with the tools needed to send a push notification to the operator's iPhone when the interceptor has pending prompts awaiting human review.

The skill uses the existing HTTP capability to call a push notification endpoint (APNs-compatible or a webhook relay). It does not require a native iOS app — a webhook-to-push service (e.g., Pushover, Gotify, or a custom APNs relay) receives the HTTP call.

Tools defined in this skill bundle:

```rust
ActionDef {
    name: "interceptor_notify_reviewer",
    description: "Send a push notification to the configured reviewer device when prompts are awaiting human review in the interceptor queue.",
    effect: Effect::WriteExternal,
    requires_approval: RequiresApproval::Never,   // notification only, not a write
    parameters: json_schema!({
        "pending_count": { "type": "integer" },
        "oldest_prompt_age_secs": { "type": "integer" },
        "summary": { "type": "string" }
    }),
}
```

Configuration:
```
BRASSCLAW_REVIEWER_PUSH_URL      — webhook or APNs relay URL
BRASSCLAW_REVIEWER_PUSH_TOKEN    — authentication token for the push service
BRASSCLAW_REVIEWER_PUSH_DEVICE   — device identifier (for APNs direct or Pushover user key)
```

The interceptor service calls `interceptor_notify_reviewer` automatically when a prompt has been in `status = 'pending'` for longer than `BRASSCLAW_INTERCEPTOR_NOTIFY_AFTER_SECS` (default: 30 s). Subsequent notifications are sent at exponential backoff (max 1 per 5 minutes) until the queue is cleared.

#### 8.5 Tests

- Unit test: interceptor service queues a prompt, waits, and returns the audit result.
- Unit test: malformed Sempai JSON is stored with error prefix; original messages forwarded.
- Unit test: `interceptor_notify_reviewer` tool fires after the configured delay.
- Unit test: human review API accepts audit and wakes the waiting orchestrator.
- Integration test: end-to-end — orchestrator submits prompt → interceptor stores it → mock Sempai connects → audit returned → revised messages forwarded to Kohai.

---

### Step 9 — Two-step validation queue for Sempai-proposed updates

**Goal**: Implement the full lifecycle for Sempai-proposed changes to skills, recipes, and tools. A proposed change must pass **two independent validation gates** before the original component is replaced. Until both gates pass, the original version remains active.

#### 9.1 The two-gate model ("Exchanging Shoes")

The metaphor: a component cannot be replaced until its replacement has been fully verified. The original component keeps its shoes on until the new version has proven it can stand on its own.

```
Sempai proposes update
          │
          ▼
  ┌─────────────────────────────┐
  │ Gate 1: Automatic Validation│
  │ - Schema validation         │
  │ - Compatibility check       │
  │ - Regression test dry-run   │
  │ - Type set consistency      │
  └──────────────┬──────────────┘
                 │ pass            │ fail → status = 'auto_failed'
                 ▼                 │         (notified in WebUI)
  ┌─────────────────────────────┐
  │ Gate 2: Manual Validation   │
  │ - Human review in WebUI     │
  │ - Admin approves or rejects │
  └──────────────┬──────────────┘
                 │ approved        │ rejected → status = 'rejected'
                 ▼
  Original component overwritten by new version
  status = 'applied'
```

Only after **both gates pass** is the original overwritten. At no point between proposal and application is the original component degraded or shadowed.

#### 9.2 Automatic validation (Gate 1)

Gate 1 runs immediately when a proposed update is queued. It is a synchronous Rust function, not an LLM call:

```rust
pub fn auto_validate_update(update: &ProposedArtifactUpdate) -> AutoValidationResult {
    match update.artifact_kind {
        ArtifactKind::Skill  => validate_skill_patch(update),
        ArtifactKind::Recipe => validate_recipe_patch(update),
        ArtifactKind::Tool   => validate_tool_patch(update),
    }
}
```

Checks performed:
- **Skill patch**: YAML frontmatter parses correctly; `types` field contains only valid `ComponentType` values; `name` and `version` fields present and valid; Markdown body non-empty.
- **Tool patch**: `ActionDef` schema validates; `effect` and `requires_approval` fields present; parameter JSON Schema is valid; tool name does not collide with a built-in tool.
- **Recipe patch**: `Recipe` struct deserialises; all `step_id` values unique; `on_success`/`on_failure` references valid step IDs; no cycles in the step graph.
- **Type consistency**: if the update changes the `types` field, the new set must be a superset of the types required by any existing usage of this component in active threads.

On failure: `interceptor_proposed_updates.auto_validated = -1`, `status = 'auto_failed'`, notes written to `auto_validation_notes`.
On pass: `auto_validated = 1`, `status = 'awaiting_manual'`.

#### 9.3 Manual validation (Gate 2)

Gate 2 is a human action via the WebUI admin panel.

New WebUI routes (admin scope only):

```
GET  /api/webchat/v2/sempai/updates                       — list updates by status
GET  /api/webchat/v2/sempai/updates/{id}                  — full update detail + diff
POST /api/webchat/v2/sempai/updates/{id}/approve           — manual approve
POST /api/webchat/v2/sempai/updates/{id}/reject            — manual reject
GET  /api/webchat/v2/sempai/audits                        — list recent Sempai audits
GET  /api/webchat/v2/sempai/audits/{id}                   — full audit detail
GET  /api/webchat/v2/interceptor/queue                    — pending prompts count + ages
```

On approve:
1. `RebornServicesApi::apply_approved_update()` is called.
2. For `artifact_kind = "skill"`: applies patch to `SkillRegistry`, writes updated `SKILL.md` to user skills path. Triggers `SkillRegistry` hot-reload.
3. For `artifact_kind = "tool"`: validates patch, calls `CapabilityRegistry::register_or_update()`.
4. For `artifact_kind = "recipe"`: applies to `RecipeRegistry` (Step 11).
5. Sets `manual_validated = 1`, `status = 'applied'`, `applied_at = now`.

On reject:
- Sets `manual_validated = -1`, `status = 'rejected'`. Original component unchanged.

#### 9.4 Notification

When a proposed update reaches `status = 'awaiting_manual'`, the same `interceptor_notify_reviewer` tool (Step 8.4) fires a push notification to the reviewer's iPhone with a summary of the proposed change.

#### 9.5 Tests

- Unit test: skill patch with invalid YAML fails Gate 1; `status = 'auto_failed'`.
- Unit test: valid skill patch passes Gate 1; `status = 'awaiting_manual'`.
- Unit test: approve → `apply_approved_update()` → skill registry hot-reloaded with new version.
- Unit test: reject → original skill unchanged; `status = 'rejected'`.
- Unit test: tool patch with missing `effect` field fails Gate 1.
- Unit test: recipe patch with a cyclic step graph fails Gate 1.
- Integration test: Sempai proposes skill patch → Gate 1 passes → admin approves → `SkillRegistry` reflects new version; old version is no longer active.

---

### Step 10 — Sempai skill bundle with role-typed components

**Goal**: Create the Sempai's own skill and tool set. These components are tagged `[Sempai]` and never appear in Kohai or Agent prompts. This bundle defines how the Sempai reads forensic packets, evaluates decisions, and proposes updates.

#### 10.1 `skills/sempai/core/SKILL.md`

```yaml
---
name: sempai_core
version: "1.0.0"
types: [sempai]
description: >
  Core operational skill for the Sempai role. Defines how the Sempai reads
  PromptForensicPackets, evaluates segment decisions, analyses KV-cache
  efficiency, and produces structured audit responses.
activation:
  tags: ["sempai", "audit", "prompt-forensics"]
credentials: []
---
```

Markdown body includes:

1. **Reading a `PromptForensicPacket`**: field-by-field guide covering every field in the forensic packet, what it means, and how to interpret it.
2. **KV-cache rules**: what makes a prompt cache-friendly; which segment positions undermine cache reuse; how to reorder content to improve the estimated hit rate.
3. **Segment decision evaluation**: how to interpret `decision_path` and `escape_paths`; how to identify a segment that chose suboptimal content; conditions under which a different escape path would have produced a better result.
4. **Token accounting**: how to interpret `model_max_len`, `effective_input_budget`, `reserved_for_completion`; when to recommend reducing a segment.
5. **Producing a valid `SempaiIntercept` JSON response**: the exact schema; when to set `was_modified = true`; how to write a useful `audit_summary`.
6. **Proposing a valid `ProposedArtifactUpdate`**: how to construct a patch; what `change_type: "update"` vs `"create"` vs `"delete"` means; how to write a useful `rationale`.
7. **Escalation criteria**: when a segment decision is severe enough to warrant a skill/recipe/tool update vs. a local prompt fix.

#### 10.2 `skills/sempai/iphone_connect/SKILL.md`

Already defined in Step 8.4. Confirmed `types: [sempai, agent]`.

#### 10.3 `ComponentType` filtering in `SkillRegistry`

`SkillRegistry::select_for_thread(goal, context_role, max_tokens)` now accepts a `context_role: ContextRole` parameter:

```rust
pub enum ContextRole {
    Kohai,
    Sempai,
    Agent,
    Llm,
}
```

Skills are filtered: a skill is included only if its `types` set contains the `ContextRole`. Skills with `types: [sempai]` are excluded from all Kohai/Agent selections. Skills with the default `types: [llm, kohai, agent]` are excluded from Sempai selections.

#### 10.4 Tests

- Unit test: `sempai_core` skill excluded from Kohai prompt selection.
- Unit test: `sempai_core` skill included when selecting for Sempai interception context.
- Unit test: `ibm_bob_people` skill (`types: [agent, kohai]`) excluded from Sempai context.
- Unit test: a skill with `types: [llm, kohai, agent, sempai]` included in all contexts.

---

### Step 11 — Recipes system foundation

**Goal**: Implement the minimal recipe type, DB table, and registry needed so that Sempai-proposed recipe artifacts have a valid destination and the validation queue can process them.

#### 11.1 `Recipe` type

File: `crates/brassclaw_skills/src/recipe.rs` (new)

```rust
pub struct Recipe {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: u32,
    /// Component type tags — same four-value system as skills and tools.
    pub types: Vec<ComponentType>,
    pub steps: Vec<RecipeStep>,
    pub created_by: RecipeAuthor,  // User | Sempai | System
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub struct RecipeStep {
    pub step_id: String,
    pub action: String,              // tool name
    pub params_template: serde_json::Value,  // may contain {{variable}} interpolation
    pub on_success: Option<String>,  // next step_id or null (= done)
    pub on_failure: Option<String>,  // next step_id or "abort"
    pub approval_required: bool,
    /// Escape path: what the agent should do if this step fails and on_failure = "abort".
    pub abort_guidance: Option<String>,
}
```

#### 11.2 DB table for recipes

```sql
CREATE TABLE recipes (
    id              TEXT    PRIMARY KEY,
    name            TEXT    NOT NULL,
    description     TEXT    NOT NULL,
    version         INTEGER NOT NULL DEFAULT 1,
    types_json      TEXT    NOT NULL,    -- Vec<ComponentType> JSON
    steps_json      TEXT    NOT NULL,    -- Vec<RecipeStep> JSON (zstd compressed)
    created_by      TEXT    NOT NULL,    -- "user" | "sempai" | "system"
    created_at      TEXT    NOT NULL,
    updated_at      TEXT    NOT NULL
);

CREATE INDEX idx_recipes_name ON recipes(name);
```

No `tenant_id` — single shared store, consistent with the interceptor tables.

#### 11.3 `RecipeRegistry` trait

File: `src/db/recipe_registry.rs` (new)

```rust
#[async_trait]
pub trait RecipeRegistry: Send + Sync {
    async fn register(&self, recipe: &Recipe) -> Result<(), DbError>;
    async fn get(&self, id: &str) -> Result<Option<Recipe>, DbError>;
    async fn list(&self) -> Result<Vec<Recipe>, DbError>;
    async fn delete(&self, id: &str) -> Result<(), DbError>;
    async fn list_by_types(&self, types: &[ComponentType]) -> Result<Vec<Recipe>, DbError>;
}
```

Added as a supertrait of `Database`. Implemented for both `PostgresDb` and `LibSqlDb`.

#### 11.4 Gate 1 validation for recipe patches

The automatic validation in Step 9.2 is updated: `validate_recipe_patch()` now fully validates `RecipeStep` graphs — unique IDs, no cycles, valid `on_success`/`on_failure` references, valid `action` names against the capability registry.

#### 11.5 Tests

- Unit test: `Recipe` with `types: [agent, kohai]` serialises/deserialises correctly.
- Unit test: `RecipeRegistry::register` + `get` round-trip for both backends.
- Unit test: `list_by_types([ComponentType::Sempai])` returns only Sempai-typed recipes.
- Unit test: cyclic recipe step graph fails Gate 1 validation.

---

### Step 12 — End-to-end validation and documentation

**Goal**: Confirm the full system works together from provider selection through interceptor audit through validation queue through component application. Fix any gaps. Leave all documentation accurate.

#### 12.1 End-to-end integration test

File: `crates/brassclaw_reborn/tests/sempai_kohai_e2e.rs` (new)

Scenario:

1. Configure a mock Kohai provider (echoes `"kohai_response"`).
2. Configure a mock Sempai provider (parses forensic packet; appends `"[sempai_audited]"` to system message; proposes a skill update for `ibm_bob_people`; proposes a new recipe `bob_onboard_employee`).
3. Submit a user turn.
4. Assert: orchestrator submitted prompt to interceptor channel and suspended.
5. Assert: interceptor stored prompt with `status = 'pending'`; all segments stored in `interceptor_prompt_segments`.
6. Assert: mock Sempai connected and returned audit response.
7. Assert: `interceptor_audits` has one row with `was_modified = true` and `audited_by = "sempai"`.
8. Assert: Kohai received the **modified** prompt (with `"[sempai_audited]"` in system message).
9. Assert: `interceptor_prompt_tokenized` row stored after tokenization.
10. Assert: `interceptor_proposed_updates` has two rows (one skill update, one new recipe); both at `status = 'awaiting_manual'` after Gate 1.
11. Assert: admin approves the skill update → Gate 2 passes → `SkillRegistry` reflects new `ibm_bob_people` version.
12. Assert: admin approves the recipe → `RecipeRegistry` contains `bob_onboard_employee`.
13. Assert: original `ibm_bob_people` version is no longer active after approval.
14. Configure `BRASSCLAW_REVIEWER_PUSH_URL` mock endpoint. Assert: `interceptor_notify_reviewer` fires when `status = 'pending'` exceeds 30 s.

#### 12.2 Documentation updates

| File | Change |
|------|--------|
| `FEATURE_PARITY.md` | Add Sempai–Kohai entry with all 12 steps listed |
| `CHANGELOG.md` | Add unreleased section: Sempai–Kohai split, IBM Bob integration, component type system |
| `crates/brassclaw_llm/CLAUDE.md` | Add `ProviderRole`, `SempaiSlot`, `SwappableLlmProvider` section |
| `crates/brassclaw_engine/src/types/forensic.rs` | Doc-comments on every public field |
| `crates/brassclaw_skills/CLAUDE.md` (if exists) | Document `ComponentType`, `types` field, `RecipeRegistry` |
| `src/db/README.md` (if exists) | Document `InterceptorStore`, `RecipeRegistry` new supertraits |
| `crates/brassclaw_webui_v2/CLAUDE.md` (if exists) | Document new Sempai admin routes and interceptor queue route |
| `AGENTS.md` | Add interceptor service to "Where to Work" table |

#### 12.3 Final lint and test run

```bash
cargo fmt
cargo clippy --all --benches --tests --examples --all-features -- -D warnings
cargo test
cargo test --features integration
```

All must pass with zero new warnings.

---

## Cross-Cutting Impact Analysis

Every subsystem touched by this plan, with the step that addresses each impact:

| Subsystem | Steps | Impact |
|-----------|-------|--------|
| `crates/brassclaw_llm` | 1, 5 | `ProviderRole`, `SempaiSlot`, `SwappableLlmProvider` extension |
| `crates/brassclaw_skills` | 1, 10, 11 | `ComponentType`, `types` field, `RecipeRegistry` trait |
| `providers.json` | 3 | IBM Bob Inference entry |
| `skills/ibm_bob/` | 2 | 17 new skill bundles, one per API group |
| `skills/sempai/` | 8, 10 | `sempai_core` + `iphone_connect` bundles |
| `crates/brassclaw_webui_v2` | 4, 9 | "Use as Kohai"/"Use as Sempai" buttons + Sempai admin panel |
| `crates/brassclaw_reborn_webui_ingress` | 4, 9 | New routes registered |
| `crates/brassclaw_product_workflow` | 4 | `set_active_llm` role-aware |
| `crates/brassclaw_engine` | 6, 7, 8, 10 | `PromptForensicPacket`, `ForensicBuilder`, `InterceptorService`, `sempai_intercept.md` |
| `crates/brassclaw_reborn` | 5, 6 | `SwappableLlmProvider` + `InterceptorService` wired into `RebornRuntime` |
| `crates/brassclaw_reborn_composition` | 5 | Sempai slot injected via `LlmReloadHandle` |
| `crates/brassclaw_extensions` | 2 | 17 IBM Bob capability modules |
| `src/db` | 6, 11 | `InterceptorStore` + `RecipeRegistry` new `Database` supertraits |
| `migrations/` | 1, 6, 11 | Three new migration files |
| `crates/brassclaw_authorization` | 9 | Admin scope for Sempai review routes |

---

## Memory, OOM, and Performance Guardrails

| Concern | Guardrail | Step |
|---------|-----------|------|
| Prompt message size | Max 4 MiB uncompressed; `verbatim_content` stripped above limit | 6 |
| All JSON columns | zstd level 3 compression on write; decompress on read | 6 |
| Prompt archival | Rows older than 90 days moved to cold archive table, not deleted | 6 |
| Forensic builder overhead when no Sempai | `#[inline(always)]` no-ops; `SempaiSlot::Unconfigured` guard | 5, 7 |
| Interceptor queue growth | Prompts queue indefinitely by design; monitor via `/interceptor/queue` route | 8 |
| iPhone notification rate | Exponential backoff; max 1 notification per 5 minutes | 8 |
| Sempai call is never retried | No retry decorator on Sempai LLM call | 8 |
| Auto-validation is synchronous Rust | No LLM call in Gate 1; no OOM risk | 9 |
| Recipe step graph validation | Cycle detection runs in O(n) before acceptance | 9, 11 |

---

## Dependency Order (Critical Path)

```
Step 1  (data model: ProviderRole + ComponentType)
  ├── Step 2  (IBM Bob General skills/recipes/tools)         ← parallel with 3
  ├── Step 3  (IBM Bob Inference provider.json entry)        ← parallel with 2
  └── Step 4  (WebUI "Use as Kohai"/"Use as Sempai" buttons)
        └── Step 5  (SempaiSlot in SwappableLlmProvider)
              └── Step 6  (Interceptor standalone service + DB schema)
                    └── Step 7  (PromptForensicPacket + ForensicBuilder segment push)
                          └── Step 8  (Interceptor runtime: Sempai wait + human/iPhone)
                                └── Step 9  (Two-step validation queue + WebUI admin)
                                      └── Step 10 (Sempai skill bundle + ComponentType filtering)
                                            └── Step 11 (Recipes foundation)
                                                  └── Step 12 (E2E test + docs + final lint)
```

Steps 2 and 3 can be merged in parallel immediately after Step 1 is merged.

---

## Non-Goals (Out of Scope for This Plan)

- The Sempai does not write to the user-visible conversation. It operates entirely behind the interceptor boundary.
- The Sempai does not have access to secrets, credentials, or the host shell. It receives only the forensic packet.
- Full IBM Bob webhook subscription management (registration, delivery verification, retry) is a follow-on workstream.
- Multi-tenant separation of interceptor stores (the `tenant_id` column is deliberately absent; separation is a follow-on).
- Native iOS app for human review (the iPhone skill uses a webhook relay; a native app is a follow-on).
- Sempai self-observation and knowledge accumulation loop (removed from this plan; may be revisited as a separate workstream once the interception foundation is stable).

---

*Document version: 2.0 — complete rewrite incorporating standalone interceptor architecture, IBM Bob dual-connection design, four-type component system, two-step validation, and human/iPhone review.*
*Author: generated by BrassClaw planning agent.*
