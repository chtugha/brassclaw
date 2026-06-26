//! System health and diagnostics CLI command.
//!
//! Checks database connectivity, session validity, embeddings,
//! WASM runtime, tool count, and channel availability.

use std::path::PathBuf;

use crate::bootstrap::brassclaw_base_dir;
use crate::cli::fmt;
use crate::settings::Settings;

/// Load settings from JSON and TOML config files, matching the runtime
/// priority: TOML overlay > settings.json > defaults.
///
/// This mirrors the loading chain in `Config::from_env_with_toml()` but
/// without resolving the full `Config` (which requires async + secrets).
fn load_settings() -> Settings {
    load_settings_from(&Settings::default_path(), &Settings::default_toml_path())
}

/// Inner implementation with injectable paths (testable).
fn load_settings_from(json_path: &std::path::Path, toml_path: &std::path::Path) -> Settings {
    let mut settings = Settings::load_from(json_path);

    match Settings::load_toml(toml_path) {
        Ok(Some(toml_settings)) => {
            settings.merge_from(&toml_settings);
        }
        Ok(None) => {} // File not found — fine for default path
        Err(e) => {
            eprintln!("Warning: failed to parse {}: {}", toml_path.display(), e);
        }
    }

    settings
}

async fn load_acp_agents_for_status()
-> Result<crate::config::acp::AcpAgentsFile, crate::config::acp::AcpConfigError> {
    match crate::config::Config::from_env().await {
        Ok(config) => {
            let db: Option<std::sync::Arc<dyn crate::db::Database>> =
                crate::db::connect_from_config(&config.database)
                    .await
                    .ok()
                    .map(|db| db as std::sync::Arc<dyn crate::db::Database>);
            crate::config::acp::load_acp_agents_for_user(db.as_deref(), &config.owner_id).await
        }
        Err(_) => crate::config::acp::load_acp_agents().await,
    }
}

