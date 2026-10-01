//! Type-level proof that Postgres migrations have completed.
//!
//! [`BootedDb`] is a newtype that wraps a [`brassclaw_pg::PgPool`]. It can only
//! be constructed by [`run_migrations_and_return_booted_db`] (which runs
//! migrations) or [`BootedDb::from_migrated_pool`] (for callers that have
//! already run migrations upstream).
//!
//! Passing `&BootedDb` to seeding and integrity functions ensures the compiler
//! rejects any call-site ordering that would bypass migration.

#![forbid(unsafe_code)]

use std::sync::Arc;

use brassclaw_pg::PgPool;

/// Proof that Postgres migrations have completed for this boot cycle.
///
/// Constructed only by [`run_migrations_and_return_booted_db`] or
/// [`BootedDb::from_migrated_pool`].
/// Pass `&BootedDb` to any function that must not run before migrations.
#[must_use]
pub struct BootedDb {
    pub(crate) pool: Arc<PgPool>,
}

impl BootedDb {
    /// Borrow the underlying pool.
    pub fn pool(&self) -> &Arc<PgPool> {
        &self.pool
    }

    /// Create a [`BootedDb`] from a pool where migrations have already been run
    /// by the caller (e.g. `factory.rs` / `serve.rs` upstream paths).
    ///
    /// # Safety contract
    ///
    /// The caller is responsible for having run
    /// `brassclaw_pg::migrations::run_migrations` against this pool before
    /// calling this function.  The type system cannot verify this; it is the
    /// caller's responsibility.
    pub(crate) fn from_migrated_pool(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

/// Run Postgres migrations and return a [`BootedDb`] token.
///
/// This is the only correct path for callers (e.g. the `brassclaw repair`
/// command) that own their own pool and must run migrations themselves before
/// seeding or integrity checking.
#[cfg(feature = "postgres")]
pub async fn run_migrations_and_return_booted_db(
    pool: Arc<PgPool>,
) -> Result<BootedDb, brassclaw_pg::PgError> {
    brassclaw_pg::migrations::run_migrations(&pool).await?;
    Ok(BootedDb { pool })
}
