//! libSQL → Postgres data migration module (§8.1 steps 3–8).
//!
//! Gated behind the `migrate-from-libsql` feature. This entire module is
//! compiled ONLY when that feature is active; it is removed together with the
//! feature in the release after the upgrade cycle completes.
//!
//! # What this module does
//!
//! The explicit upgrade caller can run these steps. This module is not wired
//! into ordinary startup; merely compiling it does not migrate an installation.
//!
//! 1. (Steps 1-2 are handled by the caller: start embedded PG + run schema
//!    migrations.)
//! 2. **Step 3** — migrate `config.toml` → `brassclaw_config` rows.
//! 3. **Step 4** — migrate `providers.json` → `brassclaw_llm_providers`.
//! 4. **Step 5** — migrate `sempai_provider.json` → `brassclaw_config`.
//! 5. **Step 6** — migrate secrets master key.
//! 6. **Step 7** — migrate `reborn-local-dev.db` → all relevant PG tables.
//! 7. **Step 8** — set `boot.initialized = true` if any artifact was found.
//!
//! Steps 9–10 are unconditional (chat-memory path, embedding role config) and
//! handled implicitly by the production wiring — not by this module.

#![cfg(feature = "migrate-from-libsql")]

use std::path::Path;

use brassclaw_pg::PgPool;
use brassclaw_reborn_config::{RebornConfigFile, RebornHome};
use thiserror::Error;

use crate::db_config::{ConfigWriteContext, save_config_key};

/// Result of running the migration.
#[derive(Debug, Default)]
pub struct MigrationReport {
    /// True if `config.toml` was found and migrated.
    pub config_migrated: bool,
    /// True if `providers.json` was found and migrated.
    pub providers_migrated: bool,
    /// True if `sempai_provider.json` was found and migrated.
    pub sempai_migrated: bool,
    /// True if the secrets master key was migrated / row upserted.
    pub secrets_master_migrated: bool,
    /// True if `reborn-local-dev.db` was found and migrated.
    pub libsql_db_migrated: bool,
    /// Legacy runnable triggers paused until schedule/effect reconciliation.
    pub paused_legacy_triggers: usize,
    /// True if `boot.initialized` was set (≥ one artifact was migrated).
    pub boot_initialized_set: bool,
}

impl MigrationReport {
    /// True if at least one step found a source artifact and migrated it.
    pub fn any_migrated(&self) -> bool {
        self.config_migrated
            || self.providers_migrated
            || self.sempai_migrated
            || self.secrets_master_migrated
            || self.libsql_db_migrated
    }
}

/// Error type for migration operations.
#[derive(Debug, Error)]
pub enum MigrationError {
    #[error("database error during migration: {reason}")]
    Db { reason: String },

    #[error("config parse error: {reason}")]
    ConfigParse { reason: String },

    #[error("providers parse error: {reason}")]
    ProvidersParse { reason: String },

    #[error("file I/O error: {reason}")]
    Io { reason: String },

    #[error(
        "passphrase ceremony detected: run \
         'brassclaw secrets rewrap --strategy passphrase-file=<path>' before starting"
    )]
    PassphraseRewrapRequired,
}

impl From<deadpool_postgres::PoolError> for MigrationError {
    fn from(e: deadpool_postgres::PoolError) -> Self {
        Self::Db {
            reason: e.to_string(),
        }
    }
}

