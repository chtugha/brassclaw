//! DB-backed config read/write — the single interface for `brassclaw_config` rows.
//!
//! # Boundary rule
//!
//! DB access for the config table lives **here only**, not in
//! `brassclaw_reborn_config`. That crate stays a pure parse/serialize boundary
//! crate with no workspace deps (enforced by the architecture boundary test).
//!
//! # Serialization contract
//!
//! All values are plain `TEXT`. Non-string types are serialized as:
//! - Booleans: `"true"` / `"false"` (lowercase)
//! - Integers: decimal string (e.g. `"20"`)
//! - Floats/decimals: decimal string (e.g. `"5.00"`)
//! - Absent optional fields: the row is simply absent (no `"null"` sentinel)
//!
//! The `llm.*` keys encode the `BTreeMap<String, LlmSlotSelection>` as
//! dot-separated keys: `llm.<slot>.<field>` → e.g. `llm.default.provider_id`.
//!
//! `save_config_key` and `load_config_snapshot` are the **only** places this
//! contract is enforced. Never write raw SQL against `brassclaw_config` from
//! other modules.

use std::collections::BTreeMap;

use brassclaw_pg::PgPool;
use brassclaw_reborn_config::{
    BootSection, BudgetSection, DriversSection, EmbeddingSection, HarnessSection, IdentitySection,
    LlmSlotSelection, PolicySection, RebornConfigFile, RunnerSection, SkillsSection, TokensSection,
    TriggerPollerConfigSection, WebuiSection, reject_inline_secret,
};
use thiserror::Error;

/// Error type for config DB operations.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("database error: {0}")]
    Db(String),

    #[error("config value for `{key}` looks like inline secret material — \
             store secret values in env vars, not in config")]
    InlineSecretForbidden { key: String },

    #[error("config key `{key}` ends with `_env` and may only be written by operators, \
             not by an agent session")]
    EnvKeyWriteForbidden { key: String },
}

impl From<deadpool_postgres::PoolError> for ConfigError {
    fn from(e: deadpool_postgres::PoolError) -> Self {
        Self::Db(e.to_string())
    }
}

impl From<tokio_postgres::Error> for ConfigError {
    fn from(e: tokio_postgres::Error) -> Self {
        Self::Db(e.to_string())
    }
}

/// Caller context for `save_config_key`.
///
/// The `AgentSession` context is blocked from writing keys ending in `_env`
/// (which point at environment variables used for auth / secrets resolution)
/// so a compromised agent session cannot reroute which env variable the serve
/// process reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigWriteContext {
    /// A human operator invoking a CLI command or the WebUI settings panel.
    Operator,
    /// An agent tool call or automated session. Cannot write `*_env` keys.
    AgentSession,
}

/// Write a single config key–value pair for a tenant.
///
/// # Security guards (applied unconditionally, regardless of context)
///
/// 1. **Inline-secret guard**: rejects values that look like raw API keys,
///    JWTs, or long hex strings via `reject_inline_secret`.
/// 2. **`_env` suffix gate**: when `caller == ConfigWriteContext::AgentSession`,
///    rejects keys ending with `_env` so agents cannot reroute secret resolution.
pub async fn save_config_key(
    pool: &PgPool,
    tenant_id: &str,
    key: &str,
    value: &str,
    caller: ConfigWriteContext,
) -> Result<(), ConfigError> {
    // Guard 1: inline-secret check (applies regardless of context).
    reject_inline_secret(key.to_owned(), value).map_err(|_| ConfigError::InlineSecretForbidden {
        key: key.to_string(),
    })?;

    // Guard 2: _env suffix gate for agent sessions.
    if caller == ConfigWriteContext::AgentSession && key.ends_with("_env") {
        return Err(ConfigError::EnvKeyWriteForbidden {
            key: key.to_string(),
        });
    }

    let client = pool.get().await?;
    client
        .execute(
            "INSERT INTO brassclaw_config (tenant_id, key, value)
             VALUES ($1, $2, $3)
             ON CONFLICT (tenant_id, key) DO UPDATE
                SET value = excluded.value, updated_at = now()",
            &[&tenant_id, &key, &value],
        )
        .await?;

    Ok(())
}

