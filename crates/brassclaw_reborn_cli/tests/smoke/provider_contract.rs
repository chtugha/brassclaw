//! Real CLI/PG contracts replacing the pre-v3 provider-file/stub expectations.
//! No fixture writes approvals or replaces the packaged global orchestrator.

use std::{collections::BTreeSet, path::PathBuf, process::Command};

use brassclaw_embedded_postgres::{EmbeddedPostgresConfig, ManagedPostgres};
use serde_json::Value;

const NAMESPACE: &str = "reborn-cli";

struct CliDatabase {
    directory: tempfile::TempDir,
    home: PathBuf,
    port: u16,
    pool: brassclaw_pg::PgPool,
    managed: ManagedPostgres,
}

impl CliDatabase {
    async fn start() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let home = directory.path().join("reborn");
        let port = super::free_pg_port();
        let mut config = EmbeddedPostgresConfig::from_reborn_home(&home);
        config.port = port;
        let managed = ManagedPostgres::start(config).await.unwrap();
        let pool = brassclaw_pg::pool::build_pool(&managed.connection_url()).unwrap();
        brassclaw_pg::migrations::run_migrations(&pool)
            .await
            .unwrap();
        Self {
            directory,
            home,
            port,
            pool,
            managed,
        }
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = super::isolated_no_llm_command(self.directory.path(), &self.home);
        command
            .args(args)
            .env("BRASSCLAW_EMBEDDED_PG_PORT", self.port.to_string())
            .env("BRASSCLAW_BASE_DIR", self.directory.path().join("v1-state"));
        command
    }

    async fn finish(self) {
        self.pool.close();
        self.managed.shutdown().await.unwrap();
        assert!(!self.home.join("postgres/data/postmaster.pid").exists());
    }

    #[cfg(feature = "skills-db")]
    fn select(&self, provider: &str, base_url: &str) {
        // Exercise the supported operator command, not a file/ENV selection.
        for (key, value) in [
            ("llm.default.provider_id", provider),
            ("llm.default.model", "cli-contract-model"),
            ("llm.default.base_url", base_url),
        ] {
            let output = self
                .command(&["config", "set", key, value])
                .output()
                .unwrap();
            successful(&output);
        }
    }

    #[cfg(feature = "skills-db")]
    async fn latest_task(&self) -> (uuid::Uuid, Value, Value) {
        let row = self.pool.get().await.unwrap().query_one(
            "SELECT run_id,outcome,task_model_selection,phase FROM brassclaw_monty_task_admissions \
             WHERE scope->>'tenant_id'=$1 AND NOT trusted_internal_turn \
             ORDER BY started_at DESC LIMIT 1", &[&NAMESPACE],
        ).await.unwrap();
        assert_eq!(row.get::<_, String>(3), "settled");
        (row.get(0), row.get(1), row.get(2))
    }
}

