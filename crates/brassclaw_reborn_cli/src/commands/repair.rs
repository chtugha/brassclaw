//! `brassclaw repair` — checksum-aware recovery of installation-seeded prompts.

use clap::Args;

use crate::context::RebornCliContext;

/// Repair checksummed installation-seeded prompt rows. Content that differs
/// from this binary is treated as a possible operator upgrade and requires an
/// interactive confirmation before it is replaced.
#[derive(Debug, Args)]
pub(crate) struct RepairCommand {
    /// Simulate the repair without writing to the database.
    #[arg(long)]
    dry_run: bool,

    /// Tenant ID to repair. Defaults to the value in config.toml
    /// (`identity.tenant`), then `"default"`.
    #[arg(long)]
    tenant: Option<String>,
}

impl RepairCommand {
    pub(crate) fn execute(self, context: RebornCliContext) -> anyhow::Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(self.run_async(context))
    }

    async fn run_async(self, context: RebornCliContext) -> anyhow::Result<()> {
        #[cfg(not(feature = "postgres"))]
        {
            let _ = context;
            anyhow::bail!("the repair command requires the `postgres` feature to be enabled");
        }

        #[cfg(feature = "postgres")]
        {
            use std::sync::Arc;

            use brassclaw_reborn_composition::booted_db::run_migrations_and_return_booted_db;
            use brassclaw_reborn_composition::repair::repair_builtin_components;

            let home = context.boot_config().home().clone();

            // Resolve tenant_id: CLI flag → config.toml → "default"
            let tenant_id = self.tenant.clone().unwrap_or_else(|| {
                brassclaw_reborn_config::RebornConfigFile::load(&home.path().join("config.toml"))
                    .ok()
                    .flatten()
                    .and_then(|c| c.identity)
                    .and_then(|id| id.tenant)
                    .unwrap_or_else(|| "default".to_string())
            });

            // Build and connect to Postgres.
            let pool = build_pg_pool().await?;
            let pool = Arc::new(pool);

            // Run migrations and get the BootedDb token.
            let booted_db = run_migrations_and_return_booted_db(pool)
                .await
                .map_err(|e| anyhow::anyhow!("migration failed: {e}"))?;

            let integrity_before =
                brassclaw_reborn_composition::content_integrity::run_content_integrity_check(
                    &booted_db,
                )
                .await
                .map_err(|e| anyhow::anyhow!("content integrity check failed: {e}"))?;
            if let brassclaw_reborn_composition::content_integrity::ContentIntegrityOutcome::Corrupted(mismatches) = &integrity_before {
                println!("Content checksum mismatches require review:");
                for mismatch in mismatches {
                    println!("  - {}:{} (expected {}, actual {})", mismatch.table, mismatch.name, mismatch.expected, mismatch.actual);
                }
            }

            let mode = if self.dry_run { "dry-run" } else { "live" };
            println!("brassclaw repair [{mode}]: tenant={tenant_id}");

            if self.dry_run {
                println!("  (no writes will be performed)");
            }

            if self.dry_run {
                let report = repair_builtin_components(&booted_db, &tenant_id, true, &[])
                    .await
                    .map_err(|e| anyhow::anyhow!("repair failed: {e}"))?;
                println!(
                    "  Would insert or validate {} checksum-matching component(s).",
                    report.restored
                );
                if !report.requires_confirmation.is_empty() {
                    println!(
                        "  Requires confirmation before replacing differing content: {}",
                        report.requires_confirmation.join(", ")
                    );
                }
                println!("[dry-run] no writes performed");
            } else {
                let preview = repair_builtin_components(&booted_db, &tenant_id, true, &[])
                    .await
                    .map_err(|e| anyhow::anyhow!("repair failed: {e}"))?;
                let confirmed_overwrite = if preview.requires_confirmation.is_empty() {
                    Vec::new()
                } else {
                    use std::io::{self, Write};
                    println!(
                        "The following system prompt(s) have content or validation state that differs from the safe repair baseline:"
                    );
                    for name in &preview.requires_confirmation {
                        println!("  - {name}");
                    }
                    print!(
                        "Replace content and reset status to validated where needed? Type 'yes' to confirm: "
                    );
                    io::stdout().flush()?;
                    let mut answer = String::new();
                    io::stdin().read_line(&mut answer)?;
                    if answer.trim() == "yes" {
                        preview.requires_confirmation.clone()
                    } else {
                        Vec::new()
                    }
                };
                if confirmed_overwrite.is_empty() && !preview.requires_confirmation.is_empty() {
                    anyhow::bail!("repair cancelled; differing components were left unchanged");
                }
                let report =
                    repair_builtin_components(&booted_db, &tenant_id, false, &confirmed_overwrite)
                        .await
                        .map_err(|e| anyhow::anyhow!("repair failed: {e}"))?;
                println!("  Restored or validated {} component(s).", report.restored);
                if let brassclaw_reborn_composition::content_integrity::ContentIntegrityOutcome::Corrupted(mismatches) =
                    brassclaw_reborn_composition::content_integrity::run_content_integrity_check(&booted_db)
                        .await
                        .map_err(|e| anyhow::anyhow!("post-repair integrity check failed: {e}"))?
                {
                    let remaining = mismatches
                        .iter()
                        .map(|mismatch| format!("{}:{}", mismatch.table, mismatch.name))
                        .collect::<Vec<_>>();
                    anyhow::bail!("repair completed only for compiled-in prompts; unresolved checksum mismatches remain and need operator review: {}", remaining.join(", "));
                }
            }

            Ok(())
        }
    }
}

/// Build a Postgres pool for the repair command.
async fn build_pg_pool() -> anyhow::Result<deadpool_postgres::Pool> {
    crate::commands::config::pg_lifecycle::build_pg_pool().await
}
