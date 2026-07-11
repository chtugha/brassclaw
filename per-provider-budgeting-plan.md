# Per-Provider Token Budgeting — Implementation Plan

> **Ground goal alignment:** The entire feature exists to let a 7B-class local model run
> as a capable agent by decomposing large problems into tiny, focused subagent turns
> instead of overwhelming a small context window with one giant prompt.  Every decision
> below is made with that in mind: minimal structs, zero hot-path lookups, O(1)
> budget resolution.

---

## Verification Findings (resolved before any code was written)

### V1 — The real struct is `ProviderDefinition`, not `ProviderEntry`
**File:** `crates/brassclaw_llm/src/registry.rs:329`
```rust
#[derive(Debug, Clone, Deserialize, Serialize)]  // both derives already present
pub struct ProviderDefinition {
    pub id: String,
    #[serde(default)] pub aliases: Vec<String>,
    pub protocol: ProviderProtocol,
    #[serde(default)] pub default_base_url: Option<String>,
    #[serde(default)] pub base_url_env: Option<String>,
    #[serde(default)] pub base_url_required: bool,
    #[serde(default)] pub api_key_env: Option<String>,
    #[serde(default)] pub api_key_required: bool,
    pub model_env: String,
    pub default_model: String,
    pub description: String,
    #[serde(default)] pub extra_headers_env: Option<String>,
    #[serde(default)] pub setup: Option<SetupHint>,
    #[serde(default, deserialize_with = "unsupported_params_de::deserialize")]
    pub unsupported_params: Vec<String>,
}
```
Loaded from compiled-in `providers.json` + optional overlay at
`$BRASSCLAW_REBORN_HOME/providers.json`. The API snapshot type that the frontend receives
is `LlmProviderView` in `crates/brassclaw_product_workflow/src/reborn_services/llm_config.rs:182`.

### V2 — `LoopExecutionState` has no active_provider_id
`LoopExecutionState` (`crates/brassclaw_agent_loop/src/state.rs:47`) carries no provider
field whatsoever.  The active provider lives in a `SwappableLlmProvider` held at the
`model_gateway` level, completely outside the loop state.

**Correct injection point:** `DefaultContextStrategy` is instantiated **once at startup**
by `families::default_with_full_config()` in
`crates/brassclaw_agent_loop/src/families/mod.rs`.  Because the active provider is known
at startup (or at the moment of hot-swap), the resolved token budget for that provider
can be baked directly into the `DefaultContextStrategy` struct's existing
`max_context_tokens` field.  No runtime provider lookup inside the loop is needed or
desirable.  When the provider changes (hot-swap), the loop family registry is rebuilt —
the same rebuild path is already taken today.

### V3 — Serde annotation pattern
| Layer | Format | Derive pattern |
|---|---|---|
| DB settings (`settings` table) | **JSON** via `serde_json` | `#[derive(Debug, Clone, Serialize, Deserialize)]` + `#[serde(default, skip_serializing_if = "Option::is_none")]` per optional field |
| Config file (`config.toml`) | **TOML** via `toml` | `#[derive(Debug, Clone, Default, Deserialize)]` + `#[serde(deny_unknown_fields)]` on the struct |
| `providers.json` (catalog) | **JSON** via `serde_json` | `#[derive(Debug, Clone, Deserialize, Serialize)]` + `#[serde(default)]` on optional fields, no `deny_unknown_fields` |

**Rule:** new fields on `ProviderDefinition` use `#[serde(default, skip_serializing_if = "Option::is_none")]`.
New fields on `LlmProviderView` use `#[serde(default, skip_serializing_if = "Option::is_none")]`.
New fields on any config-file struct use `#[serde(default)]` on the containing struct and
on the field itself, and the parent struct keeps `#[serde(deny_unknown_fields)]`.

### V4 — DB key length and provider ID constraints
`settings.key` is `TEXT NOT NULL` — SQLite TEXT has no byte-length limit.
`validate_provider_id()` (`crates/brassclaw_reborn_composition/src/llm_config_service.rs:956`)
currently accepts `[a-z0-9_-]+` of any length.  A key of the form
`provider_tokens:<provider_id>` is therefore at most `len("provider_tokens:") + len(id)`
= 16 + N bytes.  The plan adds a ≤ 64-character constraint to `validate_provider_id` so
DB keys stay bounded at 80 bytes — well under any practical limit.  No schema change is
needed.

---

## Architecture Summary

