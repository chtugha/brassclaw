# Sempai–Kohai: Dual-Role Provider Architecture & Prompt Interception System

> **Status**: Planning document — not yet implemented.  
> **Scope**: This document is the canonical implementation plan for the Sempai–Kohai split. Every step is a self-contained, shippable unit. Each step validates against the repository's zero-warning Clippy policy and full test suite before being considered done.

---

## Background and Motivation

BrassClaw currently treats all LLM providers as interchangeable inference backends selected by a single "Use" button. The Sempai–Kohai model introduces a hard role distinction and an independent interception service that sits permanently between prompt assembly and the tokenizer.

- **Kohai** (後輩 — "junior") is the primary inference provider. It receives the final assembled and tokenized prompt, executes tool calls, writes code, and produces visible output. The existing `llm.active_provider` maps directly onto the Kohai role.
- **Sempai** (先輩 — "senior") is the teaching and auditing provider. It is never visible in normal output. It operates through a **standalone interceptor service** that permanently sits between prompt assembly and the tokenizer. The interceptor operates as a binary-state switch: in **`Passthrough`** state it saves every prompt and immediately forwards it to the tokenizer → Kohai with zero Sempai involvement; in **`Intercepting`** state it saves every prompt, routes it to the Sempai for audit, and only forwards the (possibly modified) prompt after the audit completes. The interceptor switches to `Intercepting` when the Sempai successfully establishes a connection, and immediately reverts to `Passthrough` on disconnection.

The interceptor is not a function call inside the loop engine. It is a service boundary. In `Passthrough` state the only overhead is one DB write and one DB status update — the prompt is never held. In `Intercepting` state the prompt is held until the Sempai audit completes.

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
  type: basic_auth
  username_secret: hibob_service_user_id      # stored in BrassClaw secrets
  password_secret: hibob_service_user_token   # token for the service user
base_url: https://api.hibob.com/v1
headers:
  Content-Type: application/json
  Accept: application/json
# The Authorization: Basic base64(id:token) header is injected automatically
# by the HTTP capability layer using the two secrets above — it is never
# assembled as a string visible to the agent.
response_format: json
error_handling:
  401: credential_invalid
  403: permission_missing
  429: rate_limited        # Bob enforces per-endpoint rate limits
  404: not_found
  422: validation_error
