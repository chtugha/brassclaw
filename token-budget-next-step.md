# Token Budget Next Steps — Implementation Instructions

This document describes exactly how to implement outstanding gaps in the
per-provider token budgeting system **and** how to surface, configure, and
account for provider-level **prompt caching** (Anthropic automatic caching,
vLLM/OpenAI-compatible prefix caching, OpenClaw explicit cache blocks).

Each section explains **why** the change is needed, **which files and symbols**
are affected, the **precise code delta** to make, and what **tests** to add.
All proposals follow the no-`.unwrap()` rule, stay within the existing layering
model, and keep each change surgically scoped.

---

## Codebase State (as of this audit)

Steps 1–4 and the new `inline_control_tokens` + `total_input_tokens` items are
**fully implemented** (confirmed against committed source):

| Step | Status | Evidence |
|------|--------|---------|
| Step 1 — Wire `max_output` | ✅ Done | `LlmProviderModelGateway.with_max_output_tokens`, `DefaultPlannedRuntimeConfig.max_output_tokens`, threaded through `build_production_model_gateway` → `wrap_swappable_gateway`; `CompletionRequest.max_tokens` set on every provider call |
| Step 2 — Hot-swap budget gap | ✅ Done | `LlmReloadTrigger::on_provider_changed` default impl added; `RebornLlmReloadAdapter.with_on_provider_changed` callback wired in `webui.rs`; `webui_llm_reload_adapter()` returns bare adapter (not Arc-wrapped) so callback can be attached before wrapping |
| Step 3 — `DefaultContextStrategy` semantics | ✅ Done | `DefaultContextStrategy` now has `context_window_tokens`, `inline_control_tokens`, `observed_message_average` (EMA) fields; `notify_model_usage` override updates EMA; `plan_context_request` uses `TurnContextBudget::from_context_window` + EMA instead of `(budget/2)/200`; `update_message_average` public helper |
| Step 4 — Feed `context_window_tokens` | ✅ Done | `LoopFamilyConfig.context_window_tokens` forwarded to `default_with_full_config`; `DefaultCompactionStrategy.context_limit_tokens` set from provider window; `DefaultStrategySlots::with_compaction` builder added; `resolved_context_window_tokens` read from `providers.json` in `build_reborn_runtime` |
| **NEW** — Wire `inline_control_tokens` | ✅ Done | `default_with_full_config` takes 5th param `inline_control_tokens: Option<usize>`; `LoopFamilyConfig.inline_control_tokens` forwarded; `DefaultPlannedRuntimeConfig.inline_control_tokens` threaded through; DB row `inline_control` resolved and passed in `build_reborn_runtime` |
| **NEW** — Wire `total_input_tokens` pre-call guard | ✅ Done | `LlmProviderModelGateway.with_total_input_tokens` sets guard; `check_total_input_budget` called in both `stream_model` paths; DB row `total_input` resolved in `resolve_active_provider_token_budgets` (7-tuple) and passed through `build_production_model_gateway` → `build_llm_gateway` → `wrap_swappable_gateway` |
| **PARTIAL** — Hot-swap on provider change | ⚠️ Partial | `on_provider_changed` callback only refreshes `conversation_history` slot (`webui.rs` line 167). `inline_control`, `max_output`, `total_input`, and `context_window_tokens` are **not** hot-swapped; they stay at boot-time values until the next restart. See gap note below. |
| Step 5 — Schema cleanup | ⬜ Pending | Still needs `TokensSection` field removal |
| Step 6 — Phase 8 tests | ⬜ Pending | Still needs `ProviderDefinition` round-trip + live-setter regression tests |

### Hot-swap gap (Step 2 partial): fields not refreshed on provider change

The `on_provider_changed` callback in `crates/brassclaw_reborn_composition/src/webui.rs`
reads `row.conversation_history` and calls `slot.set(...)` on the
`LiveTokenBudget`. The following fields from `TokenSettingsRow` are **not**
refreshed without a restart:

| Field | Why not hot-swapped |
|-------|---------------------|
| `max_output_tokens` | Stored in `LlmProviderModelGateway.max_output_tokens` — a plain `Option<u32>` with no live slot; requires rebuilding the gateway |
| `total_input_tokens` | Same — `LlmProviderModelGateway.total_input_tokens` has no live slot |
| `inline_control_tokens` | `DefaultContextStrategy.inline_control_tokens` is a plain field; no `Arc<AtomicUsize>` equivalent exists yet |
| `context_window_tokens` | Read once from `providers.json` at boot; no hot-swap seam for `DefaultCompactionStrategy.context_limit_tokens` |

To fully close this gap, each of these four fields needs an `Arc<Atomic*>` live
slot (analogous to `LiveTokenBudget`) and the callback must call `.set()` on
each after re-reading the DB row. This is a follow-up task.

New gaps identified by this audit (prompt caching, hot-swap cache retention,
cost accounting, vLLM) are captured below as Steps 7–10.

---

## Step 5 — Phase 7.7 — Remove `[tokens]` budget fields from config schema

### Why it matters

