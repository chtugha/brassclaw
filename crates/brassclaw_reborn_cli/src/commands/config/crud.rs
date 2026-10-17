//! `brassclaw config get/set/unset/list/show-all/export/import` subcommands.
//!
//! All subcommands follow the §6.4 CLI Postgres lifecycle:
//! 1. Start embedded PG or connect to an already-running postmaster.
//! 2. Run schema migrations (idempotent).
//! 3. Perform the operation.
//! 4. Conditional shutdown: shut down embedded PG only if this command started it.

use brassclaw_reborn_composition::db_config::{
    ConfigWriteContext, delete_config_key, list_config_keys, save_config_changes, save_config_key,
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

    /// Installation namespace; defaults to the same owner as serve/run.
    #[arg(long)]
    tenant: Option<String>,
}

impl ConfigGetCommand {
    pub(crate) fn execute(self, context: RebornCliContext) -> anyhow::Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(self.run_async(context))
    }

    async fn run_async(self, context: RebornCliContext) -> anyhow::Result<()> {
        let tenant = context.storage_namespace(self.tenant.as_deref())?;
        crate::commands::config::pg_lifecycle::with_postgres(&context, |pool| async move {
            brassclaw_pg::migrations::run_migrations(&pool)
                .await
                .map_err(|e| anyhow::anyhow!("migration failed: {e}"))?;

            let rows = list_config_keys(&pool, &tenant)
                .await
                .map_err(|e| anyhow::anyhow!("config get failed: {e}"))?;

            match rows.into_iter().find(|(k, _)| k == &self.key) {
                Some((_, v)) => println!("{v}"),
                None => anyhow::bail!("key `{}` not found for tenant `{}`", self.key, tenant),
            }
            Ok(())
        })
        .await
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

    /// Installation namespace; defaults to the same owner as serve/run.
    #[arg(long)]
    tenant: Option<String>,
}

impl ConfigSetCommand {
    pub(crate) fn execute(self, context: RebornCliContext) -> anyhow::Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(self.run_async(context))
    }

    async fn run_async(self, context: RebornCliContext) -> anyhow::Result<()> {
        let tenant = context.storage_namespace(self.tenant.as_deref())?;
        crate::commands::config::pg_lifecycle::with_postgres(&context, |pool| async move {
            brassclaw_pg::migrations::run_migrations(&pool)
                .await
                .map_err(|e| anyhow::anyhow!("migration failed: {e}"))?;

            save_config_key(
                &pool,
                &tenant,
                &self.key,
                &self.value,
                ConfigWriteContext::Operator,
            )
            .await
            .map_err(|e| anyhow::anyhow!("config set failed: {e}"))?;

            println!("set `{}` = `{}`", self.key, self.value);
            Ok(())
        })
        .await
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

    /// Installation namespace; defaults to the same owner as serve/run.
    #[arg(long)]
    tenant: Option<String>,
}

impl ConfigUnsetCommand {
    pub(crate) fn execute(self, context: RebornCliContext) -> anyhow::Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(self.run_async(context))
    }

    async fn run_async(self, context: RebornCliContext) -> anyhow::Result<()> {
        let tenant = context.storage_namespace(self.tenant.as_deref())?;
        crate::commands::config::pg_lifecycle::with_postgres(&context, |pool| async move {
            brassclaw_pg::migrations::run_migrations(&pool)
                .await
                .map_err(|e| anyhow::anyhow!("migration failed: {e}"))?;

            delete_config_key(&pool, &tenant, &self.key)
                .await
                .map_err(|e| anyhow::anyhow!("config unset failed: {e}"))?;

            println!("unset `{}`", self.key);
            Ok(())
        })
        .await
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

    /// Installation namespace; defaults to the same owner as serve/run.
    #[arg(long)]
    tenant: Option<String>,
}

impl ConfigListCommand {
    pub(crate) fn execute(self, context: RebornCliContext) -> anyhow::Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(self.run_async(context))
    }

    async fn run_async(self, context: RebornCliContext) -> anyhow::Result<()> {
        let tenant = context.storage_namespace(self.tenant.as_deref())?;
        crate::commands::config::pg_lifecycle::with_postgres(&context, |pool| async move {
            brassclaw_pg::migrations::run_migrations(&pool)
                .await
                .map_err(|e| anyhow::anyhow!("migration failed: {e}"))?;

            let rows = list_config_keys(&pool, &tenant)
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
        })
        .await
    }
}

