//! Reborn-owned durable event and audit store backends.
//!
//! This crate is the production-composition side of the Reborn event
//! substrate. `brassclaw_events` owns the durable log traits and redacted record
//! vocabulary; this crate owns backend selection, fail-closed profile
//! validation, and concrete storage adapters that should not live in the
//! substrate crate.
//!
//! Backend dispatch happens at the [`RootFilesystem`] layer: the `Postgres`
//! variant of [`RebornEventStoreConfig`] opens a
//! [`PostgresRootFilesystem`](brassclaw_filesystem::PostgresRootFilesystem) and
//! routes the durable log through [`FilesystemDurableEventLog`] /
//! [`FilesystemDurableAuditLog`] over a [`ScopedFilesystem`] anchored at
//! `/events`.
//!
//! KNOWN LIMITATION (PR #3171 review #39): replay filtering currently stops
//! at project / mission / thread / process scope. The `ResourceScope` carries
//! an `invocation_id`, but `ReadScope` (defined in `brassclaw_events`) does
//! not yet expose it — so a per-invocation consumer sharing the same
//! `(tenant, user, agent)` stream cannot ask the backend to enforce the
//! invocation boundary. Adding it requires changes to `brassclaw_events` and
//! every replay caller — tracked as a follow-up.
#![warn(unreachable_pub)]

use std::sync::Arc;

use brassclaw_events::{DurableAuditLog, DurableEventLog};
#[cfg(feature = "postgres")]
use brassclaw_filesystem::{RootFilesystem, ScopedFilesystem};
#[cfg(feature = "postgres")]
use brassclaw_host_api::{MountAlias, MountGrant, MountPermissions, MountView, VirtualPath};
use secrecy::SecretString;
use thiserror::Error;

mod filesystem_store;
pub mod pg_store;

pub use filesystem_store::{FilesystemDurableAuditLog, FilesystemDurableEventLog};
pub use pg_store::{PgDurableAuditLog, PgDurableEventLog};

/// Backend configuration for Reborn durable event/audit stores.
///
/// The `Postgres` variant opens a
/// [`PostgresRootFilesystem`](brassclaw_filesystem::PostgresRootFilesystem)
/// and routes the durable log through [`FilesystemDurableEventLog`] /
/// [`FilesystemDurableAuditLog`].
#[derive(Debug)]
pub enum RebornEventStoreConfig {
    /// PostgreSQL backend configuration. The store opens a
    /// [`PostgresRootFilesystem`](brassclaw_filesystem::PostgresRootFilesystem)
    /// over the provided URL and runs durable-log ops through the unified
    /// filesystem dispatch fabric.
    Postgres { url: SecretString },
}

/// Reborn composition profile controlling which fallbacks are legal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebornProfile {
    LocalDev,
    Test,
    Production,
}

/// Durable event and audit log handles consumed by Reborn composition.
#[derive(Clone)]
pub struct RebornEventStores {
    pub events: Arc<dyn DurableEventLog>,
    pub audit: Arc<dyn DurableAuditLog>,
}

/// Redacted factory/configuration errors.
#[derive(Debug, Error)]
pub enum RebornEventStoreError {
    #[error(
        "remote Reborn Postgres event store requires sslmode=require (sslmode=disable rejected)"
    )]
    RemotePostgresClearTextDisabled,
    #[error("{backend} Reborn event store backend is not enabled in this build")]
    BackendUnavailable { backend: &'static str },
    #[error("{backend} Reborn event store failed during {operation}")]
    BackendOperation {
        backend: &'static str,
        operation: &'static str,
    },
}

impl RebornEventStoreError {
    #[cfg(feature = "postgres")]
    fn backend<E>(backend: &'static str, operation: &'static str, _source: E) -> Self {
        Self::BackendOperation { backend, operation }
    }
}