```
┌─ startup ────────────────────────────────────────────────────────────────┐
│                                                                          │
│  1. LlmConfigService reads providers.json (ProviderDefinition)           │
│  2. For the active provider, resolve_with_profile(token_budget)          │
│     → ResolvedTokenBudgets                                               │
│  3. families::default_with_full_config(conversation_context_tokens, …)   │
│     builds DefaultContextStrategy with the resolved token ceiling        │
│  4. LoopFamilyRegistry is Arc-shared across all turns                    │
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘

Provider config saved via UI
  → PUT /api/webchat/v2/providers/{id}/tokens
    → DbTokenSettingsStore.update_provider_token_settings(user_id, provider_id, …)
      → key "provider_tokens:<provider_id>"  in settings table

Hot-swap (provider changed)
  → LlmReloadTrigger.reload()
    → rebuild LoopFamilyRegistry with new provider's ResolvedTokenBudgets
    → SwappableLlmProvider.swap(inner)
```

No new fields on `LoopExecutionState`. No per-iteration DB reads. O(1) budget resolution
via a single `Option<usize>` baked into `DefaultContextStrategy.max_context_tokens`.

---

## Phase 1 — Data Model

### 1.1  Add `token_budget` to `ProviderDefinition`

**File:** `crates/brassclaw_llm/src/registry.rs`

Add one field at the end of `ProviderDefinition` (after `unsupported_params`):

```rust
/// Optional per-provider token budget.  When present, the runtime
/// resolves this against the active preset and uses it instead of the
/// global `[tokens]` section or compiled defaults.
/// `skip_serializing_if` keeps the field absent from providers.json
/// entries that don't set it, so builtins stay compact.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub token_budget: Option<ProviderTokenBudget>,
```

Add the companion struct immediately above `ProviderDefinition` in the same file:

```rust
/// Minimal token-budget overlay stored inside a `ProviderDefinition`.
///
/// Mirrors the subset of `brassclaw_reborn_config::TokensSection` that the
/// loop actually consumes. Keeping this struct in `brassclaw_llm` avoids
/// a circular dependency: `brassclaw_reborn_config` must not depend on
/// `brassclaw_llm`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ProviderTokenBudget {
    /// Named preset: `"small_7b"`, `"large"`, `"coding"`, `"chat"`.
    /// When set, all `None` fields below are filled with preset values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_history: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inline_control: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safety: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_surface: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_input: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output: Option<usize>,
}
```

Export `ProviderTokenBudget` from `crates/brassclaw_llm/src/lib.rs`:
```rust
pub use registry::{ProviderDefinition, ProviderRegistry, ProviderTokenBudget, /* existing exports */};
```

### 1.2  Add `provider_id_length_limit` to `validate_provider_id`

**File:** `crates/brassclaw_reborn_composition/src/llm_config_service.rs:956`

Add after the existing emptiness check and before the character-class check:

```rust
const PROVIDER_ID_MAX_LEN: usize = 64;
if trimmed.len() > PROVIDER_ID_MAX_LEN {
    return Err(LlmConfigServiceError::InvalidRequest {
        field: Some("id".to_string()),
        reason: format!(
            "provider id must be ≤ {} characters, got {}",
            PROVIDER_ID_MAX_LEN,
            trimmed.len()
        ),
    });
}
```

This ensures DB keys of the form `provider_tokens:<id>` are at most 80 bytes.

### 1.3  Add `token_budget` to `LlmProviderView` (the API snapshot)

**File:** `crates/brassclaw_product_workflow/src/reborn_services/llm_config.rs:182`

Add to `LlmProviderView`:
```rust
/// Token budget stored for this provider, if any.
/// `None` means the global `[tokens]` section (or compiled defaults) apply.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub token_budget: Option<ProviderTokenBudgetView>,
```

Add immediately above `LlmProviderView`:
```rust
/// Wire-format token budget view sent to the settings UI.
/// Identical shape to `TokenSettingsResponse` so the frontend can reuse
/// the same `TokensTab` component verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderTokenBudgetView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    pub conversation_history: Option<usize>,
    pub skills: Option<usize>,
    pub identity: Option<usize>,
    pub inline_control: Option<usize>,
    pub memory: Option<usize>,
    pub safety: Option<usize>,
    pub capability_surface: Option<usize>,
    pub total_input: Option<usize>,
    pub max_output: Option<usize>,
}
```

Add `ProviderTokenBudgetView` to the re-exports in
`crates/brassclaw_product_workflow/src/lib.rs`.

---

## Phase 2 — Provider Catalog Persistence

### 2.1  Persist token_budget in providers.json via `RebornProviderAdmin`

**File:** `crates/brassclaw_reborn_composition/src/provider_admin.rs`

`RebornProviderAdmin` reads and writes `ProviderDefinition` entries to
`$BRASSCLAW_REBORN_HOME/providers.json`.  Because `ProviderDefinition` already carries
`token_budget: Option<ProviderTokenBudget>` after Phase 1.1, **no code change is needed
here** — the round-trip is automatic.

### 2.2  Extend `UpsertLlmProviderRequest` to carry a token budget

**File:** `crates/brassclaw_product_workflow/src/reborn_services/llm_config.rs:216`

