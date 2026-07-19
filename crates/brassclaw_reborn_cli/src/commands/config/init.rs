//! `brassclaw config init` — first-run wizard that writes to PostgreSQL.
//!
//! Implements §6.1 and §6.2 from integrate-postgres.md:
//! - Detects first-run via `boot.initialized` key in `brassclaw_config`.
//! - Interactive prompts (all skippable with `--yes`).
//! - Writes each answer as a `brassclaw_config` row, then sets
//!   `boot.initialized = true`.
//! - Never writes a file. API key values are read at runtime from the named
//!   env var (env-only by security policy).
//! - Non-interactive guard: if `boot.initialized` is absent and stdin is NOT
//!   a TTY, fail immediately with a clear message (§6.1).

use brassclaw_reborn_composition::db_config::{
    ConfigWriteContext, list_config_keys, save_config_key,
};
use clap::Args;

use crate::context::RebornCliContext;

/// Initialize BrassClaw Reborn configuration in PostgreSQL.
///
/// Runs the interactive first-run wizard (or non-interactive with `--yes`).
/// Writes config rows to `brassclaw_config` and sets `boot.initialized = true`.
/// Safe to re-run with `--yes` — uses upsert semantics.
#[derive(Debug, Args)]
pub(crate) struct ConfigInitCommand {
    /// Run non-interactively, applying flag defaults without prompting.
    ///
    /// All required flags must be supplied when `--yes` is given.
    #[arg(long = "yes", short = 'y')]
    pub yes: bool,

    /// LLM provider ID (e.g. `openai`, `anthropic`, `ollama`).
    /// Use `--no-llm` to skip LLM setup.
    #[arg(long)]
    pub provider: Option<String>,

    /// Skip LLM provider setup entirely.
    #[arg(long, conflicts_with = "provider")]
    pub no_llm: bool,

    /// LLM model name (required when `--provider` is set).
    #[arg(long)]
    pub model: Option<String>,

    /// API key env var name for the LLM provider.
    /// Default: provider-specific (openai → OPENAI_API_KEY, anthropic →
    /// ANTHROPIC_API_KEY, ollama → none needed).
    #[arg(long)]
    pub api_key_env: Option<String>,

    /// WebUI bearer token env var name.
    #[arg(long, default_value = "BRASSCLAW_REBORN_WEBUI_TOKEN")]
    pub webui_token_env: String,

    /// WebUI user-id env var name.
    #[arg(long, default_value = "BRASSCLAW_REBORN_WEBUI_USER_ID")]
    pub webui_user_id_env: String,

    /// Tenant ID.
    #[arg(long, default_value = "default")]
    pub tenant: String,

    /// Default owner ID.
    #[arg(long, default_value = "admin")]
    pub owner: String,

    /// Daily user budget in USD (0 = unlimited).
    #[arg(long, default_value = "5.00")]
    pub budget_usd: String,

    /// WebUI public base URL (optional; skipped if omitted).
    #[arg(long)]
    pub webui_base_url: Option<String>,

    /// Allowed WebUI email domains, comma-separated (optional).
    #[arg(long)]
    pub webui_allowed_domains: Option<String>,
}