`config.toml [tokens]` budget fields (conversation_history, skills, identity,
capability_surface, max_output, …) are parsed but explicitly discarded in
`behavior_flags_from_config`.  This silently ignores operator configuration,
which is confusing.  The behavior-flag fields (capability_focus_enabled,
planning_mode_enabled, content_cache_threshold, plan_library_enabled,
skill_promotion_threshold) must be retained because they have no DB equivalent.

### Step 5.1 — Remove budget number fields from `TokensSection`

**File:** `crates/brassclaw_reborn_config/src/config_file.rs`

Remove from `TokensSection`:

```
profile
conversation_history
skills
identity
inline_control
memory
safety
capability_surface
total_input
max_output
```

Keep only the behavior-flag fields:

```rust
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokensSection {
    pub capability_focus_enabled: Option<bool>,
    pub planning_mode_enabled: Option<bool>,
    pub content_cache_threshold: Option<usize>,
    pub plan_library_enabled: Option<bool>,
    pub skill_promotion_threshold: Option<f64>,
}
```

Because `deny_unknown_fields` is on, any existing `config.toml` that still
contains budget number fields will now **fail to parse** at boot.  This is the
correct behaviour — operators must be told to remove the old fields.

### Step 5.2 — Update `behavior_flags_from_config`

**File:** `crates/brassclaw_reborn_cli/src/runtime/mod.rs`

Remove the now-unnecessary explicit-`None` block.  The function becomes:

```rust
fn behavior_flags_from_config(
    config_file: Option<&brassclaw_reborn_config::RebornConfigFile>,
) -> brassclaw_reborn_config::ResolvedTokenBudgets {
    let Some(tokens) = config_file.and_then(|file| file.tokens.as_ref()) else {
        return brassclaw_reborn_config::ResolvedTokenBudgets::default();
    };
    brassclaw_reborn_config::ResolvedTokenBudgets {
        capability_focus_enabled: tokens.capability_focus_enabled.unwrap_or(false),
        planning_mode_enabled: tokens.planning_mode_enabled.unwrap_or(false),
        content_cache_threshold: tokens.content_cache_threshold,
        plan_library_enabled: tokens.plan_library_enabled.unwrap_or(false),
        skill_promotion_threshold: tokens.skill_promotion_threshold,
        ..brassclaw_reborn_config::ResolvedTokenBudgets::default()
    }
}
```

### Step 5.3 — Remove `resolve_with_profile` if it becomes dead code

**File:** `crates/brassclaw_reborn_config/src/config_file.rs`

Check callers with `grep -r "resolve_with_profile"`. If there are no remaining
callers, delete the function and the four `TokenDistributionPreset` constants.
If they are still used (e.g. from the DB settings migration or tests), keep
them but add `#[doc(hidden)]`.

### Step 5.4 — Update doc comment on `[tokens]` section in `RebornConfigFile`

```rust
/// Behavior flags for context strategy selection and feature toggles.
/// Token budget values (conversation_history, skills, etc.) are no longer
/// accepted here; configure them per-provider in the Settings → Inference UI
/// or via PUT /api/webchat/v2/providers/{id}/tokens.
/// Retains: capability_focus_enabled, planning_mode_enabled,
/// content_cache_threshold, plan_library_enabled, skill_promotion_threshold.
pub tokens: Option<TokensSection>,
```

### Step 5.5 — Update config file tests

**File:** `crates/brassclaw_reborn_config/src/` tests.

Replace any test that writes `[tokens]\nconversation_history = 8000` with an
expected-parse-error assertion (the field is now unknown and rejected by
`deny_unknown_fields`).  Add a passing test with behavior-flag fields only.

### Step 5.6 — CHANGELOG note

Add to `CHANGELOG.md`:

```
### Breaking change
`[tokens]` in `config.toml` no longer accepts token-budget number fields
(conversation_history, skills, identity, inline_control, memory, safety,
capability_surface, total_input, max_output, profile).  Any existing config.toml
that contains these fields will fail to parse at startup.  Remove them and
configure budgets via the Settings UI (Settings → Inference → select provider →
Token Limits) or the API.
```

---

## Step 6 — Phase 8 missing tests

### Test 8.3 — `ProviderDefinition` JSON round-trip

**File:** `crates/brassclaw_llm/src/registry.rs`, add to the `#[cfg(test)]` block.