Add to `UpsertLlmProviderRequest`:
```rust
/// Token budget for this provider.  `None` leaves any existing budget
/// untouched.  Send an all-None `ProviderTokenBudgetView` to clear it.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub token_budget: Option<ProviderTokenBudgetView>,
```

### 2.3  Propagate token_budget through the upsert path

**File:** `crates/brassclaw_reborn_composition/src/llm_config_service.rs`

In the `upsert_provider` implementation (around line 400), after the `ProviderDefinition`
for the custom provider is constructed via `custom_definition(…)`, merge the token budget:

```rust
// token_budget field on ProviderDefinition is already Option<ProviderTokenBudget>.
// Convert the view type to the catalog type (same fields, different crate).
let token_budget = request.token_budget.as_ref().map(|v| brassclaw_llm::ProviderTokenBudget {
    profile: v.profile.clone(),
    conversation_history: v.conversation_history,
    skills: v.skills,
    identity: v.identity,
    inline_control: v.inline_control,
    memory: v.memory,
    safety: v.safety,
    capability_surface: v.capability_surface,
    total_input: v.total_input,
    max_output: v.max_output,
});
let definition = ProviderDefinition { token_budget, ..custom_definition(…) };
```

For **built-in** provider overrides (where only the active selection or API key is being
updated), keep the existing budget: read the current definition from the registry and
forward `token_budget` unchanged unless the request sets it.

### 2.4  Populate token_budget in `build_snapshot`

**File:** `crates/brassclaw_reborn_composition/src/llm_config_service.rs:178`

In the `for info in list.providers` loop that builds each `LlmProviderView`, read the
definition from the registry to retrieve its `token_budget` and map it to
`ProviderTokenBudgetView`:

```rust
let definition_budget = builtin_registry
    .find(&info.id)
    .and_then(|def| def.token_budget.as_ref())
    .map(|b| ProviderTokenBudgetView {
        profile: b.profile.clone(),
        conversation_history: b.conversation_history,
        skills: b.skills,
        identity: b.identity,
        inline_control: b.inline_control,
        memory: b.memory,
        safety: b.safety,
        capability_surface: b.capability_surface,
        total_input: b.total_input,
        max_output: b.max_output,
    });

providers.push(LlmProviderView {
    // … existing fields …
    token_budget: definition_budget,
});
```

The snapshot now sends token budgets to the frontend for display.

---

## Phase 3 — Settings Migration

The settings table schema does not change.  All migration is a single-pass, non-destructive
additive read at runtime startup.

### 3.1  Add per-provider methods to `TokenSettingsStore` trait

**File:** `crates/brassclaw_product_workflow/src/token_settings_store.rs`

Add two new async methods to the `TokenSettingsStore` trait below the existing ones:

```rust
async fn get_provider_token_settings(
    &self,
    user_id: &str,
    provider_id: &str,
) -> Result<TokenSettingsResponse, Box<dyn std::error::Error + Send + Sync>>;

async fn update_provider_token_settings(
    &self,
    user_id: &str,
    provider_id: &str,
    request: UpdateTokenSettingsRequest,
) -> Result<TokenSettingsResponse, Box<dyn std::error::Error + Send + Sync>>;
```

Keep the existing global methods intact.

### 3.2  Implement per-provider methods in `DbTokenSettingsStore`

**File:** `crates/brassclaw_reborn_composition/src/token_settings_store.rs`

Add a private helper above the `impl` block:
```rust
fn provider_tokens_key(provider_id: &str) -> String {
    // provider_id is already validated as [a-z0-9_-]{1,64} at the API layer.
    format!("provider_tokens:{provider_id}")
}
```

Implement `get_provider_token_settings`:  identical SQL to the global `get_token_settings`
implementation but using `provider_tokens_key(provider_id)` instead of
`TOKENS_SETTINGS_KEY`.

Implement `update_provider_token_settings`:  identical SQL to `update_token_settings`
using `provider_tokens_key(provider_id)`.

The existing global methods remain unchanged. The `settings` table primary key
`(user_id, key)` handles the new keys without any `ALTER TABLE` or migration script.

### 3.3  One-time forward migration: promote global settings to the active provider

This migration runs **once**, at startup, after the code is deployed.  It reads the
existing global token settings row and, if the active provider has no per-provider row
yet, writes the global row under the per-provider key.  The global row is left in place
(non-destructive).

**File:** `crates/brassclaw_reborn_cli/src/runtime/mod.rs`

Add a new async function `migrate_global_tokens_to_active_provider`:

```rust
async fn migrate_global_tokens_to_active_provider(
    token_settings_store: &dyn brassclaw_product_workflow::TokenSettingsStore,
    active_provider_id: &str,
    user_id: &str,
) {
    // 1. Check if per-provider settings already exist.
    let existing = token_settings_store
        .get_provider_token_settings(user_id, active_provider_id)
        .await;
    let has_existing = existing
        .as_ref()
        .map(|r| r.conversation_history.is_some() || r.profile.is_some())
        .unwrap_or(false);
    if has_existing {
        return; // already migrated; nothing to do
    }

    // 2. Read the global row.
    let Ok(global) = token_settings_store.get_token_settings(user_id).await else {
        return;
    };
    // 3. If global has any non-None field, copy it to the per-provider key.
    if global.conversation_history.is_some() || global.profile.is_some() {
        let request = brassclaw_product_workflow::UpdateTokenSettingsRequest {
            profile: global.profile,
            conversation_history: global.conversation_history,
            skills: global.skills,
            identity: global.identity,
            inline_control: global.inline_control,
            memory: global.memory,
            safety: global.safety,
            capability_surface: global.capability_surface,
            total_input: global.total_input,
            max_output: global.max_output,
        };
        if let Err(e) = token_settings_store
            .update_provider_token_settings(user_id, active_provider_id, request)
            .await
        {
            tracing::warn!(
                error = %e,
                provider_id = active_provider_id,
                "per-provider token settings migration failed; global defaults will apply"
            );
        }
    }
}
```

Call this function from `build_runtime_input_with_options` (around line 313) after the
token store and active provider ID are both available, before building
`RebornRuntimeInput`.

### 3.4  Load per-provider token settings at runtime start

**File:** `crates/brassclaw_reborn_cli/src/runtime/mod.rs`

Replace the existing `token_budgets_from_config()` call with a new
`resolve_active_provider_token_budgets` function:

```rust
async fn resolve_active_provider_token_budgets(
    config_file: Option<&RebornConfigFile>,
    token_settings_store: Option<&dyn brassclaw_product_workflow::TokenSettingsStore>,
    active_provider_id: Option<&str>,
    user_id: &str,
) -> brassclaw_reborn_config::ResolvedTokenBudgets {
    // Layer 1: file-level global [tokens] section (lowest priority, kept for compat)
    let file_budgets = config_file
        .and_then(|f| f.tokens.as_ref())
        .map(brassclaw_reborn_config::resolve_with_profile)
        .unwrap_or_default();

    // Layer 2: per-provider DB row (highest priority)
    let Some(store) = token_settings_store else {
        return file_budgets;
    };
    let Some(provider_id) = active_provider_id else {
        return file_budgets;
    };

    let db_budgets = store
        .get_provider_token_settings(user_id, provider_id)
        .await
        .ok()
        .filter(|r| r.conversation_history.is_some() || r.profile.is_some());

    let Some(db) = db_budgets else {
        return file_budgets;
    };

    // DB wins field-by-field over file
    let tokens_section = brassclaw_reborn_config::TokensSection {
        profile: db.profile.or(file_budgets.profile_name()),
        conversation_history: db.conversation_history.or(file_budgets.conversation_history),
        skills: db.skills.or(file_budgets.skills),
        identity: db.identity.or(file_budgets.identity),
        inline_control: db.inline_control.or(file_budgets.inline_control),
        memory: db.memory.or(file_budgets.memory),
        safety: db.safety.or(file_budgets.safety),
        capability_surface: db.capability_surface.or(file_budgets.capability_surface),
        total_input: db.total_input.or(file_budgets.total_input),
        max_output: db.max_output.or(file_budgets.max_output),
        // flags below come from file only (no UI control yet)
        capability_focus_enabled: None,
        planning_mode_enabled: None,
        content_cache_threshold: None,
        plan_library_enabled: None,
        skill_promotion_threshold: None,
    };
    brassclaw_reborn_config::resolve_with_profile(&tokens_section)
}
```

Note: `ResolvedTokenBudgets` needs a `profile_name()` accessor or the profile field
must be stored separately; see Phase 5.3 for the small extension to that struct.

---

## Phase 4 — Frontend UI

**Rule:** no new UI primitives.  The existing `TokensTab` component is reused verbatim
by extracting it into a shared `TokenBudgetForm` component that accepts a `providerId`
prop.

### 4.1  Add per-provider API helpers to `settings-api.js`

**File:**
`crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/settings-api.js`

Append after the existing `fetchTokenSettings` / `updateTokenSettings` exports:

```javascript
export function fetchProviderTokenSettings(providerId) {
  return apiFetch(
    `/api/webchat/v2/providers/${encodeURIComponent(providerId)}/tokens`
  );
}

export function updateProviderTokenSettings(providerId, payload) {
  return apiFetch(
    `/api/webchat/v2/providers/${encodeURIComponent(providerId)}/tokens`,
    { method: "PUT", body: JSON.stringify(payload) }
  );
}
```

### 4.2  Extract `TokenBudgetForm` from `tokens-tab.js`

**File (new):**
`crates/brassclaw_webui_v2_static/static/js/pages/settings/components/token-budget-form.js`

Move the following from `tokens-tab.js` verbatim into this new file:
- `TOKEN_FIELDS` constant
- `PRESETS` constant
- `PRESET_OPTIONS` constant
- `CUSTOM` constant
- `serverToForm()` function
- `formToPayload()` function
- `ProfileSelector` component
- `TokenField` component