impl ConfigInitCommand {
    pub(crate) fn execute(self, _context: RebornCliContext) -> anyhow::Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(self.run_async())
    }

    async fn run_async(self) -> anyhow::Result<()> {
        let pool = crate::commands::config::pg_lifecycle::build_pg_pool().await?;
        let pool = std::sync::Arc::new(pool);

        // Step 2: run schema migrations (idempotent).
        brassclaw_pg::migrations::run_migrations(&pool)
            .await
            .map_err(|e| anyhow::anyhow!("migration failed: {e}"))?;

        // §6.1 — detect already-initialized.
        let existing_keys = list_config_keys(&pool, &self.tenant)
            .await
            .map_err(|e| anyhow::anyhow!("config read failed: {e}"))?;
        let already_initialized = existing_keys
            .iter()
            .any(|(k, v)| k == "boot.initialized" && v == "true");

        if already_initialized && !self.yes {
            println!(
                "BrassClaw Reborn is already initialized for tenant `{}`. \
                 Use --yes to re-run and overwrite existing settings.",
                self.tenant
            );
            return Ok(());
        }

        // §6.1 non-interactive guard: if stdin is not a TTY and boot.initialized
        // is absent, we cannot run the interactive wizard.
        if !already_initialized && !self.yes && !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
            anyhow::bail!(
                "brassclaw: first-run setup required. \
                 Run 'brassclaw config init' before starting the service."
            );
        }

        // Resolve wizard inputs either from flags (--yes) or interactive prompts.
        let inputs = if self.yes {
            self.resolve_from_flags()?
        } else {
            self.prompt_interactive().await?
        };

        // Step 3: write all config rows.
        println!("\nWriting to PostgreSQL…");
        inputs.write_to_db(&pool, &self.tenant).await?;

        // Mark initialized.
        save_config_key(&pool, &self.tenant, "boot.initialized", "true",
                        ConfigWriteContext::Operator)
            .await
            .map_err(|e| anyhow::anyhow!("failed to set boot.initialized: {e}"))?;

        println!("✓  BrassClaw Reborn configured for tenant `{}`.", self.tenant);
        println!("   Run `brassclaw serve` to start.");
        Ok(())
    }

    /// Resolve wizard inputs from CLI flags (non-interactive / --yes mode).
    fn resolve_from_flags(&self) -> anyhow::Result<WizardInputs> {
        // Step 1 — LLM provider.
        let llm = if self.no_llm {
            None
        } else {
            let provider = self.provider.clone().ok_or_else(|| {
                anyhow::anyhow!(
                    "--provider is required with --yes (or use --no-llm to skip LLM setup)"
                )
            })?;
            let model = self.model.clone().ok_or_else(|| {
                anyhow::anyhow!("--model is required with --yes when --provider is set")
            })?;
            let api_key_env = self
                .api_key_env
                .clone()
                .or_else(|| default_api_key_env_for(&provider))
                .unwrap_or_default();
            Some(LlmInputs { provider, model, api_key_env })
        };

        Ok(WizardInputs {
            llm,
            webui_token_env: self.webui_token_env.clone(),
            webui_user_id_env: self.webui_user_id_env.clone(),
            owner: self.owner.clone(),
            budget_usd: self.budget_usd.clone(),
            webui_base_url: self.webui_base_url.clone(),
            webui_allowed_domains: self.webui_allowed_domains.clone(),
        })
    }

    /// Interactive prompts for each wizard step (all skippable via Enter).
    async fn prompt_interactive(&self) -> anyhow::Result<WizardInputs> {
        use std::io::{Write as _, stdin, stdout};

        fn prompt(label: &str, default: &str) -> anyhow::Result<String> {
            print!("  {label} [{default}]: ");
            stdout().flush().ok();
            let mut line = String::new();
            stdin().read_line(&mut line)?;
            let trimmed = line.trim().to_string();
            Ok(if trimmed.is_empty() { default.to_string() } else { trimmed })
        }

        fn prompt_optional(label: &str) -> anyhow::Result<Option<String>> {
            print!("  {label} [skip]: ");
            stdout().flush().ok();
            let mut line = String::new();
            stdin().read_line(&mut line)?;
            let trimmed = line.trim().to_string();
            Ok(if trimmed.is_empty() || trimmed == "skip" { None } else { Some(trimmed) })
        }

        println!();
        println!("┌─ BrassClaw First-Run Setup ───────────────────────────────────────┐");
        println!("│  Press Enter to accept the default shown in [brackets].            │");
        println!("│  All steps can be skipped by typing 'skip' or pressing Enter.      │");
        println!("└────────────────────────────────────────────────────────────────────┘");

        // Step 1 — LLM provider.
        println!("\n  Step 1/5  LLM Provider");
        println!("  Choose a provider [openai / anthropic / ollama / custom / skip]:");
        let provider_raw = prompt("  Provider", "openai")?;
        let llm = if provider_raw == "skip" {
            None
        } else {
            let default_model = default_model_for(&provider_raw);
            let model = prompt("  Model", &default_model)?;
            let default_key_env = default_api_key_env_for(&provider_raw).unwrap_or_default();
            let api_key_env = if default_key_env.is_empty() {
                prompt("  API key env var name", "(none)")?
            } else {
                prompt("  API key env var name", &default_key_env)?
            };
            Some(LlmInputs {
                provider: provider_raw,
                model,
                api_key_env: if api_key_env == "(none)" { String::new() } else { api_key_env },
            })
        };

        // Step 2 — WebUI access.
        println!("\n  Step 2/5  WebUI Access");
        let webui_token_env = prompt("  Bearer token env var name", "BRASSCLAW_REBORN_WEBUI_TOKEN")?;
        let webui_user_id_env = prompt("  WebUI user-id env var name", "BRASSCLAW_REBORN_WEBUI_USER_ID")?;

        // Step 3 — Identity (tenant already set from --tenant flag).
        println!("\n  Step 3/5  Identity");
        let owner = prompt("  Default owner ID", &self.owner)?;

        // Step 4 — Budget.
        println!("\n  Step 4/5  Budget");
        let budget_usd = prompt("  Daily user budget in USD (0 = unlimited)", &self.budget_usd)?;

        // Step 5 — SSO (optional).
        println!("\n  Step 5/5  SSO (optional)");
        let webui_base_url = prompt_optional("  WebUI base URL")?;
        let webui_allowed_domains = prompt_optional("  Allowed email domains (comma-separated)")?;

        Ok(WizardInputs {
            llm,
            webui_token_env,
            webui_user_id_env,
            owner,
            budget_usd,
            webui_base_url,
            webui_allowed_domains,
        })
    }
}