```rust
#[test]
fn provider_definition_token_budget_round_trips_through_json() {
    use crate::registry::ProviderTokenBudget;

    let original = ProviderDefinition {
        id: "test-provider".to_string(),
        aliases: vec![],
        protocol: ProviderProtocol::OpenAi,
        default_base_url: None,
        base_url_env: None,
        base_url_required: false,
        api_key_env: None,
        api_key_required: false,
        model_env: "TEST_MODEL".to_string(),
        default_model: "test-model".to_string(),
        description: "test".to_string(),
        extra_headers_env: None,
        setup: None,
        unsupported_params: vec![],
        token_budget: Some(ProviderTokenBudget {
            profile: Some("small_7b".to_string()),
            conversation_history: Some(4000),
            ..ProviderTokenBudget::default()
        }),
        context_window_tokens: Some(12000),
    };

    let json = serde_json::to_string(&original).expect("serialize");
    let deserialized: ProviderDefinition = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(
        deserialized.token_budget.as_ref().unwrap().profile,
        Some("small_7b".to_string())
    );
    assert_eq!(
        deserialized.token_budget.as_ref().unwrap().conversation_history,
        Some(4000)
    );
    assert_eq!(deserialized.context_window_tokens, Some(12000));
}

#[test]
fn provider_definition_absent_token_budget_omitted_from_json() {
    let original = ProviderDefinition {
        id: "no-budget".to_string(),
        aliases: vec![],
        protocol: ProviderProtocol::OpenAi,
        default_base_url: None,
        base_url_env: None,
        base_url_required: false,
        api_key_env: None,
        api_key_required: false,
        model_env: "NO_BUDGET_MODEL".to_string(),
        default_model: "no-budget-model".to_string(),
        description: "test".to_string(),
        extra_headers_env: None,
        setup: None,
        unsupported_params: vec![],
        token_budget: None,
        context_window_tokens: None,
    };

    let json = serde_json::to_string(&original).expect("serialize");
    assert!(
        !json.contains("\"token_budget\""),
        "absent token_budget must be omitted; got: {json}"
    );
}
```

### Test 8.4 — HTTP endpoint integration (live-setter regression)

**File:** `crates/brassclaw_product_workflow/src/reborn_services.rs`, test block
(or a new `#[cfg(test)]` module in that file).

The test must verify:

1. `update_provider_token_settings` calls `live_context_budget_setter` with the
   new `conversation_history` value.
2. A subsequent `get_provider_token_settings` returns the persisted value.
3. Provider-ID validation rejects IDs that are too long or contain uppercase.

```rust
#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use crate::{
        RebornServices, TokenSettingsStore, UpdateTokenSettingsRequest,
        WebUiAuthenticatedCaller, token_settings::TokenSettingsResponse,
    };

    /// Stub store that records the last upserted response.
    struct RecordingTokenStore {
        last: Mutex<Option<TokenSettingsResponse>>,
    }
    #[async_trait::async_trait]
    impl TokenSettingsStore for RecordingTokenStore {
        async fn get_provider_token_settings(
            &self, _user_id: &str, _provider_id: &str,
        ) -> Result<TokenSettingsResponse, Box<dyn std::error::Error + Send + Sync>> {
            Ok(self.last.lock().unwrap().clone().unwrap_or(TokenSettingsResponse {
                profile: None, conversation_history: None, skills: None,
                identity: None, inline_control: None, memory: None,
                safety: None, capability_surface: None,
                total_input: None, max_output: None,
            }))
        }
        async fn update_provider_token_settings(
            &self, _user_id: &str, _provider_id: &str,
            request: UpdateTokenSettingsRequest,
        ) -> Result<TokenSettingsResponse, Box<dyn std::error::Error + Send + Sync>> {
            let response = TokenSettingsResponse {
                profile: request.profile,
                conversation_history: request.conversation_history,
                skills: request.skills,
                identity: request.identity,
                inline_control: request.inline_control,
                memory: request.memory,
                safety: request.safety,
                capability_surface: request.capability_surface,
                total_input: request.total_input,
                max_output: request.max_output,
            };
            *self.last.lock().unwrap() = Some(response.clone());
            Ok(response)
        }
    }

    fn test_caller() -> WebUiAuthenticatedCaller {
        WebUiAuthenticatedCaller::for_test("test-user")
    }

    #[tokio::test]
    async fn update_provider_tokens_calls_live_setter() {
        let setter_received: Arc<Mutex<Option<Option<usize>>>> =
            Arc::new(Mutex::new(None));
        let setter_clone = Arc::clone(&setter_received);

        let store = Arc::new(RecordingTokenStore { last: Mutex::new(None) });
        let (thread_service, turn_coordinator) = crate::fakes::build_noop_services();
        let services = RebornServices::new(thread_service, turn_coordinator)
            .with_token_settings_store(
                store.clone() as Arc<dyn TokenSettingsStore>,
            )
            .with_live_context_budget_setter(Arc::new(move |v| {
                *setter_clone.lock().unwrap() = Some(v);
            }));

        services
            .update_provider_token_settings(
                test_caller(),
                "ollama",
                UpdateTokenSettingsRequest {
                    profile: None,
                    conversation_history: Some(4000),
                    skills: None, identity: None, inline_control: None,
                    memory: None, safety: None, capability_surface: None,
                    total_input: None, max_output: None,
                },
            )
            .await
            .expect("update must succeed");

        assert_eq!(
            *setter_received.lock().unwrap(),
            Some(Some(4000)),
            "live setter must be called with conversation_history=4000"
        );
    }

    #[tokio::test]
    async fn update_provider_tokens_rejects_invalid_id() {
        let store = Arc::new(RecordingTokenStore { last: Mutex::new(None) });
        let (thread_service, turn_coordinator) = crate::fakes::build_noop_services();
        let services = RebornServices::new(thread_service, turn_coordinator)
            .with_token_settings_store(store as Arc<dyn TokenSettingsStore>);

        // Too long (65 chars)
        let result = services
            .update_provider_token_settings(
                test_caller(),
                &"a".repeat(65),
                UpdateTokenSettingsRequest {
                    conversation_history: Some(1000),
                    ..Default::default()
                },
            )
            .await;
        assert!(result.is_err(), "provider ID >64 chars must be rejected");

        // Uppercase
        let result = services
            .update_provider_token_settings(
                test_caller(),
                "OllaMA",
                UpdateTokenSettingsRequest {
                    conversation_history: Some(1000),
                    ..Default::default()
                },
            )
            .await;
        assert!(result.is_err(), "uppercase provider ID must be rejected");
    }

    #[tokio::test]
    async fn get_provider_tokens_rejects_invalid_id() {
        let store = Arc::new(RecordingTokenStore { last: Mutex::new(None) });
        let (thread_service, turn_coordinator) = crate::fakes::build_noop_services();
        let services = RebornServices::new(thread_service, turn_coordinator)
            .with_token_settings_store(store as Arc<dyn TokenSettingsStore>);

        let result = services
            .get_provider_token_settings(test_caller(), "BAD_ID")
            .await;
        assert!(result.is_err(), "uppercase provider ID must be rejected");
    }
}
```

