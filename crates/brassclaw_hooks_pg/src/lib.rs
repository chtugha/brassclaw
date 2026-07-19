//! Durable PostgreSQL-backed [`PredicateStateBackend`].
//!
//! Implements the *exact same* trait contract as the in-memory backend
//! ([`brassclaw_hooks::predicate_state::InMemoryPredicateStateBackend`])
//! and is proven against the shared contract harness (see
//! `tests/predicate_state_postgres_contract.rs`). All eight contract
//! functions plus adversarial multi-host tests run against this impl.
//!
//! [`PredicateStateBackend`]:
//!     brassclaw_hooks::predicate_state::PredicateStateBackend

mod backend;
mod hashing;
mod schema;

pub use backend::PostgresPredicateStateBackend;

/// Test-only accessors for the crate-internal bucket hashing, used by the
/// adversarial integration tests to compute the `key_hash` / `scope_hash`
/// bytes a bucket maps to so a test can query rows directly. Not part of the
/// public API surface (this crate is `publish = false`); kept out of the
/// rendered docs. Production code must never depend on this module.
#[doc(hidden)]
pub mod test_support {
    use brassclaw_hooks::predicate_state::InvocationKey;

    /// `key_hash` bytes for an invocation bucket — see
    /// `crate::hashing::invocation_key_hash`.
    pub fn invocation_key_hash_bytes(key: &InvocationKey) -> [u8; 32] {
        crate::hashing::invocation_key_hash(key)
    }

    /// `scope_hash` bytes for a tenant — see `crate::hashing::scope_hash`.
    pub fn scope_hash_bytes(tenant_id: &str) -> [u8; 32] {
        crate::hashing::scope_hash(tenant_id)
    }
}