/// Run the status command, printing system health info.
pub async fn run_status_command() -> anyhow::Result<()> {
    let settings = load_settings();

    println!();
    println!("  {}BrassClaw Status{}", fmt::bold(), fmt::reset());
    println!();

    // Version
    println!(
        "{}",
        fmt::kv_line(
            "Version",
            &format!("{} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")),
            12,
        )
    );

    // Database
    let db_backend = std::env::var("DATABASE_BACKEND")
        .ok()
        .unwrap_or_else(|| "postgres".to_string());
    let db_value = match db_backend.as_str() {
        "libsql" | "turso" | "sqlite" => {
            let path = std::env::var("LIBSQL_PATH")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|_| crate::config::default_libsql_path());
            if path.exists() {
                let turso = if std::env::var("LIBSQL_URL").is_ok() {
                    " + Turso sync"
                } else {
                    ""
                };
                format!("libSQL ({}{})", path.display(), turso)
            } else {
                format!("libSQL (file missing: {})", path.display())
            }
        }
        _ => {
            if std::env::var("DATABASE_URL").is_ok() {
                match check_database().await {
                    Ok(()) => "connected (PostgreSQL)".to_string(),
                    Err(e) => format!("error ({})", e),
                }
            } else {
                "not configured".to_string()
            }
        }
    };
    println!("{}", fmt::kv_line("Database", &db_value, 12));

    // Session / Auth
    let session_path = crate::config::llm::default_session_path();
    let session_value = if session_path.exists() {
        format!("found ({})", session_path.display())
    } else {
        "not found (run `brassclaw onboard`)".to_string()
    };
    println!("{}", fmt::kv_line("Session", &session_value, 12));

    // Secrets (auto-detect from env only; skip keychain probe to avoid
    // triggering macOS system password dialogs on a simple status check)
    let secrets_value = if std::env::var("SECRETS_MASTER_KEY").is_ok() {
        "configured (env)".to_string()
    } else {
        // We don't probe the keychain here because get_generic_password()
        // triggers macOS unlock+authorization dialogs, which is bad UX for
        // a read-only status command. If onboarding completed with keychain
        // storage, the key is there; we just can't cheaply verify it.
        "env not set (keychain may be configured)".to_string()
    };
    println!("{}", fmt::kv_line("Secrets", &secrets_value, 12));

    // Embeddings
    let emb_enabled = settings.embeddings.enabled
        || std::env::var("OPENAI_API_KEY").is_ok()
        || std::env::var("EMBEDDING_ENABLED")
            .map(|v| v == "true")
            .unwrap_or(false);
    let emb_value = if emb_enabled {
        format!(
            "enabled (provider: {}, model: {})",
            settings.embeddings.provider, settings.embeddings.model
        )
    } else {
        "disabled".to_string()
    };
    println!("{}", fmt::kv_line("Embeddings", &emb_value, 12));

    // WASM tools
    let tools_dir = settings
        .wasm
        .tools_dir
        .clone()
        .unwrap_or_else(default_tools_dir);
    let tools_value = if tools_dir.exists() {
        let count = count_wasm_files(&tools_dir);
        format!("{} installed ({})", count, tools_dir.display())
    } else {
        format!("directory not found ({})", tools_dir.display())
    };
    println!("{}", fmt::kv_line("WASM Tools", &tools_value, 12));

    // Channels
    let mut channel_info = vec!["cli".to_string()];
    if settings.channels.http_enabled {
        channel_info.push(format!(
            "http:{}",
            settings.channels.http_port.unwrap_or(3000)
        ));
    }
    // Include enabled WASM channel names (e.g. telegram, signal).
    for name in &settings.channels.wasm_channels {
        channel_info.push(name.clone());
    }
    println!("{}", fmt::kv_line("Channels", &channel_info.join(", "), 12));

    // Heartbeat
    let hb_enabled = settings.heartbeat.enabled
        || std::env::var("HEARTBEAT_ENABLED")
            .map(|v| v == "true")
            .unwrap_or(false);
    let hb_value = if hb_enabled {
        format!("enabled (interval: {}s)", settings.heartbeat.interval_secs)
    } else {
        "disabled".to_string()
    };
    println!("{}", fmt::kv_line("Heartbeat", &hb_value, 12));

    // MCP servers
    // V1 - DISABLED - MCP server configuration removed
    let servers: std::collections::HashMap<String, ()> = std::collections::HashMap::new();
    let mcp_value = if servers.is_empty() {
        "none configured".to_string()
    } else {
        // V1 - DISABLED - McpServerConfig may not have 'enabled' field
        let total = servers.len();
        format!("{} configured", total)
    };
    println!("{}", fmt::kv_line("MCP Servers", &mcp_value, 12));

    // ACP agents
    let acp_value = match load_acp_agents_for_status().await {
        Ok(agents) => {
            let enabled = agents.agents.iter().filter(|a| a.enabled).count();
            let total = agents.agents.len();
            format!("{} enabled / {} configured", enabled, total)
        }
        Err(_) => "none configured".to_string(),
    };
    println!("{}", fmt::kv_line("ACP Agents", &acp_value, 12));

    // Config path
    println!();
    println!(
        "{}",
        fmt::kv_line(
            "Config",
            &crate::bootstrap::brassclaw_env_path().display().to_string(),
            12,
        )
    );

    Ok(())
}

#[cfg(feature = "postgres")]
async fn check_database() -> anyhow::Result<()> {
    let url = std::env::var("DATABASE_URL").map_err(|_| anyhow::anyhow!("DATABASE_URL not set"))?;

    let config: deadpool_postgres::Config = deadpool_postgres::Config {
        url: Some(url),
        ..Default::default()
    };
    let pool = crate::db::tls::create_pool(&config, crate::config::SslMode::from_env())
        .map_err(|e| anyhow::anyhow!("pool error: {}", e))?;

    let client = tokio::time::timeout(std::time::Duration::from_secs(5), pool.get())
        .await
        .map_err(|_| anyhow::anyhow!("timeout"))?
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    client
        .execute("SELECT 1", &[])
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    Ok(())
}

#[cfg(not(feature = "postgres"))]
async fn check_database() -> anyhow::Result<()> {
    // For non-postgres backends, just report configured
    Ok(())
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

fn default_tools_dir() -> PathBuf {
    brassclaw_base_dir().join("tools")
}

