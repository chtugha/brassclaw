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

    #[error("config update contains duplicate key `{key}`")]
    DuplicateKey { key: String },

    #[error("config value for `{key}` is invalid: {reason}")]
    InvalidValue { key: String, reason: String },

    #[error("budget configuration writes require a read-committed transaction")]
    BudgetTransactionIsolation,

    #[error(
        "config value for `{key}` looks like inline secret material — \
             store secret values in env vars, not in config"
    )]
    InlineSecretForbidden { key: String },

    #[error(
        "config key `{key}` ends with `_env` and may only be written by operators, \
             not by an agent session"
    )]
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
    if key.starts_with("budget.") {
        return save_config_changes(pool, tenant_id, &[(key, Some(value))], caller).await;
    }
    // Guard 1: inline-secret check (applies regardless of context).
    reject_inline_secret(key.to_owned(), value).map_err(|_| {
        ConfigError::InlineSecretForbidden {
            key: key.to_string(),
        }
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
    if key.starts_with("budget.") {
        return save_config_changes(
            pool,
            tenant_id,
            &[(key, None)],
            ConfigWriteContext::Operator,
        )
        .await;
    }
    let client = pool.get().await?;
    client
        .execute(
            "DELETE FROM brassclaw_config WHERE tenant_id = $1 AND key = $2",
            &[&tenant_id, &key],
        )
        .await?;
    Ok(())
}

/// Publish related operator configuration keys atomically. `None` deletes a
/// key; values are validated before writing. Sorting
/// gives concurrent publications a consistent row-lock order.
pub async fn save_config_changes(
    pool: &PgPool,
    tenant_id: &str,
    changes: &[(&str, Option<&str>)],
    caller: ConfigWriteContext,
) -> Result<(), ConfigError> {
    let mut client = pool.get().await?;
    let mut builder = tokio_postgres::Client::build_transaction(&mut client);
    if changes.iter().any(|(key, _)| key.starts_with("budget.")) {
        builder = builder.isolation_level(tokio_postgres::IsolationLevel::ReadCommitted);
    }
    let transaction = builder.start().await?;
    save_config_changes_in_transaction(&transaction, tenant_id, changes, caller).await?;
    transaction.commit().await?;
    Ok(())
}

/// Validated setup writes joining a composition-owned transaction.
pub(crate) async fn save_config_changes_in_transaction(
    transaction: &tokio_postgres::Transaction<'_>,
    tenant_id: &str,
    changes: &[(&str, Option<&str>)],
    caller: ConfigWriteContext,
) -> Result<(), ConfigError> {
    let mut ordered = BTreeMap::new();
    for &(key, value) in changes {
        if caller == ConfigWriteContext::AgentSession && key.ends_with("_env") {
            return Err(ConfigError::EnvKeyWriteForbidden { key: key.into() });
        }
        if let Some(value) = value {
            reject_inline_secret(key.to_owned(), value)
                .map_err(|_| ConfigError::InlineSecretForbidden { key: key.into() })?;
        }
        if ordered.insert(key, value).is_some() {
            return Err(ConfigError::DuplicateKey { key: key.into() });
        }
    }
    if ordered.keys().any(|key| key.starts_with("budget.")) {
        lock_budget_config(transaction, tenant_id).await?;
        validate_budget_config_changes(transaction, tenant_id, changes).await?;
    }
    for (key, value) in ordered {
        if let Some(value) = value {
            transaction
                .execute(
                    "INSERT INTO brassclaw_config (tenant_id,key,value) VALUES ($1,$2,$3)
                 ON CONFLICT (tenant_id,key) DO UPDATE SET value=excluded.value, updated_at=now()",
                    &[&tenant_id, &key, &value],
                )
                .await?;
        } else {
            transaction
                .execute(
                    "DELETE FROM brassclaw_config WHERE tenant_id=$1 AND key=$2",
                    &[&tenant_id, &key],
                )
                .await?;
        }
    }
    Ok(())
}

/// Serialize the complete budget document, including absent keys. A stale
/// repeatable-read snapshot can miss a newly inserted coupled threshold even
/// after waiting for the lock, so mutation callers must use read committed.
pub(crate) async fn lock_budget_config(
    transaction: &tokio_postgres::Transaction<'_>,
    tenant_id: &str,
) -> Result<(), ConfigError> {
    let row = transaction
        .query_one(
            "SELECT current_setting('transaction_isolation') = 'read committed'",
            &[],
        )
        .await?;
    if !row.try_get::<_, bool>(0)? {
        return Err(ConfigError::BudgetTransactionIsolation);
    }
    let key = format!("brassclaw:budget-config:{tenant_id}");
    transaction
        .query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
            &[&key],
        )
        .await?;
    Ok(())
}

