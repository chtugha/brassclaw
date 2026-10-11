//! Real binary acceptance for management-command database ownership.
#![cfg(all(feature = "postgres", feature = "root-llm-provider"))]

use std::{
    net::TcpListener,
    path::Path,
    process::{Command, Output},
};

use brassclaw_embedded_postgres::{EmbeddedPostgresConfig, ManagedPostgres};

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn command(home: &Path, port: u16, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_brassclaw-reborn"));
    command
        .args(args)
        .env_clear()
        .env("BRASSCLAW_REBORN_HOME", home)
        .env("BRASSCLAW_EMBEDDED_PG_PORT", port.to_string());
    command
}

fn successful(output: &Output) {
    assert!(
        output.status.success(),
        "stdout: {} stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!String::from_utf8_lossy(&output.stderr).contains("dropped without calling shutdown"));
}

fn stopped(home: &Path, port: u16) {
    assert!(
        !home.join("postgres/data/postmaster.pid").exists(),
        "owned server must stop before command exit"
    );
    assert!(
        std::net::TcpStream::connect(("127.0.0.1", port)).is_err(),
        "owned server port must be closed"
    );
}

#[test]
fn owned_server_stops_on_success_early_return_and_failure_without_losing_data() {
    let directory = tempfile::tempdir().unwrap();
    let home = directory.path().join("reborn");
    let port = free_port();
    let output = command(
        &home,
        port,
        &["config", "set", "identity.default_owner", "lifecycle-owner"],
    )
    .output()
    .unwrap();
    successful(&output);
    stopped(&home, port);
    let output = command(&home, port, &["config", "get", "identity.default_owner"])
        .output()
        .unwrap();
    successful(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "lifecycle-owner"
    );
    stopped(&home, port);

    let output = command(&home, port, &["config", "get", "missing.lifecycle.key"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found"), "stderr: {stderr}");
    assert!(
        !stderr.contains("dropped without calling shutdown"),
        "stderr: {stderr}"
    );
    stopped(&home, port);

    for args in [
        vec!["config", "init", "--yes", "--no-llm"],
        vec!["config", "init"], // Already initialized: early callback return.
        vec!["maintenance", "prune-old-data"],
        vec!["maintenance", "backfill-embeddings"], // No embedding provider.
        vec!["models", "list", "--json"],
        vec!["repair", "--dry-run"],
    ] {
        let output = command(&home, port, &args).output().unwrap();
        successful(&output);
        stopped(&home, port);
    }
    let output = command(&home, port, &["secrets", "rewrap", "--strategy", "raw-key"])
        .output()
        .unwrap();
    successful(&output);
    stopped(&home, port);
    // A fresh database may initialize its master key. A corrupt existing key
    // must fail instead of being silently replaced, and still release ownership.
    let key_path = home.join(".secrets-master-key");
    assert!(key_path.exists());
    std::fs::write(&key_path, "invalid-key-hex").unwrap();
    let output = command(&home, port, &["secrets", "rewrap", "--strategy", "raw-key"])
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "invalid existing master key must remain an error"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("not valid hex"));
    assert_eq!(
        std::fs::read_to_string(&key_path).unwrap(),
        "invalid-key-hex"
    );
    assert!(!String::from_utf8_lossy(&output.stderr).contains("dropped without calling shutdown"));
    stopped(&home, port);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reused_and_explicit_external_servers_keep_their_original_owner() {
    let directory = tempfile::tempdir().unwrap();
    let home = directory.path().join("reborn");
    let port = free_port();
    let mut config = EmbeddedPostgresConfig::from_reborn_home(&home);
    config.port = port;
    let managed = ManagedPostgres::start(config).await.unwrap();
    let pid_path = home.join("postgres/data/postmaster.pid");
    let original_pid = std::fs::read_to_string(&pid_path).unwrap();
    let output = command(
        &home,
        port,
        &["config", "set", "identity.default_owner", "existing-owner"],
    )
    .output()
    .unwrap();
    successful(&output);
    assert_eq!(std::fs::read_to_string(&pid_path).unwrap(), original_pid);
    let output = command(&home, port, &["config", "get", "missing.lifecycle.key"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(std::fs::read_to_string(&pid_path).unwrap(), original_pid);

    let other_home = directory.path().join("external-client");
    let output = command(
        &other_home,
        free_port(),
        &["config", "get", "identity.default_owner"],
    )
    .env("BRASSCLAW_PG_URL", managed.connection_url())
    .output()
    .unwrap();
    successful(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "existing-owner"
    );
    assert!(!other_home.join("postgres").exists());
    assert_eq!(std::fs::read_to_string(&pid_path).unwrap(), original_pid);
    managed.shutdown().await.unwrap();
    stopped(&home, port);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn models_list_uses_database_selection_and_never_hides_database_failure() {
    let directory = tempfile::tempdir().unwrap();
    let home = directory.path().join("reborn");
    let port = free_port();
    let mut config = EmbeddedPostgresConfig::from_reborn_home(&home);
    config.port = port;
    let managed = ManagedPostgres::start(config).await.unwrap();
    let pool = brassclaw_pg::pool::build_pool(&managed.connection_url()).unwrap();
    brassclaw_pg::migrations::run_migrations(&pool)
        .await
        .unwrap();
    let client = pool.get().await.unwrap();
    // A real DB-only custom definition; no provider/model call is made by list.
    let definition = serde_json::json!({"id":"db-only-provider", "protocol":"open_ai_completions",
        "model_env":"LIFECYCLE_MODEL", "default_model":"db-default", "description":"Database fixture"});
    client.execute("INSERT INTO brassclaw_llm_providers (tenant_id,id,definition) VALUES ('default','db-only-provider',$1)", &[&definition]).await.unwrap();
    client
        .batch_execute(
            "INSERT INTO brassclaw_config (tenant_id,key,value) VALUES
        ('default','llm.default.provider_id','db-only-provider'),
        ('default','llm.default.model','db-selected')",
        )
        .await
        .unwrap();
    std::fs::write(
        home.join("config.toml"),
        "[llm.default]\nprovider_id='openai'\nmodel='file-selected'\n",
    )
    .unwrap();
    let output = command(&home, port, &["models", "list", "--json"])
        .output()
        .unwrap();
    successful(&output);
    let list: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(list["providers"].as_array().unwrap().len(), 1);
    assert_eq!(list["providers"][0]["id"], "db-only-provider");
    assert_eq!(list["providers"][0]["active"], true);
    assert_eq!(list["providers"][0]["active_model"], "db-selected");

    client
        .execute(
            "UPDATE brassclaw_llm_providers SET definition='{}' WHERE id='db-only-provider'",
            &[],
        )
        .await
        .unwrap();
    let output = command(&home, port, &["models", "list", "--json"])
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "invalid authoritative row cannot become fallback success"
    );
    assert!(
        output.stdout.is_empty(),
        "failed list cannot emit a fallback catalogue"
    );
    assert!(!String::from_utf8_lossy(&output.stderr).contains("dropped without calling shutdown"));
    assert!(home.join("postgres/data/postmaster.pid").exists());
    drop(client);
    pool.close();
    managed.shutdown().await.unwrap();
    stopped(&home, port);

    let unavailable = format!("postgresql://brassclaw@127.0.0.1:{port}/brassclaw");
    let output = command(&home, port, &["models", "list", "--json"])
        .env("BRASSCLAW_PG_URL", unavailable)
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "unavailable database cannot become registry success"
    );
    assert!(output.stdout.is_empty());
    stopped(&home, port);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retention_command_prunes_expired_events_and_preserves_recent_state() {
    let directory = tempfile::tempdir().unwrap();
    let home = directory.path().join("reborn");
    let port = free_port();
    let mut config = EmbeddedPostgresConfig::from_reborn_home(&home);
    config.port = port;
    let managed = ManagedPostgres::start(config).await.unwrap();
    let pool = brassclaw_pg::pool::build_pool(&managed.connection_url()).unwrap();
    brassclaw_pg::migrations::run_migrations(&pool)
        .await
        .unwrap();
    let client = pool.get().await.unwrap();
    client.batch_execute("INSERT INTO brassclaw_events (tenant_id,kind,payload,occurred_at) VALUES
        ('default','expired','{}',now()-interval '91 days'),
        ('default','recent','{}',now()-interval '89 days');
        INSERT INTO brassclaw_audit_log (tenant_id,action,occurred_at) VALUES
        ('default','expired',now()-interval '366 days'),
        ('default','recent',now()-interval '364 days');
        INSERT INTO brassclaw_forensic_packets (id,tenant_id,run_id,captured_at) VALUES
        ('old-packet','default','old-run',now()-interval '43 days'),
        ('recent-packet','default','recent-run',now()-interval '41 days');
        INSERT INTO brassclaw_memory_chat_records (id,tenant_id,user_id,content,forensic_packet_id) VALUES
        ('old-memory','default','operator','retain memory','old-packet'),
        ('recent-memory','default','operator','retain recent memory','recent-packet');").await.unwrap();
    let output = command(&home, port, &["maintenance", "prune-old-data"])
        .output()
        .unwrap();
    successful(&output);
    let events = client
        .query("SELECT kind FROM brassclaw_events", &[])
        .await
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].get::<_, String>(0), "recent");
    let audits = client
        .query("SELECT action FROM brassclaw_audit_log", &[])
        .await
        .unwrap();
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].get::<_, String>(0), "recent");
    let packets = client
        .query("SELECT id FROM brassclaw_forensic_packets", &[])
        .await
        .unwrap();
    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0].get::<_, String>(0), "recent-packet");
    let records = client
        .query(
            "SELECT id,forensic_packet_id FROM brassclaw_memory_chat_records ORDER BY id",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].get::<_, Option<String>>(1), None);
    assert_eq!(
        records[1].get::<_, Option<String>>(1).as_deref(),
        Some("recent-packet")
    );

    // A failure late in the sweep cannot commit its earlier deletions or
    // detach memory links. No retry is performed by the CLI.
    client
        .batch_execute(
            "INSERT INTO brassclaw_events (tenant_id,kind,payload,occurred_at)
        VALUES ('default','rollback-event','{}',now()-interval '91 days');
        INSERT INTO brassclaw_forensic_packets (id,tenant_id,run_id,captured_at)
        VALUES ('rollback-packet','default','rollback-run',now()-interval '43 days');
        INSERT INTO brassclaw_memory_chat_records (id,tenant_id,user_id,content,forensic_packet_id)
        VALUES ('rollback-memory','default','operator','retained link','rollback-packet');
        CREATE FUNCTION reject_retention_delete() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN RAISE EXCEPTION 'deliberate retention deletion failure'; END $$;
        CREATE TRIGGER reject_retention_delete BEFORE DELETE ON brassclaw_forensic_packets
        FOR EACH ROW EXECUTE FUNCTION reject_retention_delete();",
        )
        .await
        .unwrap();
    let output = command(&home, port, &["maintenance", "prune-old-data"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let count: i64 = client
        .query_one(
            "SELECT count(*) FROM brassclaw_events WHERE kind='rollback-event'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 1);
    let link: Option<String> = client.query_one("SELECT forensic_packet_id FROM brassclaw_memory_chat_records WHERE id='rollback-memory'", &[]).await.unwrap().get(0);
    assert_eq!(link.as_deref(), Some("rollback-packet"));
    assert!(home.join("postgres/data/postmaster.pid").exists());
    drop(client);
    pool.close();
    managed.shutdown().await.unwrap();
    stopped(&home, port);
}