/// Build durable event and audit logs for a standalone Reborn composition path.
pub async fn build_reborn_event_stores(
    _profile: RebornProfile,
    config: RebornEventStoreConfig,
) -> Result<RebornEventStores, RebornEventStoreError> {
    match config {
        RebornEventStoreConfig::Postgres { url } => {
            #[cfg(feature = "postgres")]
            {
                postgres_backed::build(url).await
            }
            #[cfg(not(feature = "postgres"))]
            {
                let _ = url;
                Err(RebornEventStoreError::BackendUnavailable {
                    backend: "postgres",
                })
            }
        }
    }
}

/// Build a [`RebornEventStores`] from any [`RootFilesystem`] by routing the
/// durable log through [`FilesystemDurableEventLog`] /
/// [`FilesystemDurableAuditLog`] over a [`ScopedFilesystem`] anchored at
/// `/events`.
#[cfg(feature = "postgres")]
fn wrap_root_filesystem_as_event_stores<F>(
    root: Arc<F>,
) -> Result<RebornEventStores, RebornEventStoreError>
where
    F: RootFilesystem + Send + Sync + 'static,
{
    let scoped = build_events_scoped_filesystem(root)?;
    Ok(RebornEventStores {
        events: Arc::new(FilesystemDurableEventLog::new(Arc::clone(&scoped))),
        audit: Arc::new(FilesystemDurableAuditLog::new(scoped)),
    })
}

/// Wrap a [`RootFilesystem`] in a [`ScopedFilesystem`] whose [`MountView`]
/// grants the `/events` plane the permissions the durable log needs
/// (append → write, tail → read+list).
#[cfg(feature = "postgres")]
fn build_events_scoped_filesystem<F>(
    root: Arc<F>,
) -> Result<Arc<ScopedFilesystem<F>>, RebornEventStoreError>
where
    F: RootFilesystem + Send + Sync + 'static,
{
    let alias =
        MountAlias::new("/events").map_err(|_| RebornEventStoreError::BackendOperation {
            backend: "filesystem",
            operation: "construct events mount alias",
        })?;
    let target =
        VirtualPath::new("/events").map_err(|_| RebornEventStoreError::BackendOperation {
            backend: "filesystem",
            operation: "construct events mount target",
        })?;
    let view = MountView::new(vec![MountGrant::new(
        alias,
        target,
        MountPermissions {
            read: true,
            write: true,
            delete: false,
            list: true,
            execute: false,
        },
    )])
    .map_err(|_| RebornEventStoreError::BackendOperation {
        backend: "filesystem",
        operation: "construct events mount view",
    })?;
    Ok(Arc::new(ScopedFilesystem::with_fixed_view(root, view)))
}

#[cfg(feature = "postgres")]
mod postgres_backed {
    //! PostgreSQL-backed [`RootFilesystem`] construction for the durable
    //! event store. Mirrors `libsql_backed::build`: parse the URL, enforce
    //! the production TLS policy, open a pool, hand the pool to
    //! [`PostgresRootFilesystem`], and wrap the result in the standard
    //! filesystem-backed durable-log surface.

    use std::sync::Arc;