---

## Step 7 — Expose `cache_retention` in the DB / Settings UI

### Why it matters

`CacheRetention` (Anthropic automatic prompt caching: Short=5 min / Long=1 h /
None) is fully implemented in `brassclaw_llm` and wired through `RigAdapter` but
is **only configurable via `ANTHROPIC_CACHE_RETENTION` env var or
`providers.json`**. There is no DB column, no Settings UI field, no hot-swap
path. Operators on claude-3.5-sonnet or claude-3-haiku get no cache unless they
set the env var before boot. A per-provider DB field fixes this — it follows
the same budget-settings pattern already in place.

### Why prompt caching matters (economics recap)

Anthropic's automatic prompt caching charges 1.25× input rate to write a cache
block (Short TTL) or 2.0× for Long TTL, then reads it back at 10% of normal
input cost. For a 40 K-token system-prompt that is re-sent every turn:

```
Without cache:  40,000 × $3/Mtok = $0.12  per turn
With Short:     40,000 × $3.75/Mtok once (write), then 40,000 × $0.30/Mtok
                → break-even after 1 re-use; ~97% savings from turn 2 onward
```

For an agent with a large tool-description surface (tool definitions
are the second most-cached block), the savings compound per turn.

vLLM and other OpenAI-compatible endpoints perform **automatic KV-cache
prefix matching** server-side — they need no `cache_control` injection from the
client. The only client requirement is **message-ordering stability**: system
prompt first, conversation history appended in chronological order. BrassClaw
already does this naturally (see `crates/brassclaw_reborn/src/model_gateway.rs`,
`convert_messages` and `ThreadBackedLoopContextPort`). No code change needed
for vLLM.

OpenClaw's explicit cache blocks (`"type": "cache"` in individual content
parts) are a future extension; the current architecture supports it via
`additional_params` but it is not wired at the turn level. Track as a
future TODO.

### Step 7.1 — Add `cache_retention` to `TokenSettingsResponse` / `UpdateTokenSettingsRequest`

**File:** `crates/brassclaw_product_workflow/src/token_settings.rs`
(or wherever these structs live — check `grep -r "TokenSettingsResponse"`).

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenSettingsResponse {
    // existing fields …
    pub max_output: Option<usize>,
    /// Prompt cache retention policy for Anthropic providers.
    /// `None` = provider default (Short for Anthropic claude-3+, None otherwise).
    /// Accepted values: "none", "short", "long".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_retention: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateTokenSettingsRequest {
    // existing fields …
    pub max_output: Option<usize>,
    /// Same semantics as `TokenSettingsResponse.cache_retention`.
    pub cache_retention: Option<String>,
}
```

Keep the field as `Option<String>` at the product-workflow boundary so the
product-workflow crate does not depend on `brassclaw_llm::CacheRetention`
(type-layer separation). Parse to `CacheRetention` at the composition boundary
only.

### Step 7.2 — Add DB column `cache_retention` to the token-settings table

**File:** `src/db/` or the libSQL migration that creates `provider_token_settings`.

Add a nullable `TEXT` column:

```sql
ALTER TABLE provider_token_settings ADD COLUMN cache_retention TEXT;
```

Add the migration to the libSQL schema migration sequence. The PostgreSQL
implementation follows the same pattern. Both backends must store/retrieve
the value as a nullable string ("none" / "short" / "long") and validate on
write that the value is one of the three accepted strings.

### Step 7.3 — Thread `cache_retention` through `resolve_active_provider_token_budgets`

**File:** `crates/brassclaw_reborn_composition/src/runtime.rs`

The 7-tuple returned by `resolve_active_provider_token_budgets` gains one more
element:

```rust
) -> (
    Option<usize>,   // conversation_context_tokens
    Option<usize>,   // skill_context_tokens
    Option<usize>,   // identity_token_ceiling
    Option<usize>,   // capability_surface_tokens
    Option<u32>,     // max_output_tokens
    Option<usize>,   // inline_control_tokens
    Option<usize>,   // total_input_tokens
    Option<String>,  // cache_retention (new)
)
```

At the return site, add:

```rust
db.cache_retention.or(file_cache_retention),  // new
```

`file_cache_retention` comes from a new boot-config field (Step 7.4 below) or
from the env-var resolution already in `brassclaw_llm::resolution`. The
composition root must resolve this to a `CacheRetention` enum value before
passing it to `build_production_model_gateway`.

### Step 7.4 — Wire `cache_retention` into `build_production_model_gateway`

**File:** `crates/brassclaw_reborn_composition/src/runtime.rs`,
`build_production_model_gateway` helper (look for where
`LlmProviderModelGateway::new` or `RigAdapter::with_cache_retention` is called).

After resolving the value from the DB (or env-var fallback):

```rust
let cache_retention: CacheRetention = resolved_cache_retention
    .as_deref()
    .and_then(|s| s.parse().ok())
    .unwrap_or(CacheRetention::default());   // default = Short (already the crate default)