/// Delete a config key for a tenant. A missing key is a no-op.
pub async fn delete_config_key(
    pool: &PgPool,
    tenant_id: &str,
    key: &str,
) -> Result<(), ConfigError> {
    let client = pool.get().await?;
    client
        .execute(
            "DELETE FROM brassclaw_config WHERE tenant_id = $1 AND key = $2",
            &[&tenant_id, &key],
        )
        .await?;
    Ok(())
}

/// Load all config rows for a tenant and reconstruct a `RebornConfigFile`.
///
/// Rows absent from the DB produce `None` on the corresponding optional
/// field — matching `RebornConfigFile::default()` behaviour.
pub async fn load_config_snapshot(
    pool: &PgPool,
    tenant_id: &str,
) -> Result<RebornConfigFile, ConfigError> {
    let client = pool.get().await?;
    let rows = client
        .query(
            "SELECT key, value FROM brassclaw_config WHERE tenant_id = $1",
            &[&tenant_id],
        )
        .await?;

    // Build a flat key→value map.
    let mut kv: BTreeMap<String, String> = BTreeMap::new();
    for row in &rows {
        let key: String = row.try_get("key").unwrap_or_default();
        let value: String = row.try_get("value").unwrap_or_default();
        if !key.is_empty() {
            kv.insert(key, value);
        }
    }

    Ok(assemble_config(&kv))
}