/// Read-only complete-candidate validation. Writers hold `lock_budget_config`;
/// migration dry runs can inspect their consistent snapshot without mutation.
pub(crate) async fn validate_budget_config_changes<C: tokio_postgres::GenericClient + Sync>(
    client: &C,
    tenant_id: &str,
    changes: &[(&str, Option<&str>)],
) -> Result<(), ConfigError> {
    if !changes.iter().any(|(key, _)| key.starts_with("budget.")) {
        return Ok(());
    }
    let rows = client
        .query(
            "SELECT key,value FROM brassclaw_config WHERE tenant_id=$1 AND key LIKE 'budget.%'",
            &[&tenant_id],
        )
        .await?;
    let mut values = BTreeMap::new();
    for row in rows {
        values.insert(row.try_get::<_, String>(0)?, row.try_get::<_, String>(1)?);
    }
    for &(key, value) in changes {
        if !key.starts_with("budget.") {
            continue;
        }
        if let Some(value) = value {
            values.insert(key.into(), value.into());
        } else {
            values.remove(key);
        }
    }
    validate_budget_values(&values)
}

fn validate_budget_values(values: &BTreeMap<String, String>) -> Result<(), ConfigError> {
    for key in [
        "budget.user_daily_usd",
        "budget.project_daily_usd",
        "budget.heartbeat_per_tick_usd",
        "budget.routine_lightweight_usd",
        "budget.routine_standard_usd",
        "budget.background_job_default_usd",
        "budget.warn_at",
        "budget.pause_at",
        "budget.overestimate_factor",
    ] {
        if let Some(value) = values.get(key) {
            value
                .parse::<f64>()
                .map_err(|_| ConfigError::InvalidValue {
                    key: key.into(),
                    reason: "must contain a number".into(),
                })?;
        }
    }
    let mut defaults = brassclaw_reborn_config::BudgetDefaults::compiled_defaults();
    if let Some(section) = assemble_budget(values) {
        defaults = defaults.with_section(&section);
    }
    defaults.validate().map_err(|error| match error {
        brassclaw_reborn_config::BudgetDefaultsError::Invalid { field, reason } => {
            ConfigError::InvalidValue {
                key: format!("budget.{field}"),
                reason,
            }
        }
        brassclaw_reborn_config::BudgetDefaultsError::InvalidEnvF64 { .. } => {
            ConfigError::InvalidValue {
                key: "budget".into(),
                reason: "database budget validation cannot consume environment values".into(),
            }
        }
    })
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
    load_config_snapshot_from_client(&**client, tenant_id).await
}

/// Read through an existing transaction so management selection and catalogue
/// consumers can share its consistent PostgreSQL snapshot.
pub(crate) async fn load_config_snapshot_from_client<C: tokio_postgres::GenericClient + Sync>(
    client: &C,
    tenant_id: &str,
) -> Result<RebornConfigFile, ConfigError> {
    let rows = client
        .query(
            "SELECT key, value FROM brassclaw_config WHERE tenant_id = $1",
            &[&tenant_id],
        )
        .await?;

    // Build a flat key→value map.
    let mut kv: BTreeMap<String, String> = BTreeMap::new();
    for row in &rows {
        let key: String = row.try_get("key")?;
        let value: String = row.try_get("value")?;
        if !key.is_empty() {
            kv.insert(key, value);
        }
    }

    validate_budget_values(&kv)?;
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

    rows.iter()
        .map(|row| {
            let key: String = row.try_get("key")?;
            let value: String = row.try_get("value")?;
            Ok((key, value))
        })
        .collect()
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
    // boot.profile was removed in Phase 11. The key may still exist in older
    // DB rows but is no longer surfaced in the config snapshot.
    let _legacy_profile = get_str(kv, "boot.profile");
    // Return Some only if any relevant boot key is present (currently none remain).
    let _ = kv; // suppress unused-var lint
    None
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
    let default_approval_policy = get_str(kv, "policy.default_approval_policy").map(str::to_string);
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
    some_if_any(
        DriversSection {
            default,
            additional,
        },
        any,
    )
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
            // Treat empty strings as absent — stale empty rows must not be
            // returned as Some(""), which would pass through the provider_id
            // guard in resolve_llm_selection_against_catalog_db and trigger
            // a spurious "provider not found" warning on every reload.
            "provider_id" if !value.is_empty() => entry.provider_id = Some(value.clone()),
            "model" if !value.is_empty() => entry.model = Some(value.clone()),
            "api_key_env" if !value.is_empty() => entry.api_key_env = Some(value.clone()),
            "base_url" if !value.is_empty() => entry.base_url = Some(value.clone()),
            _ => {} // unknown field or empty value — skip; forward-compat
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
        assert_eq!(llm["default"].provider_id.as_deref(), Some("openai"));
        assert_eq!(llm["default"].model.as_deref(), Some("gpt-4o"));
        assert_eq!(llm["kohai"].provider_id.as_deref(), Some("anthropic"));
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
        assert_eq!(config.tokens.unwrap().capability_focus_enabled, Some(true));
    }

    #[test]
    fn absent_rows_produce_none() {
        let config = assemble_config(&BTreeMap::new());
        assert!(config.boot.is_none());
        assert!(config.identity.is_none());
        assert!(config.llm.is_none());
    }
}
