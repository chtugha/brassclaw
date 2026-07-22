use deadpool_postgres::{Config, Pool, PoolConfig, Runtime};
use tracing::warn;
use url::Url;

use crate::error::PgError;

/// Maximum pool size for any single `deadpool_postgres` pool.
///
/// Embedded Postgres is tuned to `max_connections = 20`.  The composition
/// layer creates one pool per startup; keeping this at 16 leaves four
/// connections for `psql` sessions, migrations, and pg_ctl monitoring
/// without risking `FATAL: sorry, too many clients already`.
///
/// External Postgres installations typically allow many more connections;
/// 16 is conservative and safe for both cases.
pub const MAX_POOL_SIZE: usize = 16;

/// Build a `deadpool_postgres` connection pool from a PostgreSQL URL.
///
/// For loopback URLs (`127.0.0.1`, `::1`, `localhost`) the pool connects
/// without TLS — the embedded Postgres server uses trust auth over loopback.
///
/// For non-loopback URLs the pool is built with TLS enabled via `rustls` +
/// the system's native certificate store. If the URL lacks `sslmode=`, a
/// non-suppressible `warn!` is also emitted as a security reminder.
///
/// The pool max size is capped at [`MAX_POOL_SIZE`] regardless of the
/// `deadpool` default so this pool cannot exhaust embedded Postgres's
/// `max_connections = 20` limit on its own.
pub fn build_pool(url: &str) -> Result<Pool, PgError> {
    let is_loopback = url_is_loopback(url);
    if !is_loopback {
        check_ssl_warning(url);
    }

    let mut cfg = Config::new();
    cfg.url = Some(url.to_string());
    cfg.pool = Some(PoolConfig {
        max_size: MAX_POOL_SIZE,
        ..PoolConfig::default()
    });

    let pool = if is_loopback {
        cfg.create_pool(Some(Runtime::Tokio1), tokio_postgres::NoTls)
            .map_err(|e| PgError::Pool(e.to_string()))?
    } else {
        let tls = build_tls_connector()?;
        cfg.create_pool(Some(Runtime::Tokio1), tls)
            .map_err(|e| PgError::Pool(e.to_string()))?
    };

    Ok(pool)
}

/// Build a `rustls`-backed TLS connector using the system's native certificate
/// store. Used for non-loopback Postgres connections.
fn build_tls_connector() -> Result<tokio_postgres_rustls::MakeRustlsConnect, PgError> {
    let mut root_store = rustls::RootCertStore::empty();
    let native = rustls_native_certs::load_native_certs();
    for error in &native.errors {
        tracing::warn!("pg pool: error loading system root cert: {error}");
    }
    for cert in native.certs {
        if let Err(error) = root_store.add(cert) {
            tracing::warn!("pg pool: skipping invalid system root cert: {error}");
        }
    }
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    Ok(tokio_postgres_rustls::MakeRustlsConnect::new(config))
}

/// Return true if the URL's host is a loopback address.
fn url_is_loopback(url: &str) -> bool {
    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    matches!(
        parsed.host_str().unwrap_or(""),
        "127.0.0.1" | "::1" | "localhost"
    )
}

/// Emit a non-suppressible warning when the URL points to a non-loopback host
/// without `sslmode=`. Connections to a remote Postgres without TLS expose
/// ciphertext, config, and session state in transit.
fn check_ssl_warning(url: &str) {
    if !url.contains("sslmode=") {
        warn!(
            "BRASSCLAW_PG_URL points to non-loopback host without sslmode — \
             TLS is strongly recommended"
        );
    }
}