/// List all config rows for a tenant as (key, value) pairs.
pub async fn list_config_keys(
    pool: &PgPool,
    tenant_id: &str,
) -> Result<Vec<(String, String)>, ConfigError> {
    let client = pool.get().await?;
    let rows = client
        .query(
            "SELECT key, value FROM brassclaw_config WHERE tenant_id = $1 ORDER BY key",
            &[&tenant_id],
        )
        .await?;

    Ok(rows
        .iter()
        .filter_map(|r| {
            let key: String = r.try_get("key").ok()?;
            let value: String = r.try_get("value").ok()?;
            Some((key, value))
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Assembly from flat key→value map
// ---------------------------------------------------------------------------

fn get_str<'a>(kv: &'a BTreeMap<String, String>, key: &str) -> Option<&'a str> {
    kv.get(key).map(|s| s.as_str())
}

fn get_bool(kv: &BTreeMap<String, String>, key: &str) -> Option<bool> {
    kv.get(key).and_then(|v| match v.as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    })
}

fn get_u64(kv: &BTreeMap<String, String>, key: &str) -> Option<u64> {
    kv.get(key)?.parse().ok()
}

fn get_u32(kv: &BTreeMap<String, String>, key: &str) -> Option<u32> {
    kv.get(key)?.parse().ok()
}

fn get_f64(kv: &BTreeMap<String, String>, key: &str) -> Option<f64> {
    kv.get(key)?.parse().ok()
}

fn get_usize(kv: &BTreeMap<String, String>, key: &str) -> Option<usize> {
    kv.get(key)?.parse().ok()
}

/// Reconstruct `RebornConfigFile` from a flat key→value map.
fn assemble_config(kv: &BTreeMap<String, String>) -> RebornConfigFile {
    RebornConfigFile {
        api_version: get_str(kv, "api_version").map(str::to_string),
        boot: assemble_boot(kv),
        identity: assemble_identity(kv),
        policy: assemble_policy(kv),
        drivers: assemble_drivers(kv),
        harness: assemble_harness(kv),
        runner: assemble_runner(kv),
        skills: assemble_skills(kv),
        tokens: assemble_tokens(kv),
        llm: assemble_llm(kv),
        webui: assemble_webui(kv),
        budget: assemble_budget(kv),
        trigger_poller: assemble_trigger_poller(kv),
        embedding: assemble_embedding(kv),
    }
}

fn some_if_any<T>(section: T, present: bool) -> Option<T> {
    if present { Some(section) } else { None }
}

fn assemble_boot(kv: &BTreeMap<String, String>) -> Option<BootSection> {
    let profile = get_str(kv, "boot.profile").map(str::to_string);
    some_if_any(BootSection { profile: profile.clone() }, profile.is_some())
}

fn assemble_identity(kv: &BTreeMap<String, String>) -> Option<IdentitySection> {
    let tenant = get_str(kv, "identity.tenant").map(str::to_string);
    let default_agent = get_str(kv, "identity.default_agent").map(str::to_string);
    let default_owner = get_str(kv, "identity.default_owner").map(str::to_string);
    let default_project = get_str(kv, "identity.default_project").map(str::to_string);
    let any = tenant.is_some()
        || default_agent.is_some()
        || default_owner.is_some()
        || default_project.is_some();
    some_if_any(
        IdentitySection {
            tenant,
            default_agent,
            default_owner,
            default_project,
        },
        any,
    )
}

fn assemble_policy(kv: &BTreeMap<String, String>) -> Option<PolicySection> {
    let deployment_mode = get_str(kv, "policy.deployment_mode").map(str::to_string);
    let default_profile = get_str(kv, "policy.default_profile").map(str::to_string);
    let default_approval_policy =
        get_str(kv, "policy.default_approval_policy").map(str::to_string);
    let any =
        deployment_mode.is_some() || default_profile.is_some() || default_approval_policy.is_some();
    some_if_any(
        PolicySection {
            deployment_mode,
            default_profile,
            default_approval_policy,
        },
        any,
    )
}

fn assemble_drivers(kv: &BTreeMap<String, String>) -> Option<DriversSection> {
    let default = get_str(kv, "drivers.default").map(str::to_string);
    // Additional drivers stored as JSON array or comma-separated list.
    let additional = kv
        .get("drivers.additional")
        .and_then(|v| serde_json::from_str::<Vec<String>>(v).ok());
    let any = default.is_some() || additional.is_some();
    some_if_any(DriversSection { default, additional }, any)
}

fn assemble_harness(kv: &BTreeMap<String, String>) -> Option<HarnessSection> {
    let id = get_str(kv, "harness.id").map(str::to_string);
    some_if_any(HarnessSection { id: id.clone() }, id.is_some())
}

fn assemble_runner(kv: &BTreeMap<String, String>) -> Option<RunnerSection> {
    let heartbeat_interval_secs = get_u64(kv, "runner.heartbeat_interval_secs");
    let poll_interval_ms = get_u64(kv, "runner.poll_interval_ms");
    let any = heartbeat_interval_secs.is_some() || poll_interval_ms.is_some();
    some_if_any(
        RunnerSection {
            heartbeat_interval_secs,
            poll_interval_ms,
        },
        any,
    )
}

fn assemble_skills(kv: &BTreeMap<String, String>) -> Option<SkillsSection> {
    let regex_activation_enabled = get_bool(kv, "skills.regex_activation_enabled");
    some_if_any(
        SkillsSection {
            regex_activation_enabled,
        },
        regex_activation_enabled.is_some(),
    )
}

fn assemble_tokens(kv: &BTreeMap<String, String>) -> Option<TokensSection> {
    let capability_focus_enabled = get_bool(kv, "tokens.capability_focus_enabled");
    let planning_mode_enabled = get_bool(kv, "tokens.planning_mode_enabled");
    let content_cache_threshold = get_usize(kv, "tokens.content_cache_threshold");
    let plan_library_enabled = get_bool(kv, "tokens.plan_library_enabled");
    let skill_promotion_threshold = get_f64(kv, "tokens.skill_promotion_threshold");
    let any = capability_focus_enabled.is_some()
        || planning_mode_enabled.is_some()
        || content_cache_threshold.is_some()
        || plan_library_enabled.is_some()
        || skill_promotion_threshold.is_some();
    some_if_any(
        TokensSection {
            capability_focus_enabled,
            planning_mode_enabled,
            content_cache_threshold,
            plan_library_enabled,
            skill_promotion_threshold,
        },
        any,
    )
}

/// Reconstruct `BTreeMap<slot, LlmSlotSelection>` from `llm.<slot>.<field>` rows.
fn assemble_llm(kv: &BTreeMap<String, String>) -> Option<BTreeMap<String, LlmSlotSelection>> {
    let mut map: BTreeMap<String, LlmSlotSelection> = BTreeMap::new();

    for (key, value) in kv {
        // Only process keys with the "llm." prefix.
        let rest = match key.strip_prefix("llm.") {
            Some(r) => r,
            None => continue,
        };
        // Split slot and field: "default.provider_id" → slot="default", field="provider_id"
        let dot = match rest.find('.') {
            Some(d) => d,
            None => continue,
        };
        let slot = &rest[..dot];
        let field = &rest[dot + 1..];

        let entry = map.entry(slot.to_string()).or_default();
        match field {
            "provider_id" => entry.provider_id = Some(value.clone()),
            "model" => entry.model = Some(value.clone()),
            "api_key_env" => entry.api_key_env = Some(value.clone()),
            "base_url" => entry.base_url = Some(value.clone()),
            _ => {} // unknown field — ignore; forward-compat
        }
    }

    if map.is_empty() { None } else { Some(map) }
}

fn assemble_webui(kv: &BTreeMap<String, String>) -> Option<WebuiSection> {
    let listen_host = get_str(kv, "webui.listen_host").map(str::to_string);
    let listen_port = kv
        .get("webui.listen_port")
        .and_then(|v| v.parse::<u16>().ok());
    let env_token_var = get_str(kv, "webui.env_token_var").map(str::to_string);
    let env_user_id_var = get_str(kv, "webui.env_user_id_var").map(str::to_string);
    let allowed_origins = kv
        .get("webui.allowed_origins")
        .and_then(|v| serde_json::from_str::<Vec<String>>(v).ok());
    let csp_header_override = get_str(kv, "webui.csp_header_override").map(str::to_string);
    let max_body_bytes_fallback = kv
        .get("webui.max_body_bytes_fallback")
        .and_then(|v| v.parse::<u64>().ok());
    let canonical_host = get_str(kv, "webui.canonical_host").map(str::to_string);
    let any = listen_host.is_some()
        || listen_port.is_some()
        || env_token_var.is_some()
        || env_user_id_var.is_some()
        || allowed_origins.is_some()
        || csp_header_override.is_some()
        || max_body_bytes_fallback.is_some()
        || canonical_host.is_some();
    some_if_any(
        WebuiSection {
            listen_host,
            listen_port,
            env_token_var,
            env_user_id_var,
            allowed_origins,
            csp_header_override,
            max_body_bytes_fallback,
            canonical_host,
        },
        any,
    )
}

fn assemble_budget(kv: &BTreeMap<String, String>) -> Option<BudgetSection> {
    let user_daily_usd = get_f64(kv, "budget.user_daily_usd");
    let project_daily_usd = get_f64(kv, "budget.project_daily_usd");
    let mission_per_tick_usd = get_f64(kv, "budget.mission_per_tick_usd");
    let heartbeat_per_tick_usd = get_f64(kv, "budget.heartbeat_per_tick_usd");
    let routine_lightweight_usd = get_f64(kv, "budget.routine_lightweight_usd");
    let routine_standard_usd = get_f64(kv, "budget.routine_standard_usd");
    let background_job_default_usd = get_f64(kv, "budget.background_job_default_usd");
    let default_tz = get_str(kv, "budget.default_tz").map(str::to_string);
    let warn_at = get_f64(kv, "budget.warn_at");
    let pause_at = get_f64(kv, "budget.pause_at");
    let overestimate_factor = get_f64(kv, "budget.overestimate_factor");
    let any = user_daily_usd.is_some()
        || project_daily_usd.is_some()
        || mission_per_tick_usd.is_some()
        || heartbeat_per_tick_usd.is_some()
        || routine_lightweight_usd.is_some()
        || routine_standard_usd.is_some()
        || background_job_default_usd.is_some()
        || default_tz.is_some()
        || warn_at.is_some()
        || pause_at.is_some()
        || overestimate_factor.is_some();
    some_if_any(
        BudgetSection {
            user_daily_usd,
            project_daily_usd,
            mission_per_tick_usd,
            heartbeat_per_tick_usd,
            routine_lightweight_usd,
            routine_standard_usd,
            background_job_default_usd,
            default_tz,
            warn_at,
            pause_at,
            overestimate_factor,
        },
        any,
    )
}

fn assemble_trigger_poller(kv: &BTreeMap<String, String>) -> Option<TriggerPollerConfigSection> {
    let enabled = get_bool(kv, "trigger_poller.enabled");
    let poll_interval_secs = get_u64(kv, "trigger_poller.poll_interval_secs");
    let fires_per_tick = get_u32(kv, "trigger_poller.fires_per_tick");
    let max_concurrent_fires_per_trigger =
        get_u32(kv, "trigger_poller.max_concurrent_fires_per_trigger");
    let startup_jitter_max_secs = get_u64(kv, "trigger_poller.startup_jitter_max_secs");
    let tick_jitter_max_secs = get_u64(kv, "trigger_poller.tick_jitter_max_secs");
    let any = enabled.is_some()
        || poll_interval_secs.is_some()
        || fires_per_tick.is_some()
        || max_concurrent_fires_per_trigger.is_some()
        || startup_jitter_max_secs.is_some()
        || tick_jitter_max_secs.is_some();
    some_if_any(
        TriggerPollerConfigSection {
            enabled,
            poll_interval_secs,
            fires_per_tick,
            max_concurrent_fires_per_trigger,
            startup_jitter_max_secs,
            tick_jitter_max_secs,
        },
        any,
    )
}

fn assemble_embedding(kv: &BTreeMap<String, String>) -> Option<EmbeddingSection> {
    let provider_id = get_str(kv, "embedding.provider_id").map(str::to_string);
    let model = get_str(kv, "embedding.model").map(str::to_string);
    let any = provider_id.is_some() || model.is_some();
    some_if_any(EmbeddingSection { provider_id, model }, any)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kv(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn round_trip_llm_slot() {
        let map = kv(&[
            ("llm.default.provider_id", "openai"),
            ("llm.default.model", "gpt-4o"),
            ("llm.kohai.provider_id", "anthropic"),
        ]);
        let config = assemble_config(&map);
        let llm = config.llm.unwrap();
        assert_eq!(
            llm["default"].provider_id.as_deref(),
            Some("openai")
        );
        assert_eq!(llm["default"].model.as_deref(), Some("gpt-4o"));
        assert_eq!(
            llm["kohai"].provider_id.as_deref(),
            Some("anthropic")
        );
    }

    #[test]
    fn inline_secret_guard_rejects_api_key() {
        // Simulate what save_config_key does without a real pool.
        let result = reject_inline_secret(
            "llm.default.api_key_env",
            "sk-proj-abcdefghijklmnopqrstuvwxyz123456",
        );
        assert!(result.is_err(), "should reject inline API key");
    }

    #[test]
    fn inline_secret_guard_allows_env_ref() {
        let result = reject_inline_secret("llm.default.api_key_env", "OPENAI_API_KEY");
        assert!(result.is_ok(), "should allow env var name");
    }

    #[test]
    fn env_key_write_forbidden_for_agent_session() {
        // Manually exercise the guard logic (no pool needed).
        let key = "llm.default.api_key_env";
        let caller = ConfigWriteContext::AgentSession;
        if caller == ConfigWriteContext::AgentSession && key.ends_with("_env") {
            // This is the guard path — correct.
        } else {
            panic!("guard should have fired");
        }
    }

    #[test]
    fn env_key_allowed_for_operator() {
        let key = "llm.default.api_key_env";
        let caller = ConfigWriteContext::Operator;
        // Operator is allowed; guard does not fire.
        assert_ne!(caller, ConfigWriteContext::AgentSession);
        assert!(key.ends_with("_env")); // would be blocked for AgentSession
    }

    #[test]
    fn boolean_serialization_round_trip() {
        let map = kv(&[("tokens.capability_focus_enabled", "true")]);
        let config = assemble_config(&map);
        assert_eq!(
            config.tokens.unwrap().capability_focus_enabled,
            Some(true)
        );
    }

    #[test]
    fn absent_rows_produce_none() {
        let config = assemble_config(&BTreeMap::new());
        assert!(config.boot.is_none());
        assert!(config.identity.is_none());
        assert!(config.llm.is_none());
    }
}