```

---

### Connection Type 2 — Interference (openai_completions provider for Sempai)

The Interference connection is a **standard LLM provider** registered in `providers.json` using the `openai_completions` protocol (the wire name for `ProviderProtocol::OpenAiCompletions`). It is the connection used when IBM Bob is assigned to the **Sempai role**. It connects to the HiBob API endpoint that exposes an OpenAI-compatible chat completions interface.

No wrapper or custom Rust client is needed — the existing `openai_completions` protocol handler in `crates/brassclaw_llm` covers this exactly, via the `RigAdapter` path.

> **Protocol name note**: `ProviderProtocol::OpenAiCompletions` serialises as `"openai_completions"` (snake_case). The string `"openai_compatible"` is the _provider id_ of the generic user-configurable slot, not a protocol name. Do not conflate the two.

```json
{
  "id": "ibm_bob_inference",
  "aliases": ["hibob_inference", "bob_inference", "bob_sempai"],
  "protocol": "openai_completions",
  "display_name": "IBM Bob (Inference / Sempai)",
  "setup": {
    "kind": "open_ai_compatible",
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

A `ComponentTypeSet` is `Vec<ComponentType>` with a helper `fn contains_role(role: ContextRole) -> bool`.

Note: `ContextRole` (defined in Step 10.3) is the query-side type used when filtering. `ProviderRole` (LLM-level `Kohai`/`Sempai`) is a separate concern — do not conflate the two. `ComponentType::Kohai` maps to `ContextRole::Kohai`, etc., but the mapping is explicit: a `ProviderRole` is never passed directly to `ComponentTypeSet::contains_role`.

#### 1.3 Skill frontmatter gains `types` field

Extend `SkillManifest` in `crates/brassclaw_skills/src/types.rs` with a new optional field:

```rust
pub struct SkillManifest {
    // ... all existing fields unchanged ...

    /// Component type tags — which execution contexts this skill is available in.
    /// Must be `#[serde(default)]` so existing SKILL.md files without this
    /// field continue to parse without error. The default returns `[Llm, Kohai, Agent]`.
    #[serde(default = "ComponentTypeSet::default_types")]
    pub types: Vec<ComponentType>,
}
```

The `types` field **must carry `#[serde(default)]`** — every existing SKILL.md file in `skills/` omits this field; without `serde(default)` the parser would reject every existing skill.

YAML example:
```yaml
types: [llm, kohai, agent]   # the default when absent
```

`ComponentTypeSet::default_types()` returns `vec![ComponentType::Llm, ComponentType::Kohai, ComponentType::Agent]` — available to all contexts except Sempai. This is the conservative default that protects existing skills from accidentally appearing in Sempai audit prompts.

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

#### 1.6 `LlmConfigSnapshot` additive extension

`LlmConfigSnapshot` already exists with the following shape that **must be preserved** for back-compat:

```rust
pub struct LlmConfigSnapshot {
    pub providers: Vec<LlmProviderView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<LlmActiveSelection>,
}
```

This step extends it **additively only** — existing fields are never renamed or removed:

```rust
pub struct LlmConfigSnapshot {
    pub providers: Vec<LlmProviderView>,   // unchanged
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<LlmActiveSelection>,  // unchanged; mirrors kohai_active

    // ── New fields (all optional/skip_serializing_if for back-compat) ──────
    /// Kohai (primary inference) active selection.
    /// After this step, `active` and `kohai_active` are always kept in sync
    /// so old clients reading `active` continue to see the Kohai selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kohai_active: Option<LlmActiveSelection>,
    /// Sempai (auditor) active selection. Absent = interceptor runs in Passthrough.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sempai_active: Option<LlmActiveSelection>,
}

// LlmProviderView gains two new boolean fields (default false — skipped when absent):
pub struct LlmProviderView {
    // ... all existing fields unchanged ...
    /// True when this provider is the active Kohai selection.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_kohai: bool,
    /// True when this provider is the active Sempai selection.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_sempai: bool,
}
```

> **Important**: The existing `active` field in `LlmConfigSnapshot` is always kept in sync with `kohai_active` so that clients that only know `active` observe the Kohai selection unchanged. No existing test or client is broken.

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
  - name: hibob_service_user_token
    provider: hibob
    location:
      type: basic_auth
      username: "{{secret:hibob_service_user_id}}"
      # `username` is the service user ID; `name` above is the secret key for
      # the token (password). The HTTP capability layer injects the header:
      #   Authorization: Basic base64(username:token)
      # No credential value ever appears in recipe text or agent context.
    hosts: ["api.hibob.com"]
---
```

> **`SkillCredentialLocation::BasicAuth` shape**: `{ type: basic_auth, username: <value> }`. The `name` at the outer level is the secret key for the password/token. The `username` here is a literal string for the credential spec — in practice the service user ID is loaded from a separate secret (`hibob_service_user_id`) at the HTTP capability layer, not inlined into the SKILL.md text. If a two-secret basic-auth pattern is needed, the capability executor must be extended to support a `username_secret` field; that extension is tracked in Step 2 as a sub-task.

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
    name: "bob_search_employees".to_string(),
    description: "Search for employees in IBM Bob using filters. POST /v1/people/search.".to_string(),
    effects: vec![EffectType::ReadExternal, EffectType::CredentialedNetwork],
    requires_approval: false,
    parameters_schema: json!({
        "type": "object",
        "properties": {
            "filters": { "type": "array", "description": "Bob search filter objects" },
            "fields":  { "type": "array", "items": { "type": "string" },
                         "description": "Fields to return (dot-path format)" },
            "humanReadable": { "type": "boolean", "default": true }
        }
    }),
    model_tool_surface: ModelToolSurface::FullSchema,
    discovery: None,
}
```

Write tools (`bob_create_employee`, `bob_terminate_employee`, `bob_upload_*`, `bob_create_*`, `bob_delete_*`) use `requires_approval: true` and `effects: vec![EffectType::WriteExternal, EffectType::CredentialedNetwork]`.

Read-only tools use `requires_approval: false` and `effects: vec![EffectType::ReadExternal, EffectType::CredentialedNetwork]`.

> **`ActionDef` field names**: `effects: Vec<EffectType>` (plural, not `effect: Effect`). All IBM Bob tools must also include `EffectType::CredentialedNetwork` because every call uses the stored `hibob_service_user_token` credential.

All tools tagged with `component_types: [Agent, Kohai]` (or `[Agent, LLM]` for metadata/reporting tools) matching the skill bundle.

> **Security note**: HiBob credentials must **not** be assembled in recipe text or passed through the agent's visible context. The `Authorization` header must be constructed and injected at the HTTP capability layer (sealed before the agent sees the request), using the values from `SecretsStore` fetched by key name. The recipe documents the key names (`hibob_service_user_id`, `hibob_service_user_token`) and the header template, but the actual secret values are resolved and injected by the capability executor — never interpolated into a string the agent assembles or logs.

#### 2.5 Tests

- Unit test: each skill bundle parses through `SkillRegistry::load_skill()`.
- Unit test: all 17 skill bundles resolve correct `ComponentTypeSet`.
- Unit test: `bob_search_employees` tool definition round-trips through capability registry.
- Unit test: write tools have `requires_approval: true`.
- Unit test: metadata group tools have `ComponentType::Llm` in their type set.
- Unit test: all Bob tools have `EffectType::CredentialedNetwork` in their `effects` vec.

---

### Step 3 — IBM Bob: Interference connection (openai_completions Sempai provider)

**Goal**: Register the IBM Bob Inference provider in `providers.json` so it appears in the WebUI provider list and can be assigned to the Sempai role. This is a pure data change — no new Rust code required because `openai_completions` (`ProviderProtocol::OpenAiCompletions`) already handles this protocol via `RigAdapter`.

#### 3.1 `providers.json` entry

Add to `providers.json` (same file loaded by `ProviderRegistry` via `include_str!`):

```json
{
  "id": "ibm_bob_inference",
  "aliases": ["hibob_inference", "bob_inference", "bob_sempai"],
  "protocol": "openai_completions",
  "display_name": "IBM Bob (Inference / Sempai)",
  "setup": {
    "kind": "open_ai_compatible",
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
- Unit test: `protocol` field deserialises to `ProviderProtocol::OpenAiCompletions` (wire value `"openai_completions"`).
- Unit test: the existing `test_openai_compatible_providers_have_base_url` test still passes (the new entry has `defaults.base_url`).

---

### Step 4 — WebUI v2: "Use as Kohai" + "Use as Sempai" buttons

**Goal**: Replace the single "Use" button with two role-specific buttons. Update the API to accept a `role` field. Update the provider card component.

#### 4.1 API change: `POST /api/webchat/v2/llm/active`

The existing `SetActiveLlmRequest` struct (defined in `crates/brassclaw_product_workflow/src/reborn_services/llm_config.rs`) is extended with a `role` field:

```rust
// Before (existing):
pub struct SetActiveLlmRequest {
    pub provider_id: String,
    #[serde(default)]
    pub model: Option<String>,
}

// After (this step adds `role`):
pub struct SetActiveLlmRequest {
    pub provider_id: String,
    #[serde(default)]
    pub model: Option<String>,
    /// Which role to assign this provider to.
    /// Defaults to Kohai when absent for back-compat with existing clients.
    #[serde(default)]
    pub role: Option<ProviderRole>,
}
```

The TypeScript interface the frontend sends:
```typescript
interface SetActiveLlmRequest {
  provider_id: string;
  model?: string;
  role?: "kohai" | "sempai";  // absent = Kohai (back-compat)
}
```

`set_active_llm` implementation inside `LlmConfigService::set_active()` (in `crates/brassclaw_reborn_composition/src/llm_config_service.rs`):

```rust
async fn set_active(
    &self,
    caller: WebUiAuthenticatedCaller,
    request: SetActiveLlmRequest,
) -> Result<LlmConfigSnapshot, LlmConfigServiceError> {
    // Resolve role once — default to Kohai for back-compat.
    let role = request.role.unwrap_or(ProviderRole::Kohai);

    // Cannot assign the same provider to both roles simultaneously.
    let other_key = match role {
        ProviderRole::Kohai  => "llm.sempai_provider",
        ProviderRole::Sempai => "llm.kohai_provider",
    };
    if let Some(other_id) = self.db.get_setting(other_key).await? {
        if other_id.trim_matches('"') == request.provider_id {
            return Err(LlmConfigServiceError::Conflict {
                reason: "provider_already_assigned_to_other_role".into(),
            });
        }
    }

    let key = match role {
        ProviderRole::Kohai  => "llm.kohai_provider",
        ProviderRole::Sempai => "llm.sempai_provider",
    };
    self.db.set_setting(key, &serde_json::to_string(&request.provider_id)?).await?;

    // Also keep the legacy "llm.active_provider" key in sync with Kohai
    // so existing code that reads the old key continues to work.
    if role == ProviderRole::Kohai {
        self.db.set_setting(
            "llm.active_provider",
            &serde_json::to_string(&request.provider_id)?,
        ).await?;
    }

    // Hot-reload both providers. reload() takes the current LlmConfig and
    // SessionManager — both are already held by LlmConfigService.
    self.reload_handle.reload(&self.llm_config, Arc::clone(&self.session)).await
        .map_err(LlmConfigServiceError::Reload)?;

    // Return the updated snapshot.
    self.get_snapshot(caller).await
}
```

> **`LlmReloadHandle::reload()` signature**: takes `(&self, config: &LlmConfig, session: Arc<SessionManager>)`. It is not zero-arg. `LlmConfigService` already holds references to both. Do not call `.reload()` without arguments.

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
///
/// Does NOT derive Copy — `Active` carries an `Arc<dyn LlmProvider>`.
/// Does NOT derive PartialEq or Eq — `Arc<dyn LlmProvider>` is not PartialEq.
#[derive(Clone)]
pub enum SempaiSlot {
    /// No Sempai provider configured. The interceptor runs in Passthrough
    /// state — every prompt is saved and forwarded immediately without audit.
    Unconfigured,
    /// A Sempai provider is configured and available to accept connections.
    /// The interceptor transitions to Intercepting state when the Sempai
    /// calls establish_connection().
    Active(Arc<dyn LlmProvider>),
}
```

#### 5.2 `SwappableLlmProvider` gains Sempai field

**The real `SwappableLlmProvider` uses a single `RwLock<ProviderSnapshot>` — not separate per-field `RwLock`s.** To preserve the existing atomicity guarantee (a reader always sees a consistent slice from one snapshot), the Sempai slot is added **inside `ProviderSnapshot`**:

```rust
// Existing (do not change structure):
struct ProviderSnapshot {
    inner:                 Arc<dyn LlmProvider>,
    model_name:            &'static str,
    active_model_name:     Arc<str>,
    cost_per_token:        (Decimal, Decimal),
    cache_write_multiplier: Decimal,
    cache_read_discount:   Decimal,
    // ── New field added by this step ──────────────────────────────────────
    sempai:                SempaiSlot,
}
```

`ProviderSnapshot::capture()` is extended to accept an optional Sempai provider and store it as `SempaiSlot::Active(provider)` or `SempaiSlot::Unconfigured`.

`SwappableLlmProvider` gains a `sempai_slot()` accessor that returns `SempaiSlot` (cloned from the snapshot under read lock):

```rust
impl SwappableLlmProvider {
    pub fn sempai_slot(&self) -> SempaiSlot {
        read(&self.state).sempai.clone()
    }
}
```

`LlmReloadHandle::reload()` already takes `config: &LlmConfig` and `session: Arc<SessionManager>`. The implementation reads both `llm.kohai_provider` and `llm.sempai_provider` from settings (via `config`), creates both provider instances (or `Unconfigured`), builds a new `ProviderSnapshot` with both, and atomically swaps via `SwappableLlmProvider::swap()`. No second `RwLock` or second `swap()` call is needed.

#### 5.3 Forensic builder overhead guard

The `ForensicBuilder` (Step 6) accumulates segment data only when `SempaiSlot::Active`. When `Unconfigured`, all `record_*` calls are `#[inline(always)]` no-ops. This ensures zero runtime overhead when no Sempai is configured.

#### 5.4 Tests

- Unit test: reload with no `llm.sempai_provider` setting → `SempaiSlot::Unconfigured`.
- Unit test: reload with a valid provider id → `SempaiSlot::Active(...)`.
- Unit test: reload is atomic — concurrent reads never see partial state.

---

### Step 6 — Interceptor service: standalone architecture + DB schema

**Goal**: Implement the interceptor as a standalone async service with its own database. The interceptor sits permanently between prompt assembly and the tokenizer and operates as a binary-state switch: in `Passthrough` state every prompt is saved and forwarded immediately; in `Intercepting` state every prompt is saved and routed to the Sempai for audit before being forwarded.

#### 6.1 Architecture and state machine

The interceptor is a separate `tokio` task that communicates with the orchestrator via a bounded channel. The orchestrator submits an assembled prompt and suspends until the interceptor returns a response. The interceptor responds immediately in `Passthrough` mode, or after Sempai review in `Intercepting` mode.

```
Orchestrator assembles prompt
          │
          │ PromptInterceptRequest { prompt_id, final_messages, forensic_packet, model_params }
          ▼
  ┌─────────────────────────────────────────────────────────┐
  │  InterceptorService                                     │
  │                                                         │
  │  state: Passthrough ◄──────────────────────────────┐   │
  │    │  save prompt to DB                             │   │
  │    │  forward to tokenizer → Kohai immediately      │   │
  │    │                                     on disconnect  │
  │    │  on Sempai connection_established   │           │   │
  │    ▼                                    │           │   │
  │  state: Intercepting ───────────────────┘           │   │
  │    │  save prompt to DB                             │   │
  │    │  build KV-cache-aware Sempai audit prompt      │   │
  │    │  send to Sempai provider                       │   │
  │    │  receive audit response                        │   │
  │    │  forward revised prompt to tokenizer → Kohai   │   │
  │    └───────────────────────────────────────────────►┘   │
  │                                                         │
  │  ┌─────────────────────────────┐                        │
  │  │ interceptor_db              │                        │
  │  │  · interceptor_prompts      │                        │
  │  │  · interceptor_prompt_final │                        │
  │  │  · interceptor_prompt_segs  │                        │
  │  │  · interceptor_prompt_tok   │                        │
  │  │  · interceptor_audits       │                        │
  │  │  · interceptor_proposed_upd │                        │
  │  └─────────────────────────────┘                        │
  └─────────────────────────────────────────────────────────┘
          │
          │ PromptInterceptResponse { prompt_id, revised_messages, audit_summary, proposed_updates }
          ▼
  Orchestrator forwards to tokenizer → Kohai
```

**State transitions:**
- `Passthrough` → `Intercepting`: Sempai provider calls `establish_connection()` on the interceptor. On success a keepalive loop starts.
- `Intercepting` → `Passthrough`: keepalive heartbeat is missed (connection dead), or Sempai provider returns a connection error. Transition is immediate — the in-flight prompt (if any) completes in `Passthrough` mode.

**There is no fixed timeout for switching states.** The keepalive interval is size-adaptive: a larger forensic packet gives the Sempai more processing time before a missed beat is considered a disconnect. The keepalive period is:

```
keepalive_interval = base_interval_ms + (forensic_packet_bytes / BYTES_PER_MS_FACTOR)
```

where `base_interval_ms` defaults to `BRASSCLAW_SEMPAI_KEEPALIVE_BASE_MS` (default: 2 000 ms) and `BYTES_PER_MS_FACTOR` defaults to `BRASSCLAW_SEMPAI_BYTES_PER_MS` (default: 500 bytes/ms). Both are configurable env vars. Two consecutive missed beats triggers `Passthrough` transition.

The interceptor service is created at runtime boot and wired into `RebornRuntime`. It exposes:

```rust
#[async_trait::async_trait]
pub trait PromptInterceptorService: Send + Sync {
    /// Submit an assembled prompt. Returns immediately in Passthrough mode;
    /// suspends until Sempai audit completes in Intercepting mode.
    async fn intercept(
        &self,
        request: PromptInterceptRequest,
    ) -> Result<PromptInterceptResponse, InterceptorError>;

    /// Called by the Sempai provider to establish an interception connection.
    /// Transitions the interceptor to Intercepting state and starts the
    /// keepalive loop.
    async fn establish_connection(&self, session_id: SempaiSessionId) -> Result<(), InterceptorError>;

    /// Keepalive ping from the Sempai. Must arrive within the size-adaptive
    /// interval or the connection is considered dead.
    async fn keepalive(&self, session_id: SempaiSessionId) -> Result<(), InterceptorError>;

    /// Current state of the interceptor (for monitoring/WebUI).
    fn state(&self) -> InterceptorState;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum InterceptorState {
    Passthrough,
    Intercepting { session_id: SempaiSessionId },
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
-- All CREATE TABLE / CREATE INDEX statements use IF NOT EXISTS so migrations
-- are idempotent and safe to re-run (matches project convention).
CREATE TABLE IF NOT EXISTS interceptor_prompts (
    id              TEXT    PRIMARY KEY,   -- PromptId (UUID)
    run_id          TEXT    NOT NULL,
    thread_id       TEXT    NOT NULL,
    -- route: passthrough = saved + forwarded without Sempai review
    --        intercepted  = saved + routed to Sempai before forwarding
    route           TEXT    NOT NULL DEFAULT 'passthrough',
    -- status: saved | forwarded | auditing | audited | error
    status          TEXT    NOT NULL DEFAULT 'saved',
    assembled_at    TEXT    NOT NULL,
    forwarded_at    TEXT,                  -- set when Kohai receives the prompt
    audit_started_at TEXT,
    audit_completed_at TEXT,
    -- ModelParams stored as plain JSON TEXT (not zstd — small, no gain).
    model_params_json  TEXT NOT NULL,
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE INDEX IF NOT EXISTS idx_ip_run_id    ON interceptor_prompts(run_id);
CREATE INDEX IF NOT EXISTS idx_ip_thread_id ON interceptor_prompts(thread_id);
CREATE INDEX IF NOT EXISTS idx_ip_status    ON interceptor_prompts(status);

-- The final assembled messages for each prompt (before Sempai review).
-- messages_blob stores zstd-compressed bytes. BLOB type is correct for
-- compressed binary — TEXT cannot store arbitrary byte sequences reliably.
-- PostgreSQL equivalent: BYTEA. libSQL equivalent: BLOB.
CREATE TABLE IF NOT EXISTS interceptor_prompt_final (
    prompt_id       TEXT    PRIMARY KEY REFERENCES interceptor_prompts(id),
    messages_blob   BLOB    NOT NULL,      -- Vec<AssembledMessage> → JSON → zstd → bytes
    uncompressed_len INTEGER NOT NULL      -- for the 4 MiB size check on read
);

-- The tokenized version of each prompt (stored after tokenization, post-audit).
-- token_ids stored as plain JSON array TEXT (small, no compression benefit).
CREATE TABLE IF NOT EXISTS interceptor_prompt_tokenized (
    prompt_id       TEXT    PRIMARY KEY REFERENCES interceptor_prompts(id),
    token_ids_json  TEXT    NOT NULL,      -- JSON array of u32 token ids
    token_count     INTEGER NOT NULL,
    tokenized_at    TEXT    NOT NULL
);

-- Per-segment decision data. Each segment of the prompt creation process
-- sends its data here as it finishes, tagged with the prompt_id.
-- Large text fields (decision_path, escape_paths, verbatim_content) are
-- stored as BLOB (zstd compressed) to keep row sizes manageable.
CREATE TABLE IF NOT EXISTS interceptor_prompt_segments (
    id              TEXT    PRIMARY KEY,
    prompt_id       TEXT    NOT NULL REFERENCES interceptor_prompts(id),
    segment_name    TEXT    NOT NULL,
    source          TEXT    NOT NULL,
    token_count     INTEGER NOT NULL,
    -- Decision path and escape paths compressed: BLOB.
    decision_path_blob   BLOB NOT NULL,
    -- chosen_content and verbatim_content compressed: BLOB.
    chosen_content_blob  BLOB NOT NULL,
    verbatim_content_blob BLOB NOT NULL,
    escape_paths_blob BLOB  NOT NULL,
    segment_order   INTEGER NOT NULL,      -- position in the final prompt
    -- Set to 1 when verbatim_content_blob is stripped (replaced with [0x00] marker)
    -- because the prompt's uncompressed size exceeded 4 MiB. See §6.6.
    verbatim_stripped INTEGER NOT NULL DEFAULT 0,
    received_at     TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE INDEX IF NOT EXISTS idx_ips_prompt_id ON interceptor_prompt_segments(prompt_id);

-- Sempai audit results (one per prompt).
CREATE TABLE IF NOT EXISTS interceptor_audits (
    id                  TEXT PRIMARY KEY,
    prompt_id           TEXT NOT NULL REFERENCES interceptor_prompts(id),
    audited_by          TEXT NOT NULL,     -- "sempai" | "sempai:replay" | "human:<user_id>"
    audit_summary       TEXT NOT NULL,
    was_modified        INTEGER NOT NULL DEFAULT 0,
    revised_messages_blob BLOB,            -- NULL if unchanged; zstd compressed
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE INDEX IF NOT EXISTS idx_ia_prompt_id ON interceptor_audits(prompt_id);

-- Proposed artifact updates from the Sempai (queued, never auto-applied).
CREATE TABLE IF NOT EXISTS interceptor_proposed_updates (
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

CREATE INDEX IF NOT EXISTS idx_ipu_audit_id ON interceptor_proposed_updates(audit_id);
CREATE INDEX IF NOT EXISTS idx_ipu_status   ON interceptor_proposed_updates(status);
```

PostgreSQL variant uses `UUID`, `TIMESTAMPTZ`, `JSONB` instead of `TEXT` where appropriate.

#### 6.5 `InterceptorStore` trait

File: `src/db/interceptor_store.rs` (new)

```rust
#[async_trait::async_trait]
pub trait InterceptorStore: Send + Sync {
    async fn save_prompt(&self, req: &PromptInterceptRequest) -> Result<(), DbError>;
    async fn save_segment(&self, prompt_id: &PromptId, seg: &SegmentRecord) -> Result<(), DbError>;
    async fn save_tokenized(&self, prompt_id: &PromptId, token_ids: &[u32]) -> Result<(), DbError>;
    async fn save_audit(&self, audit: &AuditRecord) -> Result<(), DbError>;
    async fn queue_proposed_update(&self, audit_id: &str, update: &ProposedArtifactUpdate) -> Result<(), DbError>;
    async fn set_prompt_status(&self, prompt_id: &PromptId, status: PromptStatus) -> Result<(), DbError>;
    async fn get_prompt(&self, prompt_id: &PromptId) -> Result<Option<StoredPrompt>, DbError>;
    /// List prompts awaiting manual human review (route = 'passthrough', no audit row).
    /// Replaces the old `list_pending_prompts` — there are no "pending" prompts in the
    /// Passthrough/Intercepting model; prompts in Passthrough are forwarded immediately.
    async fn list_unaudited_prompts(&self, since: Option<chrono::DateTime<chrono::Utc>>) -> Result<Vec<StoredPrompt>, DbError>;
    async fn list_pending_updates(&self) -> Result<Vec<PendingArtifactUpdate>, DbError>;
    async fn set_auto_validation(&self, update_id: &str, result: ValidationResult, notes: &str) -> Result<(), DbError>;
    async fn set_manual_validation(&self, update_id: &str, result: ValidationResult, by: &str, notes: &str) -> Result<(), DbError>;
    /// List prompts that were routed through Passthrough (never Sempai-audited).
    /// Used for optional backlog replay on Sempai reconnection.
    async fn list_passthrough_prompts(&self, since: Option<chrono::DateTime<chrono::Utc>>) -> Result<Vec<StoredPrompt>, DbError>;
}
```

Added as a supertrait of `Database`. Implemented for both `PostgresDb` and `LibSqlDb`.

#### 6.6 Size and compression

- `messages_blob`, `decision_path_blob`, `chosen_content_blob`, `verbatim_content_blob`, `escape_paths_blob`, `revised_messages_blob`: stored as `BLOB`/`BYTEA` containing zstd-compressed bytes (level 3). Decompressed in the `InterceptorStore` implementation before returning to callers.
- `model_params_json`, `token_ids_json`: stored as plain JSON `TEXT` — these fields are small and frequent to query; compression overhead is not justified.
- The 4 MiB threshold is checked on the **uncompressed** payload size. The `interceptor_prompt_final.uncompressed_len` column records this so the implementation can skip decompression when checking the limit.
- If a prompt's uncompressed messages exceed 4 MiB, `verbatim_content_blob` fields in all its segments are replaced with a single-byte `[0x00]` marker blob and a `verbatim_stripped = 1` flag is added to the segment row. `chosen_content_blob` is always retained.
- `interceptor_prompts` rows older than 90 days are archived to `interceptor_prompts_archive` on a nightly background job, not deleted. Archive table has identical schema.
- PostgreSQL uses `BYTEA` for all BLOB columns; `JSONB` for JSON columns that are queried by field.

#### 6.7 Tests

- Unit test: `save_prompt` + `save_segment` + `save_tokenized` + `save_audit` lifecycle for both backends.
- Unit test: zstd compress/decompress round-trip for all BLOB columns.
- Unit test: uncompressed payload exceeding 4 MiB strips `verbatim_content_blob` fields and sets `verbatim_stripped`.
- Unit test: `list_passthrough_prompts` returns only `route = 'passthrough'` rows.
- Unit test: `Passthrough` prompt has `route = 'passthrough'`; `Intercepting` prompt has `route = 'intercepted'`.
- Unit test: `uncompressed_len` stored correctly for a known payload.

---

### Step 7 — `PromptForensicPacket`: type definition + segment push model

**Goal**: Define the forensic data type and instrument every prompt-assembly subsystem to push its segment data to the interceptor **as it finishes**, tagged with the unique `PromptId`. Segments do not wait for all other segments — each pushes immediately when complete.

#### 7.1 `PromptForensicPacket` type

File: `crates/brassclaw_engine/src/types/forensic.rs` (new)

```rust
/// Full forensic description of a single assembled prompt.
/// This is the payload sent to the interceptor and from there to the Sempai.
/// NOTE: `model_params` is NOT duplicated here — it lives on `PromptInterceptRequest`
/// which wraps this packet. The Sempai prompt constructor reads model_params from
/// the request, not from this packet, to avoid having two sources of truth.
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
    /// model_params is intentionally absent here — it is on PromptInterceptRequest.
    /// TokenAccounting already contains model_id, model_max_len, and budget values
    /// derived from model params; those are the forensic-relevant fields.
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

Each prompt-assembly subsystem receives a `ForensicBuilder` reference and calls `push_segment()` **immediately when its segment is complete** — it does not wait for all other segments. The builder accumulates all state inside a single `Mutex<ForensicBuilderInner>` so that **all public methods take `&self`** (shared ref). This is required because `push_segment` is called from concurrent contexts and must use `&self`, and all other `record_*` methods must be consistent with it.

```rust
struct ForensicBuilderInner {
    segments: Vec<PromptSegment>,
    token_accounting: Option<TokenAccounting>,
    reduction_decisions: Vec<ReductionDecision>,
    skill_selection: Option<SkillSelectionOutcome>,
    kv_analysis: Option<KvCacheAnalysis>,
    // ... other fields
}

pub struct ForensicBuilder {
    prompt_id: PromptId,
    run_id: RunId,
    original_query: String,
    // Single Mutex covering all mutable state so &self works on all methods.
    inner: Mutex<ForensicBuilderInner>,
}

impl ForensicBuilder {
    pub fn new(prompt_id: PromptId, run_id: RunId, original_query: String) -> Self { ... }

    /// Called by each prompt segment source immediately when it finishes.
    /// Thread-safe — multiple segments may push concurrently.
    /// Takes &self (not &mut self) — all state lives behind the inner Mutex.
    pub fn push_segment(&self, seg: PromptSegment) {
        self.inner.lock().unwrap_or_else(|p| p.into_inner()).segments.push(seg);
    }

    /// All record_* methods also take &self for the same reason.
    pub fn record_token_accounting(&self, ta: TokenAccounting) { ... }
    pub fn record_reduction(&self, rd: ReductionDecision) { ... }
    pub fn record_skill_selection(&self, ss: SkillSelectionOutcome) { ... }
    pub fn record_kv_analysis(&self, kv: KvCacheAnalysis) { ... }

    /// Seal the packet. Consumes the builder.
    pub fn build(self, final_messages: Vec<AssembledMessage>, model_params: ModelParams) -> PromptForensicPacket { ... }
}
```

> **Why all `&self`**: `push_segment` is called from multiple concurrent callers on a shared `Arc<ForensicBuilder>`, so it cannot be `&mut self`. All other `record_*` methods must also be `&self` (not `&mut self`) for the API to be consistent and safe when the builder is behind an `Arc`. The single `Mutex<ForensicBuilderInner>` is the correct abstraction.

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

### Step 8 — Interceptor service: connection model, keepalive, state transitions, human/iPhone review

**Goal**: Implement the full interceptor service runtime — the binary `Passthrough`/`Intercepting` state machine, the size-adaptive keepalive mechanism, the Sempai audit flow in `Intercepting` state, the backlog replay setting, the human review API, and the iPhone push notification skill.

#### 8.1 Service struct and state machine

File: `crates/brassclaw_engine/src/sempai/interceptor_service.rs` (new)

```rust
pub struct InterceptorService {
    state: Arc<RwLock<InterceptorState>>,
    db: Arc<dyn InterceptorStore>,
    config: InterceptorConfig,
}

pub struct InterceptorConfig {
    /// Base keepalive interval in milliseconds.
    /// Env: BRASSCLAW_SEMPAI_KEEPALIVE_BASE_MS (default: 2_000)
    pub keepalive_base_ms: u64,
    /// Additional ms per byte of forensic packet size.
    /// Env: BRASSCLAW_SEMPAI_BYTES_PER_MS (default: 500, i.e. 500 bytes = +1 ms)
    pub bytes_per_ms_factor: u64,
    /// How many consecutive missed beats before declaring disconnect.
    /// Default: 2
    pub missed_beats_threshold: u32,
    /// Replay passthrough-routed prompts to Sempai on reconnection.
    /// Env: BRASSCLAW_SEMPAI_REPLAY_BACKLOG (default: false)
    pub replay_backlog_on_connect: bool,
}
```

**Passthrough path** (state = `Passthrough`):

```
receive PromptInterceptRequest
  1. save_prompt(route = 'passthrough', status = 'saved')
  2. save all segments
  3. set status = 'forwarded', forwarded_at = now
  4. return PromptInterceptResponse { revised_messages = original, was_modified = false, audit_summary = "" }
```

No audit happens. The prompt reaches the tokenizer → Kohai immediately. Total interceptor overhead in `Passthrough` state is one DB write + one DB update.

**Intercepting path** (state = `Intercepting`):

```
receive PromptInterceptRequest
  1. save_prompt(route = 'intercepted', status = 'saved')
  2. save all segments
  3. set status = 'auditing', audit_started_at = now
  4. build Sempai audit prompt (KV-cache-aware — see 8.2)
  5. call Sempai provider (single call, no retry)
  6. parse structured JSON response
  7. save_audit(audited_by = 'sempai', ...)
  8. queue_proposed_updates(...)
  9. set status = 'audited', audit_completed_at = now
  10. set status = 'forwarded', forwarded_at = now
  11. return PromptInterceptResponse { revised_messages, was_modified, audit_summary, proposed_updates }
```

#### 8.2 Size-adaptive keepalive mechanism

The Sempai sends `keepalive(session_id)` pings to the interceptor. The required interval is calculated per-prompt based on the forensic packet's compressed byte size:

```rust
fn keepalive_deadline(packet_bytes: usize, config: &InterceptorConfig) -> Duration {
    let extra_ms = (packet_bytes as u64) / config.bytes_per_ms_factor;
    Duration::from_millis(config.keepalive_base_ms + extra_ms)
}
```

The interceptor tracks the last received ping timestamp per session. A background watcher task checks every `keepalive_base_ms / 2` milliseconds. If `now - last_ping > keepalive_deadline(current_packet_bytes)` for two consecutive checks, the connection is declared dead:

1. State transitions immediately to `Passthrough`.
2. The in-flight prompt (if in `auditing` status) is re-routed: `route` updated to `'passthrough'`, `status` updated to `'forwarded'`, original messages returned to the orchestrator.
3. `interceptor_notify_reviewer` fires (see 8.4) to alert the human that the Sempai disconnected.

On explicit disconnect (Sempai calls `establish_connection` with a new `session_id`, or the LLM provider returns a terminal error), the same path fires immediately without waiting for the keepalive watcher.

#### 8.3 Sempai audit prompt construction

The Sempai interception prompt is assembled following KV-cache token-saving rules: **stable content first, variable content last**.

Stable prefix (shared across many prompts — high cache hit rate):
1. `sempai_core` skill text (from `skills/sempai/core/SKILL.md`)
2. Full recipe/skill/tool registry snapshot (changes infrequently)
3. Orchestrator, Monty, agent, v2 design snapshots

Variable suffix (unique per prompt — always a cache miss, kept at the end):
4. `PromptForensicPacket` header (prompt_id, run_id, assembled_at)
5. Per-segment decision data and escape paths
6. Token accounting and KV-cache analysis for this prompt
7. All model parameters for this prompt
8. The original user query
9. The final assembled messages

The prompt is built from `crates/brassclaw_engine/prompts/sempai_intercept.md` (`include_str!()`). The Sempai must respond with this JSON envelope:

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

If the JSON response is malformed, the raw response is stored in `interceptor_audits.audit_summary` with a `[PARSE_ERROR]` prefix and `was_modified = false`. The original messages are forwarded unchanged. Malformed responses are visible in the WebUI audit list.

#### 8.4 Backlog replay on reconnection

When the Sempai reconnects (`establish_connection` succeeds) and `replay_backlog_on_connect = true`:

1. `list_passthrough_prompts(since = last_disconnect_at)` is called.
2. Each passthrough prompt is replayed through the Sempai audit flow in chronological order before live prompts are processed.
3. Replay runs in a background task — it does not block the live interception channel.
4. Replay audits are stored with `audited_by = 'sempai:replay'`.

Default (`replay_backlog_on_connect = false`): on reconnection only new prompts are intercepted. Passthrough prompts remain in the DB with `route = 'passthrough'` and are available for manual human review.

#### 8.5 Human review API

The interceptor exposes a review API (internal routes, same bearer auth as WebUI):

```
GET  /internal/interceptor/state                  — current state (Passthrough/Intercepting)
GET  /internal/interceptor/prompts                — list all prompts with route + status
GET  /internal/interceptor/prompts/{id}           — full prompt + forensic data
POST /internal/interceptor/prompts/{id}/audit     — submit human audit result
```

A human reviewer can read any stored prompt (passthrough or intercepted) and submit:
```json
{ "audit_summary": "...", "revised_messages": [...], "proposed_updates": [...] }
```

Human-submitted audits are stored with `audited_by = 'human:<user_id>'`. They do not unblock any waiting orchestrator (the prompt was already forwarded in passthrough mode) — they are retrospective annotations that feed the validation queue.

#### 8.6 iPhone push notification skill

File: `skills/sempai/iphone_connect/SKILL.md` (new)

Component types: `[Sempai, Agent]`

Uses the existing HTTP capability to POST to a configured webhook relay (Pushover, Gotify, or custom APNs relay). No native iOS app required.

```rust
ActionDef {
    name: "interceptor_notify_reviewer".to_string(),
    description: "Send a push notification to the reviewer device. Fires when the Sempai disconnects or when passthrough-routed prompts accumulate.".to_string(),
    effects: vec![EffectType::WriteExternal, EffectType::CredentialedNetwork],
    requires_approval: false,   // notification-only; no approval gate needed
    parameters_schema: json!({
        "type": "object",
        "properties": {
            "event":        { "type": "string",  "enum": ["sempai_disconnected", "passthrough_accumulating", "sempai_reconnected", "backlog_replay_complete"] },
            "prompt_count": { "type": "integer" },
            "summary":      { "type": "string"  }
        },
        "required": ["event", "summary"]
    }),
    model_tool_surface: ModelToolSurface::FullSchema,
    discovery: None,
}
```

The interceptor calls `interceptor_notify_reviewer` on:
- `sempai_disconnected` — immediately when keepalive watcher fires the `Passthrough` transition.
- `passthrough_accumulating` — when `N` prompts have accumulated in passthrough route since last Sempai connection (`N` = `BRASSCLAW_SEMPAI_NOTIFY_PASSTHROUGH_COUNT`, default: 10), with exponential backoff capped at 1 notification per 5 minutes.
- `sempai_reconnected` — when `establish_connection` succeeds.
- `backlog_replay_complete` — when replay finishes (if `replay_backlog_on_connect = true`).

Configuration:
```
BRASSCLAW_REVIEWER_PUSH_URL            — webhook or APNs relay URL
BRASSCLAW_REVIEWER_PUSH_TOKEN          — auth token for the push service
BRASSCLAW_REVIEWER_PUSH_DEVICE         — device/user key
BRASSCLAW_SEMPAI_NOTIFY_PASSTHROUGH_COUNT — prompts before accumulation alert (default: 10)
```

#### 8.7 Tests

- Unit test: `Passthrough` path stores prompt with `route = 'passthrough'` and returns original messages immediately.
- Unit test: `Intercepting` path routes through Sempai and returns audit result.
- Unit test: keepalive deadline scales correctly with packet byte size.
- Unit test: two missed keepalive beats → state transitions to `Passthrough`; in-flight prompt re-routed.
- Unit test: explicit disconnect (terminal provider error) → immediate `Passthrough` transition.
- Unit test: malformed Sempai JSON → `[PARSE_ERROR]` stored; original messages returned.
- Unit test: `replay_backlog_on_connect = true` → `list_passthrough_prompts` called on reconnect; replay runs in background.
- Unit test: `replay_backlog_on_connect = false` → no replay on reconnect; passthrough prompts stay unaudited.
- Unit test: `interceptor_notify_reviewer` fires on `sempai_disconnected` event immediately.
- Unit test: human review API stores retrospective audit with `audited_by = 'human:<id>'`.
- Integration test: `Passthrough` → Sempai connects → `Intercepting` → Sempai disconnects → `Passthrough`; all state recorded correctly in DB.

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
- **Tool patch**: `ActionDef` schema validates; `effects` (plural, `Vec<EffectType>`) and `requires_approval` fields present and valid; parameter JSON Schema is valid (must be a JSON Schema object); tool name does not collide with a built-in tool.
- **Recipe patch**: `Recipe` struct deserialises; all `step_id` values unique; `on_success`/`on_failure` references valid step IDs; no cycles in the step graph.
- **Type consistency**: if the update changes the `types` field, the new set must be a superset of the types required by any existing usage of this component in active threads.

On failure: `interceptor_proposed_updates.auto_validated = -1`, `status = 'auto_failed'`, notes written to `auto_validation_notes`.
On pass: `auto_validated = 1`, `status = 'awaiting_manual'`.

#### 9.3 Manual validation (Gate 2)

Gate 2 is a human action via the WebUI admin panel.

> **Admin scope implementation**: There is no `WebUiCallerScope::Admin` enum in this codebase. Admin-only route gating must be implemented by checking the authenticated caller's user record against a DB-stored role or permission flag. Concretely: define a guard function `require_admin(caller: &WebUiAuthenticatedCaller, db: &dyn Database) -> Result<(), RebornServicesError>` that looks up the user in the DB and verifies an admin flag before the handler proceeds. The specific DB column/table for user roles must be confirmed against `src/db/` before implementing (tracked as a sub-task in Step 9).

New WebUI routes (admin-gated via `require_admin` guard):

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

When a proposed update reaches `status = 'awaiting_manual'`, the same `interceptor_notify_reviewer` tool (Step 8.6) fires a push notification to the reviewer's iPhone with a summary of the proposed change.

#### 9.5 Tests

- Unit test: skill patch with invalid YAML fails Gate 1; `status = 'auto_failed'`.
- Unit test: valid skill patch passes Gate 1; `status = 'awaiting_manual'`.
- Unit test: approve → `apply_approved_update()` → skill registry hot-reloaded with new version.
- Unit test: reject → original skill unchanged; `status = 'rejected'`.
- Unit test: tool patch with missing `effects` field (or with an empty `effects` vec when `CredentialedNetwork` is required) fails Gate 1.
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

Already defined in Step 8.6. Confirmed `types: [sempai, agent]`.

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
CREATE TABLE IF NOT EXISTS recipes (
    id                   TEXT    PRIMARY KEY,
    name                 TEXT    NOT NULL,
    description          TEXT    NOT NULL,
    version              INTEGER NOT NULL DEFAULT 1,
    types_json           TEXT    NOT NULL,    -- Vec<ComponentType> JSON (plain JSON, not compressed)
    steps_json           BLOB    NOT NULL,    -- Vec<RecipeStep> zstd-compressed JSON
    -- Uncompressed byte length of steps_json before compression.
    -- Used to check the 4 MiB guard without decompressing.
    steps_uncompressed_len INTEGER NOT NULL DEFAULT 0,
    created_by           TEXT    NOT NULL,    -- "user" | "sempai" | "system"
    created_at           TEXT    NOT NULL,
    updated_at           TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_recipes_name ON recipes(name);
```

No `tenant_id` — single shared store, consistent with the interceptor tables.

#### 11.3 `RecipeRegistry` trait

File: `src/db/recipe_registry.rs` (new)

```rust
#[async_trait::async_trait]
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

Setup: configure a mock Kohai provider (echoes `"kohai_response"`) and a mock Sempai provider (parses forensic packet; appends `"[sempai_audited]"` to system message; proposes a skill update for `ibm_bob_people` and a new recipe `bob_onboard_employee`).

**Sub-scenario A — Passthrough (Sempai not connected):**

1. Submit a user turn. Assert: interceptor is in `Passthrough` state.
2. Assert: prompt stored with `route = 'passthrough'`, `status = 'forwarded'`.
3. Assert: Kohai received the original unmodified messages.
4. Assert: no `interceptor_audits` row for this prompt.

**Sub-scenario B — Sempai connects and intercepts:**

5. Call `establish_connection(session_id)` on interceptor. Assert: state = `Intercepting`.
6. Submit a second user turn.
7. Assert: prompt stored with `route = 'intercepted'`, `status = 'auditing'` then `'audited'`.
8. Assert: mock Sempai received the KV-cache-structured audit prompt (stable prefix first, variable suffix last).
9. Assert: `interceptor_audits` row with `was_modified = true`, `audited_by = "sempai"`.
10. Assert: Kohai received the **modified** messages (with `"[sempai_audited]"` in system segment).
11. Assert: `interceptor_prompt_tokenized` row stored post-audit.
12. Assert: `interceptor_proposed_updates` has two rows; both `status = 'awaiting_manual'` after Gate 1.

**Sub-scenario C — Sempai disconnects (missed keepalive):**

13. Stop sending `keepalive()` pings. Wait two keepalive intervals.
14. Assert: state = `Passthrough`. Assert: `interceptor_notify_reviewer` fired with `event = "sempai_disconnected"`.
15. Submit a third turn. Assert: `route = 'passthrough'`; original messages forwarded immediately.

**Sub-scenario D — Validation queue (Gate 1 + Gate 2):**

16. Assert: admin approves the skill update → `SkillRegistry` reflects new `ibm_bob_people` version; original no longer active.
17. Assert: admin approves the recipe → `RecipeRegistry` contains `bob_onboard_employee`.

**Sub-scenario E — Backlog replay on reconnect:**

18. Set `replay_backlog_on_connect = true`. Call `establish_connection` again.
19. Assert: passthrough prompts from sub-scenarios A and C replayed in background; `interceptor_audits` gains rows with `audited_by = "sempai:replay"`.
20. Assert: `interceptor_notify_reviewer` fired with `event = "backlog_replay_complete"`.

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
| Passthrough overhead | One DB write + one DB update per prompt; no Sempai call | 8 |
| Keepalive watcher | Checks every keepalive_base_ms/2; size-adaptive deadline per packet | 8 |
| In-flight prompt on disconnect | Re-routed to passthrough immediately; original messages returned | 8 |
| Sempai call is never retried | No retry decorator on Sempai LLM call; malformed → original forwarded | 8 |
| iPhone notification rate | Exponential backoff; max 1 per 5 min for accumulation alerts | 8 |
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

*Document version: 2.3 — comprehensive correctness review: fixed `ProviderProtocol` wire name (`openai_completions`, not `openai_compatible`); fixed `SetActiveLlmRequest` to be additive extension with `Option<ProviderRole>`; fixed `LlmConfigSnapshot` extension to be purely additive (preserve existing `providers`/`active` fields); fixed `ActionDef` field names (`effects: Vec<EffectType>`, not `effect: Effect`); fixed `RequiresApproval::Required` → `true`; fixed `SwappableLlmProvider` extension to use single `ProviderSnapshot` lock; fixed `LlmReloadHandle::reload()` to show correct two-arg signature; fixed `InterceptorState` derive (no `Copy`, no bare `PartialEq` on struct variant); added `SempaiSessionId` newtype definition; fixed `SkillManifest.types` to use `#[serde(default)]` with correct file location; added `verbatim_stripped` column to `interceptor_prompt_segments`; added `steps_uncompressed_len` column to `recipes`; fixed `ForensicBuilder` to use single inner `Mutex` with all `&self` methods; fixed `EffectType::CredentialedNetwork` added to all Bob tools; fixed recipe credential YAML to use `SkillCredentialLocation::BasicAuth` shape; added admin-scope guard implementation note; fixed IBM Bob provider `setup.kind` to `open_ai_compatible`.*
*Author: generated by BrassClaw planning agent.*
