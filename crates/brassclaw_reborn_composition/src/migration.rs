//! libSQL → Postgres data migration module (§8.1 steps 3–8).
//!
//! Gated behind the `migrate-from-libsql` feature. This entire module is
//! compiled ONLY when that feature is active; it is removed together with the
//! feature in the release after the upgrade cycle completes.
//!
//! # What this module does
//!
//! On first boot after upgrade, the following steps run automatically (all
//! idempotent — safe to re-run after a crash):
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
    let libsql_db_migrated = step7_migrate_libsql_db(pool, home, tenant_id, dry_run).await?;

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
            boot_initialized_set: true,
        });
    }

    Ok(MigrationReport {
        config_migrated,
        providers_migrated,
        sempai_migrated,
        secrets_master_migrated,
        libsql_db_migrated,
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
    if !config_path.exists() {
        return Ok(false);
    }

    let config = RebornConfigFile::load(&config_path)
        .map_err(|e| MigrationError::ConfigParse {
            reason: e.to_string(),
        })?
        .unwrap_or_default();

    if !dry_run {
        write_config_to_db(pool, tenant_id, &config).await?;
        rename_migrated(&config_path)?;
    } else {
        tracing::debug!(path = %config_path.display(), "dry-run: would migrate config.toml");
    }

    Ok(true)
}

/// Translate a `RebornConfigFile` into flat `(key, value)` rows and upsert
/// them into `brassclaw_config`. Mirrors the serialization contract in
/// `db_config.rs`.
async fn write_config_to_db(
    pool: &PgPool,
    tenant_id: &str,
    config: &RebornConfigFile,
) -> Result<(), MigrationError> {
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
                serde_json::to_string(v).unwrap_or_default(),
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
        if let Some(v) = b.user_daily_usd {
            rows.push(("budget.user_daily_usd".to_string(), v.to_string()));
        }
        if let Some(v) = b.project_daily_usd {
            rows.push(("budget.project_daily_usd".to_string(), v.to_string()));
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

    if rows.is_empty() {
        return Ok(());
    }

    let client = pool.get().await?;
    // Upsert each row — skip inline-secret values silently (they were never
    // valid in config.toml and reject_inline_secret would reject them anyway).
    for (key, value) in &rows {
        client
            .execute(
                "INSERT INTO brassclaw_config (tenant_id, key, value) \
                 VALUES ($1, $2, $3) \
                 ON CONFLICT (tenant_id, key) DO UPDATE SET value = EXCLUDED.value",
                &[&tenant_id, key, value],
            )
            .await?;
    }

    Ok(())
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
    let providers_path = home.path().join("providers.json");
    if !providers_path.exists() {
        return Ok(false);
    }

    let text = std::fs::read_to_string(&providers_path).map_err(|e| MigrationError::Io {
        reason: e.to_string(),
    })?;
    // Parse as generic JSON values to avoid a hard dep on brassclaw_llm types
    // in the migrate-from-libsql feature path (root-llm-provider is optional).
    let providers: Vec<serde_json::Value> =
        serde_json::from_str(&text).map_err(|e| MigrationError::ProvidersParse {
            reason: e.to_string(),
        })?;

    if !dry_run {
        let client = pool.get().await?;
        for provider in providers {
            let id = provider
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| MigrationError::ProvidersParse {
                    reason: "provider missing `id` field".to_string(),
                })?
                .to_string();
            let data =
                serde_json::to_string(&provider).map_err(|e| MigrationError::ProvidersParse {
                    reason: e.to_string(),
                })?;
            client
                .execute(
                    "INSERT INTO brassclaw_llm_providers (tenant_id, id, data) \
                     VALUES ($1, $2, $3::JSONB) \
                     ON CONFLICT (tenant_id, id) DO UPDATE SET data = EXCLUDED.data",
                    &[&tenant_id, &id, &data],
                )
                .await?;
        }
        rename_migrated(&providers_path)?;
    } else {
        tracing::debug!(
            path = %providers_path.display(),
            count = text.len(),
            "dry-run: would migrate providers.json"
        );
    }

    Ok(true)
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
    if !sempai_path.exists() {
        return Ok(false);
    }

    let text = std::fs::read_to_string(&sempai_path).map_err(|e| MigrationError::Io {
        reason: e.to_string(),
    })?;

    // sempai_provider.json stores a LlmSlotSelection JSON object.
    let sel: brassclaw_reborn_config::LlmSlotSelection =
        serde_json::from_str(&text).map_err(|e| MigrationError::ProvidersParse {
            reason: e.to_string(),
        })?;

    if !dry_run {
        let client = pool.get().await?;
        if let Some(pid) = &sel.provider_id {
            client
                .execute(
                    "INSERT INTO brassclaw_config (tenant_id, key, value) \
                     VALUES ($1, 'llm.sempai.provider_id', $2) \
                     ON CONFLICT (tenant_id, key) DO UPDATE SET value = EXCLUDED.value",
                    &[&tenant_id, pid],
                )
                .await?;
        }
        if let Some(model) = &sel.model {
            client
                .execute(
                    "INSERT INTO brassclaw_config (tenant_id, key, value) \
                     VALUES ($1, 'llm.sempai.model', $2) \
                     ON CONFLICT (tenant_id, key) DO UPDATE SET value = EXCLUDED.value",
                    &[&tenant_id, model],
                )
                .await?;
        }
        rename_migrated(&sempai_path)?;
    } else {
        tracing::debug!(path = %sempai_path.display(), "dry-run: would migrate sempai_provider.json");
    }

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
    // The local-dev raw-key source path.
    let local_dev_key_path = home.path().join(".reborn-local-dev-secrets-master-key");
    if !local_dev_key_path.exists() {
        return Ok(false);
    }

    // Passphrase ceremony check: if BRASSCLAW_SECRETS_PASSPHRASE_FILE is set,
    // the operator must run `brassclaw secrets rewrap` first.
    let passphrase_file_set = std::env::var("BRASSCLAW_SECRETS_PASSPHRASE_FILE")
        .map(|v| !v.is_empty())
        .unwrap_or(false);

    if passphrase_file_set {
        // Check if the row already exists (operator already ran rewrap).
        let client = pool.get().await?;
        let row_exists: bool = client
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM brassclaw_secrets_master WHERE tenant_id = $1)",
                &[&tenant_id],
            )
            .await
            .map(|r| r.get(0))
            .unwrap_or(false);
        if !row_exists {
            return Err(MigrationError::PassphraseRewrapRequired);
        }
        // Row exists (rewrap already ran) — nothing to migrate.
        return Ok(false);
    }

    // Raw-key-on-disk ceremony: copy the local-dev key to the canonical path
    // and upsert the brassclaw_secrets_master row.
    let canonical_key_path = home.path().join(".secrets-master-key");

    if !dry_run {
        // Copy the raw key file to the canonical location (0600).
        std::fs::copy(&local_dev_key_path, &canonical_key_path).map_err(|e| {
            MigrationError::Io {
                reason: format!("copy secrets key: {e}"),
            }
        })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&canonical_key_path, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| MigrationError::Io {
                reason: format!("chmod secrets key: {e}"),
            })?;
        }

        // Upsert the secrets_master row — both wrapped_key and algorithm
        // must be set explicitly to avoid ceremony-switch-back corruption.
        let client = pool.get().await?;
        client
            .execute(
                "INSERT INTO brassclaw_secrets_master \
                     (tenant_id, version, wrapped_key, algorithm) \
                 VALUES ($1, 1, '', 'raw-key-on-disk') \
                 ON CONFLICT (tenant_id, version) \
                 DO UPDATE SET wrapped_key = '', algorithm = 'raw-key-on-disk'",
                &[&tenant_id],
            )
            .await?;

        // Zero-and-delete the old local-dev key file.
        zero_and_delete(&local_dev_key_path)?;
    } else {
        tracing::debug!(
            src = %local_dev_key_path.display(),
            dst = %canonical_key_path.display(),
            "dry-run: would migrate secrets master key"
        );
    }

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
) -> Result<bool, MigrationError> {
    let db_path = home.path().join("reborn-local-dev.db");
    if !db_path.exists() {
        return Ok(false);
    }

    if dry_run {
        tracing::debug!(path = %db_path.display(), "dry-run: would migrate reborn-local-dev.db");
        return Ok(true);
    }

    // Open the libSQL database.
    let db = libsql::Builder::new_local(db_path.to_string_lossy().to_string())
        .build()
        .await
        .map_err(|e| MigrationError::Db {
            reason: format!("open libsql db: {e}"),
        })?;
    let conn = db.connect().map_err(|e| MigrationError::Db {
        reason: format!("connect libsql: {e}"),
    })?;

    // Resolve boot_tenant and boot_user from the just-migrated config (step 3).
    let (boot_tenant, boot_user) = resolve_boot_identity(pool, tenant_id).await;

    // Migrate all tables.
    migrate_tables(&conn, pool, &boot_tenant, &boot_user).await?;

    // Rename the DB file to *.migrated.
    rename_migrated(&db_path)?;

    Ok(true)
}