fn successful(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "stdout: {} stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!String::from_utf8_lossy(&output.stderr).contains("dropped without calling shutdown"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn models_list_reports_installed_database_catalog_without_runtime_or_v1_state() {
    let db = CliDatabase::start().await;
    let output = db.command(&["models", "list"]).output().unwrap();
    successful(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("BrassClaw Reborn LLM providers"),
        "{stdout}"
    );
    assert!(stdout.contains("providers: DB-backed"), "{stdout}");
    assert!(stdout.contains("active: not-configured"), "{stdout}");
    assert!(stdout.contains("v1_state: not-used"), "{stdout}");
    let repo = brassclaw_reborn_composition::PgProviderRepo::new(db.pool.clone(), NAMESPACE);
    let installed = repo.load_all().await.unwrap();
    assert!(
        !installed.is_empty(),
        "V140 installs first-party definitions once"
    );
    assert!(installed.iter().all(|(_, builtin)| *builtin));
    let client = db.pool.get().await.unwrap();
    assert_eq!(
        client
            .query_one(
                "SELECT mode FROM brassclaw_llm_provider_initializations WHERE tenant_id=$1",
                &[&NAMESPACE]
            )
            .await
            .unwrap()
            .get::<_, String>(0),
        "fresh"
    );

    let output = db.command(&["models", "list", "--json"]).output().unwrap();
    successful(&output);
    let listed: Value = serde_json::from_slice(&output.stdout).unwrap();
    let ids: BTreeSet<_> = listed["providers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|provider| provider["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        ids,
        installed
            .iter()
            .map(|(definition, _)| definition.id.clone())
            .collect::<BTreeSet<_>>()
    );
    assert!(
        listed["providers"]
            .as_array()
            .unwrap()
            .iter()
            .all(|provider| provider["active"] == false)
    );

    // Edit one DB row and tombstone another through the existing owner. Later
    // reads must preserve both, including when all active rows are removed.
    let mut edited = installed
        .iter()
        .find(|(definition, _)| definition.id == "openai")
        .unwrap()
        .0
        .clone();
    edited.description = "Operator database override".into();
    repo.upsert(edited).await.unwrap();
    assert!(repo.delete("anthropic").await.unwrap());
    std::fs::write(db.home.join("providers.json"), "malformed legacy catalogue").unwrap();
    let output = db
        .command(&["models", "list", "openai", "--json"])
        .env("LLM_BACKEND", "anthropic")
        .output()
        .unwrap();
    successful(&output);
    let listed: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        listed["providers"][0]["description"],
        "Operator database override"
    );
    assert!(repo.get("anthropic").await.unwrap().is_none());
    for (definition, _) in repo.load_all().await.unwrap() {
        assert!(repo.delete(&definition.id).await.unwrap());
    }
    let output = db.command(&["models", "list", "--json"]).output().unwrap();
    successful(&output);
    let listed: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        listed["providers"],
        serde_json::json!([]),
        "no compiled/file overlay or reseeding"
    );
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM brassclaw_monty_task_admissions", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0,
        "provider administration must not admit runtime work"
    );
    assert!(!db.directory.path().join("v1-state").exists());
    drop(client);
    db.finish().await;
}

#[cfg(feature = "skills-db")]
fn failed_without_reply(output: &std::process::Output) {
    assert!(
        !output.status.success(),
        "an unavailable model must not produce stub success"
    );
    assert!(
        output.stdout.is_empty(),
        "failed turn emitted assistant text: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("runtime started"),
        "runtime must actually start: {stderr}"
    );
    assert!(
        stderr.contains("did not produce an assistant reply"),
        "{stderr}"
    );
    assert!(
        !stderr.contains("dropped without calling shutdown"),
        "{stderr}"
    );
}

#[cfg(feature = "skills-db")]
fn failed_no_match(outcome: &Value) {
    assert_eq!(outcome["status"], "failed", "{outcome}");
    let execution = &outcome["execution"];
    assert_eq!(
        execution["intent_outcome"], "no_match",
        "failure before matching is not this case"
    );
    // This counter counts host model attempts, including refused preparation;
    // the local trap separately proves zero provider egress. Never replay.
    assert!(execution["model_exchange_count"].as_u64().unwrap() <= 1);
    assert_eq!(execution["capability_dispatch_count"], 0);
    assert!(execution["reply"].is_null());
    assert_eq!(
        execution["recipes"],
        serde_json::json!([]),
        "no reply/history fallback after failure"
    );
}