impl From<tokio_postgres::Error> for MigrationError {
    fn from(e: tokio_postgres::Error) -> Self {
        Self::Db {
            reason: e.to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run the full migration sequence (§8.1 steps 3–8) against `pool`.
///
/// `dry_run = true` runs steps 3–8 in read-only simulation: no DB writes, no
/// file renames. The returned report reflects what *would* happen.
///
/// `home` must be the resolved `RebornHome` for the process.
pub async fn run_migration(
    pool: &PgPool,
    home: &RebornHome,
    tenant_id: &str,
    dry_run: bool,
) -> Result<MigrationReport, MigrationError> {
    let config_migrated = step3_migrate_config(pool, home, tenant_id, dry_run).await?;
    let providers_migrated = step4_migrate_providers(pool, home, tenant_id, dry_run).await?;
    let sempai_migrated = step5_migrate_sempai(pool, home, tenant_id, dry_run).await?;
    let secrets_master_migrated =
        step6_migrate_secrets_master(pool, home, tenant_id, dry_run).await?;
    let (libsql_db_migrated, paused_legacy_triggers) =
        step7_migrate_libsql_db(pool, home, tenant_id, dry_run).await?;

    let any_migrated = config_migrated
        || providers_migrated
        || sempai_migrated
        || secrets_master_migrated
        || libsql_db_migrated;

    // Step 8 — set boot.initialized if any artifact was migrated
    if any_migrated && !dry_run {
        save_config_key(
            pool,
            tenant_id,
            "boot.initialized",
            "true",
            ConfigWriteContext::Operator,
        )
        .await
        .map_err(|e| MigrationError::Db {
            reason: e.to_string(),
        })?;
        return Ok(MigrationReport {
            config_migrated,
            providers_migrated,
            sempai_migrated,
            secrets_master_migrated,
            libsql_db_migrated,
            paused_legacy_triggers,
            boot_initialized_set: true,
        });
    }

    Ok(MigrationReport {
        config_migrated,
        providers_migrated,
        sempai_migrated,
        secrets_master_migrated,
        libsql_db_migrated,
        paused_legacy_triggers,
        boot_initialized_set: false,
    })
}

// ---------------------------------------------------------------------------
// Step 3: migrate config.toml
// ---------------------------------------------------------------------------

async fn step3_migrate_config(
    pool: &PgPool,
    home: &RebornHome,
    tenant_id: &str,
    dry_run: bool,
) -> Result<bool, MigrationError> {
    let config_path = home.path().join("config.toml");
    let Some(bytes) = read_import_source(&config_path)? else {
        return Ok(false);
    };
    let text = std::str::from_utf8(&bytes).map_err(|_| MigrationError::ConfigParse {
        reason: "legacy configuration is not UTF-8".into(),
    })?;
    let config = RebornConfigFile::parse_text(text, &config_path).map_err(|_| {
        MigrationError::ConfigParse {
            reason: "legacy configuration did not pass typed validation".into(),
        }
    })?;
    let rows = config_import_rows(&config)?;
    import_settings_snapshot(pool, tenant_id, &config_path, &bytes, &rows, dry_run).await?;

    Ok(true)
}

/// Translate a `RebornConfigFile` into flat `(key, value)` rows and upsert
/// them into a complete settings snapshot. Atomic validation/publication is
/// handled by the shared import boundary below.
fn config_import_rows(config: &RebornConfigFile) -> Result<Vec<(String, String)>, MigrationError> {
    let mut rows: Vec<(String, String)> = Vec::new();

    if let Some(v) = &config.api_version {
        rows.push(("api_version".to_string(), v.clone()));
    }
    if let Some(id) = &config.identity {
        if let Some(v) = &id.tenant {
            rows.push(("identity.tenant".to_string(), v.clone()));
        }
        if let Some(v) = &id.default_agent {
            rows.push(("identity.default_agent".to_string(), v.clone()));
        }
        if let Some(v) = &id.default_owner {
            rows.push(("identity.default_owner".to_string(), v.clone()));
        }
        if let Some(v) = &id.default_project {
            rows.push(("identity.default_project".to_string(), v.clone()));
        }
    }
    if let Some(p) = &config.policy {
        if let Some(v) = &p.deployment_mode {
            rows.push(("policy.deployment_mode".to_string(), v.clone()));
        }
        if let Some(v) = &p.default_profile {
            rows.push(("policy.default_profile".to_string(), v.clone()));
        }
        if let Some(v) = &p.default_approval_policy {
            rows.push(("policy.default_approval_policy".to_string(), v.clone()));
        }
    }
    if let Some(d) = &config.drivers {
        if let Some(v) = &d.default {
            rows.push(("drivers.default".to_string(), v.clone()));
        }
        if let Some(v) = &d.additional {
            rows.push((
                "drivers.additional".to_string(),
                serde_json::to_string(v).map_err(|_| MigrationError::ConfigParse {
                    reason: "legacy driver list cannot be encoded".into(),
                })?,
            ));
        }
    }
    if let Some(h) = &config.harness
        && let Some(v) = &h.id
    {
        rows.push(("harness.id".to_string(), v.clone()));
    }
    if let Some(r) = &config.runner {
        if let Some(v) = r.heartbeat_interval_secs {
            rows.push(("runner.heartbeat_interval_secs".to_string(), v.to_string()));
        }
        if let Some(v) = r.poll_interval_ms {
            rows.push(("runner.poll_interval_ms".to_string(), v.to_string()));
        }
    }
    if let Some(s) = &config.skills
        && let Some(v) = s.regex_activation_enabled
    {
        rows.push(("skills.regex_activation_enabled".to_string(), v.to_string()));
    }
    if let Some(t) = &config.tokens {
        if let Some(v) = t.capability_focus_enabled {
            rows.push(("tokens.capability_focus_enabled".to_string(), v.to_string()));
        }
        if let Some(v) = t.planning_mode_enabled {
            rows.push(("tokens.planning_mode_enabled".to_string(), v.to_string()));
        }
    }
    // LLM slots: llm.<slot>.provider_id, .model, .api_key_env, .base_url
    if let Some(llm) = &config.llm {
        for (slot, sel) in llm {
            if let Some(v) = &sel.provider_id {
                rows.push((format!("llm.{slot}.provider_id"), v.clone()));
            }
            if let Some(v) = &sel.model {
                rows.push((format!("llm.{slot}.model"), v.clone()));
            }
            if let Some(v) = &sel.api_key_env {
                rows.push((format!("llm.{slot}.api_key_env"), v.clone()));
            }
            if let Some(v) = &sel.base_url {
                rows.push((format!("llm.{slot}.base_url"), v.clone()));
            }
        }
    }
    if let Some(w) = &config.webui {
        if let Some(v) = w.listen_port {
            rows.push(("webui.listen_port".to_string(), v.to_string()));
        }
        if let Some(v) = &w.listen_host {
            rows.push(("webui.listen_host".to_string(), v.clone()));
        }
        if let Some(v) = &w.env_token_var {
            rows.push(("webui.env_token_var".to_string(), v.clone()));
        }
        if let Some(v) = &w.env_user_id_var {
            rows.push(("webui.env_user_id_var".to_string(), v.clone()));
        }
    }
    if let Some(b) = &config.budget {
        for (key, value) in [
            ("budget.user_daily_usd", b.user_daily_usd),
            ("budget.project_daily_usd", b.project_daily_usd),
            ("budget.heartbeat_per_tick_usd", b.heartbeat_per_tick_usd),
            ("budget.routine_lightweight_usd", b.routine_lightweight_usd),
            ("budget.routine_standard_usd", b.routine_standard_usd),
            (
                "budget.background_job_default_usd",
                b.background_job_default_usd,
            ),
            ("budget.warn_at", b.warn_at),
            ("budget.pause_at", b.pause_at),
            ("budget.overestimate_factor", b.overestimate_factor),
        ] {
            if let Some(value) = value {
                rows.push((key.into(), value.to_string()));
            }
        }
        if let Some(v) = &b.default_tz {
            rows.push(("budget.default_tz".to_string(), v.clone()));
        }
    }
    if let Some(tp) = &config.trigger_poller {
        if let Some(v) = tp.enabled {
            rows.push(("trigger_poller.enabled".to_string(), v.to_string()));
        }
        if let Some(v) = tp.poll_interval_secs {
            rows.push((
                "trigger_poller.poll_interval_secs".to_string(),
                v.to_string(),
            ));
        }
        if let Some(v) = tp.fires_per_tick {
            rows.push(("trigger_poller.fires_per_tick".to_string(), v.to_string()));
        }
    }
    if let Some(e) = &config.embedding {
        if let Some(v) = &e.provider_id {
            rows.push(("embedding.provider_id".to_string(), v.clone()));
        }
        if let Some(v) = &e.model {
            rows.push(("embedding.model".to_string(), v.clone()));
        }
    }

    Ok(rows)
}

// ---------------------------------------------------------------------------
// Step 4: migrate providers.json
// ---------------------------------------------------------------------------

async fn step4_migrate_providers(
    pool: &PgPool,
    home: &RebornHome,
    tenant_id: &str,
    dry_run: bool,
) -> Result<bool, MigrationError> {
    let source = home.path().join("providers.json");
    if !source.try_exists().map_err(|_| MigrationError::Io {
        reason: "legacy provider inventory cannot be inspected".into(),
    })? {
        return Ok(false);
    }
    #[cfg(not(feature = "root-llm-provider"))]
    {
        let _ = (pool, tenant_id, dry_run);
        Err(MigrationError::ProvidersParse {
            reason: "legacy provider import requires the supported provider-definition validator"
                .into(),
        })
    }
    #[cfg(feature = "root-llm-provider")]
    {
        let bytes = std::fs::read(&source).map_err(|_| MigrationError::Io {
            reason: "legacy provider inventory cannot be read".into(),
        })?;
        let providers: Vec<brassclaw_llm::ProviderDefinition> = serde_json::from_slice(&bytes)
            .map_err(|_| MigrationError::ProvidersParse {
                reason:
                    "legacy provider inventory does not match the supported definition contract"
                        .into(),
            })?;
        let registry = brassclaw_llm::ProviderRegistry::new(providers.clone());
        crate::llm_catalog::validate_catalog(&registry).map_err(|_| {
            MigrationError::ProvidersParse {
                reason: "legacy provider inventory is invalid".into(),
            }
        })?;
        let mut identities = std::collections::BTreeMap::new();
        let mut declared = std::collections::BTreeSet::new();
        for provider in &providers {
            if !declared.insert(&provider.id) {
                return Err(MigrationError::ProvidersParse {
                    reason: "legacy provider inventory contains duplicate definitions".into(),
                });
            }
            for name in std::iter::once(&provider.id).chain(&provider.aliases) {
                if name.trim().is_empty()
                    || identities
                        .insert(name.to_lowercase(), &provider.id)
                        .is_some_and(|previous| previous != &provider.id)
                {
                    return Err(MigrationError::ProvidersParse {
                        reason: "legacy provider inventory contains conflicting identities".into(),
                    });
                }
            }
        }
        let mut client = pool.get().await?;
        let transaction = tokio_postgres::Client::build_transaction(&mut client)
            .read_only(dry_run)
            .start()
            .await?;
        if !dry_run {
            transaction
                .batch_execute("LOCK TABLE brassclaw_llm_providers IN SHARE ROW EXCLUSIVE MODE")
                .await?;
        }
        let mut pending = Vec::new();
        for provider in &providers {
            let row = transaction
                .query_opt(
                    "SELECT definition FROM brassclaw_llm_providers WHERE tenant_id=$1 AND id=$2",
                    &[&tenant_id, &provider.id],
                )
                .await?;
            if let Some(row) = row {
                let retained: brassclaw_llm::ProviderDefinition =
                    serde_json::from_value(row.try_get("definition")?).map_err(|_| {
                        MigrationError::ProvidersParse {
                            reason: "stored provider definition requires repair before upgrade"
                                .into(),
                        }
                    })?;
                if serde_json::to_value(&retained).map_err(|_| MigrationError::ProvidersParse {
                    reason: "stored provider definition cannot be encoded".into(),
                })? != serde_json::to_value(provider).map_err(|_| {
                    MigrationError::ProvidersParse {
                        reason: "legacy provider definition cannot be encoded".into(),
                    }
                })? {
                    return Err(MigrationError::ProvidersParse { reason: "legacy provider inventory conflicts with retained database definitions; neither source was replaced".into() });
                }
                // Matching data never resets origin, revision or a tombstone.
            } else {
                pending.push(provider);
            }
        }
        let existing=transaction.query("SELECT id,definition FROM brassclaw_llm_providers WHERE tenant_id=$1 AND deleted_at IS NULL", &[&tenant_id]).await?;
        let mut active = std::collections::BTreeMap::new();
        let mut active_definitions = Vec::new();
        for row in existing {
            let definition: brassclaw_llm::ProviderDefinition =
                serde_json::from_value(row.try_get("definition")?).map_err(|_| {
                    MigrationError::ProvidersParse {
                        reason: "stored active provider catalogue is invalid".into(),
                    }
                })?;
            if row.try_get::<_, String>("id")? != definition.id {
                return Err(MigrationError::ProvidersParse {
                    reason: "stored active provider identity differs from its row".into(),
                });
            }
            for name in std::iter::once(&definition.id).chain(&definition.aliases) {
                if active
                    .insert(name.to_lowercase(), definition.id.clone())
                    .is_some_and(|previous| previous != definition.id)
                {
                    return Err(MigrationError::ProvidersParse {
                        reason: "stored active provider identities conflict".into(),
                    });
                }
            }
            active_definitions.push(definition);
        }
        crate::llm_catalog::validate_catalog(&brassclaw_llm::ProviderRegistry::new(
            active_definitions,
        ))
        .map_err(|_| MigrationError::ProvidersParse {
            reason: "stored active provider catalogue requires repair before upgrade".into(),
        })?;
        for provider in &pending {
            for name in std::iter::once(&provider.id).chain(&provider.aliases) {
                if active
                    .insert(name.to_lowercase(), provider.id.clone())
                    .is_some_and(|previous| previous != provider.id)
                {
                    return Err(MigrationError::ProvidersParse {
                        reason:
                            "legacy provider identities conflict with the active database catalogue"
                                .into(),
                    });
                }
            }
        }
        if dry_run {
            transaction.rollback().await?;
            return Ok(true);
        }
        retain_import_backup(&source, &bytes)?;
        for provider in pending {
            let definition =
                serde_json::to_value(provider).map_err(|_| MigrationError::ProvidersParse {
                    reason: "legacy provider definition cannot be encoded".into(),
                })?;
            transaction
                .execute(
                    "INSERT INTO brassclaw_llm_providers(tenant_id,id,definition) VALUES($1,$2,$3)",
                    &[&tenant_id, &provider.id, &definition],
                )
                .await?;
        }
        transaction.commit().await?;
        if std::fs::read(&source).map_err(|_| MigrationError::Io {
            reason:
                "provider import committed, but its source could not be verified; backup retained"
                    .into(),
        })? != bytes
        {
            return Err(MigrationError::ProvidersParse {reason:"provider import committed, but the source changed; both source and backup retained".into()});
        }
        // Keep the original as an upgrade artifact. A portable read-then-unlink
        // cannot fence a simultaneous file replacement; never delete that newer
        // file. The final controlled upgrade retires the importer/readers.
        Ok(true)
    }
}

// ---------------------------------------------------------------------------
// Step 5: migrate sempai_provider.json
// ---------------------------------------------------------------------------

async fn step5_migrate_sempai(
    pool: &PgPool,
    home: &RebornHome,
    tenant_id: &str,
    dry_run: bool,
) -> Result<bool, MigrationError> {
    let sempai_path = home.path().join("sempai_provider.json");
    let Some(bytes) = read_import_source(&sempai_path)? else {
        return Ok(false);
    };
    let selection: brassclaw_reborn_config::LlmSlotSelection = serde_json::from_slice(&bytes)
        .map_err(|_| MigrationError::ProvidersParse {
            reason: "legacy Sempai selection did not pass typed validation".into(),
        })?;
    let mut rows = Vec::new();
    if let Some(provider) = selection.provider_id {
        rows.push(("llm.sempai.provider_id".into(), provider));
    }
    if let Some(model) = selection.model {
        rows.push(("llm.sempai.model".into(), model));
    }
    if let Some(name) = selection.api_key_env {
        rows.push(("llm.sempai.api_key_env".into(), name));
    }
    if let Some(url) = selection.base_url {
        rows.push(("llm.sempai.base_url".into(), url));
    }
    import_settings_snapshot(pool, tenant_id, &sempai_path, &bytes, &rows, dry_run).await?;

    Ok(true)
}

// ---------------------------------------------------------------------------
// Step 6: migrate secrets master key
// ---------------------------------------------------------------------------

async fn step6_migrate_secrets_master(
    pool: &PgPool,
    home: &RebornHome,
    tenant_id: &str,
    dry_run: bool,
) -> Result<bool, MigrationError> {
    let legacy = home.path().join(".reborn-local-dev-secrets-master-key");
    match std::fs::symlink_metadata(&legacy) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => {
            return Err(MigrationError::Io {
                reason: "legacy master source cannot be inspected".into(),
            });
        }
    }
    let canonical = home.path().join(".secrets-master-key");
    let mut client = pool.get().await?;
    let transaction = tokio_postgres::Client::build_transaction(&mut client)
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(dry_run)
        .start()
        .await?;
    if !dry_run {
        transaction
            .batch_execute("LOCK TABLE brassclaw_secrets_master IN SHARE ROW EXCLUSIVE MODE")
            .await?;
    }
    let row = transaction.query_opt("SELECT version,algorithm,wrapped_key FROM brassclaw_secrets_master WHERE tenant_id=$1 ORDER BY version DESC LIMIT 1", &[&tenant_id]).await?;
    let requested_wrap =
        std::env::var("BRASSCLAW_SECRETS_PASSPHRASE_FILE").is_ok_and(|value| !value.is_empty());
    if let Some(row) = &row {
        let version: i32 = row.try_get("version")?;
        let algorithm: String = row.try_get("algorithm")?;
        let wrapped: String = row.try_get("wrapped_key")?;
        if version <= 0 {
            return Err(MigrationError::ConfigParse {
                reason: "retained master-key version is invalid; nothing was changed".into(),
            });
        }
        if algorithm == "aes256gcm-argon2id" && !wrapped.is_empty() {
            // An established wrapped ceremony wins over stale raw files. Never
            // copy the old file or reinterpret any stored wrapping material.
            transaction.rollback().await?;
            return Ok(false);
        }
        if algorithm != "raw-key-on-disk" || !wrapped.is_empty() {
            return Err(MigrationError::ConfigParse {
                reason: "retained master-key ceremony requires repair; nothing was changed".into(),
            });
        }
    }
    if requested_wrap {
        return Err(MigrationError::PassphraseRewrapRequired);
    }
    let Some(key) =
        crate::secrets_master::read_raw_master_key(&legacy).map_err(|_| MigrationError::Io {
            reason: "legacy master key did not pass format/identity validation".into(),
        })?
    else {
        return Ok(false);
    };
    if let Some(existing) =
        crate::secrets_master::read_raw_master_key(&canonical).map_err(|_| MigrationError::Io {
            reason: "canonical master key did not pass validation; nothing was changed".into(),
        })?
        && existing != key
    {
        return Err(MigrationError::ConfigParse {
            reason: "canonical and legacy master keys differ; neither was replaced".into(),
        });
    }
    if dry_run {
        transaction.rollback().await?;
        return Ok(true);
    }
    crate::secrets_master::write_raw_master_key_if_absent(&canonical, &key).map_err(|_| {
        MigrationError::Io {
            reason: "canonical master key could not be published without replacement".into(),
        }
    })?;
    if row.is_none() {
        transaction.execute("INSERT INTO brassclaw_secrets_master(tenant_id,version,wrapped_key,algorithm) VALUES($1,1,'','raw-key-on-disk')", &[&tenant_id]).await?;
    }
    transaction.commit().await?;
    if crate::secrets_master::read_raw_master_key(&legacy)
        .map_err(|_| MigrationError::Io {
            reason:
                "master import committed, but its source could not be verified; both files retained"
                    .into(),
        })?
        .as_deref()
        != Some(key.as_slice())
    {
        return Err(MigrationError::Io {
            reason: "master import committed, but its source changed; both files retained".into(),
        });
    }
    // No portable conditional pathname deletion exists here. Retain the
    // private source instead of overwriting/unlinking a possible replacement.

    Ok(true)
}

// ---------------------------------------------------------------------------
// Step 7: migrate reborn-local-dev.db
// ---------------------------------------------------------------------------

async fn step7_migrate_libsql_db(
    pool: &PgPool,
    home: &RebornHome,
    tenant_id: &str,
    dry_run: bool,
) -> Result<(bool, usize), MigrationError> {
    let db_path = home.path().join("reborn-local-dev.db");
    match std::fs::symlink_metadata(&db_path) {
        Ok(metadata) if metadata.file_type().is_file() => {}
        Ok(_) => {
            return Err(MigrationError::Io {
                reason: "legacy database must be a regular file".into(),
            });
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok((false, 0)),
        Err(_) => {
            return Err(MigrationError::Io {
                reason: "legacy database cannot be inspected".into(),
            });
        }
    }
    let (boot_tenant, boot_user) = resolve_boot_identity(pool, tenant_id).await?;
    let database = libsql::Builder::new_local(&db_path)
        .flags(libsql::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .build()
        .await
        .map_err(|_| MigrationError::Db {
            reason: "legacy database cannot be opened read-only".into(),
        })?;
    let source = database.connect().map_err(|_| MigrationError::Db {
        reason: "legacy database connection failed".into(),
    })?;
    // SQLite creates one consistent logical copy, including committed WAL data.
    // A raw filesystem copy of only the main file cannot establish that state.
    let directory = tempfile::Builder::new()
        .prefix(".brassclaw-import-")
        .tempdir_in(home.path())
        .map_err(|_| MigrationError::Io {
            reason: "legacy snapshot cannot be staged".into(),
        })?;
    let snapshot_path = directory.path().join("snapshot.db");
    let destination = snapshot_path.to_str().ok_or_else(|| MigrationError::Io {
        reason: "legacy snapshot path cannot be represented for SQLite".into(),
    })?;
    source
        .execute("VACUUM INTO ?1", libsql::params![destination])
        .await
        .map_err(|_| MigrationError::Db {
            reason: "legacy database could not produce a consistent snapshot".into(),
        })?;
    std::fs::File::options()
        .read(true)
        .write(true)
        .open(&snapshot_path)
        .and_then(|file| file.sync_all())
        .map_err(|_| MigrationError::Io {
            reason: "legacy snapshot could not be synchronized".into(),
        })?;
    drop(source);
    drop(database);
    let snapshot = libsql::Builder::new_local(&snapshot_path)
        .flags(libsql::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .build()
        .await
        .map_err(|_| MigrationError::Db {
            reason: "legacy snapshot cannot be opened".into(),
        })?;
    let connection = snapshot.connect().map_err(|_| MigrationError::Db {
        reason: "legacy snapshot connection failed".into(),
    })?;
    let mut pg = pool.get().await?;
    let transaction = tokio_postgres::Client::build_transaction(&mut pg)
        .read_only(dry_run)
        .start()
        .await?;
    let paused =
        migrate_tables(&connection, &transaction, &boot_tenant, &boot_user, dry_run).await?;
    if dry_run {
        transaction.rollback().await?;
    } else {
        let bytes = read_import_source(&snapshot_path)?.ok_or_else(|| MigrationError::Io {
            reason: "legacy snapshot disappeared before commit".into(),
        })?;
        retain_import_backup(&db_path, &bytes)?;
        transaction.commit().await?;
    }
    // Preserve the original main file, WAL and previous backups. No pathname
    // rename/unlink can accidentally consume a newer operator database.

    Ok((true, paused))
}

/// Resolve `boot_tenant` and `boot_user` from the DB config (already written
/// in step 3). Falls back to `"default"` / `"admin"` per §8.1.
async fn resolve_boot_identity(
    pool: &PgPool,
    tenant_id: &str,
) -> Result<(String, String), MigrationError> {
    let config = crate::db_config::load_config_snapshot(pool, tenant_id)
        .await
        .map_err(|_| MigrationError::Db {
            reason: "migration destination identity could not be read".into(),
        })?;
    let bt = config
        .identity
        .as_ref()
        .and_then(|id| id.tenant.clone())
        .unwrap_or_else(|| "default".to_string());
    let bu = config
        .identity
        .as_ref()
        .and_then(|id| id.default_owner.clone())
        .unwrap_or_else(|| "admin".to_string());
    Ok((bt, bu))
}

/// Migrate all tables from the libSQL connection into Postgres.
async fn migrate_tables(
    conn: &libsql::Connection,
    client: &tokio_postgres::Transaction<'_>,
    boot_tenant: &str,
    boot_user: &str,
    dry_run: bool,
) -> Result<usize, MigrationError> {
    migrate_safety_config(conn, client, boot_tenant, dry_run).await?;
    migrate_token_settings(conn, client, boot_tenant, boot_user, dry_run).await?;
    migrate_memory_docs(conn, client, boot_tenant, boot_user, dry_run).await?;
    migrate_root_filesystem(conn, client, boot_tenant, dry_run).await?;
    migrate_capability_permissions(conn, client, boot_tenant, dry_run).await?;
    migrate_hooks_predicate_invocations(conn, client, dry_run).await?;
    migrate_hooks_predicate_values(conn, client, dry_run).await?;
    let paused = migrate_trigger_records(conn, client, dry_run).await?;
    migrate_local_reborn_access(conn, client, dry_run).await?;
    Ok(paused)
}

// ---------------------------------------------------------------------------
// Per-table migration helpers
// ---------------------------------------------------------------------------

async fn migrate_safety_config(
    conn: &libsql::Connection,
    client: &tokio_postgres::Transaction<'_>,
    boot_tenant: &str,
    dry_run: bool,
) -> Result<(), MigrationError> {
    let rows = query_all(conn, "safety_config", "SELECT id,user_id,category,pattern,is_enabled,is_default,created_at,updated_at FROM safety_config").await?;
    for row in rows {
        let id = get_text(&row, 0)?;
        let user = get_text(&row, 1)?;
        let category = get_text(&row, 2)?;
        if !matches!(
            category.as_str(),
            "sensitive_paths" | "workspace_rules" | "blocked_paths"
        ) {
            return Err(MigrationError::Db {
                reason: "legacy safety category is invalid".into(),
            });
        }
        let pattern = get_text(&row, 3)?;
        let enabled = get_bool(&row, 4)?;
        let default = get_bool(&row, 5)?;
        let created = get_timestamp(&row, 6)?;
        let updated = get_timestamp(&row, 7)?;
        if !dry_run {
            insert_retaining_existing(client, "INSERT INTO brassclaw_safety_config(id,tenant_id,user_id,category,pattern,is_enabled,is_default,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8::text::timestamptz,$9::text::timestamptz) ON CONFLICT DO NOTHING", "SELECT 1 FROM brassclaw_safety_config WHERE id IS NOT DISTINCT FROM $1 AND tenant_id IS NOT DISTINCT FROM $2 AND user_id IS NOT DISTINCT FROM $3 AND category IS NOT DISTINCT FROM $4 AND pattern IS NOT DISTINCT FROM $5 AND is_enabled IS NOT DISTINCT FROM $6 AND is_default IS NOT DISTINCT FROM $7 AND created_at IS NOT DISTINCT FROM $8::text::timestamptz AND updated_at IS NOT DISTINCT FROM $9::text::timestamptz", &[&id,&boot_tenant,&user,&category,&pattern,&enabled,&default,&created,&updated]).await?;
        }
    }
    Ok(())
}

async fn migrate_token_settings(
    conn: &libsql::Connection,
    client: &tokio_postgres::Transaction<'_>,
    boot_tenant: &str,
    _boot_user: &str,
    dry_run: bool,
) -> Result<(), MigrationError> {
    let rows = query_all(conn, "settings", "SELECT user_id,key,value FROM settings").await?;
    for row in rows {
        let user = get_text(&row, 0)?;
        let key = get_text(&row, 1)?;
        let value = get_text(&row, 2)?;
        let provider = key
            .strip_prefix("provider_tokens:")
            .filter(|provider| !provider.is_empty())
            .ok_or_else(|| MigrationError::Db {
                reason:
                    "legacy settings key requires an explicit supported migration; source retained"
                        .into(),
            })?;
        let settings: brassclaw_product_workflow::TokenSettingsResponse =
            serde_json::from_str(&value).map_err(|_| MigrationError::Db {
                reason: "legacy provider token settings are invalid".into(),
            })?;
        let settings = serde_json::to_value(settings).map_err(|_| MigrationError::Db {
            reason: "legacy provider token settings cannot be encoded".into(),
        })?;
        if !dry_run {
            insert_retaining_existing(client, "INSERT INTO brassclaw_token_settings(tenant_id,user_id,provider_id,settings) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING", "SELECT 1 FROM brassclaw_token_settings WHERE tenant_id IS NOT DISTINCT FROM $1 AND user_id IS NOT DISTINCT FROM $2 AND provider_id IS NOT DISTINCT FROM $3 AND settings IS NOT DISTINCT FROM $4", &[&boot_tenant,&user,&provider,&settings]).await?;
        }
    }
    Ok(())
}

async fn migrate_memory_docs(
    conn: &libsql::Connection,
    client: &tokio_postgres::Transaction<'_>,
    boot_tenant: &str,
    _boot_user: &str,
    dry_run: bool,
) -> Result<(), MigrationError> {
    let rows = query_all(conn, "memory_docs", "SELECT user_id,project_id,id,doc_type,title,content,source_thread_id,tags_json,metadata_json,created_at,updated_at FROM memory_docs").await?;
    for row in rows {
        let user = get_text(&row, 0)?;
        let project = get_text(&row, 1)?;
        let id = get_text(&row, 2)?;
        let kind = get_text(&row, 3)?;
        let title = get_text(&row, 4)?;
        let content = get_text(&row, 5)?;
        let thread = get_text_opt(&row, 6)?;
        let tags: Vec<String> =
            serde_json::from_str(&get_text(&row, 7)?).map_err(|_| MigrationError::Db {
                reason: "legacy memory tags are invalid".into(),
            })?;
        let metadata: serde_json::Value =
            serde_json::from_str(&get_text(&row, 8)?).map_err(|_| MigrationError::Db {
                reason: "legacy memory metadata is invalid".into(),
            })?;
        let created = get_timestamp(&row, 9)?;
        let updated = get_timestamp(&row, 10)?;
        if !dry_run {
            insert_retaining_existing(client, "INSERT INTO brassclaw_memory_docs(tenant_id,user_id,project_id,id,doc_type,title,content,source_thread_id,tags,metadata,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11::text::timestamptz,$12::text::timestamptz) ON CONFLICT DO NOTHING", "SELECT 1 FROM brassclaw_memory_docs WHERE tenant_id IS NOT DISTINCT FROM $1 AND user_id IS NOT DISTINCT FROM $2 AND project_id IS NOT DISTINCT FROM $3 AND id IS NOT DISTINCT FROM $4 AND doc_type IS NOT DISTINCT FROM $5 AND title IS NOT DISTINCT FROM $6 AND content IS NOT DISTINCT FROM $7 AND source_thread_id IS NOT DISTINCT FROM $8 AND tags IS NOT DISTINCT FROM $9 AND metadata IS NOT DISTINCT FROM $10 AND created_at IS NOT DISTINCT FROM $11::text::timestamptz AND updated_at IS NOT DISTINCT FROM $12::text::timestamptz", &[&boot_tenant,&user,&project,&id,&kind,&title,&content,&thread,&tags,&metadata,&created,&updated]).await?;
        }
    }
    Ok(())
}

async fn migrate_root_filesystem(
    conn: &libsql::Connection,
    client: &tokio_postgres::Transaction<'_>,
    boot_tenant: &str,
    dry_run: bool,
) -> Result<(), MigrationError> {
    let rows = query_all(conn, "root_filesystem_entries", "SELECT path,contents,is_dir,content_type,kind,indexed,version,created_at,updated_at FROM root_filesystem_entries").await?;
    for row in rows {
        let path = get_text(&row, 0)?;
        let contents = get_blob(&row, 1)?;
        let directory = get_bool(&row, 2)?;
        let content_type = get_text_opt(&row, 3)?;
        let kind = get_text_opt(&row, 4)?;
        let indexed: serde_json::Value =
            serde_json::from_str(&get_text(&row, 5)?).map_err(|_| MigrationError::Db {
                reason: "legacy filesystem index metadata is invalid".into(),
            })?;
        let version = get_integer(&row, 6)?;
        if version < 0 || !path.starts_with('/') {
            return Err(MigrationError::Db {
                reason: "legacy filesystem path/version is invalid".into(),
            });
        }
        let created = get_timestamp(&row, 7)?;
        let updated = get_timestamp(&row, 8)?;
        if !dry_run {
            insert_retaining_existing(client, "INSERT INTO brassclaw_root_filesystem(tenant_id,path,contents,is_dir,content_type,kind,indexed,version,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9::text::timestamptz,$10::text::timestamptz) ON CONFLICT DO NOTHING", "SELECT 1 FROM brassclaw_root_filesystem WHERE tenant_id IS NOT DISTINCT FROM $1 AND path IS NOT DISTINCT FROM $2 AND contents IS NOT DISTINCT FROM $3 AND is_dir IS NOT DISTINCT FROM $4 AND content_type IS NOT DISTINCT FROM $5 AND kind IS NOT DISTINCT FROM $6 AND indexed IS NOT DISTINCT FROM $7 AND version IS NOT DISTINCT FROM $8 AND created_at IS NOT DISTINCT FROM $9::text::timestamptz AND updated_at IS NOT DISTINCT FROM $10::text::timestamptz", &[&boot_tenant,&path,&contents,&directory,&content_type,&kind,&indexed,&version,&created,&updated]).await?;
        }
    }
    let rows = query_all(
        conn,
        "root_filesystem_index_specs",
        "SELECT prefix,name,keys,kind FROM root_filesystem_index_specs",
    )
    .await?;
    for row in rows {
        let prefix = get_text(&row, 0)?;
        let name = get_text(&row, 1)?;
        let keys = get_text(&row, 2)?;
        let _: serde_json::Value = serde_json::from_str(&keys).map_err(|_| MigrationError::Db {
            reason: "legacy filesystem index keys are invalid".into(),
        })?;
        let kind = get_text(&row, 3)?;
        if !dry_run {
            insert_retaining_existing(client, "INSERT INTO brassclaw_root_filesystem_index_specs(tenant_id,prefix,name,keys,kind) VALUES($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING", "SELECT 1 FROM brassclaw_root_filesystem_index_specs WHERE tenant_id IS NOT DISTINCT FROM $1 AND prefix IS NOT DISTINCT FROM $2 AND name IS NOT DISTINCT FROM $3 AND keys IS NOT DISTINCT FROM $4 AND kind IS NOT DISTINCT FROM $5", &[&boot_tenant,&prefix,&name,&keys,&kind]).await?;
        }
    }
    if !dry_run {
        client
            .batch_execute("LOCK TABLE brassclaw_root_filesystem_events IN ACCESS EXCLUSIVE MODE")
            .await?;
    }
    let rows = query_all(
        conn,
        "root_filesystem_events",
        "SELECT id,path,payload,created_at FROM root_filesystem_events ORDER BY id",
    )
    .await?;
    for row in rows {
        let sequence = get_integer(&row, 0)?;
        if sequence <= 0 {
            return Err(MigrationError::Db {
                reason: "legacy filesystem sequence is invalid".into(),
            });
        }
        let path = get_text(&row, 1)?;
        let payload = get_blob(&row, 2)?;
        let created = get_timestamp(&row, 3)?;
        if !dry_run {
            insert_retaining_existing(client, "INSERT INTO brassclaw_root_filesystem_events(seq,tenant_id,path,payload,created_at) VALUES($1,$2,$3,$4,$5::text::timestamptz) ON CONFLICT DO NOTHING", "SELECT 1 FROM brassclaw_root_filesystem_events WHERE seq IS NOT DISTINCT FROM $1 AND tenant_id IS NOT DISTINCT FROM $2 AND path IS NOT DISTINCT FROM $3 AND payload IS NOT DISTINCT FROM $4 AND created_at IS NOT DISTINCT FROM $5::text::timestamptz", &[&sequence,&boot_tenant,&path,&payload,&created]).await?;
        }
    }
    if !dry_run {
        // The imported historical sequence must not collide with future appends.
        let row = client.query_one("SELECT COALESCE(MAX(seq),0),pg_get_serial_sequence('brassclaw_root_filesystem_events','seq') FROM brassclaw_root_filesystem_events", &[]).await?;
        let next = row
            .try_get::<_, i64>(0)?
            .checked_add(1)
            .ok_or_else(|| MigrationError::Db {
                reason: "legacy event sequence exhausted its technical integer range".into(),
            })?;
        // PostgreSQL returns a quoted qualified identifier, never caller text.
        let sequence: String = row.try_get(1)?;
        client
            .batch_execute(&format!("ALTER SEQUENCE {sequence} RESTART WITH {next}"))
            .await?;
    }
    Ok(())
}

async fn migrate_capability_permissions(
    conn: &libsql::Connection,
    client: &tokio_postgres::Transaction<'_>,
    _boot_tenant: &str,
    dry_run: bool,
) -> Result<(), MigrationError> {
    let rows = query_all(
        conn,
        "capability_permissions",
        "SELECT tenant_id,capability_id,permission_mode,updated_at FROM capability_permissions",
    )
    .await?;
    for row in rows {
        let tenant = get_text(&row, 0)?;
        let capability = get_text(&row, 1)?;
        let mode = get_text(&row, 2)?;
        if !matches!(mode.as_str(), "allow" | "ask" | "deny") {
            return Err(MigrationError::Db {
                reason: "legacy permission metadata is invalid".into(),
            });
        }
        let updated = get_timestamp(&row, 3)?;
        if !dry_run {
            insert_retaining_existing(client, "INSERT INTO brassclaw_capability_permissions(tenant_id,capability_id,permission_mode,updated_at) VALUES($1,$2,$3,$4::text::timestamptz) ON CONFLICT DO NOTHING", "SELECT 1 FROM brassclaw_capability_permissions WHERE tenant_id IS NOT DISTINCT FROM $1 AND capability_id IS NOT DISTINCT FROM $2 AND permission_mode IS NOT DISTINCT FROM $3 AND updated_at IS NOT DISTINCT FROM $4::text::timestamptz", &[&tenant,&capability,&mode,&updated]).await?;
        }
    }
    // Historical scoped rows are preserved data, not new v3 invocation grants.
    Ok(())
}

async fn migrate_hooks_predicate_invocations(
    conn: &libsql::Connection,
    client: &tokio_postgres::Transaction<'_>,
    dry_run: bool,
) -> Result<(), MigrationError> {
    // libSQL column name is `recorded_at`; PG target column is `occurred_at` (TIMESTAMPTZ NOT NULL).
    let rows = query_all(
        conn,
        "hooks_predicate_invocations",
        "SELECT key_hash, scope_hash, event_id, recorded_at \
         FROM hooks_predicate_invocations",
    )
    .await?;
    if rows.is_empty() {
        return Ok(());
    }

    for row in rows {
        let key_hash = get_blob(&row, 0)?;
        let scope_hash = get_blob(&row, 1)?;
        let event_id: String = get_text(&row, 2)?;
        let recorded_at: String = get_timestamp(&row, 3)?;
        if !dry_run {
            insert_retaining_existing(client, "INSERT INTO hooks_predicate_invocations (key_hash, scope_hash, event_id, occurred_at) VALUES ($1, $2, $3, $4::text::timestamptz) ON CONFLICT DO NOTHING", "SELECT 1 FROM hooks_predicate_invocations WHERE key_hash IS NOT DISTINCT FROM $1 AND scope_hash IS NOT DISTINCT FROM $2 AND event_id IS NOT DISTINCT FROM $3 AND occurred_at IS NOT DISTINCT FROM $4::text::timestamptz", &[
                        &key_hash.as_slice(),
                        &scope_hash.as_slice(),
                        &event_id,
                        &recorded_at,
                    ]).await?;
        }
    }
    Ok(())
}

async fn migrate_hooks_predicate_values(
    conn: &libsql::Connection,
    client: &tokio_postgres::Transaction<'_>,
    dry_run: bool,
) -> Result<(), MigrationError> {
    // libSQL column name is `recorded_at`; PG target column is `occurred_at` (TIMESTAMPTZ NOT NULL).
    let rows = query_all(
        conn,
        "hooks_predicate_values",
        "SELECT key_hash, scope_hash, event_id, value, recorded_at \
         FROM hooks_predicate_values",
    )
    .await?;
    if rows.is_empty() {
        return Ok(());
    }

    for row in rows {
        let key_hash = get_blob(&row, 0)?;
        let scope_hash = get_blob(&row, 1)?;
        let event_id: String = get_text(&row, 2)?;
        let value: String = get_text(&row, 3)?;
        let recorded_at: String = get_timestamp(&row, 4)?;
        if !dry_run {
            insert_retaining_existing(client, "INSERT INTO hooks_predicate_values (key_hash, scope_hash, event_id, value, occurred_at) VALUES ($1, $2, $3, $4::text::numeric, $5::text::timestamptz) ON CONFLICT DO NOTHING", "SELECT 1 FROM hooks_predicate_values WHERE key_hash IS NOT DISTINCT FROM $1 AND scope_hash IS NOT DISTINCT FROM $2 AND event_id IS NOT DISTINCT FROM $3 AND value IS NOT DISTINCT FROM $4::text::numeric AND occurred_at IS NOT DISTINCT FROM $5::text::timestamptz", &[
                        &key_hash.as_slice(),
                        &scope_hash.as_slice(),
                        &event_id,
                        &value,
                        &recorded_at,
                    ]).await?;
        }
    }
    Ok(())
}

/// Migrate `trigger_records` → `brassclaw_triggers`.
///
/// The libSQL `trigger_records` table uses the OLD schema from before the
/// `brassclaw_triggers` redesign:
///   `id, tenant_id, creator_user_id, name, description,
///    trigger_kind, trigger_config, status, created_at, updated_at`
///
/// The PG `brassclaw_triggers` schema (V021) has completely different columns
/// (`trigger_id`, `source`, `schedule_expression`, `completion_policy`,
/// `prompt`, `state`, `next_run_at`, …). Only the columns common to both
/// schemas are mapped; required PG columns with no libSQL equivalent receive
/// safe sentinel defaults so the row can be inserted. The migrated rows will
/// not be fully functional in the new schema — they serve only as a
/// data-preservation record. Operators should re-create triggers via the UI
/// after upgrading.
async fn migrate_trigger_records(
    conn: &libsql::Connection,
    client: &tokio_postgres::Transaction<'_>,
    dry_run: bool,
) -> Result<usize, MigrationError> {
    let rows = query_all(
        conn,
        "trigger_records",
        "SELECT id, tenant_id, creator_user_id, name, \
                trigger_kind, trigger_config, status, created_at \
         FROM trigger_records",
    )
    .await?;
    if rows.is_empty() {
        return Ok(0);
    }

    let mut paused = 0usize;
    for row in rows {
        // `id` → `trigger_id` (PK renamed in new schema)
        let trigger_id: String = get_text(&row, 0)?;
        let tenant_id: String = get_text(&row, 1)?;
        let creator_user_id: String = get_text(&row, 2)?;
        let name: String = get_text(&row, 3)?;
        // `trigger_kind` is the closest equivalent to `source` (both describe
        // what fires the trigger). `trigger_config` is folded into `prompt` as
        // a JSON-encoded payload so the original config is not lost on disk.
        let trigger_kind: String = get_text(&row, 4)?;
        if !matches!(trigger_kind.as_str(), "cron" | "schedule") {
            return Err(MigrationError::Db {
                reason: "legacy trigger source needs a supported migration; source retained".into(),
            });
        }
        let target_source = "schedule";
        let trigger_config: String = get_text(&row, 5)?;
        // `status` had values 'active'/'inactive'; `state` uses 'scheduled'/
        // 'paused'/'completed'. Preserve recognized states; unknown or null
        // states are corruption, never authority to schedule new effects.
        let state: &str = match get_text_opt(&row, 6)?.as_deref() {
            Some("inactive" | "paused") => "paused",
            Some("active" | "scheduled") => {
                paused += 1;
                "paused"
            }
            Some("completed") => "completed",
            _ => {
                return Err(MigrationError::Db {
                    reason: "legacy trigger state is invalid; it was not scheduled".into(),
                });
            }
        };
        let created_at: String = get_timestamp(&row, 7)?;
        if !dry_run {
            insert_retaining_existing(client, "INSERT INTO brassclaw_triggers (trigger_id, tenant_id, creator_user_id, name, source, schedule_expression, completion_policy, prompt, state, next_run_at, created_at) VALUES ($1, $2, $3, $4, $5, '', 'complete_after_first_fire', $6, $7, '', $8) ON CONFLICT DO NOTHING", "SELECT 1 FROM brassclaw_triggers WHERE trigger_id IS NOT DISTINCT FROM $1 AND tenant_id IS NOT DISTINCT FROM $2 AND creator_user_id IS NOT DISTINCT FROM $3 AND name IS NOT DISTINCT FROM $4 AND source IS NOT DISTINCT FROM $5 AND schedule_expression IS NOT DISTINCT FROM '' AND completion_policy IS NOT DISTINCT FROM 'complete_after_first_fire' AND prompt IS NOT DISTINCT FROM $6 AND state IS NOT DISTINCT FROM $7 AND next_run_at IS NOT DISTINCT FROM '' AND created_at IS NOT DISTINCT FROM $8", &[
                        &trigger_id,
                        &tenant_id,
                        &creator_user_id,
                        &name,
                        &target_source,
                        &trigger_config,
                        &state,
                        &created_at,
                    ]).await?;
        }
    }
    Ok(paused)
}

/// Matching retained rows are idempotent. A different historical payload
/// aborts the complete SQLite import; DO NOTHING must not conceal a conflict.
async fn insert_retaining_existing(
    transaction: &tokio_postgres::Transaction<'_>,
    insert: &str,
    compare: &str,
    values: &[&(dyn tokio_postgres::types::ToSql + Sync)],
) -> Result<(), MigrationError> {
    if transaction.execute(insert, values).await? == 0
        && transaction.query_opt(compare, values).await?.is_none()
    {
        return Err(MigrationError::Db {
            reason: "legacy row conflicts with retained database data; nothing was replaced".into(),
        });
    }
    Ok(())
}

/// Skip `local_reborn_access` — the libSQL and PG schemas are incompatible.
///
/// The libSQL `local_reborn_access` table stored bearer-token hashes
/// (`id, tenant_id, user_id, token_hash, created_at, updated_at`) for
/// local-dev HTTP auth. The PG `brassclaw_local_access` table stores access
/// grants with a role/status/source model (`tenant_id, user_id, agent_id,
/// project_id, role, status, source, created_at, updated_at`) that is
/// re-seeded at every `brassclaw serve` startup by
/// `PgRebornLocalTriggerAccessStore::seed_local_access`. Token hashes from
/// the old table are not portable to the new schema and should not be
/// migrated — the table is re-populated automatically on first serve.
async fn migrate_local_reborn_access(
    _conn: &libsql::Connection,
    _client: &tokio_postgres::Transaction<'_>,
    _dry_run: bool,
) -> Result<(), MigrationError> {
    // No-op: incompatible schemas; `brassclaw_local_access` is re-seeded at
    // serve startup by PgRebornLocalTriggerAccessStore::seed_local_access.
    tracing::debug!(
        "migrate_local_reborn_access: skipped — \
         local_reborn_access (token-hash store) is incompatible with \
         brassclaw_local_access (role/status/source grant store); \
         brassclaw_local_access is re-seeded at startup"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// libSQL query helpers
// ---------------------------------------------------------------------------

type LibSqlRow = Vec<Option<libsql::Value>>;

async fn query_all(
    conn: &libsql::Connection,
    table: &str,
    sql: &str,
) -> Result<Vec<LibSqlRow>, MigrationError> {
    let mut inventory = conn
        .query(
            "SELECT 1 FROM sqlite_schema WHERE type='table' AND name=?1",
            libsql::params![table],
        )
        .await
        .map_err(|_| MigrationError::Db {
            reason: "legacy table inventory cannot be read".into(),
        })?;
    if inventory
        .next()
        .await
        .map_err(|_| MigrationError::Db {
            reason: "legacy table inventory cannot be decoded".into(),
        })?
        .is_none()
    {
        return Ok(Vec::new());
    }

    let mut rows = conn
        .query(sql, libsql::params![])
        .await
        .map_err(|e| MigrationError::Db {
            reason: format!("libsql query '{sql}': {e}"),
        })?;

    let col_count = rows.column_count();
    let mut result = Vec::new();

    while let Some(row) = rows.next().await.map_err(|e| MigrationError::Db {
        reason: format!("libsql row iter: {e}"),
    })? {
        let mut cols: LibSqlRow = Vec::with_capacity(col_count as usize);
        for i in 0..col_count {
            let val = row.get_value(i).map_err(|_| MigrationError::Db {
                reason: format!("legacy column {i} could not be decoded"),
            })?;
            cols.push(Some(val));
        }
        result.push(cols);
    }

    Ok(result)
}

fn get_text(row: &LibSqlRow, idx: usize) -> Result<String, MigrationError> {
    match row.get(idx).and_then(|v| v.as_ref()) {
        Some(libsql::Value::Text(s)) => Ok(s.clone()),
        Some(libsql::Value::Integer(n)) => Ok(n.to_string()),
        Some(libsql::Value::Real(f)) => Ok(f.to_string()),
        Some(libsql::Value::Null) | None => Err(MigrationError::Db {
            reason: format!("expected text at column {idx}, got NULL"),
        }),
        Some(libsql::Value::Blob(_)) => Err(MigrationError::Db {
            reason: format!("expected text at column {idx}, got BLOB"),
        }),
    }
}

fn get_text_opt(row: &LibSqlRow, idx: usize) -> Result<Option<String>, MigrationError> {
    match row.get(idx).and_then(|value| value.as_ref()) {
        Some(libsql::Value::Null) => Ok(None),
        Some(_) => get_text(row, idx).map(Some),
        None => Err(MigrationError::Db {
            reason: format!("missing legacy column {idx}"),
        }),
    }
}

fn get_integer(row: &LibSqlRow, idx: usize) -> Result<i64, MigrationError> {
    match row.get(idx).and_then(|value| value.as_ref()) {
        Some(libsql::Value::Integer(value)) => Ok(*value),
        _ => Err(MigrationError::Db {
            reason: format!("invalid legacy integer at column {idx}"),
        }),
    }
}
fn get_timestamp(row: &LibSqlRow, idx: usize) -> Result<String, MigrationError> {
    let failure = || MigrationError::Db {
        reason: format!("invalid legacy timestamp at column {idx}"),
    };
    match row.get(idx).and_then(|value| value.as_ref()) {
        Some(libsql::Value::Integer(millis)) => chrono::DateTime::from_timestamp_millis(*millis)
            .map(|time| time.to_rfc3339())
            .ok_or_else(failure),
        Some(libsql::Value::Text(value)) => {
            if let Ok(time) = chrono::DateTime::parse_from_rfc3339(value) {
                return Ok(time.to_rfc3339());
            }
            chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f")
                .map(|time| time.and_utc().to_rfc3339())
                .map_err(|_| failure())
        }
        _ => Err(failure()),
    }
}

fn get_bool(row: &LibSqlRow, idx: usize) -> Result<bool, MigrationError> {
    match row.get(idx).and_then(|value| value.as_ref()) {
        Some(libsql::Value::Integer(0)) => Ok(false),
        Some(libsql::Value::Integer(1)) => Ok(true),
        _ => Err(MigrationError::Db {
            reason: format!("invalid legacy boolean at column {idx}"),
        }),
    }
}

fn get_blob(row: &LibSqlRow, idx: usize) -> Result<Vec<u8>, MigrationError> {
    match row.get(idx).and_then(|v| v.as_ref()) {
        Some(libsql::Value::Blob(b)) => Ok(b.clone()),
        _ => Err(MigrationError::Db {
            reason: format!("expected blob at column {idx}"),
        }),
    }
}

// ---------------------------------------------------------------------------
// File utilities
// ---------------------------------------------------------------------------

/// Capture one regular source once. Keep it in place; a portable pathname
/// reread cannot authorize deleting a file replaced by another writer.
fn read_import_source(path: &Path) -> Result<Option<Vec<u8>>, MigrationError> {
    use std::io::Read;
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => {
            return Err(MigrationError::Io {
                reason: "legacy source cannot be inspected".into(),
            });
        }
    };
    if !metadata.file_type().is_file() {
        return Err(MigrationError::Io {
            reason: "legacy source must be a regular file, not a link or device".into(),
        });
    }
    let mut source = std::fs::File::open(path).map_err(|_| MigrationError::Io {
        reason: "legacy source cannot be opened".into(),
    })?;
    let captured = source.metadata().map_err(|_| MigrationError::Io {
        reason: "legacy source metadata is unavailable".into(),
    })?;
    if !captured.is_file() || captured.len() != metadata.len() {
        return Err(MigrationError::Io {
            reason: "legacy source changed while opening".into(),
        });
    }
    let mut bytes = Vec::new();
    let mut buffer = [0; 65_536];
    loop {
        let count = source.read(&mut buffer).map_err(|_| MigrationError::Io {
            reason: "legacy source cannot be read".into(),
        })?;
        if count == 0 {
            break;
        }
        let size = bytes
            .len()
            .checked_add(count)
            .filter(|size| (*size as u64) <= captured.len())
            .ok_or_else(|| MigrationError::Io {
                reason: "legacy source changed while capturing".into(),
            })?;
        bytes
            .try_reserve(size - bytes.len())
            .map_err(|_| MigrationError::Io {
                reason: "legacy source cannot fit in available memory".into(),
            })?;
        bytes.extend_from_slice(&buffer[..count]);
    }
    if bytes.len() as u64 != captured.len()
        || source
            .metadata()
            .map_err(|_| MigrationError::Io {
                reason: "legacy source metadata is unavailable".into(),
            })?
            .len()
            != captured.len()
    {
        return Err(MigrationError::Io {
            reason: "legacy source changed while capturing".into(),
        });
    }
    Ok(Some(bytes))
}

fn retain_import_backup(source: &Path, bytes: &[u8]) -> Result<std::path::PathBuf, MigrationError> {
    use sha2::{Digest, Sha256};
    use std::io::{Read, Write};
    let parent = source.parent().ok_or_else(|| MigrationError::Io {
        reason: "legacy source has no parent directory".into(),
    })?;
    let name = source.file_name().ok_or_else(|| MigrationError::Io {
        reason: "legacy source has no file name".into(),
    })?;
    let mut backup_name = name.to_os_string();
    backup_name.push(format!(".migrated.{}", hex::encode(Sha256::digest(bytes))));
    let backup = parent.join(backup_name);
    let mut staged = tempfile::NamedTempFile::new_in(parent).map_err(|_| MigrationError::Io {
        reason: "legacy backup cannot be staged".into(),
    })?;
    staged
        .write_all(bytes)
        .and_then(|_| staged.as_file().sync_all())
        .map_err(|_| MigrationError::Io {
            reason: "legacy backup could not be synchronized".into(),
        })?;
    match staged.persist_noclobber(&backup) {
        Ok(_) => {}
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = std::fs::symlink_metadata(&backup).map_err(|_| MigrationError::Io {
                reason: "existing legacy backup cannot be inspected".into(),
            })?;
            if !metadata.file_type().is_file() || metadata.len() != bytes.len() as u64 {
                return Err(MigrationError::Io {
                    reason: "existing legacy backup differs; nothing was replaced".into(),
                });
            }
            let file = std::fs::File::open(&backup).map_err(|_| MigrationError::Io {
                reason: "existing legacy backup cannot be opened".into(),
            })?;
            let mut retained = Vec::new();
            file.take(
                (bytes.len() as u64)
                    .checked_add(1)
                    .ok_or_else(|| MigrationError::Io {
                        reason: "legacy backup length is unrepresentable".into(),
                    })?,
            )
            .read_to_end(&mut retained)
            .map_err(|_| MigrationError::Io {
                reason: "existing legacy backup cannot be read".into(),
            })?;
            if retained != bytes {
                return Err(MigrationError::Io {
                    reason: "existing legacy backup differs; nothing was replaced".into(),
                });
            }
        }
        Err(_) => {
            return Err(MigrationError::Io {
                reason: "legacy backup cannot be created".into(),
            });
        }
    }
    #[cfg(unix)]
    std::fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| MigrationError::Io {
            reason: "legacy backup directory could not be synchronized".into(),
        })?;
    Ok(backup)
}

async fn import_settings_snapshot(
    pool: &PgPool,
    tenant_id: &str,
    source: &Path,
    bytes: &[u8],
    rows: &[(String, String)],
    dry_run: bool,
) -> Result<(), MigrationError> {
    let mut keys = std::collections::BTreeSet::new();
    for (key, value) in rows {
        if !keys.insert(key.as_str()) {
            return Err(MigrationError::ConfigParse {
                reason: "legacy configuration contains duplicate destinations".into(),
            });
        }
        brassclaw_reborn_config::reject_inline_secret(key.clone(), value).map_err(|_| {
            MigrationError::ConfigParse {
                reason: "legacy settings contain forbidden inline credential material".into(),
            }
        })?;
    }
    let mut client = pool.get().await?;
    let transaction = tokio_postgres::Client::build_transaction(&mut client)
        .isolation_level(if dry_run {
            tokio_postgres::IsolationLevel::RepeatableRead
        } else {
            tokio_postgres::IsolationLevel::ReadCommitted
        })
        .read_only(dry_run)
        .start()
        .await?;
    if !dry_run {
        if rows.iter().any(|(key, _)| key.starts_with("budget.")) {
            crate::db_config::lock_budget_config(&transaction, tenant_id)
                .await
                .map_err(|_| MigrationError::ConfigParse {
                    reason: "legacy budget configuration could not be locked".into(),
                })?;
        }
        transaction
            .batch_execute("LOCK TABLE brassclaw_config IN SHARE ROW EXCLUSIVE MODE")
            .await?;
    }
    let names: Vec<_> = rows.iter().map(|(key, _)| key.as_str()).collect();
    let existing = transaction
        .query(
            "SELECT key,value FROM brassclaw_config WHERE tenant_id=$1 AND key=ANY($2)",
            &[&tenant_id, &names],
        )
        .await?;
    let mut retained = std::collections::BTreeMap::new();
    for row in existing {
        retained.insert(
            row.try_get::<_, String>("key")?,
            row.try_get::<_, String>("value")?,
        );
    }
    let mut pending = Vec::new();
    for (key, value) in rows {
        if let Some(previous) = retained.get(key) {
            if previous != value {
                return Err(MigrationError::ConfigParse { reason: "legacy settings conflict with retained database values; nothing was replaced".into() });
            }
        } else {
            pending.push((key.as_str(), Some(value.as_str())));
        }
    }
    crate::db_config::validate_budget_config_changes(&transaction, tenant_id, &pending)
        .await
        .map_err(|_| MigrationError::ConfigParse {
            reason: "legacy budget settings failed database validation".into(),
        })?;
    if dry_run {
        transaction.rollback().await?;
        return Ok(());
    }
    retain_import_backup(source, bytes)?;
    crate::db_config::save_config_changes_in_transaction(
        &transaction,
        tenant_id,
        &pending,
        ConfigWriteContext::Operator,
    )
    .await
    .map_err(|_| MigrationError::ConfigParse {
        reason: "legacy settings failed database write validation".into(),
    })?;
    transaction.commit().await?;
    let current = read_import_source(source).map_err(|_| MigrationError::ConfigParse {
        reason: "settings import committed, but its source could not be verified; backup retained"
            .into(),
    })?;
    if current.as_deref() != Some(bytes) {
        return Err(MigrationError::ConfigParse {
            reason:
                "settings import committed, but the source changed; source and backup were retained"
                    .into(),
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_any_migrated_false_when_empty() {
        let r = MigrationReport::default();
        assert!(!r.any_migrated());
    }

    #[test]
    fn report_any_migrated_true_when_config_migrated() {
        let r = MigrationReport {
            config_migrated: true,
            ..Default::default()
        };
        assert!(r.any_migrated());
    }

    #[test]
    fn report_any_migrated_true_when_libsql_migrated() {
        let r = MigrationReport {
            libsql_db_migrated: true,
            ..Default::default()
        };
        assert!(r.any_migrated());
    }
}