```

The `RigAdapter` already has `with_cache_retention(retention)`. Wire it here.
This replaces the current env-var-only path.

> **Important:** keep the env-var resolution as a fallback so existing
> deployments that set `ANTHROPIC_CACHE_RETENTION` keep working. Priority
> order: **DB row → env-var → crate default (Short)**.

### Step 7.5 — Hot-swap `cache_retention` on provider change

**File:** `crates/brassclaw_reborn_composition/src/webui.rs`, inside the
`on_provider_changed` callback (the `tokio::spawn` block already there at
line ~161).

Extend the spawn block to also call `reload_handle.set_cache_retention(…)`.
This requires either:

a. Exposing a `set_cache_retention(CacheRetention)` method on
   `brassclaw_llm::LlmReloadHandle`, or
b. Triggering a full `reload()` (which re-resolves the full config including
   cache_retention) after the DB read.

Option (b) is simpler and already correct since `on_provider_changed` fires
**after** the reload. What's missing is that the reload itself needs to read
`cache_retention` from the DB (Step 7.4 wires this). So after Step 7.4 lands,
`reload()` already picks up the new `cache_retention` from the DB. No extra
step needed for the hot-swap — the existing `reload()` call in
`refresh_running_provider` already covers it.

The one gap: the **initial startup** read must pull `cache_retention` from the DB
(Step 7.3), and the hot-swap path (`reload()`) must also pick it up. Both are
handled by wiring `cache_retention` into `resolve_active_provider_token_budgets`
(Step 7.3) and passing it to `build_production_model_gateway` (Step 7.4).

### Step 7.6 — Expose in the Settings UI (WebUI v2)

**File:** `crates/brassclaw_webui_v2/` (React SPA settings component for
Inference → Token Limits, or a new "Prompt Caching" sub-section).

Add a `cache_retention` field to the provider token-settings form:

- Dropdown: `None` / `Short (5 min)` / `Long (1 hr)`
- Helper text: "Anthropic claude-3+ only. Short saves ~90% on repeated prompt
  prefixes. Long extends the cache window to 1 hr for slower-changing system
  prompts."
- The field is hidden/greyed-out for non-Anthropic providers (check
  `provider.protocol !== "anthropic"`).

The field maps to `PUT /api/webchat/v2/providers/{id}/tokens` with
`{ cache_retention: "short" | "long" | "none" }`.

### Step 7.7 — Unit tests

**File:** `crates/brassclaw_llm/src/rig_adapter.rs`, test block (tests already
exist for `cache_control` injection).

Add a round-trip test that verifies the DB string → `CacheRetention::from_str`
→ `RigAdapter.cache_retention` path:

```rust
#[test]
fn cache_retention_parses_from_db_string() {
    assert_eq!("short".parse::<CacheRetention>().unwrap(), CacheRetention::Short);
    assert_eq!("long".parse::<CacheRetention>().unwrap(), CacheRetention::Long);
    assert_eq!("none".parse::<CacheRetention>().unwrap(), CacheRetention::None);
    assert_eq!("off".parse::<CacheRetention>().unwrap(), CacheRetention::None);
    assert_eq!("5m".parse::<CacheRetention>().unwrap(), CacheRetention::Short);
    assert!("invalid".parse::<CacheRetention>().is_err());
}
```

Add to `crates/brassclaw_reborn_composition/src/runtime.rs` tests:

```rust
#[test]
fn resolve_provider_token_budgets_returns_cache_retention_from_db() {
    // Verify that a `cache_retention = "long"` DB row is returned in
    // position 8 of the tuple and is not overridden by a None file value.
    // Use the in-memory test stub for DbTokenSettingsStore.
}
```

---

## Step 8 — Surface cache hit/miss metrics in `LoopModelUsage`

### Why it matters

`cache_read_input_tokens` and `cache_creation_input_tokens` flow through
`CompletionResponse` and `ToolCompletionResponse` all the way to
`HostManagedModelResponse.with_usage(LoopModelUsage { … })` — but `LoopModelUsage`
only carries `input_tokens` and `output_tokens`. Cache hits are invisible to:

- The budget accountant (undercounts cost when cache_creation > 0 at 1.25× rate)
- The `ObservedMessageAverage` EMA (slightly off on cache-heavy conversations)
- The admin UI / telemetry

The fix is minimal: extend `LoopModelUsage` with the cache fields and thread
them through the gateway response path.

### Step 8.1 — Extend `LoopModelUsage`

**File:** `crates/brassclaw_turns/src/run_profile/` (wherever `LoopModelUsage` is defined).

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LoopModelUsage {
    /// Provider-reported prompt tokens (full, before cache discount).
    pub input_tokens: u32,
    /// Provider-reported completion tokens.
    pub output_tokens: u32,
    /// Tokens read from the provider's prompt cache (Anthropic automatic
    /// caching, vLLM prefix cache). These tokens are already included in
    /// `input_tokens` — the split is for cost accounting.
    /// Zero when caching is not active or not reported.
    pub cache_read_input_tokens: u32,
    /// Tokens written to the provider's prompt cache this turn (cache-creation
    /// cost). These are NOT in `input_tokens` — they are charged separately at
    /// the provider's write-surcharge rate.
    /// Zero when no new cache block was created.
    pub cache_creation_input_tokens: u32,
}
```