#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn run_uses_qualified_reply_recipe_without_provider_and_rejects_no_match() {
    let db = CliDatabase::start().await;
    // This is the existing packaged host-post-reply public variant, not a new
    // fixture Recipe. Its own binding, usage approval and kernel remain intact.
    let literal = "CLI literal ' ü {{not_source}} %";
    let command = format!("publish literal reply {literal}");
    let output = db.command(&["run", "-m", &command]).output().unwrap();
    successful(&output);
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("{literal}\n")
    );
    let (run_id, outcome, models) = db.latest_task().await;
    assert_eq!(outcome["status"], "completed");
    let execution = &outcome["execution"];
    assert_eq!(execution["intent_outcome"], "match");
    assert_eq!(execution["root_completed"], true);
    assert_eq!(execution["all_selected_recipes_complete"], true);
    assert_eq!(execution["model_exchange_count"], 0);
    assert!(
        execution["reply"]["reference"]
            .as_str()
            .unwrap()
            .starts_with("msg:")
    );
    assert_eq!(
        execution["recipes"].as_array().unwrap().len(),
        2,
        "reply and history must both execute"
    );
    assert_eq!(
        models["selection"]["roles"]["kohai"]["state"],
        "unconfigured"
    );
    let client = db.pool.get().await.unwrap();
    // Persisted immutable selections and real Tool invocations prove that a
    // CLI print/echo shortcut cannot satisfy the success assertions above.
    // Stable installed_monty_catalogue identities; revisions/checksums must be
    // supplied by the real approved selection, never by this test.
    let reply_id = uuid::Uuid::parse_str("fc750224-1147-5c52-9386-7b2e1e7c5736").unwrap();
    let history_id = uuid::Uuid::parse_str("77853af4-426f-571f-aafa-88d2cd98264a").unwrap();
    let recipes: BTreeSet<uuid::Uuid> = client
        .query(
            "SELECT recipe_id FROM brassclaw_monty_recipe_selections WHERE run_id=$1",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(0))
        .collect();
    assert_eq!(recipes, BTreeSet::from([reply_id, history_id]));
    let invocations = client.query(
        "SELECT recipe_id,phase,attempt_count FROM brassclaw_monty_tool_invocations WHERE run_id=$1", &[&run_id]
    ).await.unwrap();
    assert_eq!(invocations.len(), 2);
    for row in invocations {
        assert!(recipes.contains(&row.get::<_, uuid::Uuid>(0)));
        assert_eq!(row.get::<_, String>(1), "answered");
        assert_eq!(row.get::<_, i16>(2), 1);
    }
    let reply = execution["recipes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|recipe| recipe["recipe_id"] == reply_id.to_string())
        .unwrap();
    assert_eq!(reply["normal_match"]["recipe_completed"], true);
    assert_eq!(reply["normal_match"]["task_completed"], true);
    assert_eq!(
        reply["normal_match"]["matching"]["recipe_uuid"],
        reply_id.to_string()
    );
    let selected: String = client.query_one(
        "SELECT selection_bytes FROM brassclaw_monty_recipe_selections WHERE run_id=$1 AND recipe_id=$2",
        &[&run_id, &reply_id],
    ).await.unwrap().get(0);
    let selected: Value = serde_json::from_str(&selected).unwrap();
    let pin = &selected["association_approvals"]["0:2"];
    let approval_id: uuid::Uuid = serde_json::from_value(pin["approval_id"].clone()).unwrap();
    let checksum: String = client
        .query_one(
            "SELECT checksum FROM reborn_skill_association_approvals WHERE approval_id=$1",
            &[&approval_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(pin["checksum"], checksum);
    drop(client);

    // Preserve the original unmet need too: ordinary No-Match without an LLM
    // really fails after routing, with no assistant text or replayed Recipe.
    let output = db
        .command(&["run", "-m", "CLI unmatched model request 7b1590"])
        .output()
        .unwrap();
    failed_without_reply(&output);
    let (_, outcome, models) = db.latest_task().await;
    failed_no_match(&outcome);
    assert_eq!(
        models["selection"]["roles"]["kohai"]["state"],
        "unconfigured"
    );
    db.finish().await;
}

#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn run_requires_native_codex_binding_instead_of_legacy_auth_file() {
    let db = CliDatabase::start().await;
    let trap = EgressTrap::start();
    db.select("openai_codex", &trap.base_url());
    let path = db.directory.path().join("legacy-codex-auth.json");
    let legacy = r#"{"OPENAI_API_KEY":"cli-legacy-codex-secret-sentinel"}"#;
    // Missing and present legacy files must have identical installed behavior.
    // A host-managed Codex role requires its actual native authenticated head.
    for present in [false, true] {
        if present {
            std::fs::write(&path, legacy).unwrap();
        }
        let output = db
            .command(&["run", "-m", "CLI unmatched Codex request 04a19a"])
            .env("LLM_BACKEND", "openai_codex")
            .env("LLM_USE_CODEX_AUTH", "true")
            .env("CODEX_AUTH_PATH", &path)
            .output()
            .unwrap();
        failed_without_reply(&output);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("primary_configuration_invalid"),
            "native selection must fail preparation: {stderr}"
        );
        assert!(
            !stderr.contains(path.to_str().unwrap()),
            "legacy auth path leaked"
        );
        assert!(
            !stderr.contains("cli-legacy-codex-secret-sentinel"),
            "legacy credential leaked"
        );
        let (_, outcome, models) = db.latest_task().await;
        failed_no_match(&outcome);
        let role = &models["selection"]["roles"]["kohai"];
        assert_eq!(role["state"], "captured");
        assert_eq!(role["binding"]["revision"]["provider_id"], "openai_codex");
        assert!(role["binding"]["credential_ref"].is_null());
    }
    assert_eq!(std::fs::read_to_string(&path).unwrap(), legacy);
    trap.finish();
    db.finish().await;
}