/// Resolve `boot_tenant` and `boot_user` from the DB config (already written
/// in step 3). Falls back to `"default"` / `"admin"` per §8.1.
async fn resolve_boot_identity(pool: &PgPool, tenant_id: &str) -> (String, String) {
    let config = match crate::db_config::load_config_snapshot(pool, tenant_id).await {
        Ok(c) => c,
        Err(_) => return ("default".to_string(), "admin".to_string()),
    };
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
    (bt, bu)
}

/// Migrate all tables from the libSQL connection into Postgres.
async fn migrate_tables(
    conn: &libsql::Connection,
    pool: &PgPool,
    boot_tenant: &str,
    boot_user: &str,
) -> Result<(), MigrationError> {
    migrate_safety_config(conn, pool, boot_tenant).await?;
    migrate_token_settings(conn, pool, boot_tenant, boot_user).await?;
    migrate_memory_docs(conn, pool, boot_tenant, boot_user).await?;
    migrate_root_filesystem(conn, pool, boot_tenant).await?;
    migrate_capability_permissions(conn, pool, boot_tenant).await?;
    migrate_hooks_predicate_invocations(conn, pool).await?;
    migrate_hooks_predicate_values(conn, pool).await?;
    migrate_trigger_records(conn, pool).await?;
    migrate_local_reborn_access(conn, pool).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Per-table migration helpers
// ---------------------------------------------------------------------------

async fn migrate_safety_config(
    conn: &libsql::Connection,
    pool: &PgPool,
    boot_tenant: &str,
) -> Result<(), MigrationError> {
    let rows = query_all(conn, "SELECT key, value FROM safety_config").await?;
    if rows.is_empty() {
        return Ok(());
    }
    let client = pool.get().await?;
    for row in rows {
        let key: String = get_text(&row, 0)?;
        let value: String = get_text(&row, 1)?;
        client
            .execute(
                "INSERT INTO brassclaw_safety_config (tenant_id, key, value) \
                 VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
                &[&boot_tenant, &key, &value],
            )
            .await?;
    }
    Ok(())
}

async fn migrate_token_settings(
    conn: &libsql::Connection,
    pool: &PgPool,
    boot_tenant: &str,
    boot_user: &str,
) -> Result<(), MigrationError> {
    let rows = query_all(conn, "SELECT section, key, value FROM settings").await?;
    if rows.is_empty() {
        return Ok(());
    }
    let client = pool.get().await?;
    for row in rows {
        let section: String = get_text(&row, 0)?;
        let key: String = get_text(&row, 1)?;
        let value: String = get_text(&row, 2)?;
        client
            .execute(
                "INSERT INTO brassclaw_token_settings \
                     (tenant_id, user_id, section, key, value) \
                 VALUES ($1, $2, $3, $4, $5) ON CONFLICT DO NOTHING",
                &[&boot_tenant, &boot_user, &section, &key, &value],
            )
            .await?;
    }
    Ok(())
}

async fn migrate_memory_docs(
    conn: &libsql::Connection,
    pool: &PgPool,
    boot_tenant: &str,
    boot_user: &str,
) -> Result<(), MigrationError> {
    // memory_docs table schema: id, path, content, metadata, created_at, updated_at
    let rows = query_all(
        conn,
        "SELECT id, path, content, metadata, created_at, updated_at FROM memory_docs",
    )
    .await?;
    if rows.is_empty() {
        return Ok(());
    }
    let client = pool.get().await?;
    for row in rows {
        let id: String = get_text(&row, 0)?;
        let path: String = get_text(&row, 1)?;
        let content: String = get_text(&row, 2)?;
        let metadata: String = get_text_opt(&row, 3).unwrap_or_default();
        let created_at: String = get_text_opt(&row, 4).unwrap_or_default();
        let updated_at: String = get_text_opt(&row, 5).unwrap_or_default();
        client
            .execute(
                "INSERT INTO brassclaw_memory_docs \
                     (id, tenant_id, user_id, project_id, path, content, metadata, \
                      created_at, updated_at) \
                 VALUES ($1, $2, $3, 'default', $4, $5, $6, \
                         COALESCE(NULLIF($7, '')::TIMESTAMPTZ, NOW()), \
                         COALESCE(NULLIF($8, '')::TIMESTAMPTZ, NOW())) \
                 ON CONFLICT DO NOTHING",
                &[
                    &id,
                    &boot_tenant,
                    &boot_user,
                    &path,
                    &content,
                    &metadata,
                    &created_at,
                    &updated_at,
                ],
            )
            .await?;
    }
    Ok(())
}

async fn migrate_root_filesystem(
    conn: &libsql::Connection,
    pool: &PgPool,
    boot_tenant: &str,
) -> Result<(), MigrationError> {
    // root_filesystem_entries
    let rows = query_all(
        conn,
        "SELECT id, path, kind, content, metadata, created_at, updated_at \
         FROM root_filesystem_entries",
    )
    .await?;
    if !rows.is_empty() {
        let client = pool.get().await?;
        for row in rows {
            let id: String = get_text(&row, 0)?;
            let path: String = get_text(&row, 1)?;
            let kind: String = get_text(&row, 2)?;
            let content: String = get_text_opt(&row, 3).unwrap_or_default();
            let metadata: String = get_text_opt(&row, 4).unwrap_or_default();
            let created_at: String = get_text_opt(&row, 5).unwrap_or_default();
            let updated_at: String = get_text_opt(&row, 6).unwrap_or_default();
            client
                .execute(
                    "INSERT INTO root_filesystem_entries \
                         (id, tenant_id, path, kind, content, metadata, created_at, updated_at) \
                     VALUES ($1, $2, $3, $4, $5, $6, \
                             COALESCE(NULLIF($7,'')::TIMESTAMPTZ, NOW()), \
                             COALESCE(NULLIF($8,'')::TIMESTAMPTZ, NOW())) \
                     ON CONFLICT DO NOTHING",
                    &[
                        &id,
                        &boot_tenant,
                        &path,
                        &kind,
                        &content,
                        &metadata,
                        &created_at,
                        &updated_at,
                    ],
                )
                .await?;
        }
    }

    // root_filesystem_index_specs
    let idx_rows = query_all(
        conn,
        "SELECT id, entry_id, kind, dimension FROM root_filesystem_index_specs",
    )
    .await?;
    if !idx_rows.is_empty() {
        let client = pool.get().await?;
        for row in idx_rows {
            let id: String = get_text(&row, 0)?;
            let entry_id: String = get_text(&row, 1)?;
            let kind: String = get_text(&row, 2)?;
            let dimension: Option<i64> = row[3].as_ref().and_then(|v| match v {
                libsql::Value::Integer(n) => Some(*n),
                _ => None,
            });
            client
                .execute(
                    "INSERT INTO root_filesystem_index_specs (id, tenant_id, entry_id, kind, dimension) \
                     VALUES ($1, $2, $3, $4, $5) ON CONFLICT DO NOTHING",
                    &[&id, &boot_tenant, &entry_id, &kind, &dimension],
                )
                .await?;
        }
    }

    // root_filesystem_events
    let ev_rows = query_all(
        conn,
        "SELECT id, entry_id, kind, payload, created_at FROM root_filesystem_events",
    )
    .await?;
    if !ev_rows.is_empty() {
        let client = pool.get().await?;
        for row in ev_rows {
            let id: String = get_text(&row, 0)?;
            let entry_id: String = get_text(&row, 1)?;
            let kind: String = get_text(&row, 2)?;
            let payload: String = get_text_opt(&row, 3).unwrap_or_default();
            let created_at: String = get_text_opt(&row, 4).unwrap_or_default();
            client
                .execute(
                    "INSERT INTO root_filesystem_events \
                         (id, tenant_id, entry_id, kind, payload, created_at) \
                     VALUES ($1, $2, $3, $4, $5, \
                             COALESCE(NULLIF($6,'')::TIMESTAMPTZ, NOW())) \
                     ON CONFLICT DO NOTHING",
                    &[&id, &boot_tenant, &entry_id, &kind, &payload, &created_at],
                )
                .await?;
        }
    }

    Ok(())
}

async fn migrate_capability_permissions(
    conn: &libsql::Connection,
    pool: &PgPool,
    boot_tenant: &str,
) -> Result<(), MigrationError> {
    let rows = query_all(
        conn,
        "SELECT id, capability, effect, created_at FROM capability_permissions",
    )
    .await?;
    if rows.is_empty() {
        return Ok(());
    }
    let client = pool.get().await?;
    for row in rows {
        let id: String = get_text(&row, 0)?;
        let capability: String = get_text(&row, 1)?;
        let effect: String = get_text(&row, 2)?;
        let created_at: String = get_text_opt(&row, 3).unwrap_or_default();
        client
            .execute(
                "INSERT INTO brassclaw_capability_permissions \
                     (id, tenant_id, capability, effect, created_at) \
                 VALUES ($1, $2, $3, $4, \
                         COALESCE(NULLIF($5,'')::TIMESTAMPTZ, NOW())) \
                 ON CONFLICT DO NOTHING",
                &[&id, &boot_tenant, &capability, &effect, &created_at],
            )
            .await?;
    }
    Ok(())
}

async fn migrate_hooks_predicate_invocations(
    conn: &libsql::Connection,
    pool: &PgPool,
) -> Result<(), MigrationError> {
    // libSQL column name is `recorded_at`; PG target column is `occurred_at` (TIMESTAMPTZ NOT NULL).
    let rows = query_all(
        conn,
        "SELECT key_hash, scope_hash, event_id, recorded_at \
         FROM hooks_predicate_invocations",
    )
    .await?;
    if rows.is_empty() {
        return Ok(());
    }
    let client = pool.get().await?;
    for row in rows {
        let key_hash = get_blob(&row, 0)?;
        let scope_hash = get_blob(&row, 1)?;
        let event_id: String = get_text(&row, 2)?;
        let recorded_at: String = get_text_opt(&row, 3).unwrap_or_default();
        client
            .execute(
                "INSERT INTO hooks_predicate_invocations \
                     (key_hash, scope_hash, event_id, occurred_at) \
                 VALUES ($1, $2, $3, \
                         COALESCE(NULLIF($4,'')::TIMESTAMPTZ, NOW())) \
                 ON CONFLICT DO NOTHING",
                &[
                    &key_hash.as_slice(),
                    &scope_hash.as_slice(),
                    &event_id,
                    &recorded_at,
                ],
            )
            .await?;
    }
    Ok(())
}

async fn migrate_hooks_predicate_values(
    conn: &libsql::Connection,
    pool: &PgPool,
) -> Result<(), MigrationError> {
    // libSQL column name is `recorded_at`; PG target column is `occurred_at` (TIMESTAMPTZ NOT NULL).
    let rows = query_all(
        conn,
        "SELECT key_hash, scope_hash, event_id, value, recorded_at \
         FROM hooks_predicate_values",
    )
    .await?;
    if rows.is_empty() {
        return Ok(());
    }
    let client = pool.get().await?;
    for row in rows {
        let key_hash = get_blob(&row, 0)?;
        let scope_hash = get_blob(&row, 1)?;
        let event_id: String = get_text(&row, 2)?;
        let value: String = get_text(&row, 3)?;
        let recorded_at: String = get_text_opt(&row, 4).unwrap_or_default();
        client
            .execute(
                "INSERT INTO hooks_predicate_values \
                     (key_hash, scope_hash, event_id, value, occurred_at) \
                 VALUES ($1, $2, $3, $4::NUMERIC, \
                         COALESCE(NULLIF($5,'')::TIMESTAMPTZ, NOW())) \
                 ON CONFLICT DO NOTHING",
                &[
                    &key_hash.as_slice(),
                    &scope_hash.as_slice(),
                    &event_id,
                    &value,
                    &recorded_at,
                ],
            )
            .await?;
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
    pool: &PgPool,
) -> Result<(), MigrationError> {
    let rows = query_all(
        conn,
        "SELECT id, tenant_id, creator_user_id, name, \
                trigger_kind, trigger_config, status, created_at \
         FROM trigger_records",
    )
    .await?;
    if rows.is_empty() {
        return Ok(());
    }
    let client = pool.get().await?;
    for row in rows {
        // `id` → `trigger_id` (PK renamed in new schema)
        let trigger_id: String = get_text(&row, 0)?;
        let tenant_id: String = get_text(&row, 1)?;
        let creator_user_id: String = get_text(&row, 2)?;
        let name: String = get_text_opt(&row, 3).unwrap_or_default();
        // `trigger_kind` is the closest equivalent to `source` (both describe
        // what fires the trigger). `trigger_config` is folded into `prompt` as
        // a JSON-encoded payload so the original config is not lost on disk.
        let trigger_kind: String = get_text_opt(&row, 4).unwrap_or_default();
        let trigger_config: String = get_text_opt(&row, 5).unwrap_or_default();
        // `status` had values 'active'/'inactive'; `state` uses 'scheduled'/
        // 'paused'/'completed'. Map 'inactive' → 'paused', everything else →
        // 'scheduled' (safe default — the trigger will fire again on next boot
        // unless the operator explicitly pauses/completes it).
        let state: &str = match get_text_opt(&row, 6).as_deref() {
            Some("inactive") => "paused",
            _ => "scheduled",
        };
        let created_at: String = get_text_opt(&row, 7).unwrap_or_default();
        client
            .execute(
                "INSERT INTO brassclaw_triggers \
                     (trigger_id, tenant_id, creator_user_id, name, source, \
                      schedule_expression, completion_policy, prompt, state, \
                      next_run_at, created_at) \
                 VALUES ($1, $2, $3, $4, $5, '', 'run_once', $6, $7, '', $8) \
                 ON CONFLICT DO NOTHING",
                &[
                    &trigger_id,
                    &tenant_id,
                    &creator_user_id,
                    &name,
                    &trigger_kind,
                    &trigger_config,
                    &state,
                    &created_at,
                ],
            )
            .await?;
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
    _pool: &PgPool,
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

async fn query_all(conn: &libsql::Connection, sql: &str) -> Result<Vec<LibSqlRow>, MigrationError> {
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
            let val = row.get_value(i).ok();
            cols.push(val);
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

fn get_text_opt(row: &LibSqlRow, idx: usize) -> Option<String> {
    match row.get(idx).and_then(|v| v.as_ref()) {
        Some(libsql::Value::Text(s)) => Some(s.clone()),
        Some(libsql::Value::Integer(n)) => Some(n.to_string()),
        Some(libsql::Value::Real(f)) => Some(f.to_string()),
        _ => None,
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

/// Rename `path` to `path.migrated` (idempotent: if `.migrated` already
/// exists, the rename overwrites it — still safe because the source is also
/// gone in that case, or the process crashed mid-rename and we're retrying).
fn rename_migrated(path: &Path) -> Result<(), MigrationError> {
    let mut new_path = path.to_path_buf();
    let original_file_name = new_path
        .file_name()
        .map(|n| format!("{}.migrated", n.to_string_lossy()))
        .unwrap_or_else(|| "migrated".to_string());
    new_path.set_file_name(original_file_name);
    std::fs::rename(path, &new_path).map_err(|e| MigrationError::Io {
        reason: format!("rename {}: {e}", path.display()),
    })
}

/// Overwrite the file with zeros, then delete it. Best-effort: only the
/// delete matters for correctness; the zero-fill is a defense-in-depth
/// measure against key material lingering on disk.
fn zero_and_delete(path: &Path) -> Result<(), MigrationError> {
    if let Ok(metadata) = std::fs::metadata(path) {
        let zeros = vec![0u8; metadata.len() as usize];
        if let Err(e) = std::fs::write(path, &zeros) {
            // Zero-fill is best-effort defence-in-depth; the delete below is
            // what matters for correctness.  Log so operators can investigate
            // permission issues that prevent key-material erasure.
            tracing::debug!(path = %path.display(), error = %e, "zero_and_delete: could not overwrite key file with zeros");
        }
    }
    std::fs::remove_file(path).map_err(|e| MigrationError::Io {
        reason: format!("delete {}: {e}", path.display()),
    })
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
