//! `brassclaw status` — lightweight system health snapshot.
//!
//! Reads env vars and local file paths only.  No DB connection, no keychain
//! probe.  Safe to run at any point in the bootstrap lifecycle.

use clap::Args;

use crate::context::RebornCliContext;

#[derive(Debug, Args)]
pub(crate) struct StatusCommand;

impl StatusCommand {
    pub(crate) fn execute(self, context: RebornCliContext) -> anyhow::Result<()> {
        let home = context.boot_config().home().path().to_path_buf();
        let runtime_profile =
            std::env::var("BRASSCLAW_RUNTIME_PROFILE").unwrap_or_else(|_| "local_dev".to_string());

        println!();
        println!("  BrassClaw Status");
        println!();

        // Version
        kv("Version", &format!("v{}", env!("CARGO_PKG_VERSION")), 12);

        // Reborn home directory
        kv("Reborn home", &home.display().to_string(), 12);

        // Active runtime profile
        kv("Profile", &runtime_profile, 12);

        // Database backend (env-only, no connection attempted)
        let db_backend = std::env::var("DATABASE_BACKEND")
            .ok()
            .unwrap_or_else(|| "libsql".to_string());
        let db_value = match db_backend.as_str() {
            "libsql" | "turso" | "sqlite" => {
                let path = std::env::var("LIBSQL_PATH")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|_| home.join("state.db"));
                if path.exists() {
                    let turso = if std::env::var("LIBSQL_URL").is_ok() {
                        " + Turso sync"
                    } else {
                        ""
                    };
                    format!("libSQL ({}{})", path.display(), turso)
                } else {
                    format!("libSQL (not yet created: {})", path.display())
                }
            }
            _ => {
                if std::env::var("DATABASE_URL").is_ok() {
                    "PostgreSQL (DATABASE_URL set; run serve to verify connection)".to_string()
                } else {
                    "not configured".to_string()
                }
            }
        };
        kv("Database", &db_value, 12);

        // LLM session / API key presence (env-only)
        let session_path = std::env::var("LLM_SESSION_PATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| home.join("session.json"));
        let llm_api_key_set = std::env::var("LLM_API_KEY").is_ok()
            || std::env::var("OPENAI_API_KEY").is_ok()
            || std::env::var("ANTHROPIC_API_KEY").is_ok();
        let session_value = if llm_api_key_set {
            "API key set via env".to_string()
        } else if session_path.exists() {
            format!("session file found ({})", session_path.display())
        } else {
            "not found (run `brassclaw onboard` or set LLM_API_KEY)".to_string()
        };
        kv("LLM auth", &session_value, 12);

        // Secrets master key
        let secrets_value = if std::env::var("SECRETS_MASTER_KEY").is_ok() {
            "configured (env)".to_string()
        } else {
            "env not set (keychain may be configured)".to_string()
        };
        kv("Secrets", &secrets_value, 12);

        // WASM tools dir
        let tools_dir = std::env::var("BRASSCLAW_TOOLS_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| home.join("tools"));
        let tools_value = if tools_dir.exists() {
            let count = count_wasm_files(&tools_dir);
            format!("{} installed ({})", count, tools_dir.display())
        } else {
            format!("directory not found ({})", tools_dir.display())
        };
        kv("WASM tools", &tools_value, 12);

        println!();
        Ok(())
    }
}

fn kv(key: &str, value: &str, width: usize) {
    println!("  {:width$}  {}", key, value, width = width);
}

fn count_wasm_files(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "wasm"))
                .count()
        })
        .unwrap_or(0)
}
