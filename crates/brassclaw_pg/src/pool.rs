use deadpool_postgres::{Config, Pool, Runtime};
use tracing::warn;
use url::Url;

use crate::error::PgError;

/// Build a `deadpool_postgres` connection pool from a PostgreSQL URL.
///
/// ## SSL warning
///
/// If the URL host is not a loopback address (`127.0.0.1`, `::1`, `localhost`)
/// and the URL does not contain `sslmode=`, a `warn!`-level message is emitted.
/// The pool still connects — TLS may be enforced server-side — but the warning
/// is non-suppressible per security rules.
pub fn build_pool(url: &str) -> Result<Pool, PgError> {
    check_ssl_warning(url);

    let pg_config: tokio_postgres::Config = url.parse().map_err(|e: tokio_postgres::Error| {
        PgError::InvalidUrl(e.to_string())
    })?;

    let mut cfg = Config::new();
    cfg.url = Some(url.to_string());

    let pool = cfg
        .create_pool(Some(Runtime::Tokio1), tokio_postgres::NoTls)
        .map_err(|e| PgError::Pool(e.to_string()))?;

    // Validate the config was understood.
    let _ = pg_config;

    Ok(pool)
}

/// Emit a non-suppressible warning when the URL points to a non-loopback host
/// without `sslmode=`. Connections to a remote Postgres without TLS expose
/// ciphertext, config, and session state in transit.
fn check_ssl_warning(url: &str) {
    let Ok(parsed) = Url::parse(url) else {
        return;
    };

    let host = parsed.host_str().unwrap_or("");
    let is_loopback = matches!(host, "127.0.0.1" | "::1" | "localhost");

    if !is_loopback && !url.contains("sslmode=") {
        warn!(
            "BRASSCLAW_PG_URL points to non-loopback host without sslmode — \
             TLS is strongly recommended"
        );
    }
}