Then export a new `TokenBudgetForm` component that wraps the form body and accepts two
props:

```javascript
/**
 * @param {string|null} providerId
 *   When set, reads/writes per-provider endpoints.
 *   When null, falls back to the global /api/webchat/v2/tokens endpoints.
 * @param {Array} queryKey
 *   React Query cache key, e.g. ["provider-tokens", providerId].
 * @param {string} [searchQuery=""]
 *   Optional search filter forwarded from the parent settings search box.
 */
export function TokenBudgetForm({ providerId, queryKey, searchQuery = "" }) {
  // … identical logic to the current TokensTab body, but using:
  //   providerId
  //     ? fetchProviderTokenSettings(providerId)
  //     : fetchTokenSettings()
  // and:
  //   providerId
  //     ? updateProviderTokenSettings(providerId, payload)
  //     : updateTokenSettings(payload)
}
```

### 4.3  Rewrite `tokens-tab.js` to delegate to `TokenBudgetForm`

**File:**
`crates/brassclaw_webui_v2_static/static/js/pages/settings/components/tokens-tab.js`

Replace the entire file body with:

```javascript
import { html } from "../../../lib/html.js";
import { TokenBudgetForm } from "./token-budget-form.js";

// The global Tokens tab is retained during the transition period.
// It will be removed once all users have had per-provider budgets
// migrated (see cleanup Phase 7).
export function TokensTab({ searchQuery = "" }) {
  return html`
    <${TokenBudgetForm}
      providerId=${null}
      queryKey=${["tokens"]}
      searchQuery=${searchQuery}
    />
  `;
}
```

All existing behaviour is preserved — the global tab still works during the transition.

### 4.4  Embed `TokenBudgetForm` in `provider-dialog.js`

**File:**
`crates/brassclaw_webui_v2_static/static/js/pages/settings/components/provider-dialog.js`

1. Add import at the top of the file:
   ```javascript
   import { TokenBudgetForm } from "./token-budget-form.js";
   ```