#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn run_uses_database_selection_and_requires_encrypted_key_before_egress() {
    let db = CliDatabase::start().await;
    let trap = EgressTrap::start();
    db.select("openai", &trap.base_url());
    let config = "[llm.default]\nprovider_id='anthropic'\nmodel='ignored-file-model'\napi_key_env='REBORN_TEST_UNSET_BC8F4D_KEY'\n";
    std::fs::write(db.home.join("config.toml"), config).unwrap();
    // Even a plausible legacy environment key is not an encrypted credential.
    let secret = "cli-environment-key-secret-sentinel";
    let output = db
        .command(&["run", "-m", "CLI unmatched API-key request 7f9c29"])
        .env("LLM_BACKEND", "anthropic")
        .env("LLM_MODEL", "ignored-environment-model")
        .env("OPENAI_API_KEY", secret)
        .output()
        .unwrap();
    failed_without_reply(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("primary_configuration_invalid"),
        "missing encrypted key must fail preparation: {stderr}"
    );
    assert!(!stderr.contains(secret), "environment credential leaked");
    let (_, outcome, models) = db.latest_task().await;
    failed_no_match(&outcome);
    let role = &models["selection"]["roles"]["kohai"];
    assert_eq!(role["state"], "captured");
    assert_eq!(role["binding"]["revision"]["provider_id"], "openai");
    assert_eq!(role["binding"]["revision"]["model"], "cli-contract-model");
    assert!(role["binding"]["credential_ref"].is_null());
    assert_eq!(
        std::fs::read_to_string(db.home.join("config.toml")).unwrap(),
        config
    );
    trap.finish();
    db.finish().await;
}

/// A local rejection endpoint only: any connection is a test failure. Returning
/// 401 ensures a regression cannot hang waiting for a response or reach an issuer.
#[cfg(feature = "skills-db")]
struct EgressTrap {
    address: std::net::SocketAddr,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    worker: Option<std::thread::JoinHandle<usize>>,
}

#[cfg(feature = "skills-db")]
impl EgressTrap {
    fn start() -> Self {
        use std::io::Write;
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let worker = std::thread::spawn(move || {
            let mut connections = 0;
            loop {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        connections += 1;
                        let _ = stream.write_all(b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if stopping.load(Ordering::Acquire) {
                            return connections;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(error) => panic!("local egress trap failed: {error}"),
                }
            }
        });
        Self {
            address,
            stop,
            worker: Some(worker),
        }
    }

    fn base_url(&self) -> String {
        format!("http://{}/v1", self.address)
    }

    fn finish(mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Release);
        assert_eq!(
            self.worker.take().unwrap().join().unwrap(),
            0,
            "missing credentials allowed provider egress"
        );
    }
}

#[cfg(feature = "skills-db")]
impl Drop for EgressTrap {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
