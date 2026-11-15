//! Native PostgreSQL lifecycle shared by unit and integration test rigs.

use std::sync::Arc;

use brassclaw_embedded_postgres::{EmbeddedPostgresConfig, ManagedPostgres};
use brassclaw_pg::PgPool;
use brassclaw_secrets::SecretMaterial;
use secrecy::ExposeSecret;

static NATIVE_INSTANCES: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);

pub(crate) struct NativePostgres {
    pub(crate) pool: Arc<PgPool>,
    pub(crate) url: SecretMaterial,
    server: Option<ManagedPostgres>,
    // Last field: the cluster must stop before its directory is deleted.
    pub(super) directory: tempfile::TempDir,
    _permit: tokio::sync::SemaphorePermit<'static>,
}

impl NativePostgres {
    pub(crate) async fn start() -> Self {
        Self::start_schema(None).await
    }

    pub(crate) async fn start_schema(version: Option<u32>) -> Self {
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
            url: SecretMaterial::from(url),
            server: Some(server),
            directory,
            _permit: permit,
        };
        if let Some(version) = version {
            let mut client = rig.pool.get().await.expect("previous-schema connection");
            brassclaw_pg::migrations::migrations::runner()
                .set_target(refinery::Target::Version(version))
                .run_async(&mut **client)
                .await
                .expect("actual previous-schema migrations");
        } else {
            brassclaw_pg::migrations::run_migrations(&rig.pool)
                .await
                .expect("native PostgreSQL schema migrations");
        }
        let verified = rig
            .pool
            .get()
            .await
            .expect("native PostgreSQL migration verification connection")
            .query_one(
                "SELECT to_regclass('brassclaw_turn_snapshot_threads') IS NOT NULL, inet_server_port()",
                &[],
            )
            .await
            .expect("native PostgreSQL migration verification");
        assert!(
            verified.get::<_, bool>(0),
            "the compiled migration bundle must include V089 before exercising turn persistence"
        );
        let retained_url: tokio_postgres::Config = rig
            .url
            .expose_secret()
            .parse()
            .expect("native PostgreSQL retained URL");
        assert_eq!(
            verified.get::<_, i32>(1),
            i32::from(retained_url.get_ports()[0]),
            "native pool must address this fixture's retained server"
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
        // Borrow the owner: moving it into the cleanup thread would run its
        // immediate-stop Drop fallback on a failed stop or a thread panic.
        // A scoped Builder also reports thread-start failure while ownership
        // remains here, instead of unwinding through the cluster directory.
        let result = std::thread::scope(|scope| {
            std::thread::Builder::new()
                .name("native-postgres-stop".into())
                .spawn_scoped(scope, || {
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("PostgreSQL cleanup runtime")
                        .block_on(server.shutdown())
                })
                .map(|handle| handle.join())
        });
        match result {
            Ok(Ok(Ok(()))) => {}
            other => {
                // A failed orderly stop is not permission to force-stop or to
                // delete live cluster files. Retain the directory for explicit
                // recovery and deliberately retain this small Rust owner too:
                // dropping it would send an immediate shutdown behind us.
                self.directory.disable_cleanup(true);
                std::mem::forget(server);
                let reason = format!(
                    "native PostgreSQL cleanup failed; resources retained at {}: {other:?}",
                    self.directory.path().display()
                );
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
