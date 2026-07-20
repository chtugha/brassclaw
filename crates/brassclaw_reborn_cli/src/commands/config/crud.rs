//! `brassclaw config get/set/unset/list/show-all/export/import` subcommands.
//!
//! All subcommands follow the §6.4 CLI Postgres lifecycle:
//! 1. Start embedded PG or connect to an already-running postmaster.
//! 2. Run schema migrations (idempotent).
//! 3. Perform the operation.
//! 4. Conditional shutdown: shut down embedded PG only if this command started it.

use brassclaw_reborn_composition::db_config::{
    ConfigWriteContext, delete_config_key, list_config_keys, save_config_key,
};
use clap::Args;

use crate::context::RebornCliContext;

// ---------------------------------------------------------------------------
// get
// ---------------------------------------------------------------------------

/// Read a single config key from the database.
#[derive(Debug, Args)]
pub(crate) struct ConfigGetCommand {
    /// The config key to read (e.g. `llm.default.provider_id`).
    key: String,

    /// Tenant ID (default: "default").
    #[arg(long, default_value = "default")]
    tenant: String,
}

impl ConfigGetCommand {
    pub(crate) fn execute(self, _context: RebornCliContext) -> anyhow::Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(self.run_async())
    }

    async fn run_async(self) -> anyhow::Result<()> {
        let pool = crate::commands::config::pg_lifecycle::build_pg_pool().await?;
        let pool = std::sync::Arc::new(pool);
        brassclaw_pg::migrations::run_migrations(&pool)
            .await
            .map_err(|e| anyhow::anyhow!("migration failed: {e}"))?;

        let rows = list_config_keys(&pool, &self.tenant)
            .await
            .map_err(|e| anyhow::anyhow!("config get failed: {e}"))?;

        match rows.into_iter().find(|(k, _)| k == &self.key) {
            Some((_, v)) => println!("{v}"),
            None => anyhow::bail!("key `{}` not found for tenant `{}`", self.key, self.tenant),
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// set
// ---------------------------------------------------------------------------

/// Write a config key→value pair to the database.
#[derive(Debug, Args)]
pub(crate) struct ConfigSetCommand {
    /// The config key to write (e.g. `llm.default.provider_id`).
    key: String,

    /// The value to store.
    value: String,

    /// Tenant ID (default: "default").
    #[arg(long, default_value = "default")]
    tenant: String,
}

impl ConfigSetCommand {
    pub(crate) fn execute(self, _context: RebornCliContext) -> anyhow::Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(self.run_async())
    }

    async fn run_async(self) -> anyhow::Result<()> {
        let pool = crate::commands::config::pg_lifecycle::build_pg_pool().await?;
        let pool = std::sync::Arc::new(pool);
        brassclaw_pg::migrations::run_migrations(&pool)
            .await
            .map_err(|e| anyhow::anyhow!("migration failed: {e}"))?;

        save_config_key(
            &pool,
            &self.tenant,
            &self.key,
            &self.value,
            ConfigWriteContext::Operator,
        )
        .await
        .map_err(|e| anyhow::anyhow!("config set failed: {e}"))?;

        println!("set `{}` = `{}`", self.key, self.value);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// unset
// ---------------------------------------------------------------------------

/// Remove a config key from the database.
#[derive(Debug, Args)]
pub(crate) struct ConfigUnsetCommand {
    /// The config key to remove.
    key: String,

    /// Tenant ID (default: "default").
    #[arg(long, default_value = "default")]
    tenant: String,
}

impl ConfigUnsetCommand {
    pub(crate) fn execute(self, _context: RebornCliContext) -> anyhow::Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(self.run_async())
    }

    async fn run_async(self) -> anyhow::Result<()> {
        let pool = crate::commands::config::pg_lifecycle::build_pg_pool().await?;
        let pool = std::sync::Arc::new(pool);
        brassclaw_pg::migrations::run_migrations(&pool)
            .await
            .map_err(|e| anyhow::anyhow!("migration failed: {e}"))?;

        delete_config_key(&pool, &self.tenant, &self.key)
            .await
            .map_err(|e| anyhow::anyhow!("config unset failed: {e}"))?;

        println!("unset `{}`", self.key);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

/// List all config keys for a tenant (optionally filtered by section prefix).
#[derive(Debug, Args)]
pub(crate) struct ConfigListCommand {
    /// Only show keys beginning with this section prefix (e.g. `llm`).
    #[arg(long)]
    section: Option<String>,

    /// Tenant ID (default: "default").
    #[arg(long, default_value = "default")]
    tenant: String,
}

impl ConfigListCommand {
    pub(crate) fn execute(self, _context: RebornCliContext) -> anyhow::Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(self.run_async())
    }

    async fn run_async(self) -> anyhow::Result<()> {
        let pool = crate::commands::config::pg_lifecycle::build_pg_pool().await?;
        let pool = std::sync::Arc::new(pool);
        brassclaw_pg::migrations::run_migrations(&pool)
            .await
            .map_err(|e| anyhow::anyhow!("migration failed: {e}"))?;

        let rows = list_config_keys(&pool, &self.tenant)
            .await
            .map_err(|e| anyhow::anyhow!("config list failed: {e}"))?;

        let prefix = self.section.as_deref().unwrap_or("");
        let mut printed = false;
        for (key, value) in &rows {
            if prefix.is_empty() || key.starts_with(prefix) {
                println!("{key} = {value}");
                printed = true;
            }
        }
        if !printed && !prefix.is_empty() {
            println!("(no keys found under section `{prefix}`)");
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// show-all
// ---------------------------------------------------------------------------

/// Render all DB config rows back as TOML (matches the `config.toml` shape).
#[derive(Debug, Args)]
pub(crate) struct ConfigShowAllCommand {
    /// Tenant ID (default: "default").
    #[arg(long, default_value = "default")]
    tenant: String,
}

impl ConfigShowAllCommand {
    pub(crate) fn execute(self, _context: RebornCliContext) -> anyhow::Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(self.run_async())
    }

    async fn run_async(self) -> anyhow::Result<()> {
        let pool = crate::commands::config::pg_lifecycle::build_pg_pool().await?;
        let pool = std::sync::Arc::new(pool);
        brassclaw_pg::migrations::run_migrations(&pool)
            .await
            .map_err(|e| anyhow::anyhow!("migration failed: {e}"))?;

        // Render DB rows as flat TOML-style `key = "value"` lines.
        // RebornConfigFile is Deserialize-only so we render the flat list.
        let rows = list_config_keys(&pool, &self.tenant)
            .await
            .map_err(|e| anyhow::anyhow!("config show-all failed: {e}"))?;

        for (key, value) in &rows {
            println!("{key} = {value:?}");
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// export
// ---------------------------------------------------------------------------

/// Export all DB config rows as TOML to stdout (for backup / migration).
///
/// Usage: `brassclaw config export > config-backup.toml`
#[derive(Debug, Args)]
pub(crate) struct ConfigExportCommand {
    /// Tenant ID (default: "default").
    #[arg(long, default_value = "default")]
    tenant: String,
}

impl ConfigExportCommand {
    pub(crate) fn execute(self, _context: RebornCliContext) -> anyhow::Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(self.run_async())
    }

    async fn run_async(self) -> anyhow::Result<()> {
        let pool = crate::commands::config::pg_lifecycle::build_pg_pool().await?;
        let pool = std::sync::Arc::new(pool);
        brassclaw_pg::migrations::run_migrations(&pool)
            .await
            .map_err(|e| anyhow::anyhow!("migration failed: {e}"))?;

        // Export as flat `key = "value"` lines (TOML-compatible scalar format).
        let rows = list_config_keys(&pool, &self.tenant)
            .await
            .map_err(|e| anyhow::anyhow!("config export failed: {e}"))?;

        for (key, value) in &rows {
            println!("{key} = {value:?}");
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// import
// ---------------------------------------------------------------------------

/// Import config from a TOML file into the database.
///
/// Usage: `brassclaw config import < config-backup.toml`
///
/// Reads a `RebornConfigFile`-shaped TOML from stdin, then upserts each
/// non-empty key into `brassclaw_config`. Existing keys are overwritten.
#[derive(Debug, Args)]
pub(crate) struct ConfigImportCommand {
    /// Tenant ID (default: "default").
    #[arg(long, default_value = "default")]
    tenant: String,
}

impl ConfigImportCommand {
    pub(crate) fn execute(self, _context: RebornCliContext) -> anyhow::Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(self.run_async())
    }

    async fn run_async(self) -> anyhow::Result<()> {
        use std::io::Read as _;

        let pool = crate::commands::config::pg_lifecycle::build_pg_pool().await?;
        let pool = std::sync::Arc::new(pool);
        brassclaw_pg::migrations::run_migrations(&pool)
            .await
            .map_err(|e| anyhow::anyhow!("migration failed: {e}"))?;

        let mut stdin_text = String::new();
        std::io::stdin()
            .read_to_string(&mut stdin_text)
            .map_err(|e| anyhow::anyhow!("failed to read stdin: {e}"))?;

        // Parse as a raw TOML Value and flatten to dot-path key→value pairs.
        // This avoids requiring `Serialize` on `RebornConfigFile` while still
        // accepting the `config.toml` shape for operator-written files.
        let config_value: toml::Value =
            toml::from_str(&stdin_text).map_err(|e| anyhow::anyhow!("invalid config TOML: {e}"))?;

        let mut count = 0usize;
        for (key, value) in flatten_toml("", &config_value) {
            save_config_key(
                &pool,
                &self.tenant,
                &key,
                &value,
                ConfigWriteContext::Operator,
            )
            .await
            .map_err(|e| anyhow::anyhow!("failed to import key `{key}`: {e}"))?;
            count += 1;
        }

        println!(
            "imported {count} config key(s) into tenant `{}`",
            self.tenant
        );
        Ok(())
    }
}

/// Flatten a TOML `Value` tree into `(dot.path, string_value)` pairs.
fn flatten_toml(prefix: &str, value: &toml::Value) -> Vec<(String, String)> {
    match value {
        toml::Value::Table(table) => {
            let mut out = Vec::new();
            for (k, v) in table {
                let path = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                out.extend(flatten_toml(&path, v));
            }
            out
        }
        toml::Value::String(s) => vec![(prefix.to_string(), s.clone())],
        toml::Value::Integer(i) => vec![(prefix.to_string(), i.to_string())],
        toml::Value::Float(f) => vec![(prefix.to_string(), f.to_string())],
        toml::Value::Boolean(b) => {
            vec![(
                prefix.to_string(),
                if *b { "true" } else { "false" }.to_string(),
            )]
        }
        toml::Value::Array(arr) => {
            // Serialize arrays as JSON (the §4.2 contract for list fields).
            let json = serde_json::to_string(arr).unwrap_or_default();
            vec![(prefix.to_string(), json)]
        }
        toml::Value::Datetime(dt) => vec![(prefix.to_string(), dt.to_string())],
    }
}