> **Note:** `input_tokens` already includes the cache-read tokens on Anthropic's
> API. `cache_creation_input_tokens` is billed *in addition to* input_tokens at
> 1.25× or 2.0× depending on TTL. The accountant must handle both correctly
> (see Step 8.3).

### Step 8.2 — Thread cache fields through the gateway response path

**File:** `crates/brassclaw_reborn/src/model_gateway.rs`

In `tool_response_to_host`, the `with_usage(...)` call already uses
`response.cache_read_input_tokens` and `response.cache_creation_input_tokens`
in the struct literal — they just need to land on `LoopModelUsage`:

```rust
// In tool_response_to_host and response_to_host_reply:
.with_usage(LoopModelUsage {
    input_tokens: response.input_tokens,
    output_tokens: response.output_tokens,
    cache_read_input_tokens: response.cache_read_input_tokens,     // new
    cache_creation_input_tokens: response.cache_creation_input_tokens, // new
})
```

Apply the same change in `response_to_host_reply` (text-only path).

All test stubs that construct `LoopModelUsage { input_tokens, output_tokens }`
will need `..Default::default()` appended or the new fields set to 0.

### Step 8.3 — Update the budget accountant cost calculation

**File:** `crates/brassclaw_loop_support/src/` (wherever the accountant computes
spend from `LoopModelUsage` and `ModelCost`).

Current calculation (approximate):

```
spend = input_tokens × input_cost + output_tokens × output_cost
```

Correct calculation with prompt caching:

```
// cache_read tokens were already discounted on the provider side;
// the raw usage.input_tokens figure is the *discounted* count.
// cache_creation tokens are billed at write_surcharge rate on top.
spend =   (input_tokens - cache_read_input_tokens) × input_cost
        + cache_read_input_tokens × (input_cost / cache_read_discount)
        + cache_creation_input_tokens × (input_cost × cache_write_multiplier)
        + output_tokens × output_cost
```

`cache_read_discount` and `cache_write_multiplier` are already exposed on
`LlmProvider` trait (`cache_read_discount()` → 10 for Anthropic, 1 for others;
`cache_write_multiplier()` → 1.25 or 2.0). They need to be threaded into the
accountant.

The cleanest approach: expose them on `ModelCost`:

```rust
pub struct ModelCost {
    pub input_per_token: Decimal,
    pub output_per_token: Decimal,
    pub max_output_tokens: u32,
    /// Discount divisor applied to cache-read tokens (Anthropic: 10 = 90% off).
    /// 1 = no discount (default for non-caching providers).
    pub cache_read_discount: Decimal,
    /// Multiplier applied to cache-creation tokens (Anthropic: 1.25 or 2.0).
    /// 1 = no surcharge (default for non-caching providers).
    pub cache_write_multiplier: Decimal,
}
```

`LlmModelProfilePolicy::build_cost_table` already constructs the table from
the policy; extend it to pull the discount/multiplier from the provider
via the trait methods (Step 8.2 already makes them available).

### Step 8.4 — Emit cache metrics as debug events

**File:** `crates/brassclaw_reborn/src/model_gateway.rs`, `complete_model_request`
tracing instrumentation.

The existing `tracing::debug!` blocks in `complete` and `complete_with_tools`
in `rig_adapter.rs` already log `cache_read`. Extend the model gateway's
`stream_model` / `stream_model_with_capabilities` callers to log at `debug`
level when cache hits occur, following the existing pattern for `input_tokens`
/ `output_tokens`. No `info!` or `warn!` — the REPL constraint applies.