/// Inputs collected by the wizard (interactive or --yes).
struct WizardInputs {
    llm: Option<LlmInputs>,
    webui_token_env: String,
    webui_user_id_env: String,
    owner: String,
    budget_usd: String,
    webui_base_url: Option<String>,
    webui_allowed_domains: Option<String>,
}

struct LlmInputs {
    provider: String,
    model: String,
    api_key_env: String,
}

impl WizardInputs {
    async fn write_to_db(
        &self,
        pool: &std::sync::Arc<brassclaw_pg::PgPool>,
        tenant_id: &str,
    ) -> anyhow::Result<()> {
        let ctx = ConfigWriteContext::Operator;

        // Step 1: LLM provider (into llm.default.* keys).
        if let Some(llm) = &self.llm {
            save_config_key(pool, tenant_id, "llm.default.provider_id", &llm.provider, ctx)
                .await
                .map_err(|e| anyhow::anyhow!("failed to write llm.default.provider_id: {e}"))?;
            save_config_key(pool, tenant_id, "llm.default.model", &llm.model, ctx)
                .await
                .map_err(|e| anyhow::anyhow!("failed to write llm.default.model: {e}"))?;
            if !llm.api_key_env.is_empty() {
                save_config_key(pool, tenant_id, "llm.default.api_key_env", &llm.api_key_env, ctx)
                    .await
                    .map_err(|e| anyhow::anyhow!("failed to write llm.default.api_key_env: {e}"))?;
            }
        }

        // Step 2: WebUI access.
        save_config_key(pool, tenant_id, "webui.env_token_var", &self.webui_token_env, ctx)
            .await
            .map_err(|e| anyhow::anyhow!("failed to write webui.env_token_var: {e}"))?;
        save_config_key(pool, tenant_id, "webui.env_user_id_var", &self.webui_user_id_env, ctx)
            .await
            .map_err(|e| anyhow::anyhow!("failed to write webui.env_user_id_var: {e}"))?;

        // Step 3: Identity.
        save_config_key(pool, tenant_id, "identity.default_owner", &self.owner, ctx)
            .await
            .map_err(|e| anyhow::anyhow!("failed to write identity.default_owner: {e}"))?;
        save_config_key(pool, tenant_id, "identity.tenant", tenant_id, ctx)
            .await
            .map_err(|e| anyhow::anyhow!("failed to write identity.tenant: {e}"))?;

        // Step 4: Budget.
        save_config_key(pool, tenant_id, "budget.user_daily_usd", &self.budget_usd, ctx)
            .await
            .map_err(|e| anyhow::anyhow!("failed to write budget.user_daily_usd: {e}"))?;

        // Step 5: SSO (optional).
        if let Some(base_url) = &self.webui_base_url {
            save_config_key(pool, tenant_id, "webui.base_url", base_url, ctx)
                .await
                .map_err(|e| anyhow::anyhow!("failed to write webui.base_url: {e}"))?;
        }
        if let Some(domains) = &self.webui_allowed_domains {
            save_config_key(pool, tenant_id, "webui.allowed_domains", domains, ctx)
                .await
                .map_err(|e| anyhow::anyhow!("failed to write webui.allowed_domains: {e}"))?;
        }

        Ok(())
    }
}

fn default_model_for(provider: &str) -> String {
    match provider {
        "openai" => "gpt-4o-mini".to_string(),
        "anthropic" => "claude-3-5-sonnet-latest".to_string(),
        "ollama" => "llama3.2".to_string(),
        _ => String::new(),
    }
}

fn default_api_key_env_for(provider: &str) -> Option<String> {
    match provider {
        "openai" => Some("OPENAI_API_KEY".to_string()),
        "anthropic" => Some("ANTHROPIC_API_KEY".to_string()),
        "ollama" => None, // Ollama does not require a key
        _ => None,
    }
}