// ---------------------------------------------------------------------------
// show-all
// ---------------------------------------------------------------------------

/// Render all DB config rows back as TOML (matches the `config.toml` shape).
#[derive(Debug, Args)]
pub(crate) struct ConfigShowAllCommand {
    /// Installation namespace; defaults to the same owner as serve/run.
    #[arg(long)]
    tenant: Option<String>,
}

impl ConfigShowAllCommand {
    pub(crate) fn execute(self, context: RebornCliContext) -> anyhow::Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(self.run_async(context))
    }

    async fn run_async(self, context: RebornCliContext) -> anyhow::Result<()> {
        let tenant = context.storage_namespace(self.tenant.as_deref())?;
        crate::commands::config::pg_lifecycle::with_postgres(&context, |pool| async move {
            brassclaw_pg::migrations::run_migrations(&pool)
                .await
                .map_err(|e| anyhow::anyhow!("migration failed: {e}"))?;

            // Render DB rows as flat TOML-style `key = "value"` lines.
            // RebornConfigFile is Deserialize-only so we render the flat list.
            let rows = list_config_keys(&pool, &tenant)
                .await
                .map_err(|e| anyhow::anyhow!("config show-all failed: {e}"))?;

            let values: std::collections::BTreeMap<_, _> = rows.into_iter().collect();
            print!("{}", toml::to_string(&values)?);
            Ok(())
        })
        .await
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
    /// Installation namespace; defaults to the same owner as serve/run.
    #[arg(long)]
    tenant: Option<String>,
}

impl ConfigExportCommand {
    pub(crate) fn execute(self, context: RebornCliContext) -> anyhow::Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(self.run_async(context))
    }

    async fn run_async(self, context: RebornCliContext) -> anyhow::Result<()> {
        let tenant = context.storage_namespace(self.tenant.as_deref())?;
        crate::commands::config::pg_lifecycle::with_postgres(&context, |pool| async move {
            brassclaw_pg::migrations::run_migrations(&pool)
                .await
                .map_err(|e| anyhow::anyhow!("migration failed: {e}"))?;

            // Export as flat `key = "value"` lines (TOML-compatible scalar format).
            let rows = list_config_keys(&pool, &tenant)
                .await
                .map_err(|e| anyhow::anyhow!("config export failed: {e}"))?;

            let values: std::collections::BTreeMap<_, _> = rows.into_iter().collect();
            print!("{}", toml::to_string(&values)?);
            Ok(())
        })
        .await
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
    /// Installation namespace; defaults to the same owner as serve/run.
    #[arg(long)]
    tenant: Option<String>,
}

impl ConfigImportCommand {
    pub(crate) fn execute(self, context: RebornCliContext) -> anyhow::Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(self.run_async(context))
    }

    async fn run_async(self, context: RebornCliContext) -> anyhow::Result<()> {
        let tenant = context.storage_namespace(self.tenant.as_deref())?;
        use std::io::Read as _;

        crate::commands::config::pg_lifecycle::with_postgres(&context, |pool| async move {
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
            let config_value: toml::Value = toml::from_str(&stdin_text)
                .map_err(|e| anyhow::anyhow!("invalid config TOML: {e}"))?;

            let rows = flatten_toml("", &config_value)?;
            let changes: Vec<_> = rows
                .iter()
                .map(|(key, value)| (key.as_str(), Some(value.as_str())))
                .collect();
            save_config_changes(&pool, &tenant, &changes, ConfigWriteContext::Operator)
                .await
                .map_err(|e| anyhow::anyhow!("config import failed; no keys changed: {e}"))?;
            let count = rows.len();

            println!("imported {count} config key(s) into tenant `{}`", tenant);
            Ok(())
        })
        .await
    }
}

/// Flatten a TOML `Value` tree into `(dot.path, string_value)` pairs.
fn flatten_toml(prefix: &str, value: &toml::Value) -> anyhow::Result<Vec<(String, String)>> {
    Ok(match value {
        toml::Value::Table(table) => {
            let mut out = Vec::new();
            for (k, v) in table {
                let path = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                out.extend(flatten_toml(&path, v)?);
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
            let json = serde_json::to_string(arr)?;
            vec![(prefix.to_string(), json)]
        }
        toml::Value::Datetime(dt) => vec![(prefix.to_string(), dt.to_string())],
    })
}
