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
/// Prefer `SharedPool` when a Postgres pool is already open so the event store
/// reuses it instead of opening a second connection pool.  The `Postgres { url }`
/// variant is retained for contexts where no shared pool exists.
#[derive(Debug)]
pub enum RebornEventStoreConfig {
    /// PostgreSQL backend configuration. The store opens a
    /// [`PostgresRootFilesystem`](brassclaw_filesystem::PostgresRootFilesystem)
    /// over the provided URL and runs durable-log ops through the unified
    /// filesystem dispatch fabric.
    ///
    /// Prefer `SharedPool` when a shared `PgPool` is already open; this
    /// variant creates a second independent connection pool.
    Postgres { url: SecretString },
    /// Reuse an already-open shared Postgres pool.  Uses `PgDurableEventLog`
    /// and `PgDurableAuditLog` directly — no second pool is opened.
    #[cfg(feature = "postgres")]
    SharedPool {
        pool: std::sync::Arc<brassclaw_pg::PgPool>,
        tenant_id: String,
    },
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
        #[cfg(feature = "postgres")]
        RebornEventStoreConfig::SharedPool { pool, tenant_id } => {
            // Reuse the existing pool — no second connection pool opened.
            Ok(RebornEventStores {
                events: std::sync::Arc::new(PgDurableEventLog::new(
                    pool.clone(),
                    tenant_id.clone(),
                )),
                audit: std::sync::Arc::new(PgDurableAuditLog::new(pool, tenant_id)),
            })
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
    use secrecy::{ExposeSecret, SecretString};

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

    async fn build_pool(url: SecretString) -> Result<brassclaw_pg::PgPool, RebornEventStoreError> {
        brassclaw_pg::pool::build_pool(url.expose_secret()).map_err(|source| match source {
            brassclaw_pg::PgError::RemoteClearTextDisabled => {
                RebornEventStoreError::RemotePostgresClearTextDisabled
            }
            source => RebornEventStoreError::backend("postgres", "build pool", source),
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn standalone_event_store_reuses_remote_tls_rejection() {
            for dsn in [
                "postgres://user:private-password@db.internal/db?sslmode=disable",
                "host=localhost hostaddr=10.0.0.5 user=brassclaw password=private-password sslmode=disable",
                "postgresql:///db?host=db.internal&user=brassclaw&sslmode=disable",
            ] {
                let error = build_pool(SecretString::from(dsn))
                    .await
                    .err()
                    .expect("remote plaintext rejected");
                assert!(matches!(
                    error,
                    RebornEventStoreError::RemotePostgresClearTextDisabled
                ));
                assert!(!format!("{error:?} {error}").contains("private-password"));
            }
        }
    }
}
