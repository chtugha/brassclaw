//! Native PostgreSQL lifecycle shared by unit and integration test rigs.

use std::sync::Arc;

use brassclaw_embedded_postgres::{EmbeddedPostgresConfig, ManagedPostgres};
use brassclaw_pg::PgPool;

static NATIVE_INSTANCES: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);

pub(crate) struct NativePostgres {
    pub(crate) pool: Arc<PgPool>,
    server: Option<ManagedPostgres>,
    // Last field: the cluster must stop before its directory is deleted.
    directory: tempfile::TempDir,
    _permit: tokio::sync::SemaphorePermit<'static>,
}

impl NativePostgres {
    pub(crate) async fn start() -> Self {
        let permit = NATIVE_INSTANCES
            .acquire()
            .await
            .expect("native test capacity");
        let directory = tempfile::tempdir().expect("isolated PostgreSQL directory");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("reserve native PostgreSQL test port");
        let port = listener.local_addr().expect("test port").port();
        drop(listener);
        let config = EmbeddedPostgresConfig {
            port,
            data_dir: directory.path().join("data"),
            bin_cache_dir: directory.path().join("bin"),
            database: "brassclaw_test".to_owned(),
            superuser: "brassclaw".to_owned(),
        };
        let url = config.connection_url();
        let server = ManagedPostgres::start(config)
            .await
            .expect("start native PostgreSQL test server");
        let pool = Arc::new(brassclaw_pg::pool::build_pool(&url).expect("test pool"));
        // Construct the cleanup owner before migrations: failed initialization
        // also closes the pool and stops the private server.
        let rig = Self {
            pool,
            server: Some(server),
            directory,
            _permit: permit,
        };
        brassclaw_pg::migrations::run_migrations(&rig.pool)
            .await
            .expect("native PostgreSQL schema migrations");
        let linked_snapshots: bool = rig
            .pool
            .get()
            .await
            .expect("native PostgreSQL migration verification connection")
            .query_one(
                "SELECT to_regclass('brassclaw_turn_snapshot_threads') IS NOT NULL",
                &[],
            )
            .await
            .expect("native PostgreSQL migration verification")
            .get(0);
        assert!(
            linked_snapshots,
            "the compiled migration bundle must include V089 before exercising turn persistence"
        );
        rig
    }
}

impl Drop for NativePostgres {
    fn drop(&mut self) {
        self.pool.close();
        let Some(server) = self.server.take() else {
            return;
        };
        // A test may drop its last owner inside a Tokio runtime or after that
        // runtime exits. A separate thread avoids nested block_on and keeps
        // pg_ctl completion supervised rather than spawning detached cleanup.
        let result = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("PostgreSQL cleanup runtime")
                .block_on(server.shutdown())
        })
        .join();
        match result {
            Ok(Ok(())) => {}
            other => {
                let reason = format!("native PostgreSQL cleanup failed: {other:?}");
                if std::thread::panicking() {
                    eprintln!("{reason}");
                } else {
                    panic!("{reason}");
                }
            }
        }
        // Retain the directory until the supervised shutdown completes.
        debug_assert!(self.directory.path().exists());
    }
}
