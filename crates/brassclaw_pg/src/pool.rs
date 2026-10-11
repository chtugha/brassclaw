use deadpool_postgres::{Manager, Pool, Runtime};

use crate::connection::{prepare, tls_connector};
use crate::error::PgError;

/// Default instance pool size. Embedded Postgres has 20 connections; the
/// remaining four accommodate dedicated listeners and administration. Separate
/// pools must be included in the deployment's aggregate capacity calculation.
pub const MAX_POOL_SIZE: usize = 16;

/// Build a pool using the common parsed endpoint/TLS policy.
/// Remote connections require certificate-validated TLS; explicit local TLS is
/// honored. URL and keyword DSNs follow the same driver configuration grammar.
pub fn build_pool(connection_string: &str) -> Result<Pool, PgError> {
    build_pool_with_max_size(connection_string, MAX_POOL_SIZE)
}

/// Build a separately owned pool with an explicit connection reservation.
/// This supports standalone utilities and isolated credentials without forking
/// transport policy. The caller owns aggregate capacity and pool lifetime.
pub fn build_pool_with_max_size(connection_string: &str, max_size: usize) -> Result<Pool, PgError> {
    if max_size == 0 {
        return Err(PgError::Configuration("pool size must be positive"));
    }
    let prepared = prepare(connection_string)?;
    let manager = if prepared.tls {
        Manager::new(prepared.config, tls_connector()?)
    } else {
        Manager::new(prepared.config, tokio_postgres::NoTls)
    };
    Pool::builder(manager)
        .max_size(max_size)
        .runtime(Runtime::Tokio1)
        .build()
        .map_err(PgError::from)
}