### Step 8.5 — Unit tests

**File:** `crates/brassclaw_reborn/tests/llm_gateway.rs`

The existing tests set `cache_read_input_tokens: 0` and
`cache_creation_input_tokens: 0`. Add one test that sets non-zero values and
asserts they are forwarded to `HostManagedModelResponse.usage()`:

```rust
#[tokio::test]
async fn gateway_forwards_cache_usage_fields() {
    // Stub provider returns cache_read_input_tokens=1000, cache_creation=200.
    // Assert HostManagedModelResponse.usage().cache_read_input_tokens == 1000.
}
```

---

## Step 9 — vLLM / OpenAI-compatible prefix caching: stability guarantee

### Why it matters

vLLM's automatic prefix-caching (APC) works purely by KV-cache key match on
the **prefix** of the token sequence. It requires no API changes. However it
only saves compute when the prefix is stable turn-to-turn. Three patterns break
it silently:

1. **Shuffled tool definitions** — if `FocusedCapabilityStrategy` returns tools
   in a different order each turn, the system-message prefix changes and APC
   misses.
2. **Unstable inline control messages** — if inline loop-control messages are
   prepended *before* the system prompt rather than *after* the history, they
   corrupt the prefix.
3. **Timestamp/nonce injection in system prompts** — any time-varying field in
   the system prompt kills prefix stability.

BrassClaw already orders messages as: `[System]` → `[History...]` → `[Inline
control]`, which is correct for vLLM APC. The main risk is tool-definition
ordering.

### Step 9.1 — Sort tool definitions by name before building the request

**File:** `crates/brassclaw_reborn/src/model_gateway.rs`,
`complete_model_request` (the tool path that builds `ToolCompletionRequest`).

```rust
// Before calling ToolCompletionRequest::from_completion_request:
let mut llm_tool_definitions: Vec<_> = tool_definitions
    .into_iter()
    .map(provider_tool_definition_to_llm)
    .collect();
// Sort by name for stable prefix — required for vLLM/Anthropic prefix cache efficiency.
llm_tool_definitions.sort_unstable_by(|a, b| a.name.cmp(&b.name));
```

`FocusedCapabilityStrategy` already narrows the set, but the remaining tools
must arrive in a stable order. Alphabetical by name is the simplest invariant.

### Step 9.2 — Add invariant test

**File:** `crates/brassclaw_reborn/tests/llm_gateway.rs`

```rust
#[tokio::test]
async fn tool_definitions_are_sorted_by_name_for_cache_stability() {
    // Supply tools in reverse-alpha order to the stub provider.
    // Assert the provider receives them in sorted order.
}
```

### Step 9.3 — Document the ordering contract

Add a doc comment to `complete_model_request` in `model_gateway.rs`:

```rust
/// Tool definitions are sorted by name before building the tool request.
/// This guarantees prefix stability for vLLM/Anthropic automatic prefix
/// caching: the system prompt + tool list form the KV-cache key prefix
/// and must not change across turns for a cache hit to occur.
```

---

## Step 10 — Per-provider `cache_retention` in `ProviderDefinition`

### Why it matters

Operators who add a custom `anthropic` provider to `providers.json` should be
able to set a default `cache_retention` there, alongside other provider defaults
like `context_window_tokens`. Today they can only set `cache_retention` via env
var or the DB. A `providers.json` default is the right ergonomic for initial
setup before the UI is used.

### Step 10.1 — Add `cache_retention` to `ProviderDefinition`

**File:** `crates/brassclaw_llm/src/registry.rs`

```rust
pub struct ProviderDefinition {
    // … existing fields …
    pub context_window_tokens: Option<u32>,
    /// Default prompt-cache retention for this provider.
    /// Only meaningful for Anthropic claude-3+ providers.
    /// When `None`, the resolved value falls through to env-var then crate default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_retention: Option<String>,
}
```

### Step 10.2 — Wire into resolution

**File:** `crates/brassclaw_llm/src/resolution.rs`

After reading `provider_id` from env, look up the `ProviderDefinition` and pull
`cache_retention` as a fallback:

```rust
// Priority: ANTHROPIC_CACHE_RETENTION env > providers.json default > crate default (Short)
let cache_retention = nonempty_env("ANTHROPIC_CACHE_RETENTION")
    .and_then(|s| s.parse::<CacheRetention>().ok())
    .or_else(|| {
        registry_def.cache_retention.as_deref()
            .and_then(|s| s.parse().ok())
    })
    .unwrap_or_default();
config.cache_retention = cache_retention;
```

### Step 10.3 — Expose as build-cost-table input

The `cache_retention` value from the provider definition feeds directly into
`RigAdapter::with_cache_retention` — no new struct field on `ProviderDefinition`
is needed beyond what Step 10.1 adds. The resolution path in Step 10.2
feeds the existing `LlmConfig.cache_retention` field which already reaches
`RigAdapter`.

### Step 10.4 — Unit test