2. In `ModalBody`, after the Default Model row (after the `models.length > 0` block),
   add a collapsible Token Limits section.  Show it only when `provider.id` is defined
   (not during the first-time create flow where the ID isn't yet persisted):

   ```javascript
   ${provider?.id && html`
     <details>
       <summary className="mt-4 cursor-pointer text-sm font-medium text-[var(--v2-text-strong)]">
         ${t("llm.tokenLimits")}
       </summary>
       <div className="mt-3">
         <${TokenBudgetForm}
           providerId=${provider.id}
           queryKey=${["provider-tokens", provider.id]}
         />
       </div>
     </details>
   `}
   ```

### 4.5  Add i18n key

**File:** locate the primary locale file by running:
```bash
grep -rl '"llm.configureProvider"' crates/brassclaw_webui_v2_static/
```
Add to that file:
```
"llm.tokenLimits": "Token Limits",
"llm.tokenLimits.desc": "Override the default prompt-composition token limits for this provider."
```
Add the same keys to every other locale file in the same directory, using English text
as the value (translators will replace them).

### 4.6  No change required to `useLlmProviders.js`

The `GET /api/webchat/v2/llm/providers` response now includes `token_budget` per
provider (Phase 2.4).  The hook exposes this transparently through
`providers[i].token_budget`.  `TokenBudgetForm` manages its own query/mutation via
React Query; the hook does not wrap it.

---

## Phase 5 — API Layer

### 5.1  Add per-provider token endpoint handlers

**File:** `crates/brassclaw_webui_v2/src/handlers/tokens.rs`

Add two new handler functions below the existing global handlers:

```rust
pub async fn get_provider_token_settings(
    State(state): State<WebUiV2State>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    Path(provider_id): Path<String>,
) -> Result<Json<TokenSettingsResponse>, WebUiV2HttpError> {
    state
        .services()
        .get_provider_token_settings(caller, &provider_id)
        .await
        .map(Json)
        .map_err(Into::into)
}

pub async fn update_provider_token_settings(
    State(state): State<WebUiV2State>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    Path(provider_id): Path<String>,
    Json(body): Json<UpdateTokenSettingsRequest>,
) -> Result<Json<TokenSettingsResponse>, WebUiV2HttpError> {
    state
        .services()
        .update_provider_token_settings(caller, &provider_id, body)
        .await
        .map(Json)
        .map_err(Into::into)
}
```

### 5.2  Mount the new routes

**File:** `crates/brassclaw_webui_v2/src/router.rs`

Locate the existing `…/{id}/delete` route for LLM providers (currently the only
`/{id}/…` sub-route).  Add the token routes alongside it:

```rust
router.at(
    "/api/webchat/v2/providers/:provider_id/tokens",
    get(handlers::tokens::get_provider_token_settings)
        .put(handlers::tokens::update_provider_token_settings),
)
```

**Note:** the path parameter is `:provider_id` here but `:id` on the delete route —
use `:provider_id` for the token routes to be explicit.

### 5.3  Add `get_provider_token_settings` / `update_provider_token_settings` to `RebornServices`

**File:** `crates/brassclaw_product_workflow/src/reborn_services.rs`

Add two methods to the `LlmConfigService`-adjacent section (near line 2150 where
`get_token_settings` / `update_token_settings` are implemented).  Both delegate to
`self.token_settings_store`:

```rust
pub async fn get_provider_token_settings(
    &self,
    caller: WebUiAuthenticatedCaller,
    provider_id: &str,
) -> Result<TokenSettingsResponse, RebornServicesError> {
    let Some(store) = &self.token_settings_store else {
        return Err(RebornServicesError::Unavailable);
    };
    store
        .get_provider_token_settings(&caller.user_id.to_string(), provider_id)
        .await
        .map_err(|e| RebornServicesError::Internal { reason: e.to_string() })
}

pub async fn update_provider_token_settings(
    &self,
    caller: WebUiAuthenticatedCaller,
    provider_id: &str,
    request: UpdateTokenSettingsRequest,
) -> Result<TokenSettingsResponse, RebornServicesError> {
    let Some(store) = &self.token_settings_store else {
        return Err(RebornServicesError::Unavailable);
    };
    // Validate the provider_id at the service boundary (same rule as UpsertLlmProvider).
    if !provider_id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        || provider_id.is_empty()
        || provider_id.len() > 64
    {
        return Err(RebornServicesError::InvalidRequest {
            reason: "provider_id must be [a-z0-9_-]{1,64}".to_string(),
        });
    }
    store
        .update_provider_token_settings(&caller.user_id.to_string(), provider_id, request)
        .await
        .map_err(|e| RebornServicesError::Internal { reason: e.to_string() })
}
```

Also add a `profile_name()` accessor to `ResolvedTokenBudgets`
(`crates/brassclaw_reborn_config/src/config_file.rs`) so Phase 3.4's merge code can
read back a profile name from the resolved struct when layering DB over file:

```rust
impl ResolvedTokenBudgets {
    /// The preset name that was active when this budget was resolved, if any.
    pub fn profile_name(&self) -> Option<String> {
        self.profile.clone()     // add `pub profile: Option<String>` field to the struct
    }
}
```

Add `pub profile: Option<String>` to `ResolvedTokenBudgets` and populate it in
`resolve_with_profile()` with `overrides.profile.clone()` (already available).

---

## Phase 6 — Loop Wiring

This is the section where the verified facts from V2 fully determine the approach.

### Principle

`LoopExecutionState` carries no provider field.  The active provider is decided at
startup and lives in the `SwappableLlmProvider` at the model gateway.  The loop family
registry is rebuilt when the provider hot-swaps.  Therefore, the correct and only
necessary change is: **bake the active provider's resolved token budget into the
`DefaultContextStrategy` struct at the moment the loop family registry is built**.
No new types, no runtime lookups, no changes to the loop executor.

### 6.1  Thread resolved conversation_history into `families::default_with_full_config`

The call site already passes `conversation_context_tokens: Option<usize>`.  After Phase
3.4, `resolve_active_provider_token_budgets` returns the correct per-provider-aware
`ResolvedTokenBudgets`.  The caller in `crates/brassclaw_reborn_cli/src/runtime/mod.rs`
already does:

```rust
.with_conversation_context_tokens(token_budgets.conversation_history)
```

No structural change is needed in the loop family wiring.  The budget from the correct
provider flows through the existing path.

### 6.2  Hot-swap path: rebuild the registry on provider change

**File:** `crates/brassclaw_reborn_composition/src/llm_config_service.rs`, method
`refresh_running_provider` (~line 158).

The `LlmReloadTrigger.reload()` call already causes the runtime to rebuild the model
gateway.  To also rebuild the loop family registry with the new provider's token budget,
the reload trigger must be extended OR the token budget must be read from the same
`SwappableLlmProvider` that carries the model.

**The clean solution** (no new reload machinery):

In `crates/brassclaw_reborn_composition/src/runtime.rs`, the function
`build_production_model_gateway` already constructs the `SwappableLlmProvider`.  Extend
the `LlmReloadHandle` (or equivalent reload closure) to also call a
`LoopFamilyRegistryReloadFn` — a `Arc<dyn Fn(ResolvedTokenBudgets) + Send + Sync>` that
rebuilds the `LoopFamilyRegistry` and atomically swaps it.

Concretely:

1. In `crates/brassclaw_reborn/src/runtime.rs`, add to `DefaultPlannedRuntimeParts`:
   ```rust
   pub loop_family_registry_reload:
       Option<Arc<dyn Fn(Option<usize>) + Send + Sync>>,
   ```

2. After `build_loop_family_registry_with_full_config` is called at boot, wrap the
   registry in an `Arc<RwLock<Arc<LoopFamilyRegistry>>>` and expose a reload closure:
   ```rust
   let registry_slot: Arc<RwLock<Arc<LoopFamilyRegistry>>> =
       Arc::new(RwLock::new(family_registry));
   let registry_slot_clone = Arc::clone(&registry_slot);
   let reload_fn: Arc<dyn Fn(Option<usize>) + Send + Sync> = Arc::new(
       move |new_conversation_tokens: Option<usize>| {
           let new_registry = build_loop_family_registry_with_full_config(
               LoopFamilyConfig {
                   conversation_context_tokens: new_conversation_tokens,
                   // capability_surface_tokens / flags re-read from same config
                   ..LoopFamilyConfig::default()
               },
           )
           .unwrap_or_else(|_| {
               // Infallible in practice (static family list); keep old if it fails
               Arc::clone(&*registry_slot_clone.read().unwrap_or_else(|p| p.into_inner()))
           });
           *registry_slot_clone.write().unwrap_or_else(|p| p.into_inner()) = new_registry;
       },
   );
   ```

3. The turn runner reads from `registry_slot` (reads the current `Arc` under the
   `RwLock` at each turn start — one cheap read lock) instead of holding a direct
   `Arc<LoopFamilyRegistry>`.

4. The `LlmReloadTrigger.reload()` implementation calls both the provider swap **and**
   the registry reload closure with the freshly resolved token budget for the new active
   provider.

**Simpler alternative** (acceptable for v1 of this feature): document that a provider
change requires a restart to apply the new token budget.  On restart, the existing path
already picks up the correct per-provider budget.  The hot-swap reload extension above
can be a Phase 6 follow-up — mark with `// TODO(per-provider-budget): hot-swap reload`.
This avoids the `RwLock` machinery in a first iteration.

---

## Phase 7 — Cleanup (post-release, separate PR)

Do not merge these until per-provider budgets have been live for at least one release
cycle and the migration function has run for all users.

### 7.1  Remove the global Tokens tab from the frontend nav

**File:**
`crates/brassclaw_webui_v2_static/static/js/app/routes.js`

Remove `{ id: "tokens", labelKey: "settings.tokens", icon: "bolt" }` from
`SETTINGS_SUB_ROUTES`.

### 7.2  Delete `tokens-tab.js`

Delete `crates/brassclaw_webui_v2_static/static/js/pages/settings/components/tokens-tab.js`.

### 7.3  Remove global token HTTP endpoints

**File:** `crates/brassclaw_webui_v2/src/router.rs`

Remove the route block mounting:
```rust
"/api/webchat/v2/tokens"  →  get(handlers::tokens::get_token_settings)
                              .put(handlers::tokens::update_token_settings)
```

### 7.4  Remove global token methods from handlers/tokens.rs

Delete `get_token_settings` and `update_token_settings` from
`crates/brassclaw_webui_v2/src/handlers/tokens.rs`.

### 7.5  Remove global token methods from `RebornServices`

Remove `get_token_settings` and `update_token_settings` from
`crates/brassclaw_product_workflow/src/reborn_services.rs`.

### 7.6  Remove `get_token_settings` / `update_token_settings` from `TokenSettingsStore` trait

**File:** `crates/brassclaw_product_workflow/src/token_settings_store.rs`

Remove the two global methods from the trait and all implementations.

### 7.7  Remove `[tokens]` section from config file schema

**File:** `crates/brassclaw_reborn_config/src/config_file.rs`

Remove `pub tokens: Option<TokensSection>` from `RebornConfigFile`.

Remove the now-unreachable `TokensSection` struct and `resolve_with_profile()` function
(or keep `TokensSection` if the TOML `[llm.<slot>].token_budget` sub-table still needs it
— keep the function, remove the top-level field).

---

## Phase 8 — Tests

### 8.1  Unit — `validate_provider_id` length limit

**File:** `crates/brassclaw_reborn_composition/src/llm_config_service.rs`, existing
`provider_id_validation_rejects_bad_input` test block.

Add:
```rust
assert!(validate_provider_id(&"a".repeat(64)).is_ok());
assert!(validate_provider_id(&"a".repeat(65)).is_err());
```

### 8.2  Unit — `DbTokenSettingsStore` per-provider round-trip

**File:** `crates/brassclaw_reborn_composition/src/token_settings_store.rs`, new `#[cfg(test)]` block.

Tests:
- `provider_tokens_key_format` — asserts `provider_tokens_key("ollama") == "provider_tokens:ollama"`.
- `store_and_retrieve_provider_tokens` — open an in-memory DB, upsert for provider "a",
  retrieve, assert all fields round-trip.
- `two_providers_are_isolated` — upsert different values for "a" and "b", assert they
  don't overwrite each other.
- `missing_provider_returns_all_none` — get for a provider that was never written returns
  `TokenSettingsResponse` with all `None` fields.

### 8.3  Unit — `ProviderDefinition` token_budget round-trips through JSON

**File:** `crates/brassclaw_llm/src/registry.rs`, new test.

- Serialize a `ProviderDefinition` with `token_budget: Some(ProviderTokenBudget { profile: Some("small_7b".into()), conversation_history: Some(4000), ..Default::default() })` to JSON.
- Deserialize back, assert field values are preserved.
- Serialize a `ProviderDefinition` with `token_budget: None`, assert the `"token_budget"` key is absent from the JSON (due to `skip_serializing_if`).

### 8.4  Integration — HTTP endpoint round-trip

**File:** `crates/brassclaw_webui_v2/tests/` or the existing handler test module.

Tests:
- `GET /api/webchat/v2/providers/ollama/tokens` on a fresh store returns 200 with all-None body.
- `PUT /api/webchat/v2/providers/ollama/tokens` with `{ "profile": "small_7b" }` returns
  200; subsequent GET returns the same body.
- `PUT` for provider "a" and "b" independently returns different values on subsequent GETs.
- `PUT /api/webchat/v2/providers/BAD-ID/tokens` returns 400 (fails validation in `RebornServices`).

### 8.5  Regression — all existing token tests stay green

The 13 tests in `crates/brassclaw_agent_loop/src/token_budget.rs` and the 9 tests in
`crates/brassclaw_agent_loop/src/strategies/context.rs` must pass unchanged throughout
all phases.  Run with:
```bash
cargo test -p brassclaw_agent_loop token_budget
cargo test -p brassclaw_agent_loop strategies::context
```

---

## Execution Checklist

```
Phase 1 — Data model
  1.1  Add ProviderTokenBudget struct + token_budget field to ProviderDefinition
  1.2  Add ≤64-char constraint to validate_provider_id
  1.3  Add ProviderTokenBudgetView + token_budget field to LlmProviderView

Phase 2 — Catalog persistence
  2.1  ProviderDefinition already round-trips (no code change after 1.1)
  2.2  Add token_budget field to UpsertLlmProviderRequest
  2.3  Propagate token_budget in upsert_provider
  2.4  Populate token_budget in build_snapshot

Phase 3 — Migration
  3.1  Extend TokenSettingsStore trait with per-provider methods
  3.2  Implement per-provider methods in DbTokenSettingsStore
  3.3  Add + call migrate_global_tokens_to_active_provider at startup
  3.4  Add resolve_active_provider_token_budgets; replace token_budgets_from_config call
  3.5  Add profile field + profile_name() to ResolvedTokenBudgets

Phase 4 — Frontend
  4.1  Add fetchProviderTokenSettings / updateProviderTokenSettings to settings-api.js
  4.2  Extract TokenBudgetForm from tokens-tab.js into token-budget-form.js
  4.3  Rewrite tokens-tab.js to delegate to TokenBudgetForm(providerId=null)
  4.4  Embed TokenBudgetForm in provider-dialog.js
  4.5  Add i18n key llm.tokenLimits

Phase 5 — API layer
  5.1  Add get_provider_token_settings / update_provider_token_settings handlers
  5.2  Mount new routes in router.rs
  5.3  Add service methods to RebornServices + validation

Phase 6 — Loop wiring
  6.1  Verify resolve_active_provider_token_budgets feeds correct value through
       existing .with_conversation_context_tokens() path (no new code needed)
  6.2  Optional: RwLock-based registry hot-swap on provider change
       (acceptable to defer with // TODO comment for v1)

Phase 7 — Cleanup (separate PR, post-release)
  7.1–7.7  Remove global token tab, endpoints, trait methods, config key

Phase 8 — Tests
  8.1  validate_provider_id length test
  8.2  DbTokenSettingsStore per-provider unit tests (4 tests)
  8.3  ProviderDefinition JSON round-trip tests
  8.4  HTTP endpoint integration tests (4 tests)
  8.5  Regression: all existing token tests pass
```

---

## Design Invariants Met

| Invariant | How it is met |
|---|---|
| O(1) budget resolution at loop-start | Budget is baked into `DefaultContextStrategy.max_context_tokens` at startup; no map lookup or DB read during iterations |
| No hot-path network round-trips | DB read happens once at startup in `resolve_active_provider_token_budgets`; the result is a plain `usize` |
| Minimal structs | `ProviderTokenBudget` has exactly the 10 fields the runtime consumes; no extras |
| Provider IDs ≤ 64 chars | Enforced by `validate_provider_id` — the only path by which a provider ID enters the system |
| Non-destructive migration | Global settings row is never deleted; it is only read and copied |
| Existing tests stay green | No signature changes to `DefaultContextStrategy`, `TokenBudgetTracker`, `estimate_tokens`, or any existing public API |