    use brassclaw_filesystem::PostgresRootFilesystem;
    use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod, Runtime};
    use secrecy::{ExposeSecret, SecretString};
    use tokio_postgres::config::{Host, SslMode};
    use tokio_postgres::{Config, NoTls};
    use tokio_postgres_rustls::MakeRustlsConnect;

    use super::{RebornEventStoreError, RebornEventStores, wrap_root_filesystem_as_event_stores};

    pub(super) async fn build(
        url: SecretString,
    ) -> Result<RebornEventStores, RebornEventStoreError> {
        let pool = build_pool(url).await?;
        let filesystem = Arc::new(PostgresRootFilesystem::new(pool));
        filesystem.run_migrations().await.map_err(|source| {
            RebornEventStoreError::backend("postgres", "run migrations", source)
        })?;
        wrap_root_filesystem_as_event_stores(filesystem)
    }

    async fn build_pool(url: SecretString) -> Result<Pool, RebornEventStoreError> {
        let raw_url = url.expose_secret();
        let mut pg_config: Config = raw_url.parse().map_err(|source| {
            RebornEventStoreError::backend("postgres", "parse connection string", source)
        })?;
        let manager_config = ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        };
        let local = is_local_postgres_config(&pg_config);
        let local_wants_tls = local && matches!(pg_config.get_ssl_mode(), SslMode::Require);
        let manager = if local && !local_wants_tls {
            // Local without an explicit `sslmode=require`: NoTls is acceptable
            // because the connection never leaves the host.
            Manager::from_config(pg_config, NoTls, manager_config)
        } else {
            if !local {
                // Remote: TLS is mandatory. Reject `sslmode=disable` and upgrade
                // `Prefer` to `Require` before handing the config to the manager.
                enforce_remote_ssl_mode(&mut pg_config)?;
            }
            // For local-with-`sslmode=require` we pass the config through
            // unchanged: the user explicitly opted in to TLS, so we route
            // through the rustls connector. Use cases include TLS-only
            // loopback Postgres and a local TLS-terminating proxy.
            let tls = make_rustls_connector()?;
            Manager::from_config(pg_config, tls, manager_config)
        };
        Pool::builder(manager)
            .runtime(Runtime::Tokio1)
            .build()
            .map_err(|source| RebornEventStoreError::backend("postgres", "build pool", source))
    }

    /// Returns true if the parsed Postgres `Config` targets only loopback
    /// hosts or Unix sockets. Anything else — including mixed lists where a
    /// remote host appears alongside a socket path — is treated as remote
    /// and must use TLS.
    ///
    /// We inspect the parsed `Config` rather than re-parsing the raw
    /// connection string so that all libpq forms are normalised:
    /// - `host=db.example.com` (keyword TCP)
    /// - `hostaddr=10.0.0.5` (numeric-IP keyword, returns no `Host` entry but
    ///   does add a hostaddr)
    /// - `postgresql:///db?host=db.example.com` (URL with empty authority +
    ///   `host` query param)
    /// - `host=/var/run/postgresql,db.example.com` (mixed list)
    fn is_local_postgres_config(config: &Config) -> bool {
        let hosts = config.get_hosts();
        let hostaddrs = config.get_hostaddrs();

        // Empty host list means libpq's compiled-in default socket directory —
        // treat as local only if there are no overriding hostaddrs.
        if hosts.is_empty() && hostaddrs.is_empty() {
            return true;
        }

        for host in hosts {
            match host {
                #[cfg(unix)]
                Host::Unix(_) => continue,
                Host::Tcp(name) if !is_local_host_literal(name) => {
                    return false;
                }
                Host::Tcp(_) => {}
            }
        }
        for addr in hostaddrs {
            if !addr.is_loopback() && !addr.is_unspecified() {
                return false;
            }
        }
        true
    }

    fn is_local_host_literal(host: &str) -> bool {
        matches!(
            host,
            "localhost" | "127.0.0.1" | "::1" | "[::1]" | "0.0.0.0"
        )
    }

    /// Reject `sslmode=disable` for any non-local Postgres config.
    ///
    /// Passing a rustls connector to `tokio-postgres` is not enough on its
    /// own: the connector is *only* used when `Config::ssl_mode` is `Prefer`
    /// or `Require`. An explicit `sslmode=disable` in the connection string
    /// returns a plaintext stream before the connector is consulted, so a
    /// misconfigured production URL can silently downgrade. We reject that
    /// here, and force `Require` if the config left the default `Prefer` in
    /// place — otherwise `tokio-postgres` would still complete a `Prefer`
    /// connection that the server happens to refuse TLS on.
    fn enforce_remote_ssl_mode(config: &mut Config) -> Result<(), RebornEventStoreError> {
        match config.get_ssl_mode() {
            SslMode::Disable => Err(RebornEventStoreError::RemotePostgresClearTextDisabled),
            SslMode::Prefer => {
                config.ssl_mode(SslMode::Require);
                Ok(())
            }
            SslMode::Require => Ok(()),
            // Forward-compat: future tokio-postgres SslMode variants we don't
            // recognise are treated as already strict.
            _ => Ok(()),
        }
    }

    /// Build a rustls TLS connector for remote Postgres connections.
    ///
    /// Mirrors `src/db/tls.rs`: prefer the platform's native certificate
    /// store, fall back to Mozilla's bundled webpki roots when the system
    /// store is empty.
    fn make_rustls_connector() -> Result<MakeRustlsConnect, RebornEventStoreError> {
        let mut root_store = rustls::RootCertStore::empty();
        let native = rustls_native_certs::load_native_certs();
        for error in &native.errors {
            tracing::warn!("postgres event-store: error loading system root certs: {error}");
        }
        for cert in native.certs {
            if let Err(error) = root_store.add(cert) {
                tracing::warn!("postgres event-store: skipping invalid system root cert: {error}");
            }
        }
        if root_store.is_empty() {
            tracing::info!(
                "postgres event-store: no system root certificates found, using bundled Mozilla roots"
            );
            root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        }
        let config = rustls::ClientConfig::builder_with_provider(
            rustls::crypto::ring::default_provider().into(),
        )
        .with_safe_default_protocol_versions()
        .map_err(|source| RebornEventStoreError::backend("postgres", "configure rustls", source))?
        .with_root_certificates(root_store)
        .with_no_client_auth();
        Ok(MakeRustlsConnect::new(config))
    }

    #[cfg(test)]
    mod tests {
        use super::{Config, enforce_remote_ssl_mode, is_local_postgres_config};
        use crate::RebornEventStoreError;
        use tokio_postgres::config::SslMode;

        fn parse(url: &str) -> Config {
            url.parse::<Config>().unwrap_or_else(|e| {
                panic!("test connection string `{url}` failed to parse: {e}");
            })
        }

        fn is_local(url: &str) -> bool {
            is_local_postgres_config(&parse(url))
        }

        #[test]
        fn local_postgres_urls_are_recognised() {
            for url in [
                "postgres://user:pass@localhost/db",
                "postgres://user@127.0.0.1:5432/db",
                "postgresql://localhost/db",
                "postgres://[::1]/db",
                "postgres://user@0.0.0.0/db",
                // Unix-socket-style: libpq treats these as local.
                "host=/var/run/postgresql user=brassclaw dbname=brassclaw",
            ] {
                assert!(is_local(url), "expected `{url}` to be detected as local");
            }
        }

        #[test]
        fn remote_postgres_urls_require_tls() {
            for url in [
                "postgres://user:pass@db.internal/db",
                "postgres://user@10.0.0.5:5432/db",
                "postgresql://user@managed-postgres.example.com/db",
                "postgres://user@[2001:db8::1]/db",
            ] {
                assert!(!is_local(url), "expected `{url}` to require TLS");
            }
        }

        #[test]
        fn libpq_keyword_strings_to_remote_hosts_require_tls() {
            // Regression for the High-severity finding on PR #3171: libpq
            // keyword form was previously treated as local because the
            // original check fired on `!url.contains("://")`.
            for url in [
                "host=db.example.com user=event_user dbname=brassclaw",
                "host=10.0.0.5 port=5432 user=brassclaw",
                "user=brassclaw host=managed-pg.internal",
            ] {
                assert!(
                    !is_local(url),
                    "expected libpq keyword string `{url}` to require TLS"
                );
            }
        }

        #[test]
        fn libpq_keyword_strings_without_host_or_with_socket_path_are_local() {
            for url in [
                // No host= keyword: libpq default = local socket.
                "user=brassclaw dbname=brassclaw",
                // Socket directory.
                "host=/var/run/postgresql user=brassclaw dbname=brassclaw",
                // Localhost literal.
                "host=localhost user=brassclaw",
                "host=127.0.0.1 user=brassclaw",
            ] {
                assert!(is_local(url), "expected `{url}` to be detected as local");
            }
        }

        #[test]
        fn libpq_hostaddr_to_remote_address_requires_tls() {
            // Regression for the High-severity finding (round 2) on PR
            // #3171: hostaddr= is a libpq keyword that bypassed the previous
            // raw-string detector entirely; switching to
            // Config::get_hostaddrs() catches it.
            assert!(!is_local("hostaddr=10.0.0.5 user=brassclaw"));
            assert!(!is_local("hostaddr=2001:db8::1 user=brassclaw"));
        }

        #[test]
        fn libpq_hostaddr_to_loopback_is_local() {
            assert!(is_local("hostaddr=127.0.0.1 user=brassclaw"));
            assert!(is_local("hostaddr=::1 user=brassclaw"));
        }

        #[test]
        fn libpq_mixed_socket_and_remote_host_list_requires_tls() {
            // host=/var/run/postgresql,db.example.com — first socket, second
            // TCP. tokio-postgres parses this as two Host entries; if any
            // TCP host isn't loopback the whole config is remote.
            assert!(!is_local(
                "host=/var/run/postgresql,db.example.com user=brassclaw"
            ));
        }

        #[test]
        fn url_with_empty_authority_and_query_host_uses_query_host() {
            // postgresql:///db?host=db.example.com — empty authority routes
            // to a host listed in the query string, which the parsed Config
            // exposes as a TCP Host entry.
            assert!(!is_local(
                "postgresql:///db?host=db.example.com&user=brassclaw"
            ));
        }

        #[test]
        fn enforce_remote_ssl_mode_rejects_disable() {
            let mut config = parse("postgres://user@db.example.com/db?sslmode=disable");
            let err = enforce_remote_ssl_mode(&mut config)
                .expect_err("sslmode=disable on remote must be rejected");
            assert!(matches!(
                err,
                RebornEventStoreError::RemotePostgresClearTextDisabled
            ));
        }

        #[test]
        fn enforce_remote_ssl_mode_upgrades_prefer_to_require() {
            // Default sslmode is `prefer`, which silently downgrades when
            // the server declines TLS — for remote we force `require`.
            let mut config = parse("postgres://user@db.example.com/db");
            assert!(matches!(config.get_ssl_mode(), SslMode::Prefer));
            enforce_remote_ssl_mode(&mut config).expect("default prefer must upgrade to require");
            assert!(matches!(config.get_ssl_mode(), SslMode::Require));
        }

        #[test]
        fn enforce_remote_ssl_mode_keeps_require() {
            let mut config = parse("postgres://user@db.example.com/db?sslmode=require");
            enforce_remote_ssl_mode(&mut config).expect("require should pass through");
            assert!(matches!(config.get_ssl_mode(), SslMode::Require));
        }

        // --- libpq quoted / whitespace-tolerant keyword strings (issues #35, #47) ---

        #[test]
        fn libpq_quoted_socket_path_is_local() {
            // Regression for review finding #35: a libpq DSN that quotes the
            // socket path was previously misclassified as remote because the
            // string-level detector saw the value as `'/var/run/postgresql'`
            // (with the leading quote) instead of the unquoted path.
            // Switching to `Config::get_hosts()` parses the libpq
            // single-quote form correctly: the value is a
            // `Host::Unix("/var/run/postgresql")` and the config is local.
            // (libpq only recognises single quotes; double quotes are not a
            // libpq quoting mechanism.)
            let url = "host='/var/run/postgresql' user=brassclaw dbname=brassclaw";
            assert!(
                is_local(url),
                "expected quoted-socket DSN `{url}` to be local"
            );
        }

        #[test]
        fn libpq_whitespace_around_equals_classifies_remote_correctly() {
            // Regression for review finding #47: tokenising the raw DSN on
            // whitespace and looking for `host=` previously caused a
            // remote-host DSN with whitespace around `=` to be treated as
            // having no host, falling through to NoTls. Parsing through
            // `tokio_postgres::Config` normalises this — the resulting
            // `Host` entry is a TCP host that fails the local-literal
            // check.
            for url in [
                "host = db.internal user=brassclaw",
                "host =db.internal user=brassclaw",
                "host= db.internal user=brassclaw",
            ] {
                assert!(
                    !is_local(url),
                    "expected whitespace-keyword DSN `{url}` to require TLS"
                );
            }
        }
    }
}