```rust
#[test]
fn provider_definition_cache_retention_used_when_env_absent() {
    // Build a RegistryProviderConfig from a ProviderDefinition with
    // cache_retention = "long" and no env var set.
    // Assert the resolved config.cache_retention == CacheRetention::Long.
}
```

---

## Execution Order

| Order | Step | Rationale |
|-------|------|-----------|
| 1 | Step 6 — Tests 8.3 + live-setter regression | Zero-risk, no behaviour change, immediate regression coverage |
| 2 | Step 5 — Schema cleanup | Prevents operators hitting the silent-ignore trap |
| 3 | Step 9 — Tool sort for vLLM cache stability | Zero-cost prefix-cache win, low-risk one-liner + test |
| 4 | Step 8.1–8.2 — LoopModelUsage cache fields | Required for correct cost accounting; extend LoopModelUsage first |
| 5 | Step 8.3 — Budget accountant cache cost | Requires Step 8.1; fixes silent cost undercount on Anthropic |
| 6 | Step 7.1–7.3 — DB/settings `cache_retention` column | DB migration + settings layer |
| 7 | Step 7.4–7.5 — Wire cache_retention into gateway | Requires Step 7.1; enables per-provider caching at runtime |
| 8 | Step 7.6 — UI exposure | Requires Steps 7.1–7.5 |
| 9 | Step 10 — `providers.json` cache_retention default | Cleanup; can run in parallel with Steps 7–8 |
| 10 | Step 8.4–8.5 — Metrics and tests | Final polish |

Steps 5 and 6 are independently mergeable. Steps 8.1–8.3 must land as a unit
(split would leave the accountant inconsistent). Steps 7.1–7.5 are a single
PR. Step 9 is a one-file change that should ship as early as possible.

---

## Design Invariants to Preserve

- No `.unwrap()` or `.expect()` in production paths.
- `info!` / `warn!` do not appear in hot-path per-turn code (use `debug!`).
- The `live_context_budget` slot remains the single write point;
  `DefaultContextStrategy` reads it atomically — no mutex, no DB read per turn.
- No new fields on `LoopExecutionState`.
- All DB reads happen at startup or on explicit user action (PUT); never during
  a running turn.
- Provider-ID validation (`[a-z0-9_-]{1,64}`) is enforced at the service
  boundary before any DB key is constructed.
- `TokensSection` in `config.toml` must not re-grow budget number fields; the
  DB is the sole runtime source of truth for per-provider limits.
- Tool definitions must always be sorted by name before entering the provider
  call to guarantee KV-cache prefix stability (Step 9 invariant).
- `LoopModelUsage.cache_creation_input_tokens` is **additional** cost, not
  included in `input_tokens`; the accountant formula must not double-count it.
- `cache_retention` priority: DB row > `ANTHROPIC_CACHE_RETENTION` env var >
  `providers.json` definition default > crate default (`Short`).
- Cache settings apply only to Anthropic claude-3+ models; the
  `supports_prompt_cache()` guard in `RigAdapter::with_cache_retention` already
  enforces this — do not bypass it.

---

## Prompt Caching Technology Reference

### Anthropic Automatic Prompt Caching
- **Mechanism:** Set top-level `cache_control: {"type": "ephemeral"}` (Short,
  5-min TTL) or `{"type": "ephemeral", "ttl": "1h"}` (Long) on the request.
  Anthropic places the cache breakpoint at the last cacheable block automatically.
- **Supported models:** claude-3-haiku, claude-3-sonnet, claude-3-5-sonnet,
  claude-3-opus, claude-3-5-haiku, claude-4-sonnet, claude-4-opus and later.
- **Cost:** Write: 1.25× (Short) or 2.0× (Long) input rate.
  Read: 0.1× (10%) input rate.
- **BrassClaw current state:** Fully wired in `RigAdapter.cache_retention`,
  `CacheRetention` enum, `supports_prompt_cache()` guard. Missing: DB/UI
  surface and cost accounting in `LoopModelUsage` (Steps 7 and 8).

### vLLM / OpenAI-compatible Prefix Caching
- **Mechanism:** Fully automatic server-side KV-cache match on request prefix.
  No client-side API changes. Works for any OpenAI-compatible endpoint
  (`openai_compatible` provider in BrassClaw).
- **Requirement:** Stable prompt prefix across turns. BrassClaw message
  ordering is already correct (System → History → Inline). The only risk is
  shuffled tool definitions (Step 9 fixes this).
- **Benefit:** Free for clients, no cost surcharge, typical 20–90% TTFT
  reduction on repeated prefixes.
- **BrassClaw current state:** Transparent — no code changes required beyond
  Step 9 (sort stability).

### OpenClaw / Explicit Cache Blocks
- **Mechanism:** Mark individual content parts with `{"type": "cache"}` to pin
  specific blocks (e.g., a long document, RAG results) independent of the
  automatic prefix.
- **BrassClaw current state:** Not wired. Would require per-message
  `cache_control` injection in `convert_messages` in `rig_adapter.rs`.
  Track as a future TODO once the per-turn tool-result cache (`content_cache`)
  matures.
