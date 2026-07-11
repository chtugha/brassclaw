//! Test utilities: stub LLM, stub channel, and test DB helpers.

pub mod credentials;

// `StubLlm`, `StubErrorKind`, and `fault_injection` live in `brassclaw_llm`
// (the natural home for the trait they implement). Re-exported under the
// existing `crate::testing::*` paths so existing test imports keep working.
pub use brassclaw_llm::testing::{StubErrorKind, StubLlm, fault_injection};

use std::sync::Arc;

use crate::db::Database;

/// Create a libSQL-backed test database in a temporary directory.
///
/// Returns the database and a `TempDir` guard — the database file is
/// deleted when the guard is dropped.
#[cfg(feature = "libsql")]
pub async fn test_db() -> (Arc<dyn Database>, tempfile::TempDir) {
    use crate::db::libsql::LibSqlBackend;

    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = dir.path().join("test.db");
    let backend = LibSqlBackend::new_local(&path)
        .await
        .expect("failed to create test LibSqlBackend");
    backend
        .run_migrations()
        .await
        .expect("failed to run migrations");
    (Arc::new(backend) as Arc<dyn Database>, dir)
}

