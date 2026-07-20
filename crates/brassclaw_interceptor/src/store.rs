//! `InterceptorStore` — persistence trait for `ForensicPacket`s.
//!
//! [`crate::PgInterceptorStore`] is the sole durable implementation, backed by
//! the `brassclaw_forensic_packets` table (migration V026).
//! [`NoopInterceptorStore`] is retained for test hosts and noop mode only;
//! it discards all writes and returns empty reads.
//!
//! # Storage contract
//!
//! - `save` is an upsert keyed on `packet.id` — calling it twice with the
//!   same id overwrites the first record (status transitions are additive,
//!   so the latest version is always authoritative).
//! - `get` returns `None` for unknown ids (not an error).
//! - `list_recent` returns at most `limit` packets ordered by
//!   `captured_at DESC`.

use async_trait::async_trait;

use crate::error::InterceptorError;
use crate::packet::{ForensicPacket, PacketId};

/// Persistence port for `ForensicPacket`s.
///
/// Implementations must be `Send + Sync + 'static` so they can be placed
/// behind an `Arc` and shared across the async runtime.
#[async_trait]
pub trait InterceptorStore: Send + Sync {
    /// Upsert a `ForensicPacket`.  Overwrites any existing record with the
    /// same `packet.id`.
    async fn save(&self, packet: &ForensicPacket) -> Result<(), InterceptorError>;

    /// Retrieve a packet by id.  Returns `None` if the id is unknown.
    async fn get(&self, packet_id: &PacketId) -> Result<Option<ForensicPacket>, InterceptorError>;

    /// Return the most recent packets, newest first, capped at `limit`.
    async fn list_recent(&self, limit: usize) -> Result<Vec<ForensicPacket>, InterceptorError>;
}

/// An `InterceptorStore` that discards all writes and returns empty reads.
///
/// Used when the interceptor is wired in passive mode (no persistence
/// configured) or in test hosts that do not need storage assertions.
pub struct NoopInterceptorStore;

#[async_trait]
impl InterceptorStore for NoopInterceptorStore {
    async fn save(&self, _packet: &ForensicPacket) -> Result<(), InterceptorError> {
        Ok(())
    }

    async fn get(&self, _packet_id: &PacketId) -> Result<Option<ForensicPacket>, InterceptorError> {
        Ok(None)
    }

    async fn list_recent(&self, _limit: usize) -> Result<Vec<ForensicPacket>, InterceptorError> {
        Ok(Vec::new())
    }
}
